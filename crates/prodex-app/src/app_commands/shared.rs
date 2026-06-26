use crate::audit_log::append_audit_event;
use anyhow::Result;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::env;
use std::io::{self, IsTerminal};

pub(crate) use prodex_core::{absolutize, default_codex_home};
#[cfg(test)]
pub(crate) use prodex_core::{same_path, select_default_codex_home};

pub(crate) fn audit_log_event_best_effort(
    component: &str,
    action: &str,
    outcome: &str,
    details: serde_json::Value,
) {
    let _ = append_audit_event(component, action, outcome, details);
}

pub(crate) fn print_launch_status(message: &str) {
    if print_launch_status_tui(message).is_err() {
        eprintln!("Prodex launch: {message}");
    }
}

pub(crate) fn try_inline_stdout_terminal(
    height: u16,
) -> Option<Terminal<CrosstermBackend<io::Stdout>>> {
    if !inline_tui_allowed() || !io::stdout().is_terminal() {
        return None;
    }
    Terminal::with_options(
        CrosstermBackend::new(io::stdout()),
        ratatui::TerminalOptions {
            viewport: ratatui::Viewport::Inline(height),
        },
    )
    .ok()
}

pub(crate) fn try_inline_stderr_terminal(
    height: u16,
) -> Option<Terminal<CrosstermBackend<io::Stderr>>> {
    if !inline_tui_allowed() || !io::stderr().is_terminal() {
        return None;
    }
    Terminal::with_options(
        CrosstermBackend::new(io::stderr()),
        ratatui::TerminalOptions {
            viewport: ratatui::Viewport::Inline(height),
        },
    )
    .ok()
}

fn inline_tui_allowed() -> bool {
    if env::var_os("PRODEX_FORCE_TUI").is_some() {
        return true;
    }
    env::var_os("CODEX_CI").is_none()
        && env::var("CI")
            .map(|value| {
                !matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes"
                )
            })
            .unwrap_or(true)
}

fn print_launch_status_tui(message: &str) -> Result<()> {
    let Some(mut terminal) = try_inline_stderr_terminal(5) else {
        anyhow::bail!("stderr is not a terminal");
    };
    terminal.draw(|frame| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(frame.area());
        let header = Paragraph::new(Line::from(vec![
            Span::styled(
                "Prodex Launch",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("preflight", Style::default().fg(Color::DarkGray)),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        );
        frame.render_widget(header, chunks[0]);

        let body = Paragraph::new(Line::from(vec![
            Span::styled("Status ", Style::default().fg(Color::DarkGray)),
            Span::styled(message.to_string(), Style::default().fg(Color::White)),
        ]))
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
