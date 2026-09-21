use anyhow::{Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use prodex_cli::SuperExposeArgs;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::io::{self, IsTerminal};
use std::time::Duration;
use terminal_ui::{
    AlternateScreenTerminal, tui_border_style, tui_connected_footer_block,
    tui_connected_header_block, tui_muted_style, tui_primary_style, tui_title_style,
};

const INPUT_POLL: Duration = Duration::from_millis(100);

pub(super) fn should_confirm_from_tui() -> bool {
    io::stdin().is_terminal()
        && io::stderr().is_terminal()
        && !std::env::args_os().any(|arg| {
            matches!(
                arg.to_str(),
                Some("--listen" | "--no-tunnel" | "--tunnel" | "--openai-tunnel-id" | "--dry-run")
            )
        })
}

pub(super) fn confirm(expose: &SuperExposeArgs) -> Result<bool> {
    let mut terminal = AlternateScreenTerminal::stderr("Super expose TUI")?;
    loop {
        terminal.draw(|frame| draw_frame(frame, expose))?;
        if !event::poll(INPUT_POLL)? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match key.code {
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => return Ok(true),
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(false),
            KeyCode::Char('c') | KeyCode::Char('z')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                bail!("Super expose prompt cancelled")
            }
            _ => {}
        }
    }
}

fn draw_frame(frame: &mut Frame<'_>, expose: &SuperExposeArgs) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(frame.area());
    let header = Paragraph::new(Line::from(vec![
        Span::styled("Prodex Super Expose", tui_title_style()),
        Span::raw("  "),
        Span::styled("ready", tui_muted_style()),
    ]))
    .block(tui_connected_header_block(tui_border_style()));
    frame.render_widget(header, chunks[0]);

    let mode = if expose.mode.exec_only() {
        "exec-only (prodex_super_exec)"
    } else {
        "full MCP surface"
    };
    let body = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Mode: ", tui_muted_style()),
            Span::styled(mode, tui_primary_style()),
        ]),
        Line::from(vec![
            Span::styled("Listen: ", tui_muted_style()),
            Span::styled(expose.listen.clone(), tui_primary_style()),
        ]),
        Line::from("Local loopback access only. The capability URL will be shown after startup."),
        Line::from("Use --openai-tunnel-id explicitly to attach an OpenAI Secure MCP Tunnel."),
    ])
    .block(
        Block::default()
            .borders(Borders::LEFT | Borders::RIGHT)
            .border_style(tui_border_style()),
    )
    .wrap(Wrap { trim: false });
    frame.render_widget(body, chunks[1]);

    let footer = Paragraph::new(Line::styled(
        "enter/y start | q/esc cancel",
        tui_title_style(),
    ))
    .block(tui_connected_footer_block(tui_border_style()));
    frame.render_widget(footer, chunks[2]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn expose_prompt_renders_mode_and_safe_endpoint_guidance() {
        let command = prodex_cli::parse_cli_command_from([
            "prodex",
            "s",
            "expose",
            "exec",
            "--no-presidio",
            "--dry-run",
        ])
        .expect("expose args should parse");
        let prodex_cli::Commands::SuperExpose(args) = command else {
            panic!("expected expose args");
        };
        let mut terminal = Terminal::new(TestBackend::new(96, 12)).expect("test terminal");
        terminal
            .draw(|frame| draw_frame(frame, &args))
            .expect("TUI prompt should render");
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("Prodex Super Expose"));
        assert!(rendered.contains("exec-only"));
        assert!(rendered.contains("--openai-tunnel-id"));
    }
}
