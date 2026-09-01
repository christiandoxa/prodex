use super::runtime::{
    ExistingCloudflareSelection, OpenAiTunnelCredentials, discover_existing_cloudflare,
    ensure_openai_tunnel_available,
};
use super::super_expose::{
    ExposeEndpointMode, ExposeLifecycleEvent, ExposeLifecyclePhase, ExposeReadyState,
    validate_existing_cloudflare_hostname,
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
use std::thread::JoinHandle;
use std::time::Duration;
use terminal_ui::{
    AlternateScreenTerminal, tui_border_style, tui_error_style, tui_hint_style, tui_muted_style,
    tui_primary_style, tui_success_style, tui_title_style,
};

#[path = "super_expose_ui_endpoint.rs"]
mod endpoint;
#[path = "super_expose_ui_loop.rs"]
mod loop_support;
#[path = "super_expose_ui_openai.rs"]
mod openai;
#[path = "super_expose_ui_status.rs"]
mod status;
#[path = "super_expose_ui_support.rs"]
mod support;
use endpoint::endpoint_body;
pub(super) use openai::{OpenAiSetupField, OpenAiSetupState, setup_body};
pub(super) use status::{ready_body, status_body};
#[cfg(test)]
use support::copy_public_url_to_clipboard_with;

const EXPOSE_TUI_EVENT_CAPACITY: usize = 32;
const EXPOSE_TUI_INPUT_POLL: Duration = Duration::from_millis(100);

type ExposeTuiTerminal = AlternateScreenTerminal<io::Stderr>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExposeTuiPhase {
    EndpointSelection,
    OpenAiSetup(OpenAiSetupField),
    Preflight(ExposeLifecyclePhase),
    Ready,
    Stopping,
    Stopped,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EndpointChoice {
    LocalOnly,
    QuickTunnel,
    ExistingCloudflareTunnel,
    OpenAiSecureMcp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EndpointField {
    Hostname,
    OriginPort,
}

enum ExposeTuiAction {
    None,
    Start {
        endpoint: ExposeEndpointMode,
        existing: Option<Box<ExistingCloudflareSelection>>,
        openai_credentials: Option<OpenAiTunnelCredentials>,
    },
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
    existing_cloudflare: Option<ExistingCloudflareSelection>,
    openai_tunnel_id: Option<String>,
    openai_setup: Option<OpenAiSetupState>,
    ready: Option<ExposeReadyState>,
    status: Option<String>,
    body_scroll: usize,
    redraw_needed: bool,
}

impl ExposeTuiState {
    fn new(args: &SuperArgs, workspace_name: String, display_name: String) -> Self {
        let existing = discover_existing_cloudflare().ok().flatten();
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
            endpoint_choice: EndpointChoice::LocalOnly,
            endpoint_field: EndpointField::Hostname,
            hostname: existing
                .as_ref()
                .map_or_else(String::new, |selection| selection.hostname.clone()),
            origin_port: existing.as_ref().map_or_else(
                || "8765".to_string(),
                |selection| selection.origin_port.to_string(),
            ),
            existing_cloudflare: existing,
            openai_tunnel_id: None,
            openai_setup: None,
            ready: None,
            status: None,
            body_scroll: 0,
            redraw_needed: true,
        }
    }

    #[cfg(test)]
    fn phase(&self) -> ExposeTuiPhase {
        self.phase
    }

    #[cfg(test)]
    fn body_scroll(&self) -> usize {
        self.body_scroll
    }

    fn handle_event(&mut self, event: Event) -> ExposeTuiAction {
        match event {
            Event::Resize(_, _) => {
                self.redraw_needed = true;
                ExposeTuiAction::None
            }
            Event::Paste(text) => {
                if let ExposeTuiPhase::OpenAiSetup(field) = self.phase
                    && let Some(setup) = self.openai_setup.as_mut()
                {
                    setup.handle_paste(field, &text);
                    self.status = None;
                    self.redraw_needed = true;
                }
                ExposeTuiAction::None
            }
            Event::Key(key) if key.kind == KeyEventKind::Press => self.handle_key(key),
            _ => ExposeTuiAction::None,
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> ExposeTuiAction {
        let openai_setup = matches!(self.phase, ExposeTuiPhase::OpenAiSetup(_));
        let setup_cancel = key.code == KeyCode::Esc
            || (key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('c' | 'C')));
        if (openai_setup && setup_cancel) || (!openai_setup && support::is_stop_key(key)) {
            return ExposeTuiAction::Stop;
        }
        match self.phase {
            ExposeTuiPhase::EndpointSelection => self.handle_endpoint_key(key),
            ExposeTuiPhase::OpenAiSetup(field) => self.handle_openai_setup_key(field, key),
            ExposeTuiPhase::Ready => {
                match key.code {
                    KeyCode::Char('c')
                    | KeyCode::Char('C')
                    | KeyCode::Char('y')
                    | KeyCode::Char('Y')
                        if self
                            .ready
                            .as_ref()
                            .is_some_and(|ready| ready.public_url.is_some()) =>
                    {
                        return ExposeTuiAction::CopyUrl;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.body_scroll = self.body_scroll.saturating_sub(1);
                        self.redraw_needed = true;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.body_scroll = self.body_scroll.saturating_add(1);
                        self.redraw_needed = true;
                    }
                    KeyCode::PageUp => {
                        self.body_scroll = self.body_scroll.saturating_sub(10);
                        self.redraw_needed = true;
                    }
                    KeyCode::PageDown => {
                        self.body_scroll = self.body_scroll.saturating_add(10);
                        self.redraw_needed = true;
                    }
                    KeyCode::Home => {
                        self.body_scroll = 0;
                        self.redraw_needed = true;
                    }
                    KeyCode::End => {
                        self.body_scroll = usize::MAX;
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
            KeyCode::Up | KeyCode::Char('k') => self.select_previous_endpoint(),
            KeyCode::Down | KeyCode::Char('j') => self.select_next_endpoint(),
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
            KeyCode::Char('1') => self.select_endpoint(EndpointChoice::LocalOnly),
            KeyCode::Char('2') => self.select_endpoint(EndpointChoice::QuickTunnel),
            KeyCode::Char('3') => self.select_endpoint(EndpointChoice::ExistingCloudflareTunnel),
            KeyCode::Char('4') => self.select_endpoint(EndpointChoice::OpenAiSecureMcp),
            KeyCode::Char(character) => {
                self.edit_endpoint_field(|value| value.push(character));
            }
            KeyCode::Enter => return self.selected_endpoint(),
            _ => {}
        }
        ExposeTuiAction::None
    }

    fn select_previous_endpoint(&mut self) {
        self.select_endpoint(match self.endpoint_choice {
            EndpointChoice::LocalOnly => EndpointChoice::OpenAiSecureMcp,
            EndpointChoice::QuickTunnel => EndpointChoice::LocalOnly,
            EndpointChoice::ExistingCloudflareTunnel => EndpointChoice::QuickTunnel,
            EndpointChoice::OpenAiSecureMcp => EndpointChoice::ExistingCloudflareTunnel,
        });
    }

    fn select_next_endpoint(&mut self) {
        self.select_endpoint(match self.endpoint_choice {
            EndpointChoice::LocalOnly => EndpointChoice::QuickTunnel,
            EndpointChoice::QuickTunnel => EndpointChoice::ExistingCloudflareTunnel,
            EndpointChoice::ExistingCloudflareTunnel => EndpointChoice::OpenAiSecureMcp,
            EndpointChoice::OpenAiSecureMcp => EndpointChoice::LocalOnly,
        });
    }

    fn select_endpoint(&mut self, endpoint: EndpointChoice) {
        self.endpoint_choice = endpoint;
        self.endpoint_field = EndpointField::Hostname;
        self.status = None;
        self.redraw_needed = true;
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
            EndpointChoice::LocalOnly => ExposeTuiAction::Start {
                endpoint: ExposeEndpointMode::LocalOnly,
                existing: None,
                openai_credentials: None,
            },
            EndpointChoice::QuickTunnel => ExposeTuiAction::Start {
                endpoint: ExposeEndpointMode::QuickTunnel,
                existing: None,
                openai_credentials: None,
            },
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
                let existing = self.existing_cloudflare.as_ref().map(|selection| {
                    let mut selection = selection.clone();
                    selection.hostname.clone_from(&hostname);
                    selection.origin_port = origin_port;
                    selection
                });
                if existing.is_none() {
                    self.status = Some(
                        "no usable Cloudflare config detected; use --cloudflare-config or a token file"
                            .to_string(),
                    );
                    self.redraw_needed = true;
                    return ExposeTuiAction::None;
                }
                ExposeTuiAction::Start {
                    endpoint: ExposeEndpointMode::ExistingCloudflareTunnel {
                        hostname,
                        origin_port,
                        tunnel: existing
                            .as_ref()
                            .and_then(|selection| selection.tunnel.clone()),
                    },
                    existing: existing.map(Box::new),
                    openai_credentials: None,
                }
            }
            EndpointChoice::OpenAiSecureMcp => {
                self.begin_openai_setup();
                ExposeTuiAction::None
            }
        }
    }

    fn begin_openai_setup(&mut self) {
        self.openai_setup = Some(OpenAiSetupState::new(self.openai_tunnel_id.as_deref()));
        self.phase = ExposeTuiPhase::OpenAiSetup(OpenAiSetupField::TunnelId);
        self.status = None;
        self.redraw_needed = true;
    }

    fn handle_openai_setup_key(
        &mut self,
        field: OpenAiSetupField,
        key: KeyEvent,
    ) -> ExposeTuiAction {
        let input = match self.openai_setup.as_mut() {
            Some(setup) => setup.handle_key(field, key),
            None => {
                self.status = Some("OpenAI tunnel setup is unavailable".to_string());
                self.redraw_needed = true;
                return ExposeTuiAction::None;
            }
        };
        match input {
            Ok(openai::OpenAiSetupInput::Ignored) => ExposeTuiAction::None,
            Ok(openai::OpenAiSetupInput::Edited) => {
                self.status = None;
                self.redraw_needed = true;
                ExposeTuiAction::None
            }
            Ok(openai::OpenAiSetupInput::Next) => {
                self.phase = ExposeTuiPhase::OpenAiSetup(OpenAiSetupField::ApiKey);
                self.status = None;
                self.redraw_needed = true;
                ExposeTuiAction::None
            }
            Ok(openai::OpenAiSetupInput::Credentials(credentials)) => {
                let tunnel_id = credentials.tunnel_id().to_owned();
                let client_version = match ensure_openai_tunnel_available(&tunnel_id) {
                    Ok(client_version) => client_version,
                    Err(error) => {
                        self.status = Some(error.to_string());
                        self.redraw_needed = true;
                        return ExposeTuiAction::None;
                    }
                };
                self.openai_setup.take();
                ExposeTuiAction::Start {
                    endpoint: ExposeEndpointMode::OpenAiSecureMcp {
                        tunnel_id,
                        client_version,
                    },
                    existing: None,
                    openai_credentials: Some(credentials),
                }
            }
            Err(error) => {
                self.status = Some(error.to_string());
                self.redraw_needed = true;
                ExposeTuiAction::None
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
                self.ready = Some(*ready);
                self.body_scroll = 0;
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

    fn endpoint_label(&self) -> &'static str {
        match self.endpoint_choice {
            EndpointChoice::LocalOnly => "Local only",
            EndpointChoice::QuickTunnel => "Quick Tunnel",
            EndpointChoice::ExistingCloudflareTunnel => "Existing Cloudflare Tunnel",
            EndpointChoice::OpenAiSecureMcp => "OpenAI Secure MCP Tunnel",
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
    state.openai_tunnel_id = args.openai_tunnel_id.clone();
    if args.tunnel_provider == Some(prodex_cli::ExposeTunnelProvider::OpenAi) {
        state.endpoint_choice = EndpointChoice::OpenAiSecureMcp;
        state.begin_openai_setup();
    }
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
            loop_support::reap_finished_worker(&mut state, &mut worker)?;
            if loop_support::handle_signal(&mut state, &cancel, &worker, &mut stopping) {
                break Ok(());
            }

            loop_support::redraw_if_needed(&mut terminal, &mut state)?;

            if loop_support::should_finish(&state, &worker, stopping) {
                break worker_result(&state);
            }

            if event::poll(EXPOSE_TUI_INPUT_POLL)
                .context("failed to poll Super expose TUI input")?
                && loop_support::handle_input(
                    &mut state,
                    &event_tx,
                    &cancel,
                    &mut launch,
                    &mut worker,
                    &mut stopping,
                )?
            {
                break Ok(());
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

fn draw(terminal: &mut ExposeTuiTerminal, state: &mut ExposeTuiState) -> Result<()> {
    terminal
        .draw(|frame| draw_frame(frame, state))
        .context("failed to draw Super expose TUI")?;
    Ok(())
}

fn draw_frame(frame: &mut Frame<'_>, state: &mut ExposeTuiState) {
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
        ExposeTuiPhase::OpenAiSetup(OpenAiSetupField::TunnelId) => "OpenAI setup · Tunnel ID",
        ExposeTuiPhase::OpenAiSetup(OpenAiSetupField::ApiKey) => "OpenAI setup · API key",
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
        ExposeTuiPhase::OpenAiSetup(field) => setup_body(
            state.openai_setup.as_ref(),
            field,
            state.status.as_deref(),
            usize::from(chunks[1].width.saturating_sub(2)),
        ),
        ExposeTuiPhase::Preflight(phase) => preflight_body(state, phase),
        ExposeTuiPhase::Ready => ready_body(state, usize::from(chunks[1].width.saturating_sub(2))),
        ExposeTuiPhase::Stopping | ExposeTuiPhase::Stopped | ExposeTuiPhase::Failed => {
            status_body(state)
        }
    };
    let body_height = usize::from(chunks[1].height.saturating_sub(2));
    let body_len = body.len();
    let body_scroll = if state.phase == ExposeTuiPhase::Ready {
        state.body_scroll = state.body_scroll.min(body_len.saturating_sub(body_height));
        state.body_scroll
    } else {
        state.body_scroll = 0;
        0
    };
    let body = body
        .into_iter()
        .skip(body_scroll)
        .take(body_height)
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(body).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(tui_border_style()),
        ),
        chunks[1],
    );

    let footer = match state.phase {
        ExposeTuiPhase::EndpointSelection => "↑↓/jk select · Tab edit · Enter · q/Ctrl-C cancel",
        ExposeTuiPhase::OpenAiSetup(_) => {
            "Enter continue · Backspace/Delete edit · Ctrl-U clear · Esc/Ctrl-C cancel"
        }
        ExposeTuiPhase::Ready => "c copy · ↑↓/jk scroll · PgUp/Dn · Home/End · q/Ctrl-C stop",
        ExposeTuiPhase::Stopping => "waiting for cleanup...",
        ExposeTuiPhase::Stopped | ExposeTuiPhase::Failed => "q/Ctrl-C exit",
        ExposeTuiPhase::Preflight(_) => "q/Ctrl-C stop",
    };
    let footer = if state.phase == ExposeTuiPhase::Ready {
        let max_scroll = body_len.saturating_sub(body_height);
        match (body_scroll > 0, body_scroll < max_scroll) {
            (false, true) => format!("{footer} · ↓ more"),
            (true, false) => format!("{footer} · ↑ more"),
            (true, true) => format!("{footer} · ↑/↓ more"),
            (false, false) => footer.to_string(),
        }
    } else {
        footer.to_string()
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

fn preflight_body(state: &ExposeTuiState, current: ExposeLifecyclePhase) -> Vec<Line<'static>> {
    const PHASES: [ExposeLifecyclePhase; 11] = [
        ExposeLifecyclePhase::Preparing,
        ExposeLifecyclePhase::CheckingCloudflared,
        ExposeLifecyclePhase::StartingSuper,
        ExposeLifecyclePhase::LocalMcpInitialize,
        ExposeLifecyclePhase::LocalMcpTools,
        ExposeLifecyclePhase::LocalBrowser,
        ExposeLifecyclePhase::Cloudflare,
        ExposeLifecyclePhase::OpenAiTunnel,
        ExposeLifecyclePhase::PublicMcpInitialize,
        ExposeLifecyclePhase::PublicMcpTools,
        ExposeLifecyclePhase::PublicBrowser,
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
        let skipped = match state.endpoint_choice {
            EndpointChoice::LocalOnly => matches!(
                phase,
                ExposeLifecyclePhase::CheckingCloudflared
                    | ExposeLifecyclePhase::LocalBrowser
                    | ExposeLifecyclePhase::Cloudflare
                    | ExposeLifecyclePhase::OpenAiTunnel
                    | ExposeLifecyclePhase::PublicMcpInitialize
                    | ExposeLifecyclePhase::PublicMcpTools
                    | ExposeLifecyclePhase::PublicBrowser
            ),
            EndpointChoice::QuickTunnel => phase == ExposeLifecyclePhase::OpenAiTunnel,
            EndpointChoice::ExistingCloudflareTunnel => {
                phase == ExposeLifecyclePhase::CheckingCloudflared
                    || phase == ExposeLifecyclePhase::OpenAiTunnel
            }
            EndpointChoice::OpenAiSecureMcp => matches!(
                phase,
                ExposeLifecyclePhase::CheckingCloudflared
                    | ExposeLifecyclePhase::LocalBrowser
                    | ExposeLifecyclePhase::Cloudflare
                    | ExposeLifecyclePhase::PublicMcpInitialize
                    | ExposeLifecyclePhase::PublicMcpTools
                    | ExposeLifecyclePhase::PublicBrowser
            ),
        };
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

#[cfg(test)]
#[path = "super_expose_ui_tests.rs"]
mod tests;
