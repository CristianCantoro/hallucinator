use hallucinator_core::{Status, ValidationResult};

/// Processing phase of a single reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefPhase {
    Pending,
    Checking,
    Done,
}

/// State of a single reference within a paper.
#[derive(Debug, Clone)]
pub struct RefState {
    pub index: usize,
    pub title: String,
    pub phase: RefPhase,
    pub result: Option<ValidationResult>,
}

impl RefState {
    pub fn verdict_label(&self) -> &str {
        match &self.result {
            None => match self.phase {
                RefPhase::Pending => "—",
                RefPhase::Checking => "...",
                RefPhase::Done => "—",
            },
            Some(r) => match r.status {
                Status::Verified => {
                    if r.retraction_info.as_ref().map_or(false, |ri| ri.is_retracted) {
                        "RETRACTED"
                    } else {
                        "Verified"
                    }
                }
                Status::NotFound => "Not Found",
                Status::AuthorMismatch => "Mismatch",
            },
        }
    }

    pub fn source_label(&self) -> &str {
        match &self.result {
            Some(r) => r.source.as_deref().unwrap_or("—"),
            None => "—",
        }
    }
}
