use super::*;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::io::IsTerminal;
use terminal_ui::{
    tui_border_style, tui_connected_footer_block, tui_connected_header_block, tui_detail_style,
    tui_hint_style, tui_primary_style, tui_secondary_style, tui_success_style, tui_title_style,
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
mod gui;
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
pub(crate) use self::gui::*;
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

pub(super) fn handle_super(args: SuperArgs) -> Result<()> {
    args.validate_urls().map_err(anyhow::Error::msg)?;
    let use_presidio = if matches!(args.cli, Some(SuperCliAgent::Kiro | SuperCliAgent::Agy)) {
        false
    } else {
        match args.presidio_preference() {
            Some(use_presidio) => use_presidio,
            None => prompt_super_presidio_opt_in()?,
        }
    };
    if matches!(
        args.cli,
        Some(
            SuperCliAgent::Gemini
                | SuperCliAgent::Copilot
                | SuperCliAgent::Kiro
                | SuperCliAgent::Agy
        )
    ) {
        return crate::runtime_gemini_cli::handle_super_native_cli(args, use_presidio);
    }
    handle_runtime_tools(args.into_runtime_tool_args_with_presidio(use_presidio))
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn resolve_runtime_launch_profile_name(
    state: &AppState,
    requested: Option<&str>,
) -> Result<String> {
    runtime_launch::resolve_runtime_launch_profile_name(state, requested)
}

pub(super) fn prompt_super_presidio_opt_in() -> Result<bool> {
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        return Ok(false);
    }

    let mut tui = terminal_ui::AlternateScreenTerminal::stderr("Presidio prompt TUI")?;
    loop {
        tui.terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(4),
                    Constraint::Length(3),
                ])
                .split(frame.area());
            let header = Paragraph::new(Line::from(vec![
                Span::styled("Prodex Super", tui_title_style()),
                Span::raw("  "),
                Span::styled("Presidio opt-in", tui_detail_style()),
            ]))
            .block(tui_connected_header_block(tui_border_style()));
            frame.render_widget(header, chunks[0]);

            let body = Paragraph::new(vec![
                Line::from(Span::styled(
                    "Use Presidio for data safety?",
                    tui_primary_style().add_modifier(Modifier::BOLD),
                )),
                Line::raw(""),
                Line::from(Span::styled(
                    "Detected sensitive data is redacted from request bodies before upstream delivery.",
                    tui_secondary_style(),
                )),
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
                    bail!("Presidio prompt cancelled");
                }
                _ => {}
            }
        }
    }
}
