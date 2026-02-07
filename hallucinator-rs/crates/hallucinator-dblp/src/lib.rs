use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DblpError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("download error: {0}")]
    Download(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// A publication record from the offline DBLP database.
#[derive(Debug, Clone)]
pub struct DblpRecord {
    pub title: String,
    pub authors: Vec<String>,
    pub url: Option<String>,
}

/// Query result from the offline DBLP database.
#[derive(Debug, Clone)]
pub struct DblpQueryResult {
    pub record: DblpRecord,
    pub score: f64,
}

/// Offline DBLP database handle.
pub struct DblpDatabase {
    _db_path: std::path::PathBuf,
}

impl DblpDatabase {
    /// Open an existing offline DBLP database.
    pub fn open(_path: &Path) -> Result<Self, DblpError> {
        todo!("Phase 4: implement DBLP database opening")
    }

    /// Query the database for a title, returning the best match if above threshold.
    pub fn query(&self, _title: &str) -> Result<Option<DblpQueryResult>, DblpError> {
        todo!("Phase 4: implement DBLP query")
    }

    /// Check if the database is stale (older than threshold days).
    pub fn is_stale(&self, _threshold_days: u64) -> bool {
        todo!("Phase 4: implement staleness check")
    }
}

/// Download and build the offline DBLP database from dblp.org.
pub fn build_database(
    _output_path: &Path,
    _progress: impl Fn(u64, u64),
) -> Result<(), DblpError> {
    todo!("Phase 4: implement DBLP database builder")
}
