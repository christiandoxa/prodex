use super::{DoctorPanel, first_line_of_error};
use anyhow::Result;
use crossterm::terminal;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use redaction::redaction_redact_secret_like_text;
use terminal_ui::{
    fit_cell, print_blank_line, print_panel, print_stdout_line, text_width, tui_border_style,
    tui_connected_header_block, tui_primary_style, tui_secondary_style, tui_title_style,
};

pub(super) fn print_doctor_output(
    panels: &[DoctorPanel],
    suggestion_lines: &[String],
) -> Result<()> {
    let height = doctor_tui_height(panels, suggestion_lines);
    let Some(mut terminal) = crate::try_inline_stdout_terminal(height) else {
        for panel in panels {
            print_panel(&panel.title, &panel.fields)?;
        }
        if !suggestion_lines.is_empty() {
            print_blank_line()?;
            for line in suggestion_lines {
                print_stdout_line(line)?;
            }
        }
        return Ok(());
    };
    terminal.draw(|frame| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(frame.area());
        let header = Paragraph::new(Line::from(vec![
            Span::styled("Prodex Doctor", tui_title_style()),
            Span::raw("  "),
            Span::styled(format!("{} panel(s)", panels.len()), tui_secondary_style()),
        ]))
        .block(tui_connected_header_block(tui_border_style()));
        frame.render_widget(header, chunks[0]);

        let body_width = usize::from(chunks[1].width.saturating_sub(2));
        let body_rows = usize::from(chunks[1].height.saturating_sub(1));
        let body = Paragraph::new(doctor_tui_text_for_viewport(
            panels,
            suggestion_lines,
            body_width,
            body_rows,
        ))
        .block(
            Block::default()
                .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                .border_style(tui_border_style()),
        );
        frame.render_widget(body, chunks[1]);
    })?;
    let _ = terminal.show_cursor();
    Ok(())
}

fn doctor_tui_height(panels: &[DoctorPanel], suggestion_lines: &[String]) -> u16 {
    let rows = doctor_tui_text(panels, suggestion_lines)
        .lines
        .len()
        .saturating_add(4)
        .max(4);
    let terminal_height = terminal::size()
        .map(|(_, height)| usize::from(height))
        .unwrap_or(24);
    rows.min(terminal_height).max(1) as u16
}

fn doctor_tui_text(panels: &[DoctorPanel], suggestion_lines: &[String]) -> Text<'static> {
    doctor_tui_text_for_viewport(panels, suggestion_lines, usize::MAX, usize::MAX)
}

fn doctor_tui_text_for_viewport(
    panels: &[DoctorPanel],
    suggestion_lines: &[String],
    width: usize,
    max_rows: usize,
) -> Text<'static> {
    let mut lines = Vec::new();
    for panel in panels {
        lines.push((
            Line::styled(fit_cell(&panel.title, width), tui_title_style()),
            false,
        ));
        let label_width = panel
            .fields
            .iter()
            .map(|(label, _)| text_width(label))
            .max()
            .unwrap_or(0)
            .min(24)
            .min(width.saturating_div(2));
        for (label, value) in &panel.fields {
            let color = doctor_value_color(label, value);
            let rendered_label = fit_cell(
                &format!(
                    "{label}{} ",
                    " ".repeat(label_width.saturating_sub(text_width(label)))
                ),
                label_width.saturating_add(1).min(width),
            );
            let value_width = width.saturating_sub(text_width(&rendered_label));
            lines.push((
                Line::from(vec![
                    Span::styled(
                        rendered_label,
                        tui_secondary_style().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(fit_cell(value, value_width), Style::default().fg(color)),
                ]),
                color == Color::Red,
            ));
        }
    }
    if !suggestion_lines.is_empty() {
        lines.push((Line::raw(""), false));
        lines.push((
            Line::styled(fit_cell("Policy Suggestions", width), tui_title_style()),
            false,
        ));
        for line in suggestion_lines {
            lines.push((
                Line::styled(fit_cell(line, width), tui_primary_style()),
                false,
            ));
        }
    }
    if lines.len() > max_rows {
        if max_rows == 0 {
            return Text::default();
        }
        let visible_rows = max_rows.saturating_sub(1);
        let hidden = lines.len().saturating_sub(visible_rows);
        let hidden_critical = lines
            .iter()
            .skip(visible_rows)
            .find(|(_, critical)| *critical)
            .map(|(line, _)| line.clone());
        lines.truncate(visible_rows);
        if let Some(critical) = hidden_critical
            && let Some(last) = lines.last_mut()
        {
            last.0 = critical;
        }
        lines.push((
            Line::styled(
                fit_cell(&format!("… {hidden} row(s) hidden"), width),
                tui_secondary_style(),
            ),
            false,
        ));
    }
    Text::from(lines.into_iter().map(|(line, _)| line).collect::<Vec<_>>())
}

fn doctor_value_color(label: &str, value: &str) -> Color {
    let lower = value.to_ascii_lowercase();
    if lower.contains("error")
        || lower.contains("missing")
        || lower.contains("blocked")
        || lower.contains("warning")
        || lower.contains("orphan")
        || lower.contains("critical")
        || lower.contains("thin")
        || lower.contains("degraded")
    {
        Color::Red
    } else if lower.contains("ready") || lower.contains("yes") || lower.contains("exists") {
        Color::Green
    } else if label.contains("Runtime") || label.contains("Quota") || label.contains("Main") {
        Color::Cyan
    } else {
        Color::Reset
    }
}

pub(super) fn doctor_quota_error_summary(err: &str) -> String {
    let redacted = redaction_redact_secret_like_text(err);
    format!("Error ({})", first_line_of_error(&redacted))
}

#[cfg(test)]
mod tests {
    use super::{
        Color, DoctorPanel, doctor_quota_error_summary, doctor_tui_text,
        doctor_tui_text_for_viewport, doctor_value_color,
    };

    #[test]
    fn doctor_tui_text_contains_panels_and_suggestions() {
        let panels = vec![DoctorPanel {
            title: "Doctor".to_string(),
            fields: vec![
                ("Runtime".to_string(), "ready".to_string()),
                ("Quota".to_string(), "Blocked".to_string()),
            ],
        }];
        let suggestions = vec!["increase active_request_limit".to_string()];
        let text = format!("{:?}", doctor_tui_text(&panels, &suggestions));
        assert!(text.contains("Doctor"));
        assert!(text.contains("ready"));
        assert!(text.contains("Blocked"));
        assert!(text.contains("Policy Suggestions"));
    }

    #[test]
    fn doctor_tui_text_does_not_pad_between_panels() {
        let panels = vec![
            DoctorPanel {
                title: "One".to_string(),
                fields: vec![("Runtime".to_string(), "ready".to_string())],
            },
            DoctorPanel {
                title: "Two".to_string(),
                fields: vec![("Quota".to_string(), "Ready".to_string())],
            },
        ];

        let lines = doctor_tui_text(&panels, &[]).lines;
        assert_eq!(lines.len(), 4);
        assert!(format!("{:?}", lines[2]).contains("Two"));
    }

    #[test]
    fn doctor_value_color_highlights_status() {
        assert_eq!(doctor_value_color("Quota", "Blocked"), Color::Red);
        assert_eq!(doctor_value_color("Runtime", "ready"), Color::Green);
        assert_eq!(doctor_value_color("Runtime", "critical"), Color::Red);
    }

    #[test]
    fn doctor_quota_error_summary_redacts_secret_like_material() {
        let err = "failed: Authorization: Bearer fixture-token-123 url=https://example.test?api_key=sk-fixture-123";

        let summary = doctor_quota_error_summary(err);

        assert!(summary.contains("Authorization: Bearer <redacted>"));
        assert!(summary.contains("api_key=<redacted>"));
        assert!(!summary.contains("fixture-token-123"));
        assert!(!summary.contains("sk-fixture-123"));
    }

    #[test]
    fn doctor_tui_layout_fits_short_terminals_and_retains_an_error() {
        let panels = vec![DoctorPanel {
            title: "Doctor".to_string(),
            fields: (0..12)
                .map(|index| {
                    (
                        format!("Status {index}"),
                        if index == 11 {
                            "critical error".to_string()
                        } else {
                            "ready with a deliberately long diagnostic value".to_string()
                        },
                    )
                })
                .collect(),
        }];

        for (width, height) in [(40_usize, 8_usize), (60, 10), (80, 12)] {
            let body_rows = height.saturating_sub(4);
            let text = doctor_tui_text_for_viewport(&panels, &[], width - 2, body_rows);
            assert!(text.lines.len() <= body_rows);
            assert!(text.lines.iter().all(|line| line.width() <= width - 2));
            let rendered = format!("{text:?}");
            assert!(rendered.contains("critical error"));
            assert!(rendered.contains("hidden"));
        }
    }
}
