use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PdfError {
    #[error("failed to open PDF: {0}")]
    OpenError(String),
    #[error("failed to extract text: {0}")]
    ExtractionError(String),
    #[error("no references section found")]
    NoReferencesSection,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// A parsed reference extracted from a PDF.
#[derive(Debug, Clone)]
pub struct Reference {
    pub raw_citation: String,
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
}

/// Statistics about references that were skipped during extraction.
#[derive(Debug, Clone, Default)]
pub struct SkipStats {
    pub url_only: usize,
    pub short_title: usize,
    pub no_title: usize,
}

/// Result of extracting references from a PDF.
#[derive(Debug, Clone)]
pub struct ExtractionResult {
    pub references: Vec<Reference>,
    pub skip_stats: SkipStats,
}

/// Extract references from a PDF file.
///
/// This function:
/// 1. Extracts text from the PDF using MuPDF
/// 2. Locates the References/Bibliography section
/// 3. Segments individual references
/// 4. Parses titles, authors, DOIs, and arXiv IDs from each reference
pub fn extract_references(_pdf_path: &Path) -> Result<ExtractionResult, PdfError> {
    todo!("Phase 1: implement PDF extraction")
}
