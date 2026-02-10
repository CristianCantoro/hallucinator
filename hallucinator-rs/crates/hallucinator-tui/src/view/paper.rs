use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::model::paper::RefPhase;
use crate::theme::Theme;
use crate::view::{spinner_char, truncate};

/// Render the Paper detail screen.
pub fn render(f: &mut Frame, app: &App, paper_index: usize) {
    let theme = &app.theme;
    let area = f.area();
    let paper = &app.papers[paper_index];
    let show_preview = area.height >= 40;

    let mut constraints = vec![
        Constraint::Length(1), // breadcrumb
        Constraint::Length(3), // progress bar
        Constraint::Min(8),   // ref table
    ];
    if show_preview {
        constraints.push(Constraint::Length(6)); // raw citation preview
    }
    constraints.push(Constraint::Length(1)); // footer

    let chunks = Layout::vertical(constraints).split(area);

    render_breadcrumb(f, chunks[0], &paper.filename, theme);
    render_progress(f, chunks[1], paper, app.tick, theme);
    render_ref_table(f, chunks[2], app, paper_index);

    let footer_chunk = if show_preview {
        // Render preview of the selected reference's raw citation
        render_preview(f, chunks[3], app, paper_index);
        chunks[4]
    } else {
        chunks[3]
    };

    render_footer(f, footer_chunk, paper, theme);
}

fn render_breadcrumb(f: &mut Frame, area: Rect, filename: &str, theme: &Theme) {
    let breadcrumb = Line::from(vec![
        Span::styled(" HALLUCINATOR ", theme.header_style()),
        Span::styled(" > ", Style::default().fg(theme.dim)),
        Span::styled(filename, Style::default().fg(theme.text).add_modifier(Modifier::BOLD)),
    ]);
    f.render_widget(Paragraph::new(breadcrumb), area);
}

fn render_progress(
    f: &mut Frame,
    area: Rect,
    paper: &crate::model::queue::PaperState,
    tick: usize,
    theme: &Theme,
) {
    let done = paper.completed_count();
    let total = paper.total_refs;
    let ratio = if total > 0 {
        done as f64 / total as f64
    } else {
        0.0
    };

    let label = format!(
        "{} {} / {} refs",
        spinner_char(tick),
        done,
        total
    );

    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border_style()),
        )
        .gauge_style(Style::default().fg(theme.active))
        .ratio(ratio)
        .label(label);

    f.render_widget(gauge, area);
}

fn render_ref_table(f: &mut Frame, area: Rect, app: &App, paper_index: usize) {
    let theme = &app.theme;
    let wide = area.width >= 80;

    let header_cells = if wide {
        vec!["#", "Reference", "Verdict", "Source"]
    } else {
        vec!["#", "Reference", "Verdict"]
    };
    let header = Row::new(
        header_cells
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(theme.text).add_modifier(Modifier::BOLD))),
    )
    .height(1);

    let refs = &app.ref_states[paper_index];
    let rows: Vec<Row> = refs
        .iter()
        .map(|rs| {
            let num = format!("{}", rs.index + 1);
            let title_display = match rs.phase {
                RefPhase::Checking => {
                    format!("{} {}", spinner_char(app.tick), rs.title)
                }
                _ => rs.title.clone(),
            };
            let title_text = truncate(&title_display, (area.width as usize).saturating_sub(30));
            let phase_style = theme.ref_phase_style(&rs.phase);

            let verdict = rs.verdict_label();
            let verdict_style = match &rs.result {
                Some(r) => {
                    let color = if r.retraction_info.as_ref().map_or(false, |ri| ri.is_retracted) {
                        theme.retracted
                    } else {
                        theme.status_color(&r.status)
                    };
                    Style::default().fg(color).add_modifier(Modifier::BOLD)
                }
                None => phase_style,
            };

            let mut cells = vec![
                Cell::from(num).style(phase_style),
                Cell::from(title_text).style(phase_style),
                Cell::from(verdict).style(verdict_style),
            ];

            if wide {
                cells.push(Cell::from(rs.source_label()).style(phase_style));
            }

            Row::new(cells)
        })
        .collect();

    let widths = if wide {
        vec![
            Constraint::Length(4),
            Constraint::Min(20),
            Constraint::Length(12),
            Constraint::Length(18),
        ]
    } else {
        vec![
            Constraint::Length(4),
            Constraint::Min(15),
            Constraint::Length(12),
        ]
    };

    let table = Table::new(rows, &widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border_style())
                .title(" References "),
        )
        .row_highlight_style(theme.highlight_style());

    let mut state = TableState::default();
    state.select(Some(app.paper_cursor));
    f.render_stateful_widget(table, area, &mut state);
}

fn render_preview(f: &mut Frame, area: Rect, app: &App, paper_index: usize) {
    let theme = &app.theme;
    let refs = &app.ref_states[paper_index];

    let text = if app.paper_cursor < refs.len() {
        let rs = &refs[app.paper_cursor];
        match &rs.result {
            Some(r) => r.raw_citation.clone(),
            None => "Pending...".to_string(),
        }
    } else {
        String::new()
    };

    let preview = Paragraph::new(text)
        .style(Style::default().fg(theme.dim))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border_style())
                .title(" Raw Citation "),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(preview, area);
}

fn render_footer(
    f: &mut Frame,
    area: Rect,
    paper: &crate::model::queue::PaperState,
    theme: &Theme,
) {
    let footer = Line::from(vec![
        Span::styled(
            format!(
                " V:{} M:{} NF:{} R:{} ",
                paper.stats.verified,
                paper.stats.author_mismatch,
                paper.stats.not_found,
                paper.stats.retracted
            ),
            Style::default().fg(theme.text),
        ),
        Span::styled(
            " | j/k:nav  Enter:detail  Esc:back  ?:help  q:quit",
            theme.footer_style(),
        ),
    ]);

    f.render_widget(Paragraph::new(footer), area);
}
