use crate::audit_log::{
    AuditLogQuery, audit_logs_json_value, read_recent_audit_events_with_scope,
    render_audit_events_human_with_scope,
};
use crate::cli_args::AuditArgs;
use anyhow::{Context, Result};
use crossterm::terminal;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use terminal_ui::print_stdout_line;

pub(crate) fn handle_audit(args: AuditArgs) -> Result<()> {
    let query = AuditLogQuery {
        tail: args.tail,
        component: args.component.as_deref().map(str::trim).map(str::to_string),
        action: args.action.as_deref().map(str::trim).map(str::to_string),
        outcome: args.outcome.as_deref().map(str::trim).map(str::to_string),
    };
    let result = read_recent_audit_events_with_scope(&query)?;

    if args.json {
        let json = serde_json::to_string_pretty(&serde_json::json!({
            "audit_logs": audit_logs_json_value(),
            "search_scope": result.search_scope,
            "filters": {
                "tail": query.tail,
                "component": query.component,
                "action": query.action,
                "outcome": query.outcome,
            },
            "events": result.events,
        }))
        .context("failed to serialize audit log output")?;
        print_stdout_line(&json);
        return Ok(());
    }

    let output = render_audit_events_human_with_scope(
        &result.search_scope.path,
        &query,
        &result.events,
        Some(&result.search_scope),
    );
    print_audit_human_output(&output)?;

    Ok(())
}

fn print_audit_human_output(output: &str) -> Result<()> {
    let height = audit_tui_height(output);
    let Some(mut terminal) = crate::try_inline_stdout_terminal(height) else {
        print_stdout_line(output);
        return Ok(());
    };
    terminal.draw(|frame| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(frame.area());
        let header = Paragraph::new(Line::styled(
            "Prodex Audit",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        );
        frame.render_widget(header, chunks[0]);

        let body = Paragraph::new(audit_tui_text(output))
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(body, chunks[1]);
    })?;
    let _ = terminal.show_cursor();
    Ok(())
}

fn audit_tui_height(output: &str) -> u16 {
    let rows = output.lines().count().saturating_add(4).max(8);
    let terminal_height = terminal::size()
        .map(|(_, height)| usize::from(height))
        .unwrap_or(24);
    rows.min(terminal_height).max(1) as u16
}

fn audit_tui_text(output: &str) -> Text<'static> {
    Text::from(
        output
            .lines()
            .map(|line| Line::from(audit_tui_spans(line)))
            .collect::<Vec<_>>(),
    )
}

fn audit_tui_spans(line: &str) -> Vec<Span<'static>> {
    if line.starts_with("== ") || line.starts_with("Audit") {
        return vec![Span::styled(
            line.to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )];
    }
    if line.contains("success") {
        return vec![Span::styled(
            line.to_string(),
            Style::default().fg(Color::Green),
        )];
    }
    if line.contains("failure") || line.contains("error") {
        return vec![Span::styled(
            line.to_string(),
            Style::default().fg(Color::Red),
        )];
    }
    if line.contains("Filters") || line.contains("Scope") || line.contains("No audit") {
        return vec![Span::styled(
            line.to_string(),
            Style::default().fg(Color::Yellow),
        )];
    }
    vec![Span::raw(line.to_string())]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_tui_text_contains_output() {
        let text = format!("{:?}", audit_tui_text("Audit Log\nsuccess event"));
        assert!(text.contains("Audit Log"));
        assert!(text.contains("success event"));
    }

    #[test]
    fn audit_tui_height_is_bounded_positive() {
        assert!(audit_tui_height("one\ntwo") >= 1);
    }
}
