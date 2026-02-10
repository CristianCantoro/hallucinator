use hallucinator_core::{ProgressEvent, ValidationResult};
use hallucinator_pdf::SkipStats;

/// Events flowing from the backend processing task to the TUI.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum BackendEvent {
    /// PDF text extraction started for paper at queue index.
    ExtractionStarted {
        paper_index: usize,
    },
    /// PDF extraction completed — references parsed.
    ExtractionComplete {
        paper_index: usize,
        ref_count: usize,
        ref_titles: Vec<String>,
        skip_stats: SkipStats,
    },
    /// PDF extraction failed.
    ExtractionFailed {
        paper_index: usize,
        error: String,
    },
    /// Progress event from check_references (checking/result/warning/retry).
    Progress {
        paper_index: usize,
        event: ProgressEvent,
    },
    /// All references for a paper have been checked.
    PaperComplete {
        paper_index: usize,
        results: Vec<ValidationResult>,
    },
    /// All papers have been processed.
    BatchComplete,
}
