use super::{DoctorPanel, first_line_of_error};
use anyhow::Result;
use crossterm::terminal;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use redaction::redaction_redact_secret_like_text;
use terminal_ui::{
    print_blank_line, print_panel, print_stdout_line, text_width, tui_border_style,
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

        let body = Paragraph::new(doctor_tui_text(panels, suggestion_lines))
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                    .border_style(tui_border_style()),
            )
            .wrap(Wrap { trim: false });
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
    let mut lines = Vec::new();
    for panel in panels {
        lines.push(Line::styled(panel.title.clone(), tui_title_style()));
        let label_width = panel
            .fields
            .iter()
            .map(|(label, _)| text_width(label))
            .max()
            .unwrap_or(0)
            .min(24);
        for (label, value) in &panel.fields {
            lines.push(Line::from(vec![
                Span::styled(
                    format!(
                        "{label}{} ",
                        " ".repeat(label_width.saturating_sub(text_width(label)))
                    ),
                    tui_secondary_style().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    value.clone(),
                    Style::default().fg(doctor_value_color(label, value)),
                ),
            ]));
        }
    }
    if !suggestion_lines.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled("Policy Suggestions", tui_title_style()));
        for line in suggestion_lines {
            lines.push(Line::styled(line.clone(), tui_primary_style()));
        }
    }
    Text::from(lines)
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
        Color, DoctorPanel, doctor_quota_error_summary, doctor_tui_text, doctor_value_color,
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
}
