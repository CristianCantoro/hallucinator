//! Per-database rate limiting with adaptive governor instances and 429 retry.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use arc_swap::ArcSwap;
use governor::clock::DefaultClock;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter};

use crate::db::{DatabaseBackend, DbQueryResult};

/// Type alias for governor's direct rate limiter.
type DirectLimiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

/// Error type for database queries, distinguishing rate limiting from other errors.
#[derive(Debug, Clone)]
pub enum DbQueryError {
    /// Server returned 429 Too Many Requests.
    RateLimited { retry_after: Option<Duration> },
    /// Any other error.
    Other(String),
}

impl std::fmt::Display for DbQueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbQueryError::RateLimited {
                retry_after: Some(d),
            } => write!(f, "Rate limited (429), retry after {:.1}s", d.as_secs_f64()),
            DbQueryError::RateLimited { retry_after: None } => write!(f, "Rate limited (429)"),
            DbQueryError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for DbQueryError {}

impl From<String> for DbQueryError {
    fn from(s: String) -> Self {
        DbQueryError::Other(s)
    }
}

/// Per-DB rate limiter with adaptive rate adjustment via ArcSwap.
///
/// When a 429 is received, the governor is atomically swapped to a slower rate.
/// After a cooldown period (60s) with no 429s, the original rate is restored.
pub struct AdaptiveDbLimiter {
    limiter: ArcSwap<DirectLimiter>,
    /// Base period between allowed requests.
    base_period: Duration,
    /// Current slowdown factor (1 = normal, 2 = half rate, etc.).
    current_factor: AtomicU32,
    /// Timestamp of the last 429 response.
    last_429: std::sync::Mutex<Option<Instant>>,
}

impl AdaptiveDbLimiter {
    /// Create a new limiter with the given period between requests.
    pub fn new(period: Duration) -> Self {
        let quota = Quota::with_period(period).expect("period must be > 0");
        let limiter = Arc::new(DirectLimiter::direct(quota));
        Self {
            limiter: ArcSwap::from(limiter),
            base_period: period,
            current_factor: AtomicU32::new(1),
            last_429: std::sync::Mutex::new(None),
        }
    }

    /// Create a limiter allowing `n` requests per second.
    pub fn per_second(n: u32) -> Self {
        let ms = 1000 / n.max(1) as u64;
        Self::new(Duration::from_millis(ms))
    }

    /// Wait until the rate limiter allows a request. Checks for decay first.
    pub async fn acquire(&self) {
        self.try_decay();
        let limiter = self.limiter.load();
        limiter.until_ready().await;
    }

    /// Called when a 429 is received. Doubles the slowdown factor and swaps the governor.
    pub fn on_rate_limited(&self) {
        if let Ok(mut last) = self.last_429.lock() {
            *last = Some(Instant::now());
        }

        // Double factor, cap at 16x slowdown
        let _ = self.current_factor.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |f| {
            Some((f * 2).min(16))
        });

        let factor = self.current_factor.load(Ordering::SeqCst);
        if let Some(scaled) = self.base_period.checked_mul(factor) {
            if let Some(quota) = Quota::with_period(scaled) {
                let new_limiter = Arc::new(DirectLimiter::direct(quota));
                self.limiter.store(new_limiter);
            }
        }
    }

    /// If 60s have passed since the last 429, restore the original rate.
    fn try_decay(&self) {
        let should_restore = self
            .last_429
            .lock()
            .ok()
            .and_then(|last| last.map(|t| t.elapsed().as_secs() >= 60))
            .unwrap_or(false);

        if should_restore && self.current_factor.load(Ordering::SeqCst) > 1 {
            self.current_factor.store(1, Ordering::SeqCst);
            let quota = Quota::with_period(self.base_period).expect("base period valid");
            let limiter = Arc::new(DirectLimiter::direct(quota));
            self.limiter.store(limiter);
        }
    }
}

/// Collection of per-database rate limiters.
pub struct RateLimiters {
    limiters: HashMap<&'static str, AdaptiveDbLimiter>,
}

impl Default for RateLimiters {
    fn default() -> Self {
        Self::new(false, false)
    }
}

impl RateLimiters {
    /// Build rate limiters based on whether API keys/mailto are configured.
    pub fn new(has_crossref_mailto: bool, has_s2_api_key: bool) -> Self {
        let mut limiters = HashMap::new();

        // CrossRef: 1/s without mailto, 3/s with mailto
        let crossref_rate = if has_crossref_mailto { 3 } else { 1 };
        limiters.insert("CrossRef", AdaptiveDbLimiter::per_second(crossref_rate));

        // arXiv: 1 request per 3 seconds
        limiters.insert("arXiv", AdaptiveDbLimiter::new(Duration::from_secs(3)));

        // DBLP (online): ~1/s guideline
        limiters.insert("DBLP", AdaptiveDbLimiter::per_second(1));

        // Semantic Scholar: keyless=shared pool (~10/s conservative), keyed=1/s
        let s2_rate = if has_s2_api_key { 1 } else { 10 };
        limiters.insert("Semantic Scholar", AdaptiveDbLimiter::per_second(s2_rate));

        // Europe PMC: not documented, conservative 2/s
        limiters.insert("Europe PMC", AdaptiveDbLimiter::per_second(2));

        // PubMed: 3/s without key
        limiters.insert("PubMed", AdaptiveDbLimiter::per_second(3));

        // ACL Anthology (online scraping): conservative 2/s
        limiters.insert("ACL Anthology", AdaptiveDbLimiter::per_second(2));

        // OpenAlex: 100/s — effectively unlimited for our use case, skip limiter
        // SSRN: disabled, skip limiter
        // NeurIPS: disabled, skip limiter
        // Offline DBs (DBLP offline, ACL offline) share names but don't make HTTP requests

        Self { limiters }
    }

    /// Get the rate limiter for a given database, if one exists.
    pub fn get(&self, db_name: &str) -> Option<&AdaptiveDbLimiter> {
        self.limiters.get(db_name)
    }
}

/// Check if an HTTP response is a 429 and extract Retry-After if present.
///
/// Returns `Err(DbQueryError::RateLimited { .. })` if 429, `Ok(())` otherwise.
pub fn check_rate_limit_response(resp: &reqwest::Response) -> Result<(), DbQueryError> {
    if resp.status().as_u16() == 429 {
        let retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(parse_retry_after);
        Err(DbQueryError::RateLimited { retry_after })
    } else {
        Ok(())
    }
}

/// Parse a Retry-After header value (seconds or HTTP-date).
pub fn parse_retry_after(value: &str) -> Option<Duration> {
    // Try parsing as integer seconds first
    if let Ok(secs) = value.trim().parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }
    // Try parsing as HTTP-date (e.g. "Wed, 21 Oct 2015 07:28:00 GMT")
    // For simplicity, just use a conservative fallback if it looks like a date
    if value.contains(',') || value.contains("GMT") {
        return Some(Duration::from_secs(5));
    }
    None
}

/// Query a database with proactive governor rate limiting and reactive 429 retry.
///
/// 1. Acquires the per-DB governor (waits if needed)
/// 2. Calls `db.query()`
/// 3. On 429: adapts governor to slower rate, backs off, retries
/// 4. On other errors or success: returns immediately
pub async fn query_with_retry(
    db: &dyn DatabaseBackend,
    title: &str,
    client: &reqwest::Client,
    timeout: Duration,
    rate_limiters: &RateLimiters,
    max_retries: u32,
) -> Result<DbQueryResult, DbQueryError> {
    let limiter = rate_limiters.get(db.name());

    for attempt in 0..=max_retries {
        // Proactive: wait for governor permit
        if let Some(lim) = limiter {
            lim.acquire().await;
        }

        match db.query(title, client, timeout).await {
            Ok(result) => return Ok(result),
            Err(DbQueryError::RateLimited { retry_after }) => {
                if attempt == max_retries {
                    return Err(DbQueryError::RateLimited { retry_after });
                }

                // Adapt governor to slower rate
                if let Some(lim) = limiter {
                    lim.on_rate_limited();
                }

                // Backoff: use Retry-After if available, else exponential with jitter
                let backoff = retry_after.unwrap_or_else(|| {
                    let base_ms = 1000u64 * (1 << attempt.min(4)); // 1s, 2s, 4s, 8s, 16s
                    let jitter_ms = fastrand::u64(0..500);
                    Duration::from_millis(base_ms + jitter_ms)
                });
                let capped = backoff.min(Duration::from_secs(30));

                log::info!(
                    "{}: 429 rate limited, retry {}/{} after {:.1}s",
                    db.name(),
                    attempt + 1,
                    max_retries,
                    capped.as_secs_f64()
                );

                tokio::time::sleep(capped).await;
            }
            Err(other) => return Err(other),
        }
    }

    unreachable!()
}
