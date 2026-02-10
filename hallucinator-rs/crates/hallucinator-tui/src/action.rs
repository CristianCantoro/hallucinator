/// Actions that the TUI can process, mapped from keyboard input or internal events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Quit,
    NavigateBack,
    DrillIn,
    MoveUp,
    MoveDown,
    PageUp,
    PageDown,
    GoTop,
    GoBottom,
    CycleSort,
    ToggleHelp,
    Tick,
    Resize(u16, u16),
    None,
}
