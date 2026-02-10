pub mod detail;
pub mod help;
pub mod paper;
pub mod queue;

/// Spinner frames for animated progress indication.
const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Get the current spinner character based on a tick counter.
pub fn spinner_char(tick: usize) -> char {
    SPINNER_FRAMES[tick % SPINNER_FRAMES.len()]
}

/// Truncate a string to fit in `max_width` columns, appending "…" if truncated.
pub fn truncate(s: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if s.len() <= max_width {
        return s.to_string();
    }
    let mut truncated: String = s.chars().take(max_width.saturating_sub(1)).collect();
    truncated.push('…');
    truncated
}
