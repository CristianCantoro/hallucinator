use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::action::Action;

/// Map a crossterm terminal event to a TUI action.
pub fn map_event(event: &Event) -> Action {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => map_key(key),
        Event::Resize(w, h) => Action::Resize(*w, *h),
        _ => Action::None,
    }
}

fn map_key(key: &KeyEvent) -> Action {
    // Ctrl+C always quits
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Action::Quit;
    }

    match key.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
        KeyCode::Enter => Action::DrillIn,
        KeyCode::Esc => Action::NavigateBack,
        KeyCode::Char('g') => Action::GoTop,
        KeyCode::Char('G') => Action::GoBottom,
        KeyCode::Char('s') => Action::CycleSort,
        KeyCode::Char('?') => Action::ToggleHelp,
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::PageDown,
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::PageUp,
        KeyCode::PageDown => Action::PageDown,
        KeyCode::PageUp => Action::PageUp,
        KeyCode::Home => Action::GoTop,
        KeyCode::End => Action::GoBottom,
        _ => Action::None,
    }
}
