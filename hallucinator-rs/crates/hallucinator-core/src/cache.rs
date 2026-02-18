//! In-memory cache for remote database query results.
//!
//! Avoids redundant HTTP calls when the same paper title is queried against the
//! same database multiple times (e.g. across PDFs that share citations).
//!
//! Cache keys use [`normalize_title`](crate::matching::normalize_title) so that
//! minor variations (diacritics, HTML entities, Greek letters) produce the same
//! key. Only successful results are cached; transient errors (timeouts, network
//! failures) are never cached.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use dashmap::DashMap;

use crate::db::DbQueryResult;
use crate::matching::normalize_title;

/// Default time-to-live for positive (found) cache entries.
const DEFAULT_POSITIVE_TTL: Duration = Duration::from_secs(24 * 60 * 60); // 24 hours

/// Default time-to-live for negative (not found) cache entries.
const DEFAULT_NEGATIVE_TTL: Duration = Duration::from_secs(6 * 60 * 60); // 6 hours

/// Cache key: normalized title + database name.
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
struct CacheKey {
    normalized_title: String,
    db_name: String,
}

/// What we store: either a found result or a not-found marker.
#[derive(Clone, Debug)]
enum CachedResult {
    /// Paper found: (title, authors, url).
    Found {
        title: String,
        authors: Vec<String>,
        url: Option<String>,
    },
    /// Paper not found in this database.
    NotFound,
}

/// A timestamped cache entry.
#[derive(Clone, Debug)]
struct CacheEntry {
    result: CachedResult,
    inserted_at: Instant,
}

/// Thread-safe in-memory cache for database query results.
///
/// Uses [`DashMap`] for lock-free concurrent access from multiple drainer tasks.
pub struct QueryCache {
    entries: DashMap<CacheKey, CacheEntry>,
    positive_ttl: Duration,
    negative_ttl: Duration,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl Default for QueryCache {
    fn default() -> Self {
        Self::new(DEFAULT_POSITIVE_TTL, DEFAULT_NEGATIVE_TTL)
    }
}

impl QueryCache {
    /// Create a cache with custom TTLs.
    pub fn new(positive_ttl: Duration, negative_ttl: Duration) -> Self {
        Self {
            entries: DashMap::new(),
            positive_ttl,
            negative_ttl,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// Look up a cached result for the given title and database.
    ///
    /// Returns `Some(result)` on cache hit (within TTL), `None` on miss.
    /// The title is normalized before lookup.
    pub fn get(&self, title: &str, db_name: &str) -> Option<DbQueryResult> {
        let key = CacheKey {
            normalized_title: normalize_title(title),
            db_name: db_name.to_string(),
        };

        let entry = match self.entries.get(&key) {
            Some(e) => e,
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                return None;
            }
        };
        let ttl = match &entry.result {
            CachedResult::Found { .. } => self.positive_ttl,
            CachedResult::NotFound => self.negative_ttl,
        };

        if entry.inserted_at.elapsed() > ttl {
            // Expired — remove and treat as miss
            drop(entry);
            self.entries.remove(&key);
            self.misses.fetch_add(1, Ordering::Relaxed);
            return None;
        }

        self.hits.fetch_add(1, Ordering::Relaxed);

        Some(match &entry.result {
            CachedResult::Found {
                title,
                authors,
                url,
            } => (Some(title.clone()), authors.clone(), url.clone()),
            CachedResult::NotFound => (None, vec![], None),
        })
    }

    /// Insert a query result into the cache.
    ///
    /// Only caches successful results (found or not-found). Errors should NOT
    /// be passed to this method.
    pub fn insert(&self, title: &str, db_name: &str, result: &DbQueryResult) {
        let key = CacheKey {
            normalized_title: normalize_title(title),
            db_name: db_name.to_string(),
        };

        let cached = match result {
            (Some(found_title), authors, url) => CachedResult::Found {
                title: found_title.clone(),
                authors: authors.clone(),
                url: url.clone(),
            },
            (None, _, _) => CachedResult::NotFound,
        };

        self.entries.insert(
            key,
            CacheEntry {
                result: cached,
                inserted_at: Instant::now(),
            },
        );
    }

    /// Number of cache hits since creation.
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    /// Number of cache misses since creation.
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    /// Number of entries currently in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl std::fmt::Debug for QueryCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryCache")
            .field("entries", &self.entries.len())
            .field("hits", &self.hits())
            .field("misses", &self.misses())
            .field("positive_ttl", &self.positive_ttl)
            .field("negative_ttl", &self.negative_ttl)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_miss_on_empty() {
        let cache = QueryCache::default();
        assert!(cache.get("Some Title", "CrossRef").is_none());
        assert_eq!(cache.misses(), 1);
        assert_eq!(cache.hits(), 0);
    }

    #[test]
    fn cache_hit_after_insert_found() {
        let cache = QueryCache::default();
        let result: DbQueryResult = (
            Some("Attention Is All You Need".into()),
            vec!["Vaswani".into()],
            Some("https://doi.org/10.1234".into()),
        );
        cache.insert("Attention Is All You Need", "CrossRef", &result);
        let cached = cache.get("Attention Is All You Need", "CrossRef");
        assert!(cached.is_some());
        let (title, authors, url) = cached.unwrap();
        assert_eq!(title.unwrap(), "Attention Is All You Need");
        assert_eq!(authors, vec!["Vaswani"]);
        assert_eq!(url.unwrap(), "https://doi.org/10.1234");
        assert_eq!(cache.hits(), 1);
    }

    #[test]
    fn cache_hit_after_insert_not_found() {
        let cache = QueryCache::default();
        let result: DbQueryResult = (None, vec![], None);
        cache.insert("Nonexistent Paper", "arXiv", &result);
        let cached = cache.get("Nonexistent Paper", "arXiv");
        assert!(cached.is_some());
        let (title, authors, url) = cached.unwrap();
        assert!(title.is_none());
        assert!(authors.is_empty());
        assert!(url.is_none());
    }

    #[test]
    fn cache_miss_different_db() {
        let cache = QueryCache::default();
        let result: DbQueryResult = (Some("A Paper".into()), vec![], None);
        cache.insert("A Paper", "CrossRef", &result);
        assert!(cache.get("A Paper", "arXiv").is_none());
    }

    #[test]
    fn cache_normalized_key() {
        let cache = QueryCache::default();
        let result: DbQueryResult = (Some("Résumé of Methods".into()), vec![], None);
        // Insert with accented title
        cache.insert("Résumé of Methods", "CrossRef", &result);
        // Look up with ASCII equivalent (normalization strips accents)
        let cached = cache.get("Resume of Methods", "CrossRef");
        assert!(cached.is_some());
    }

    #[test]
    fn cache_expired_positive() {
        let cache = QueryCache::new(Duration::from_millis(1), Duration::from_secs(3600));
        let result: DbQueryResult = (Some("Paper".into()), vec![], None);
        cache.insert("Paper", "CrossRef", &result);
        // Sleep briefly to let TTL expire
        std::thread::sleep(Duration::from_millis(10));
        assert!(cache.get("Paper", "CrossRef").is_none());
    }

    #[test]
    fn cache_expired_negative() {
        let cache = QueryCache::new(Duration::from_secs(3600), Duration::from_millis(1));
        let result: DbQueryResult = (None, vec![], None);
        cache.insert("Paper", "CrossRef", &result);
        std::thread::sleep(Duration::from_millis(10));
        assert!(cache.get("Paper", "CrossRef").is_none());
    }

    #[test]
    fn cache_len_and_empty() {
        let cache = QueryCache::default();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        cache.insert("Paper", "DB", &(Some("Paper".into()), vec![], None));
        assert!(!cache.is_empty());
        assert_eq!(cache.len(), 1);
    }
}
