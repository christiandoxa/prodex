use super::super_expose::{
    ExposeEndpointMode, ExposeEngineRequest, ExposeLifecycleEvent, ExposeLifecyclePhase,
    ExposeReadyState, run_super_expose_engine, validate_existing_cloudflare_hostname,
};
use super::{PublicMcpEndpoint, expose_main_provider};
use crate::ExposeArgs;
use anyhow::{Context, Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use prodex_cli::SuperArgs;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use terminal_ui::{
    AlternateScreenTerminal, tui_border_style, tui_error_style, tui_hint_style, tui_muted_style,
    tui_primary_style, tui_success_style, tui_title_style,
};

#[path = "super_expose_ui_support.rs"]
mod support;
#[cfg(test)]
use support::copy_public_url_to_clipboard_with;
use support::{copy_public_url_to_clipboard, visible_url};

const EXPOSE_TUI_EVENT_CAPACITY: usize = 32;
const EXPOSE_TUI_INPUT_POLL: Duration = Duration::from_millis(100);

type ExposeTuiTerminal = AlternateScreenTerminal<io::Stderr>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExposeTuiPhase {
    EndpointSelection,
    Preflight(ExposeLifecyclePhase),
    Ready,
    Stopping,
    Stopped,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EndpointChoice {
    QuickTunnel,
    ExistingCloudflareTunnel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EndpointField {
    Hostname,
    OriginPort,
}

enum ExposeTuiAction {
    None,
    Start(ExposeEndpointMode),
    Stop,
    CopyUrl,
}

pub(super) struct ExposeTuiState {
    phase: ExposeTuiPhase,
    workspace_name: String,
    display_name: String,
    provider: String,
    model: String,
    effort: String,
    sub_agent: bool,
    sub_agent_model: String,
    sub_agent_effort: String,
    endpoint_choice: EndpointChoice,
    endpoint_field: EndpointField,
    hostname: String,
    origin_port: String,
    ready: Option<ExposeReadyState>,
    status: Option<String>,
    url_offset: usize,
    redraw_needed: bool,
}

impl ExposeTuiState {
    fn new(args: &SuperArgs, workspace_name: String, display_name: String) -> Self {
        Self {
            phase: ExposeTuiPhase::EndpointSelection,
            workspace_name,
            display_name,
            provider: expose_main_provider(args).label().to_string(),
            model: args
                .local_model
                .clone()
                .or_else(|| crate::codex_cli_config_override_value(&args.codex_args, "model"))
                .unwrap_or_else(|| "remembered/default".to_string()),
            effort: crate::codex_cli_config_override_value(
                &args.codex_args,
                "model_reasoning_effort",
            )
            .unwrap_or_else(|| "remembered/default".to_string()),
            sub_agent: args.sub_agent,
            sub_agent_model: args
                .sub_agent_model
                .clone()
                .unwrap_or_else(|| "provider default".to_string()),
            sub_agent_effort: args.sub_agent_model_reasoning_effort.map_or_else(
                || "provider default".to_string(),
                |effort| effort.as_str().to_string(),
            ),
            endpoint_choice: EndpointChoice::QuickTunnel,
            endpoint_field: EndpointField::Hostname,
            hostname: String::new(),
            origin_port: "8765".to_string(),
            ready: None,
            status: None,
            url_offset: 0,
            redraw_needed: true,
        }
    }

    #[cfg(test)]
    fn phase(&self) -> ExposeTuiPhase {
        self.phase
    }

    fn handle_event(&mut self, event: Event) -> ExposeTuiAction {
        match event {
            Event::Resize(_, _) => {
                self.redraw_needed = true;
                ExposeTuiAction::None
            }
            Event::Key(key) if key.kind == KeyEventKind::Press => self.handle_key(key),
            _ => ExposeTuiAction::None,
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> ExposeTuiAction {
        if is_stop_key(key) {
            return ExposeTuiAction::Stop;
        }
        match self.phase {
            ExposeTuiPhase::EndpointSelection => self.handle_endpoint_key(key),
            ExposeTuiPhase::Ready => {
                match key.code {
                    KeyCode::Char('c')
                    | KeyCode::Char('C')
                    | KeyCode::Char('y')
                    | KeyCode::Char('Y') => return ExposeTuiAction::CopyUrl,
                    KeyCode::Left => self.scroll_url(-1),
                    KeyCode::Right => self.scroll_url(1),
                    KeyCode::Home => {
                        self.url_offset = 0;
                        self.redraw_needed = true;
                    }
                    KeyCode::End => {
                        self.url_offset = self
                            .ready
                            .as_ref()
                            .map_or(0, |ready| ready.public_url.as_str().chars().count());
                        self.redraw_needed = true;
                    }
                    _ => {}
                }
                ExposeTuiAction::None
            }
            ExposeTuiPhase::Preflight(_)
            | ExposeTuiPhase::Stopping
            | ExposeTuiPhase::Stopped
            | ExposeTuiPhase::Failed => ExposeTuiAction::None,
        }
    }

    fn handle_endpoint_key(&mut self, key: KeyEvent) -> ExposeTuiAction {
        match key.code {
            KeyCode::Up | KeyCode::Down => {
                self.endpoint_choice = match self.endpoint_choice {
                    EndpointChoice::QuickTunnel => EndpointChoice::ExistingCloudflareTunnel,
                    EndpointChoice::ExistingCloudflareTunnel => EndpointChoice::QuickTunnel,
                };
                self.status = None;
                self.redraw_needed = true;
            }
            KeyCode::Tab => {
                if self.endpoint_choice == EndpointChoice::ExistingCloudflareTunnel {
                    self.endpoint_field = match self.endpoint_field {
                        EndpointField::Hostname => EndpointField::OriginPort,
                        EndpointField::OriginPort => EndpointField::Hostname,
                    };
                    self.redraw_needed = true;
                }
            }
            KeyCode::Backspace | KeyCode::Delete => {
                self.edit_endpoint_field(|value| {
                    value.pop();
                });
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.edit_endpoint_field(String::clear);
            }
            KeyCode::Char('2') if self.endpoint_choice == EndpointChoice::QuickTunnel => {
                self.endpoint_choice = EndpointChoice::ExistingCloudflareTunnel;
                self.endpoint_field = EndpointField::Hostname;
                self.status = None;
                self.redraw_needed = true;
            }
            KeyCode::Char(character) => {
                self.edit_endpoint_field(|value| value.push(character));
            }
            KeyCode::Enter => return self.selected_endpoint(),
            _ => {}
        }
        ExposeTuiAction::None
    }

    fn edit_endpoint_field(&mut self, edit: impl FnOnce(&mut String)) {
        if self.endpoint_choice != EndpointChoice::ExistingCloudflareTunnel {
            return;
        }
        let value = match self.endpoint_field {
            EndpointField::Hostname => &mut self.hostname,
            EndpointField::OriginPort => &mut self.origin_port,
        };
        edit(value);
        self.status = None;
        self.redraw_needed = true;
    }

    fn selected_endpoint(&mut self) -> ExposeTuiAction {
        match self.endpoint_choice {
            EndpointChoice::QuickTunnel => ExposeTuiAction::Start(ExposeEndpointMode::QuickTunnel),
            EndpointChoice::ExistingCloudflareTunnel => {
                let hostname = match validate_existing_cloudflare_hostname(self.hostname.trim()) {
                    Ok(hostname) => hostname,
                    Err(error) => {
                        self.status = Some(error.to_string());
                        self.redraw_needed = true;
                        return ExposeTuiAction::None;
                    }
                };
                let origin_port = match self.origin_port.trim().parse::<u16>() {
                    Ok(port) if port > 0 => port,
                    _ => {
                        self.status = Some("origin port must be 1-65535".to_string());
                        self.redraw_needed = true;
                        return ExposeTuiAction::None;
                    }
                };
                ExposeTuiAction::Start(ExposeEndpointMode::ExistingCloudflareTunnel {
                    hostname,
                    origin_port,
                })
            }
        }
    }

    fn apply_engine_event(&mut self, event: ExposeLifecycleEvent) {
        match event {
            ExposeLifecycleEvent::Phase(phase) => {
                self.phase = ExposeTuiPhase::Preflight(phase);
                self.status = None;
            }
            ExposeLifecycleEvent::Ready(ready) => {
                self.phase = ExposeTuiPhase::Ready;
                self.ready = Some(ready);
                self.url_offset = 0;
                self.status = None;
            }
            ExposeLifecycleEvent::Stopped => {
                self.phase = ExposeTuiPhase::Stopped;
                self.status = Some("access revoked; expose stopped".to_string());
            }
            ExposeLifecycleEvent::Failed(error) => {
                self.phase = ExposeTuiPhase::Failed;
                self.status = Some(error);
            }
        }
        self.redraw_needed = true;
    }

    fn set_stopping(&mut self) {
        self.phase = ExposeTuiPhase::Stopping;
        self.status = Some("stopping and revoking access...".to_string());
        self.redraw_needed = true;
    }

    fn set_status(&mut self, status: impl Into<String>) {
        self.status = Some(status.into());
        self.redraw_needed = true;
    }

    fn scroll_url(&mut self, delta: isize) {
        let length = self
            .ready
            .as_ref()
            .map_or(0, |ready| ready.public_url.as_str().chars().count());
        self.url_offset = if delta.is_negative() {
            self.url_offset.saturating_sub(delta.unsigned_abs())
        } else {
            self.url_offset.saturating_add(delta as usize).min(length)
        };
        self.redraw_needed = true;
    }

    fn endpoint_label(&self) -> &'static str {
        match self.endpoint_choice {
            EndpointChoice::QuickTunnel => "Quick Tunnel",
            EndpointChoice::ExistingCloudflareTunnel => "Existing Cloudflare Tunnel",
        }
    }
}

pub(super) fn run(
    args: ExposeArgs,
    super_args: SuperArgs,
    workspace_root: PathBuf,
    workspace_name: String,
    display_name: String,
) -> Result<()> {
    let mut terminal = ExposeTuiTerminal::stderr("Super expose TUI")?;
    let mut state = ExposeTuiState::new(&super_args, workspace_name.clone(), display_name.clone());
    let (event_tx, event_rx) = mpsc::sync_channel(EXPOSE_TUI_EVENT_CAPACITY);
    let cancel = Arc::new(AtomicBool::new(false));
    let mut launch = Some((
        args,
        super_args,
        workspace_root,
        workspace_name,
        display_name,
    ));
    let mut worker: Option<JoinHandle<Result<()>>> = None;
    let mut stopping = false;

    let result = (|| -> Result<()> {
        loop {
            drain_engine_events(&mut state, &event_rx);
            if worker.as_ref().is_some_and(JoinHandle::is_finished) {
                let Some(worker_handle) = worker.take() else {
                    continue;
                };
                let finished = worker_handle
                    .join()
                    .map_err(|_| anyhow::anyhow!("expose lifecycle worker panicked"))?;
                if let Err(error) = finished
                    && !matches!(
                        state.phase,
                        ExposeTuiPhase::Failed | ExposeTuiPhase::Stopped
                    )
                {
                    state.apply_engine_event(ExposeLifecycleEvent::Failed(
                        redaction::redaction_redact_secret_like_text(&format!("{error:#}")),
                    ));
                }
            }

            if signal_requested() && !stopping {
                if worker.is_some() {
                    cancel.store(true, Ordering::SeqCst);
                    state.set_stopping();
                    stopping = true;
                } else {
                    break Ok(());
                }
            }

            if state.redraw_needed {
                terminal
                    .autoresize()
                    .context("failed to resize Super expose TUI")?;
                draw(&mut terminal, &state)?;
                state.redraw_needed = false;
            }

            if worker.is_none()
                && (stopping
                    || matches!(
                        state.phase,
                        ExposeTuiPhase::Stopped | ExposeTuiPhase::Failed
                    ))
            {
                break worker_result(&state);
            }

            if event::poll(EXPOSE_TUI_INPUT_POLL)
                .context("failed to poll Super expose TUI input")?
            {
                let action = state
                    .handle_event(event::read().context("failed to read Super expose TUI input")?);
                match action {
                    ExposeTuiAction::None => {}
                    ExposeTuiAction::CopyUrl => {
                        let Some(ready) = state.ready.as_ref() else {
                            continue;
                        };
                        match copy_public_url_to_clipboard(&ready.public_url) {
                            Ok(()) => state.set_status("MCP URL copied to clipboard"),
                            Err(_) => state.set_status("clipboard is unavailable"),
                        }
                    }
                    ExposeTuiAction::Start(endpoint) => {
                        let (args, super_args, workspace_root, workspace_name, display_name) =
                            launch.take().context("expose endpoint selected twice")?;
                        let event_tx = event_tx.clone();
                        let cancel = Arc::clone(&cancel);
                        worker = Some(thread::spawn(move || {
                            run_super_expose_engine(
                                ExposeEngineRequest {
                                    args,
                                    super_args,
                                    workspace_root,
                                    workspace_name,
                                    display_name,
                                    endpoint,
                                },
                                Some(event_tx),
                                cancel,
                            )
                        }));
                    }
                    ExposeTuiAction::Stop => {
                        if worker.is_none() {
                            break Ok(());
                        }
                        cancel.store(true, Ordering::SeqCst);
                        state.set_stopping();
                        stopping = true;
                    }
                }
            }
        }
    })();

    if let Some(worker) = worker.take() {
        cancel.store(true, Ordering::SeqCst);
        let _ = worker.join();
    }
    result
}

fn drain_engine_events(
    state: &mut ExposeTuiState,
    event_rx: &mpsc::Receiver<ExposeLifecycleEvent>,
) {
    while let Ok(event) = event_rx.try_recv() {
        state.apply_engine_event(event);
    }
}

fn worker_result(state: &ExposeTuiState) -> Result<()> {
    if let ExposeTuiPhase::Failed = state.phase
        && let Some(error) = state.status.as_deref()
    {
        bail!("{error}")
    }
    Ok(())
}

fn draw(terminal: &mut ExposeTuiTerminal, state: &ExposeTuiState) -> Result<()> {
    terminal
        .draw(|frame| draw_frame(frame, state))
        .context("failed to draw Super expose TUI")?;
    Ok(())
}

fn draw_frame(frame: &mut Frame<'_>, state: &ExposeTuiState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(frame.area());
    let phase = match state.phase {
        ExposeTuiPhase::EndpointSelection => "Endpoint selection",
        ExposeTuiPhase::Preflight(phase) => phase.label(),
        ExposeTuiPhase::Ready => "Ready",
        ExposeTuiPhase::Stopping => "Stopping",
        ExposeTuiPhase::Stopped => "Stopped",
        ExposeTuiPhase::Failed => "Failed",
    };
    let header = Paragraph::new(Line::from(vec![
        Span::styled("Prodex Super Expose", tui_title_style()),
        Span::raw("  "),
        Span::styled(phase, tui_muted_style()),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(tui_border_style()),
    );
    frame.render_widget(header, chunks[0]);

    let body = match state.phase {
        ExposeTuiPhase::EndpointSelection => endpoint_body(state),
        ExposeTuiPhase::Preflight(phase) => preflight_body(state, phase),
        ExposeTuiPhase::Ready => ready_body(state, chunks[1].width.saturating_sub(2)),
        ExposeTuiPhase::Stopping | ExposeTuiPhase::Stopped | ExposeTuiPhase::Failed => {
            status_body(state)
        }
    };
    frame.render_widget(
        Paragraph::new(body).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(tui_border_style()),
        ),
        chunks[1],
    );

    let footer = match state.phase {
        ExposeTuiPhase::EndpointSelection => {
            "↑/↓ select · Tab edit · Enter start · q/Ctrl-C cancel"
        }
        ExposeTuiPhase::Ready => "c copy URL · ←/→ scroll · Home/End · q/Ctrl-C stop",
        ExposeTuiPhase::Stopping => "waiting for cleanup...",
        ExposeTuiPhase::Stopped | ExposeTuiPhase::Failed => "q/Ctrl-C exit",
        ExposeTuiPhase::Preflight(_) => "q/Ctrl-C stop",
    };
    frame.render_widget(
        Paragraph::new(Line::styled(footer, tui_hint_style())).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(tui_border_style()),
        ),
        chunks[2],
    );
}

fn endpoint_body(state: &ExposeTuiState) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::styled(
            format!("Workspace: {}", state.workspace_name),
            tui_primary_style(),
        ),
        Line::from(format!("Instance name: {}", state.display_name)),
        Line::from(format!(
            "Super: {} · {} · {} · sub-agents {}",
            state.provider,
            state.model,
            state.effort,
            if state.sub_agent { "on" } else { "off" },
        )),
        Line::from(""),
        option_line(
            state.endpoint_choice == EndpointChoice::QuickTunnel,
            "Quick Tunnel",
            "random trycloudflare.com hostname",
        ),
        option_line(
            state.endpoint_choice == EndpointChoice::ExistingCloudflareTunnel,
            "Existing Cloudflare Tunnel",
            "use a configured hostname and loopback origin",
        ),
    ];
    if state.endpoint_choice == EndpointChoice::ExistingCloudflareTunnel {
        lines.extend([
            Line::from(format!(
                "Hostname{}: {}",
                if state.endpoint_field == EndpointField::Hostname {
                    "*"
                } else {
                    ""
                },
                if state.hostname.is_empty() {
                    "<type a public DNS name>"
                } else {
                    &state.hostname
                },
            )),
            Line::from(format!(
                "Origin port{}: {}",
                if state.endpoint_field == EndpointField::OriginPort {
                    "*"
                } else {
                    ""
                },
                state.origin_port,
            )),
        ]);
    }
    if let Some(status) = state.status.as_deref() {
        lines.push(Line::styled(status.to_string(), tui_error_style()));
    }
    lines
}

fn option_line(selected: bool, name: &str, description: &str) -> Line<'static> {
    let marker = if selected { ">" } else { " " };
    let style = if selected {
        tui_success_style().add_modifier(Modifier::BOLD)
    } else {
        tui_muted_style()
    };
    Line::from(vec![
        Span::styled(format!("{marker} {name}"), style),
        Span::raw(format!(" — {description}")),
    ])
}

fn preflight_body(state: &ExposeTuiState, current: ExposeLifecyclePhase) -> Vec<Line<'static>> {
    const PHASES: [ExposeLifecyclePhase; 8] = [
        ExposeLifecyclePhase::Preparing,
        ExposeLifecyclePhase::CheckingCloudflared,
        ExposeLifecyclePhase::StartingSuper,
        ExposeLifecyclePhase::LocalMcpInitialize,
        ExposeLifecyclePhase::LocalMcpTools,
        ExposeLifecyclePhase::Cloudflare,
        ExposeLifecyclePhase::PublicMcpInitialize,
        ExposeLifecyclePhase::PublicMcpTools,
    ];
    let endpoint = state.endpoint_label();
    let mut lines = vec![
        Line::styled(
            format!("Workspace: {} · endpoint: {endpoint}", state.workspace_name),
            tui_primary_style(),
        ),
        Line::from(""),
    ];
    lines.extend(PHASES.into_iter().map(|phase| {
        let skipped = phase == ExposeLifecyclePhase::CheckingCloudflared
            && state.endpoint_choice == EndpointChoice::ExistingCloudflareTunnel;
        let (marker, style) = if skipped {
            ("–", tui_muted_style())
        } else if phase.order() < current.order() {
            ("✓", tui_success_style())
        } else if phase == current {
            (">", tui_primary_style().add_modifier(Modifier::BOLD))
        } else {
            ("·", tui_muted_style())
        };
        Line::from(vec![
            Span::styled(format!("{marker} "), style),
            Span::styled(phase.label(), style),
        ])
    }));
    if let Some(status) = state.status.as_deref() {
        lines.push(Line::styled(status.to_string(), tui_error_style()));
    }
    lines
}

fn ready_body(state: &ExposeTuiState, width: u16) -> Vec<Line<'static>> {
    let Some(ready) = state.ready.as_ref() else {
        return vec![Line::styled("Ready", tui_success_style())];
    };
    let active_runs = ready
        .mcp
        .run_manager
        .list()
        .into_iter()
        .filter(|run| !run.state.terminal())
        .count();
    let endpoint = match &ready.endpoint {
        ExposeEndpointMode::QuickTunnel => "Quick Tunnel",
        ExposeEndpointMode::ExistingCloudflareTunnel { .. } => "Existing Cloudflare Tunnel",
    };
    let url_prefix = "MCP URL: ";
    let url_width = usize::from(width).saturating_sub(url_prefix.len());
    let url = visible_url(ready.public_url.as_str(), state.url_offset, url_width);
    let mut lines = vec![
        Line::styled(
            "Ready — public MCP passed initialize and tools/list",
            tui_success_style().add_modifier(Modifier::BOLD),
        ),
        Line::from(format!(
            "Instance: {} ({})",
            ready.display_name, ready.instance_id
        )),
        Line::from(format!("Workspace: {}", ready.workspace_name)),
        Line::from(format!("Provider: {}", state.provider)),
        Line::from(format!("Model: {}", state.model)),
        Line::from(format!("Effort: {}", state.effort)),
        Line::from(format!(
            "Sub-agents: {}{}",
            if state.sub_agent {
                "enabled"
            } else {
                "disabled"
            },
            if state.sub_agent {
                format!(" · {}/{}", state.sub_agent_model, state.sub_agent_effort)
            } else {
                String::new()
            },
        )),
        Line::from(format!("Endpoint: {endpoint}")),
        Line::from(format!("Active runs: {active_runs}")),
        Line::from(""),
        Line::from(vec![
            Span::styled("MCP URL: ", tui_primary_style()),
            Span::raw(url),
        ]),
        Line::from(vec![
            Span::styled("Browser URL: ", tui_muted_style()),
            Span::raw(ready.local_url.clone()),
        ]),
        Line::from(""),
        Line::styled(
            "Access: full Super authority as the current OS user; initial directory is context only.",
            tui_error_style(),
        ),
        Line::styled(
            "The MCP URL is an ephemeral bearer capability; stop expose to revoke it.",
            tui_error_style(),
        ),
    ];
    if let Some(status) = state.status.as_deref() {
        lines.push(Line::styled(status.to_string(), tui_success_style()));
    }
    lines
}

fn status_body(state: &ExposeTuiState) -> Vec<Line<'static>> {
    let style = if state.phase == ExposeTuiPhase::Failed {
        tui_error_style()
    } else {
        tui_muted_style()
    };
    vec![Line::styled(
        state.status.clone().unwrap_or_else(|| "done".to_string()),
        style,
    )]
}

fn is_stop_key(key: KeyEvent) -> bool {
    matches!(
        key.code,
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc
    ) || (key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('c' | 'C')))
}

fn signal_requested() -> bool {
    #[cfg(unix)]
    {
        crate::InteractiveSigintGuard::count() > 0
    }
    #[cfg(not(unix))]
    {
        false
    }
}

#[cfg(test)]
#[path = "super_expose_ui_tests.rs"]
mod tests;
