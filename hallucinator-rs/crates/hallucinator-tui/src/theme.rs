use ratatui::style::{Color, Modifier, Style};

use crate::model::paper::RefPhase;
use crate::model::queue::PaperPhase;
use hallucinator_core::Status;

/// Color theme for the TUI.
pub struct Theme {
    pub verified: Color,
    pub not_found: Color,
    pub author_mismatch: Color,
    pub retracted: Color,

    pub header_fg: Color,
    pub header_bg: Color,
    pub border: Color,
    pub text: Color,
    pub dim: Color,
    pub highlight_bg: Color,
    pub active: Color,
    pub queued: Color,
    pub spinner: Color,
    pub footer_fg: Color,
    pub footer_bg: Color,
}

impl Theme {
    /// Hacker-green terminal theme.
    pub fn hacker() -> Self {
        Self {
            verified: Color::Green,
            not_found: Color::Red,
            author_mismatch: Color::Yellow,
            retracted: Color::Magenta,

            header_fg: Color::Black,
            header_bg: Color::Green,
            border: Color::DarkGray,
            text: Color::White,
            dim: Color::DarkGray,
            highlight_bg: Color::Rgb(30, 50, 30),
            active: Color::Cyan,
            queued: Color::DarkGray,
            spinner: Color::Cyan,
            footer_fg: Color::DarkGray,
            footer_bg: Color::Reset,
        }
    }

    pub fn status_color(&self, status: &Status) -> Color {
        match status {
            Status::Verified => self.verified,
            Status::NotFound => self.not_found,
            Status::AuthorMismatch => self.author_mismatch,
        }
    }

    pub fn paper_phase_color(&self, phase: &PaperPhase) -> Color {
        match phase {
            PaperPhase::Queued => self.queued,
            PaperPhase::Extracting => self.active,
            PaperPhase::ExtractionFailed => self.not_found,
            PaperPhase::Checking => self.active,
            PaperPhase::Retrying => self.author_mismatch,
            PaperPhase::Complete => self.verified,
        }
    }

    pub fn ref_phase_style(&self, phase: &RefPhase) -> Style {
        match phase {
            RefPhase::Pending => Style::default().fg(self.dim),
            RefPhase::Checking => Style::default().fg(self.spinner).add_modifier(Modifier::BOLD),
            RefPhase::Done => Style::default().fg(self.text),
        }
    }

    pub fn header_style(&self) -> Style {
        Style::default().fg(self.header_fg).bg(self.header_bg).add_modifier(Modifier::BOLD)
    }

    pub fn highlight_style(&self) -> Style {
        Style::default().bg(self.highlight_bg).add_modifier(Modifier::BOLD)
    }

    pub fn border_style(&self) -> Style {
        Style::default().fg(self.border)
    }

    pub fn footer_style(&self) -> Style {
        Style::default().fg(self.footer_fg).bg(self.footer_bg)
    }
}
