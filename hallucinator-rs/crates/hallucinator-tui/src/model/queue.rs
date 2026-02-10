use hallucinator_core::{CheckStats, ValidationResult, Status};

/// Processing phase of a paper in the queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaperPhase {
    Queued,
    Extracting,
    ExtractionFailed,
    Checking,
    Retrying,
    Complete,
}

impl PaperPhase {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Extracting => "Extracting...",
            Self::ExtractionFailed => "Failed",
            Self::Checking => "Checking...",
            Self::Retrying => "Retrying...",
            Self::Complete => "Done",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Complete | Self::ExtractionFailed)
    }
}

/// State of a single paper in the queue.
#[derive(Debug, Clone)]
pub struct PaperState {
    pub filename: String,
    pub phase: PaperPhase,
    pub total_refs: usize,
    pub stats: CheckStats,
    /// Indexed by reference position; `None` = not yet completed.
    pub results: Vec<Option<ValidationResult>>,
    pub error: Option<String>,
}

impl PaperState {
    pub fn new(filename: String) -> Self {
        Self {
            filename,
            phase: PaperPhase::Queued,
            total_refs: 0,
            stats: CheckStats::default(),
            results: Vec::new(),
            error: None,
        }
    }

    /// Pre-allocate result slots once the reference count is known.
    pub fn init_results(&mut self, count: usize) {
        self.results = vec![None; count];
    }

    /// Record (or replace) a validation result at the given index.
    ///
    /// If the slot already contains a result (retry pass), the old status
    /// counters are decremented before the new ones are incremented, preventing
    /// double-counting.
    pub fn record_result(&mut self, index: usize, result: ValidationResult) {
        // Grow if needed (shouldn't happen after init_results, but be safe)
        if index >= self.results.len() {
            self.results.resize(index + 1, None);
        }

        // Decrement old counters if replacing
        if let Some(old) = &self.results[index] {
            match old.status {
                Status::Verified => self.stats.verified = self.stats.verified.saturating_sub(1),
                Status::NotFound => self.stats.not_found = self.stats.not_found.saturating_sub(1),
                Status::AuthorMismatch => {
                    self.stats.author_mismatch = self.stats.author_mismatch.saturating_sub(1)
                }
            }
            if old.retraction_info.as_ref().map_or(false, |r| r.is_retracted) {
                self.stats.retracted = self.stats.retracted.saturating_sub(1);
            }
        }

        // Increment new counters
        match result.status {
            Status::Verified => self.stats.verified += 1,
            Status::NotFound => self.stats.not_found += 1,
            Status::AuthorMismatch => self.stats.author_mismatch += 1,
        }
        if result.retraction_info.as_ref().map_or(false, |r| r.is_retracted) {
            self.stats.retracted += 1;
        }

        self.results[index] = Some(result);
    }

    /// Number of completed results.
    pub fn completed_count(&self) -> usize {
        self.results.iter().filter(|r| r.is_some()).count()
    }

    /// Number of problems (not_found + author_mismatch + retracted).
    pub fn problems(&self) -> usize {
        self.stats.not_found + self.stats.author_mismatch + self.stats.retracted
    }
}

/// Sort order for the queue table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Original,
    Problems,
    Name,
}

impl SortOrder {
    pub fn next(self) -> Self {
        match self {
            Self::Original => Self::Problems,
            Self::Problems => Self::Name,
            Self::Name => Self::Original,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Original => "order",
            Self::Problems => "problems",
            Self::Name => "name",
        }
    }
}
