use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use hallucinator_core::Status;

use crate::app::App;
use crate::theme::Theme;
use crate::view::truncate;

/// Render the Reference Detail screen.
pub fn render(f: &mut Frame, app: &App, paper_index: usize, ref_index: usize) {
    let theme = &app.theme;
    let area = f.area();
    let paper = &app.papers[paper_index];
    let refs = &app.ref_states[paper_index];
    let rs = &refs[ref_index];

    let chunks = Layout::vertical([
        Constraint::Length(1), // breadcrumb
        Constraint::Min(5),   // scrollable content
        Constraint::Length(1), // footer
    ])
    .split(area);

    // --- Breadcrumb ---
    let title_short = truncate(&rs.title, 40);
    let breadcrumb = Line::from(vec![
        Span::styled(" HALLUCINATOR ", theme.header_style()),
        Span::styled(" > ", Style::default().fg(theme.dim)),
        Span::styled(
            &paper.filename,
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" > ", Style::default().fg(theme.dim)),
        Span::styled(
            format!("#{} {}", ref_index + 1, title_short),
            Style::default().fg(theme.text),
        ),
    ]);
    f.render_widget(Paragraph::new(breadcrumb), chunks[0]);

    // --- Content ---
    let mut lines: Vec<Line> = Vec::new();

    // CITATION section
    section_header(&mut lines, "CITATION", theme);
    labeled_line(&mut lines, "Title", &rs.title, theme);

    if let Some(result) = &rs.result {
        if !result.raw_citation.is_empty() {
            labeled_line(&mut lines, "Raw Citation", &result.raw_citation, theme);
        }
        if !result.ref_authors.is_empty() {
            labeled_line(
                &mut lines,
                "Ref Authors",
                &result.ref_authors.join(", "),
                theme,
            );
        }

        lines.push(Line::from(""));

        // VALIDATION section
        section_header(&mut lines, "VALIDATION", theme);

        let (status_text, status_color) =
            if result.retraction_info.as_ref().map_or(false, |ri| ri.is_retracted) {
                ("RETRACTED", theme.retracted)
            } else {
                match result.status {
                    Status::Verified => ("Verified", theme.verified),
                    Status::NotFound => ("Not Found", theme.not_found),
                    Status::AuthorMismatch => ("Author Mismatch", theme.author_mismatch),
                }
            };

        lines.push(Line::from(vec![
            Span::styled("  Status:        ", Style::default().fg(theme.dim)),
            Span::styled(
                status_text,
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        if let Some(source) = &result.source {
            labeled_line(&mut lines, "Source", source, theme);
        }
        if !result.found_authors.is_empty() {
            labeled_line(
                &mut lines,
                "Found Authors",
                &result.found_authors.join(", "),
                theme,
            );
        }

        // IDENTIFIERS section
        let has_doi = result.doi_info.is_some();
        let has_arxiv = result.arxiv_info.is_some();
        let has_url = result.paper_url.is_some();
        if has_doi || has_arxiv || has_url {
            lines.push(Line::from(""));
            section_header(&mut lines, "IDENTIFIERS", theme);

            if let Some(doi) = &result.doi_info {
                let validity = if doi.valid { "valid" } else { "invalid" };
                labeled_line(
                    &mut lines,
                    "DOI",
                    &format!("{} ({})", doi.doi, validity),
                    theme,
                );
            }
            if let Some(arxiv) = &result.arxiv_info {
                let validity = if arxiv.valid { "valid" } else { "invalid" };
                labeled_line(
                    &mut lines,
                    "arXiv",
                    &format!("{} ({})", arxiv.arxiv_id, validity),
                    theme,
                );
            }
            if let Some(url) = &result.paper_url {
                labeled_line(&mut lines, "Paper URL", url, theme);
            }
        }

        // RETRACTION section
        if let Some(retraction) = &result.retraction_info {
            if retraction.is_retracted {
                lines.push(Line::from(""));
                section_header(&mut lines, "RETRACTION", theme);
                lines.push(Line::from(Span::styled(
                    "  ⚠ This paper has been retracted!",
                    Style::default()
                        .fg(theme.retracted)
                        .add_modifier(Modifier::BOLD),
                )));
                if let Some(rdoi) = &retraction.retraction_doi {
                    labeled_line(&mut lines, "Retraction DOI", rdoi, theme);
                }
                if let Some(rsrc) = &retraction.retraction_source {
                    labeled_line(&mut lines, "Source", rsrc, theme);
                }
            }
        }

        // FAILED DATABASES section
        if !result.failed_dbs.is_empty() {
            lines.push(Line::from(""));
            section_header(&mut lines, "FAILED DATABASES", theme);
            for db in &result.failed_dbs {
                lines.push(Line::from(Span::styled(
                    format!("  - {db}"),
                    Style::default().fg(theme.not_found),
                )));
            }
        }
    } else {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Result pending...",
            Style::default().fg(theme.dim),
        )));
    }

    let content = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border_style()),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.detail_scroll, 0));

    f.render_widget(content, chunks[1]);

    // --- Footer ---
    render_footer(f, chunks[2], theme);
}

fn section_header<'a>(lines: &mut Vec<Line<'a>>, title: &'a str, theme: &Theme) {
    lines.push(Line::from(Span::styled(
        format!("  {title}"),
        Style::default()
            .fg(theme.active)
            .add_modifier(Modifier::BOLD),
    )));
}

fn labeled_line<'a>(lines: &mut Vec<Line<'a>>, label: &'a str, value: &str, theme: &Theme) {
    lines.push(Line::from(vec![
        Span::styled(
            format!("  {label:<16}"),
            Style::default().fg(theme.dim),
        ),
        Span::styled(value.to_string(), Style::default().fg(theme.text)),
    ]));
}

fn render_footer(f: &mut Frame, area: Rect, theme: &Theme) {
    let footer = Line::from(Span::styled(
        " j/k:scroll  Esc:back  ?:help  q:quit",
        theme.footer_style(),
    ));
    f.render_widget(Paragraph::new(footer), area);
}
