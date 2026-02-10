use hallucinator_core::ProgressEvent;

use crate::action::Action;
use crate::tui_event::BackendEvent;
use crate::model::paper::{RefPhase, RefState};
use crate::model::queue::{PaperPhase, PaperState, SortOrder};
use crate::theme::Theme;

/// Which screen is currently displayed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Queue,
    Paper(usize),                // index into papers vec
    RefDetail(usize, usize),     // (paper_index, ref_index)
}

/// Main application state.
pub struct App {
    pub screen: Screen,
    pub papers: Vec<PaperState>,
    /// Per-paper reference states, indexed in parallel with `papers`.
    pub ref_states: Vec<Vec<RefState>>,
    pub queue_cursor: usize,
    pub paper_cursor: usize,
    pub sort_order: SortOrder,
    /// Maps visual row index → paper index (recomputed on sort/tick).
    pub queue_sorted: Vec<usize>,
    pub tick: usize,
    pub theme: Theme,
    pub should_quit: bool,
    pub batch_complete: bool,
    pub show_help: bool,
    pub detail_scroll: u16,
    /// Height of the visible table area (set on resize, used for page up/down).
    pub visible_rows: usize,
}

impl App {
    pub fn new(filenames: Vec<String>) -> Self {
        let papers: Vec<PaperState> = filenames
            .into_iter()
            .map(PaperState::new)
            .collect();
        let ref_states = vec![Vec::new(); papers.len()];
        let queue_sorted: Vec<usize> = (0..papers.len()).collect();

        Self {
            screen: Screen::Queue,
            papers,
            ref_states,
            queue_cursor: 0,
            paper_cursor: 0,
            sort_order: SortOrder::Original,
            queue_sorted,
            tick: 0,
            theme: Theme::hacker(),
            should_quit: false,
            batch_complete: false,
            show_help: false,
            detail_scroll: 0,
            visible_rows: 20,
        }
    }

    /// Recompute `queue_sorted` based on the current `sort_order`.
    pub fn recompute_sorted_indices(&mut self) {
        let mut indices: Vec<usize> = (0..self.papers.len()).collect();
        match self.sort_order {
            SortOrder::Original => {} // already in order
            SortOrder::Problems => {
                indices.sort_by(|&a, &b| {
                    self.papers[b]
                        .problems()
                        .cmp(&self.papers[a].problems())
                        .then_with(|| a.cmp(&b))
                });
            }
            SortOrder::Name => {
                indices.sort_by(|&a, &b| {
                    self.papers[a].filename.cmp(&self.papers[b].filename)
                });
            }
        }
        self.queue_sorted = indices;
    }

    /// Process a user action and update state. Returns true if the app should quit.
    pub fn update(&mut self, action: Action) -> bool {
        // When help overlay is shown, only allow a few actions through
        if self.show_help {
            match action {
                Action::Quit => {
                    self.should_quit = true;
                    return true;
                }
                Action::ToggleHelp | Action::NavigateBack => {
                    self.show_help = false;
                }
                Action::Tick => {
                    self.tick = self.tick.wrapping_add(1);
                }
                Action::Resize(_w, h) => {
                    self.visible_rows = (h as usize).saturating_sub(6);
                }
                _ => {} // swallow everything else
            }
            return false;
        }

        match action {
            Action::Quit => {
                self.should_quit = true;
                return true;
            }
            Action::ToggleHelp => {
                self.show_help = true;
            }
            Action::NavigateBack => match &self.screen {
                Screen::RefDetail(paper_idx, _) => {
                    let paper_idx = *paper_idx;
                    self.screen = Screen::Paper(paper_idx);
                    // paper_cursor is preserved (not reset)
                }
                Screen::Paper(_) | Screen::Queue => {
                    self.screen = Screen::Queue;
                    self.paper_cursor = 0;
                }
            },
            Action::DrillIn => match &self.screen {
                Screen::Queue => {
                    if self.queue_cursor < self.queue_sorted.len() {
                        let paper_idx = self.queue_sorted[self.queue_cursor];
                        self.screen = Screen::Paper(paper_idx);
                        self.paper_cursor = 0;
                    }
                }
                Screen::Paper(idx) => {
                    let idx = *idx;
                    let ref_count = self.ref_states[idx].len();
                    if self.paper_cursor < ref_count {
                        self.detail_scroll = 0;
                        self.screen = Screen::RefDetail(idx, self.paper_cursor);
                    }
                }
                Screen::RefDetail(..) => {}
            },
            Action::MoveDown => match &self.screen {
                Screen::Queue => {
                    if self.queue_cursor + 1 < self.papers.len() {
                        self.queue_cursor += 1;
                    }
                }
                Screen::Paper(idx) => {
                    let max = self.ref_states[*idx].len().saturating_sub(1);
                    if self.paper_cursor < max {
                        self.paper_cursor += 1;
                    }
                }
                Screen::RefDetail(..) => {
                    self.detail_scroll = self.detail_scroll.saturating_add(1);
                }
            },
            Action::MoveUp => match &self.screen {
                Screen::Queue => {
                    self.queue_cursor = self.queue_cursor.saturating_sub(1);
                }
                Screen::Paper(_) => {
                    self.paper_cursor = self.paper_cursor.saturating_sub(1);
                }
                Screen::RefDetail(..) => {
                    self.detail_scroll = self.detail_scroll.saturating_sub(1);
                }
            },
            Action::PageDown => {
                let page = self.visible_rows.max(1);
                match &self.screen {
                    Screen::Queue => {
                        self.queue_cursor = (self.queue_cursor + page)
                            .min(self.papers.len().saturating_sub(1));
                    }
                    Screen::Paper(idx) => {
                        let max = self.ref_states[*idx].len().saturating_sub(1);
                        self.paper_cursor = (self.paper_cursor + page).min(max);
                    }
                    Screen::RefDetail(..) => {
                        self.detail_scroll =
                            self.detail_scroll.saturating_add(page as u16);
                    }
                }
            }
            Action::PageUp => {
                let page = self.visible_rows.max(1);
                match &self.screen {
                    Screen::Queue => {
                        self.queue_cursor = self.queue_cursor.saturating_sub(page);
                    }
                    Screen::Paper(_) => {
                        self.paper_cursor = self.paper_cursor.saturating_sub(page);
                    }
                    Screen::RefDetail(..) => {
                        self.detail_scroll =
                            self.detail_scroll.saturating_sub(page as u16);
                    }
                }
            }
            Action::GoTop => match &self.screen {
                Screen::Queue => self.queue_cursor = 0,
                Screen::Paper(_) => self.paper_cursor = 0,
                Screen::RefDetail(..) => self.detail_scroll = 0,
            },
            Action::GoBottom => match &self.screen {
                Screen::Queue => {
                    self.queue_cursor = self.papers.len().saturating_sub(1);
                }
                Screen::Paper(idx) => {
                    self.paper_cursor = self.ref_states[*idx].len().saturating_sub(1);
                }
                Screen::RefDetail(..) => {
                    self.detail_scroll = u16::MAX; // clamped by Paragraph rendering
                }
            },
            Action::CycleSort => {
                if self.screen == Screen::Queue {
                    self.sort_order = self.sort_order.next();
                    self.recompute_sorted_indices();
                }
            }
            Action::Tick => {
                self.tick = self.tick.wrapping_add(1);
                if self.screen == Screen::Queue {
                    self.recompute_sorted_indices();
                }
            }
            Action::Resize(_w, h) => {
                // Rough estimate: total height minus header/footer/borders
                self.visible_rows = (h as usize).saturating_sub(6);
            }
            Action::None => {}
        }
        false
    }

    /// Process a backend event and update model state.
    pub fn handle_backend_event(&mut self, event: BackendEvent) {
        match event {
            BackendEvent::ExtractionStarted { paper_index } => {
                if let Some(paper) = self.papers.get_mut(paper_index) {
                    paper.phase = PaperPhase::Extracting;
                }
            }
            BackendEvent::ExtractionComplete {
                paper_index,
                ref_count,
                ref_titles,
                skip_stats: _,
            } => {
                if let Some(paper) = self.papers.get_mut(paper_index) {
                    paper.total_refs = ref_count;
                    paper.init_results(ref_count);
                    paper.phase = PaperPhase::Checking;
                }
                // Initialize ref states for this paper
                if paper_index < self.ref_states.len() {
                    self.ref_states[paper_index] = ref_titles
                        .into_iter()
                        .enumerate()
                        .map(|(i, title)| RefState {
                            index: i,
                            title,
                            phase: RefPhase::Pending,
                            result: None,
                        })
                        .collect();
                }
            }
            BackendEvent::ExtractionFailed { paper_index, error } => {
                if let Some(paper) = self.papers.get_mut(paper_index) {
                    paper.phase = PaperPhase::ExtractionFailed;
                    paper.error = Some(error);
                }
            }
            BackendEvent::Progress { paper_index, event } => {
                self.handle_progress(paper_index, event);
            }
            BackendEvent::PaperComplete {
                paper_index,
                results: _,
            } => {
                if let Some(paper) = self.papers.get_mut(paper_index) {
                    if paper.phase != PaperPhase::ExtractionFailed {
                        paper.phase = PaperPhase::Complete;
                    }
                }
            }
            BackendEvent::BatchComplete => {
                self.batch_complete = true;
            }
        }
    }

    fn handle_progress(&mut self, paper_index: usize, event: ProgressEvent) {
        match event {
            ProgressEvent::Checking { index, .. } => {
                if let Some(refs) = self.ref_states.get_mut(paper_index) {
                    if let Some(rs) = refs.get_mut(index) {
                        rs.phase = RefPhase::Checking;
                    }
                }
            }
            ProgressEvent::Result {
                index, result, ..
            } => {
                if let Some(paper) = self.papers.get_mut(paper_index) {
                    paper.record_result(index, result.clone());
                }
                if let Some(refs) = self.ref_states.get_mut(paper_index) {
                    if let Some(rs) = refs.get_mut(index) {
                        rs.phase = RefPhase::Done;
                        rs.result = Some(result);
                    }
                }
            }
            ProgressEvent::Warning { .. } => {
                // Warnings are informational; the result event will follow
            }
            ProgressEvent::RetryPass { .. } => {
                if let Some(paper) = self.papers.get_mut(paper_index) {
                    paper.phase = PaperPhase::Retrying;
                }
            }
        }
    }

    /// Render the current screen.
    pub fn view(&self, f: &mut ratatui::Frame) {
        match &self.screen {
            Screen::Queue => crate::view::queue::render(f, self),
            Screen::Paper(idx) => crate::view::paper::render(f, self, *idx),
            Screen::RefDetail(paper_idx, ref_idx) => {
                crate::view::detail::render(f, self, *paper_idx, *ref_idx)
            }
        }

        if self.show_help {
            crate::view::help::render(f, &self.theme);
        }
    }
}
