use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::theme::Theme;

/// Render the help overlay as a centered popup.
pub fn render(f: &mut Frame, theme: &Theme) {
    let area = f.area();
    let popup = centered_rect(60, 22, area);

    let lines = vec![
        Line::from(Span::styled(
            " Keyboard Shortcuts ",
            Style::default()
                .fg(theme.header_fg)
                .bg(theme.header_bg)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        section_header("Navigation", theme),
        key_line("j / ↓", "Move down", theme),
        key_line("k / ↑", "Move up", theme),
        key_line("Ctrl+d / PgDn", "Page down", theme),
        key_line("Ctrl+u / PgUp", "Page up", theme),
        key_line("g / Home", "Go to top", theme),
        key_line("G / End", "Go to bottom", theme),
        key_line("Enter", "Drill in (open paper / reference)", theme),
        key_line("Esc", "Go back", theme),
        Line::from(""),
        section_header("Queue Screen", theme),
        key_line("s", "Cycle sort order", theme),
        Line::from(""),
        section_header("Global", theme),
        key_line("?", "Toggle this help", theme),
        key_line("q", "Quit", theme),
        key_line("Ctrl+c", "Force quit", theme),
    ];

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.active))
                .title(" Help "),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(Clear, popup);
    f.render_widget(paragraph, popup);
}

fn section_header<'a>(title: &'a str, theme: &Theme) -> Line<'a> {
    Line::from(Span::styled(
        format!("  {title}"),
        Style::default()
            .fg(theme.active)
            .add_modifier(Modifier::BOLD),
    ))
}

fn key_line<'a>(key: &'a str, desc: &'a str, theme: &Theme) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("    {key:<16}"), Style::default().fg(theme.text)),
        Span::styled(desc, Style::default().fg(theme.dim)),
    ])
}

/// Create a centered rectangle of the given width (columns) and height (rows).
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(area);
    Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .split(vertical[0])[0]
}
