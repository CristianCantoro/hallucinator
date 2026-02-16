//! Global reference validation pool with shared worker tasks.
//!
//! Replaces per-paper semaphores with a single mpmc work queue.
//! Workers process refs from any paper, with cancellation via oneshot drop.

use std::sync::Arc;

use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::checker::{check_single_reference, check_single_reference_retry};
use crate::{Config, DbResult, ProgressEvent, Reference, Status, ValidationResult};

/// A reference validation job submitted to the pool.
pub struct RefJob {
    pub reference: Reference,
    pub result_tx: oneshot::Sender<ValidationResult>,
    pub ref_index: usize,
    pub total: usize,
    /// Progress callback for this job (emits Checking, Result, Warning, etc.).
    pub progress: Arc<dyn Fn(ProgressEvent) + Send + Sync>,
}

/// A pool of worker tasks that process reference validation jobs.
///
/// Submit jobs via [`submit()`](ValidationPool::submit), receive results via
/// the oneshot receiver returned with each job.
pub struct ValidationPool {
    job_tx: async_channel::Sender<RefJob>,
    workers_handle: JoinHandle<()>,
}

impl ValidationPool {
    /// Create a new pool with `num_workers` concurrent workers.
    pub fn new(config: Arc<Config>, cancel: CancellationToken, num_workers: usize) -> Self {
        let (job_tx, job_rx) = async_channel::unbounded::<RefJob>();
        let client = reqwest::Client::new();

        let workers_handle = tokio::spawn(async move {
            let mut handles = Vec::with_capacity(num_workers);
            for _ in 0..num_workers {
                let rx = job_rx.clone();
                let config = config.clone();
                let cancel = cancel.clone();
                let client = client.clone();
                handles.push(tokio::spawn(worker_loop(rx, config, client, cancel)));
            }
            // Drop our clone of the receiver so workers can exit when sender closes
            drop(job_rx);
            for h in handles {
                let _ = h.await;
            }
        });

        Self {
            job_tx,
            workers_handle,
        }
    }

    /// Get a cloneable sender for submitting jobs from multiple tasks.
    pub fn sender(&self) -> async_channel::Sender<RefJob> {
        self.job_tx.clone()
    }

    /// Submit a job to the pool.
    pub async fn submit(&self, job: RefJob) {
        let _ = self.job_tx.send(job).await;
    }

    /// Close the pool and wait for all workers to finish.
    pub async fn shutdown(self) {
        self.job_tx.close();
        let _ = self.workers_handle.await;
    }
}

/// Worker loop: receive jobs, process them, send results via oneshot.
async fn worker_loop(
    job_rx: async_channel::Receiver<RefJob>,
    config: Arc<Config>,
    client: reqwest::Client,
    cancel: CancellationToken,
) {
    while let Ok(job) = job_rx.recv().await {
        if cancel.is_cancelled() {
            break;
        }

        let RefJob {
            reference,
            mut result_tx,
            ref_index,
            total,
            progress,
        } = job;

        let title = reference.title.clone().unwrap_or_default();

        // Emit Checking event
        progress(ProgressEvent::Checking {
            index: ref_index,
            total,
            title: title.clone(),
        });

        // Build per-ref DB completion callback
        let progress_for_db = progress.clone();
        let on_db_complete = move |db_result: DbResult| {
            progress_for_db(ProgressEvent::DatabaseQueryComplete {
                paper_index: 0, // overridden by TUI layer
                ref_index,
                db_name: db_result.db_name.clone(),
                status: db_result.status.clone(),
                elapsed: db_result.elapsed.unwrap_or_default(),
            });
        };

        // First pass — cancellable via oneshot drop or CancellationToken
        let result = tokio::select! {
            biased;
            _ = result_tx.closed() => continue,
            _ = cancel.cancelled() => break,
            result = check_single_reference(&reference, &config, &client, false, Some(&on_db_complete)) => result,
        };

        // Inline retry if NotFound with failed DBs
        let final_result = if result.status == Status::NotFound && !result.failed_dbs.is_empty() {
            let failed_dbs = result.failed_dbs.clone();

            // Rebuild the callback for retry (the previous one was moved)
            let progress_for_retry = progress.clone();
            let on_retry_complete = move |db_result: DbResult| {
                progress_for_retry(ProgressEvent::DatabaseQueryComplete {
                    paper_index: 0,
                    ref_index,
                    db_name: db_result.db_name.clone(),
                    status: db_result.status.clone(),
                    elapsed: db_result.elapsed.unwrap_or_default(),
                });
            };

            let retry = tokio::select! {
                biased;
                _ = result_tx.closed() => continue,
                _ = cancel.cancelled() => break,
                retry = check_single_reference_retry(
                    &reference, &config, &client, &failed_dbs, Some(&on_retry_complete)
                ) => retry,
            };

            if retry.status != Status::NotFound {
                retry
            } else {
                result
            }
        } else {
            result
        };

        // Emit warning if some databases failed/timed out
        if !final_result.failed_dbs.is_empty() {
            let context = match final_result.status {
                Status::NotFound => "not found in other DBs".to_string(),
                Status::Verified => format!(
                    "verified via {}",
                    final_result.source.as_deref().unwrap_or("unknown")
                ),
                Status::AuthorMismatch => format!(
                    "author mismatch via {}",
                    final_result.source.as_deref().unwrap_or("unknown")
                ),
            };
            progress(ProgressEvent::Warning {
                index: ref_index,
                total,
                title: title.clone(),
                failed_dbs: final_result.failed_dbs.clone(),
                message: format!(
                    "{} timed out; {}",
                    final_result.failed_dbs.join(", "),
                    context
                ),
            });
        }

        // Emit Result event
        progress(ProgressEvent::Result {
            index: ref_index,
            total,
            result: Box::new(final_result.clone()),
        });

        let _ = result_tx.send(final_result);
    }
}
