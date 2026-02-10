use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use hallucinator_core::{Config, ProgressEvent};
use hallucinator_pdf::ExtractionResult;

use crate::tui_event::BackendEvent;

/// Run batch validation of PDFs sequentially, sending events to the TUI.
///
/// Each paper is processed one at a time (extraction is blocking via mupdf,
/// then check_references runs with its own internal concurrency).
/// Uses unbounded-style channel (large buffer) to avoid dropping events
/// from the sync progress callback.
pub async fn run_batch(
    pdfs: Vec<PathBuf>,
    config: Config,
    tx: mpsc::UnboundedSender<BackendEvent>,
    cancel: CancellationToken,
) {
    let config = Arc::new(config);

    for (paper_index, pdf_path) in pdfs.iter().enumerate() {
        if cancel.is_cancelled() {
            break;
        }

        // Signal extraction start
        let _ = tx.send(BackendEvent::ExtractionStarted { paper_index });

        // Extract references (blocking MuPDF call)
        let path = pdf_path.clone();
        let extraction: Result<ExtractionResult, String> =
            tokio::task::spawn_blocking(move || {
                hallucinator_pdf::extract_references(&path)
                    .map_err(|e| format!("PDF extraction failed: {}", e))
            })
            .await
            .unwrap_or_else(|e| Err(format!("Task join error: {}", e)));

        let extraction = match extraction {
            Ok(ext) => ext,
            Err(error) => {
                let _ = tx.send(BackendEvent::ExtractionFailed { paper_index, error });
                continue;
            }
        };

        let skip_stats = extraction.skip_stats.clone();
        let refs = extraction.references;
        let ref_titles: Vec<String> = refs
            .iter()
            .map(|r| r.title.clone().unwrap_or_default())
            .collect();

        let _ = tx.send(BackendEvent::ExtractionComplete {
            paper_index,
            ref_count: refs.len(),
            ref_titles,
            skip_stats,
        });

        if refs.is_empty() {
            let _ = tx.send(BackendEvent::PaperComplete {
                paper_index,
                results: vec![],
            });
            continue;
        }

        // Build per-paper config (clone the Arc's inner to get owned Config)
        let paper_config = (*config).clone();

        // Bridge sync progress callback → async channel via unbounded send
        let tx_progress = tx.clone();
        let progress_cb = move |event: ProgressEvent| {
            let _ = tx_progress.send(BackendEvent::Progress {
                paper_index,
                event,
            });
        };

        let paper_cancel = cancel.clone();
        let results =
            hallucinator_core::check_references(refs, paper_config, progress_cb, paper_cancel)
                .await;

        let _ = tx.send(BackendEvent::PaperComplete {
            paper_index,
            results,
        });
    }

    let _ = tx.send(BackendEvent::BatchComplete);
}

/// Open offline DBLP database if a path is configured, returning the Arc<Mutex<..>> handle.
pub fn open_dblp_db(
    path: &PathBuf,
) -> anyhow::Result<Arc<Mutex<hallucinator_dblp::DblpDatabase>>> {
    if !path.exists() {
        anyhow::bail!(
            "Offline DBLP database not found at {}. Use hallucinator-cli --update-dblp={} to build it.",
            path.display(),
            path.display()
        );
    }
    let db = hallucinator_dblp::DblpDatabase::open(path)?;
    Ok(Arc::new(Mutex::new(db)))
}
