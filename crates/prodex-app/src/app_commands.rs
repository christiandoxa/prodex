use super::*;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::io::{BufRead, IsTerminal, Write};
use terminal_ui::{
    tui_border_style, tui_connected_footer_block, tui_connected_header_block, tui_hint_style,
    tui_primary_style, tui_secondary_style, tui_success_style, tui_title_style,
};

mod app_server_broker;
mod audit;
mod broker;
mod capability;
mod child_process;
mod cleanup;
mod context;
mod dashboard;
mod doctor;
mod gateway;
mod info;
mod info_handler;
mod log;
mod log_format;
mod log_tui;
mod log_upstream;
mod log_upstream_payload;
mod mcp_jsonl_bridge;
mod ping;
mod presidio;
mod quota;
mod redeem;
pub(crate) mod runtime_launch;
mod selection;
mod session;
mod shared;
mod status;

pub(crate) use self::app_server_broker::*;
pub(crate) use self::audit::*;
pub(crate) use self::broker::*;
pub(crate) use self::capability::*;
pub(crate) use self::child_process::*;
pub(crate) use self::cleanup::*;
pub(crate) use self::context::*;
pub(crate) use self::dashboard::*;
pub(crate) use self::doctor::*;
pub(crate) use self::gateway::*;
pub(crate) use self::info::*;
pub(crate) use self::info_handler::*;
pub(crate) use self::log::*;
pub(crate) use self::mcp_jsonl_bridge::*;
pub(crate) use self::ping::*;
pub(crate) use self::presidio::*;
pub(crate) use self::quota::*;
pub(crate) use self::redeem::*;
pub(crate) use self::selection::*;
pub(crate) use self::session::*;
pub(crate) use self::shared::*;
pub(crate) use self::status::*;

pub(super) fn handle_run(args: RunArgs) -> Result<()> {
    runtime_launch::handle_run(args)
}

pub(crate) fn start_policy_gateway_backend_inner(
    preferred_listen_addr: Option<String>,
) -> Result<GatewayBackend> {
    runtime_launch::start_policy_gateway_backend_inner(preferred_listen_addr)
}

pub(crate) fn start_policy_gateway_application_inner(
    service_mode: RuntimePolicyServiceMode,
) -> Result<GatewayApplication> {
    runtime_launch::start_policy_gateway_application_inner(service_mode)
}

pub(super) fn handle_super(args: SuperArgs) -> Result<()> {
    args.validate_urls().map_err(anyhow::Error::msg)?;
    let use_presidio = match args.presidio_preference() {
        Some(use_presidio) => use_presidio,
        None => prompt_super_presidio_opt_in()?,
    };
    if matches!(
        args.cli,
        Some(SuperCliAgent::Gemini | SuperCliAgent::Kiro | SuperCliAgent::Agy)
    ) {
        return crate::runtime_gemini_cli::handle_super_google_cli(args, use_presidio);
    }
    handle_caveman(args.into_caveman_args_with_presidio(use_presidio))
}

pub(super) fn prepare_runtime_launch_with_harness(
    request: RuntimeLaunchRequest<'_>,
    resolved_harness: prodex_provider_core::ResolvedHarnessMode,
) -> Result<PreparedRuntimeLaunch> {
    runtime_launch::prepare_runtime_launch_with_harness(request, resolved_harness)
}

pub(super) fn prepare_runtime_launch_dry_run(
    request: RuntimeLaunchRequest<'_>,
) -> Result<PreparedRuntimeLaunch> {
    runtime_launch::prepare_runtime_launch_dry_run(request)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn resolve_runtime_launch_profile_name(
    state: &AppState,
    requested: Option<&str>,
) -> Result<String> {
    runtime_launch::resolve_runtime_launch_profile_name(state, requested)
}

fn prompt_super_presidio_opt_in() -> Result<bool> {
    let stdin = io::stdin();
    let stderr = io::stderr();
    let stdin_is_terminal = stdin.is_terminal();
    let stderr_is_terminal = stderr.is_terminal();
    if stdin_is_terminal
        && stderr_is_terminal
        && let Ok(enabled) = prompt_super_opt_in_tui(
            "Prodex Super",
            "Use Presidio for data safety?",
            "Presidio redacts sensitive text through the configured local Presidio services before runtime proxy forwarding.",
        )
    {
        return Ok(enabled);
    }
    prompt_super_presidio_opt_in_from(stdin_is_terminal, stderr_is_terminal, stdin.lock(), stderr)
}

fn prompt_super_presidio_opt_in_from<R, W>(
    stdin_is_terminal: bool,
    stderr_is_terminal: bool,
    mut input: R,
    mut output: W,
) -> Result<bool>
where
    R: BufRead,
    W: Write,
{
    if !stdin_is_terminal || !stderr_is_terminal {
        return Ok(false);
    }

    write!(output, "Use Presidio for data safety? [y/N] ")?;
    output.flush().context("failed to flush prompt")?;
    let mut answer = String::new();
    input
        .read_line(&mut answer)
        .context("failed to read Presidio prompt answer")?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

type SuperPromptTui = terminal_ui::AlternateScreenTerminal<io::Stderr>;

fn prompt_super_opt_in_tui(title: &str, question: &str, detail: &str) -> Result<bool> {
    let mut tui = SuperPromptTui::stderr("super prompt TUI")?;
    loop {
        tui.terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(3),
                    Constraint::Length(3),
                ])
                .split(frame.area());
            let header = Paragraph::new(Line::from(vec![
                Span::styled(title.to_string(), tui_title_style()),
                Span::raw("  "),
                Span::styled("launch option", tui_secondary_style()),
            ]))
            .block(tui_connected_header_block(tui_border_style()));
            frame.render_widget(header, chunks[0]);

            let body = Paragraph::new(vec![
                Line::from(Span::styled(
                    question.to_string(),
                    tui_primary_style().add_modifier(Modifier::BOLD),
                )),
                Line::raw(""),
                Line::from(Span::styled(detail.to_string(), tui_secondary_style())),
            ])
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT)
                    .border_style(tui_border_style()),
            )
            .wrap(Wrap { trim: false });
            frame.render_widget(body, chunks[1]);

            let footer = Paragraph::new(Line::from(vec![
                Span::styled("y", tui_success_style()),
                Span::raw(" enable  "),
                Span::styled("n", tui_hint_style()),
                Span::raw(" skip  "),
                Span::styled("enter", tui_hint_style()),
                Span::raw(" skip  "),
                Span::styled("esc", tui_hint_style()),
                Span::raw(" skip"),
            ]))
            .block(tui_connected_footer_block(tui_border_style()));
            frame.render_widget(footer, chunks[2]);
        })?;

        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => return Ok(true),
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Enter | KeyCode::Esc => {
                    return Ok(false);
                }
                KeyCode::Char('c') | KeyCode::Char('z')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    return Ok(false);
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt_answer(answer: &str) -> Result<(bool, String)> {
        let mut output = Vec::new();
        let enabled =
            prompt_super_presidio_opt_in_from(true, true, io::Cursor::new(answer), &mut output)?;
        let output = String::from_utf8(output).context("prompt output should be UTF-8")?;
        Ok((enabled, output))
    }

    #[test]
    fn super_presidio_prompt_accepts_y_and_yes() -> Result<()> {
        for answer in ["y\n", "Y\n", "yes\n", "YES\n"] {
            let (enabled, output) = prompt_answer(answer)?;
            assert!(enabled, "{answer:?} should opt in");
            assert_eq!(output, "Use Presidio for data safety? [y/N] ");
        }
        Ok(())
    }

    #[test]
    fn super_presidio_prompt_rejects_n_and_default() -> Result<()> {
        for answer in ["n\n", "N\n", "\n", "no\n"] {
            let (enabled, output) = prompt_answer(answer)?;
            assert!(!enabled, "{answer:?} should not opt in");
            assert_eq!(output, "Use Presidio for data safety? [y/N] ");
        }
        Ok(())
    }

    #[test]
    fn super_presidio_prompt_skips_non_terminal_io() -> Result<()> {
        for (stdin_is_terminal, stderr_is_terminal) in [(false, true), (true, false)] {
            let mut output = Vec::new();
            let enabled = prompt_super_presidio_opt_in_from(
                stdin_is_terminal,
                stderr_is_terminal,
                io::Cursor::new("y\n"),
                &mut output,
            )?;
            assert!(!enabled);
            assert!(output.is_empty());
        }
        Ok(())
    }
}
