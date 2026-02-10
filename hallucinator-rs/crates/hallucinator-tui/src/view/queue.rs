use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use ratatui::Frame;

use crate::app::App;
use crate::model::queue::PaperPhase;
use crate::theme::Theme;
use crate::view::{spinner_char, truncate};

/// Render the Queue screen.
pub fn render(f: &mut Frame, app: &App) {
    let theme = &app.theme;
    let area = f.area();

    let chunks = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Min(5),   // table
        Constraint::Length(1), // footer / stats
    ])
    .split(area);

    render_header(f, chunks[0], theme);
    render_table(f, chunks[1], app);
    render_footer(f, chunks[2], app);
}

fn render_header(f: &mut Frame, area: Rect, theme: &Theme) {
    let header = Paragraph::new(Line::from(vec![
        Span::styled(" HALLUCINATOR ", theme.header_style()),
        Span::styled(" Queue", Style::default().fg(theme.text).add_modifier(Modifier::BOLD)),
    ]));
    f.render_widget(header, area);
}

fn render_table(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let wide = area.width >= 100;

    // Build header row
    let header_cells = if wide {
        vec!["#", "Paper", "Refs", "OK", "Mis", "NF", "Ret", "Status"]
    } else {
        vec!["#", "Paper", "Refs", "Prob", "Status"]
    };
    let header = Row::new(
        header_cells
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(theme.text).add_modifier(Modifier::BOLD))),
    )
    .height(1);

    // Use the pre-computed sorted indices
    let indices = &app.queue_sorted;

    // Build data rows
    let rows: Vec<Row> = indices
        .iter()
        .enumerate()
        .map(|(display_idx, &paper_idx)| {
            let paper = &app.papers[paper_idx];
            let num = format!("{}", display_idx + 1);
            let name = truncate(&paper.filename, (area.width as usize).saturating_sub(40));

            let phase_style = Style::default().fg(theme.paper_phase_color(&paper.phase));

            let status_text = match &paper.phase {
                PaperPhase::Checking | PaperPhase::Extracting | PaperPhase::Retrying => {
                    format!("{} {}", spinner_char(app.tick), paper.phase.label())
                }
                _ => paper.phase.label().to_string(),
            };

            if wide {
                let refs = if paper.total_refs > 0 {
                    format!("{}", paper.total_refs)
                } else {
                    "—".to_string()
                };
                Row::new(vec![
                    Cell::from(num),
                    Cell::from(name),
                    Cell::from(refs),
                    Cell::from(format!("{}", paper.stats.verified))
                        .style(Style::default().fg(theme.verified)),
                    Cell::from(format!("{}", paper.stats.author_mismatch))
                        .style(Style::default().fg(theme.author_mismatch)),
                    Cell::from(format!("{}", paper.stats.not_found))
                        .style(Style::default().fg(theme.not_found)),
                    Cell::from(format!("{}", paper.stats.retracted))
                        .style(Style::default().fg(theme.retracted)),
                    Cell::from(status_text).style(phase_style),
                ])
            } else {
                let problems = paper.problems();
                let prob_style = if problems > 0 {
                    Style::default().fg(theme.not_found)
                } else {
                    Style::default().fg(theme.dim)
                };
                Row::new(vec![
                    Cell::from(num),
                    Cell::from(name),
                    Cell::from(if paper.total_refs > 0 {
                        format!("{}", paper.total_refs)
                    } else {
                        "—".to_string()
                    }),
                    Cell::from(format!("{}", problems)).style(prob_style),
                    Cell::from(status_text).style(phase_style),
                ])
            }
        })
        .collect();

    let widths = if wide {
        vec![
            Constraint::Length(4),
            Constraint::Min(20),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(14),
        ]
    } else {
        vec![
            Constraint::Length(4),
            Constraint::Min(15),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(14),
        ]
    };

    let table = Table::new(rows, &widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border_style())
                .title(format!(" Sort: {} (s) ", app.sort_order.label())),
        )
        .row_highlight_style(theme.highlight_style());

    let mut state = TableState::default();
    state.select(Some(app.queue_cursor));
    f.render_stateful_widget(table, area, &mut state);
}

fn render_footer(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let total = app.papers.len();
    let done = app
        .papers
        .iter()
        .filter(|p| p.phase.is_terminal())
        .count();

    let total_verified: usize = app.papers.iter().map(|p| p.stats.verified).sum();
    let total_not_found: usize = app.papers.iter().map(|p| p.stats.not_found).sum();
    let total_mismatch: usize = app.papers.iter().map(|p| p.stats.author_mismatch).sum();
    let total_retracted: usize = app.papers.iter().map(|p| p.stats.retracted).sum();

    let footer = Line::from(vec![
        Span::styled(
            format!(" {}/{} papers ", done, total),
            Style::default().fg(theme.text),
        ),
        Span::styled(
            format!("V:{} ", total_verified),
            Style::default().fg(theme.verified),
        ),
        Span::styled(
            format!("M:{} ", total_mismatch),
            Style::default().fg(theme.author_mismatch),
        ),
        Span::styled(
            format!("NF:{} ", total_not_found),
            Style::default().fg(theme.not_found),
        ),
        Span::styled(
            format!("R:{} ", total_retracted),
            Style::default().fg(theme.retracted),
        ),
        Span::styled(
            " | j/k:nav  Enter:details  s:sort  ?:help  q:quit",
            theme.footer_style(),
        ),
    ]);

    f.render_widget(Paragraph::new(footer), area);
}

