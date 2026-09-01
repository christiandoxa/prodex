use super::super::mcp::ExposeMcpEndpoint;
use super::super::runtime::ExistingCloudflareSelection;
use super::{
    ExposeEndpointMode, ExposeLifecycleEvent, ExposeLifecyclePhase, ExposeReadyState,
    ExposeTuiAction, ExposeTuiPhase, ExposeTuiState, PublicMcpEndpoint,
    copy_public_url_to_clipboard_with, draw_frame, ready_body, support::labeled_value_lines,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use prodex_cli::SuperArgs;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::text::Line;
use std::path::PathBuf;
use terminal_ui::{chunk_token, text_width};

fn state() -> ExposeTuiState {
    state_with_args(&super_args())
}

fn super_args() -> SuperArgs {
    let crate::Commands::Super(args) =
        crate::parse_cli_command_from(["prodex", "s"]).expect("test Super args should parse")
    else {
        panic!("expected Super args");
    };
    args
}

fn state_with_args(args: &SuperArgs) -> ExposeTuiState {
    ExposeTuiState::new(args, "workspace".to_string(), "workspace".to_string())
}

#[test]
fn endpoint_selection_is_stateful_and_validates_existing_tunnel() {
    let mut state = state();
    state.existing_cloudflare = Some(ExistingCloudflareSelection {
        config_path: Some(PathBuf::from("/home/test-user/.cloudflared/config.yml")),
        tunnel: Some("prodex-main".to_string()),
        token_file: None,
        hostname: "configured.example.com".to_string(),
        origin_port: 8765,
    });
    assert_eq!(state.phase(), ExposeTuiPhase::EndpointSelection);
    assert!(matches!(
        state.handle_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE)),
        ExposeTuiAction::None
    ));
    for character in "shell.example.com".chars() {
        state.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
    }
    assert!(matches!(
        state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        ExposeTuiAction::Start {
            endpoint: ExposeEndpointMode::ExistingCloudflareTunnel {
                hostname,
                origin_port: 8765,
                ..
            },
            ..
        } if hostname == "shell.example.com"
    ));
}

#[test]
fn picker_defaults_to_local_and_cycles_all_connection_modes() {
    let mut state = state();
    assert_eq!(state.endpoint_label(), "Local only");
    assert!(matches!(
        state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        ExposeTuiAction::Start {
            endpoint: ExposeEndpointMode::LocalOnly,
            ..
        }
    ));
    state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(state.endpoint_label(), "Quick Tunnel");
    state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(state.endpoint_label(), "Existing Cloudflare Tunnel");
    state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(state.endpoint_label(), "OpenAI Secure MCP Tunnel");
    state.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(state.endpoint_label(), "Existing Cloudflare Tunnel");
}

#[test]
fn lifecycle_events_reach_ready_and_stop() {
    let mut state = state();
    state.apply_engine_event(ExposeLifecycleEvent::Phase(
        ExposeLifecyclePhase::LocalMcpInitialize,
    ));
    assert_eq!(
        state.phase(),
        ExposeTuiPhase::Preflight(ExposeLifecyclePhase::LocalMcpInitialize)
    );
    let url = PublicMcpEndpoint::new("https://shell.example.com", "capability").unwrap();
    let ready = ExposeReadyState {
        local_url: "http://127.0.0.1:1234/expose#bootstrap=bootstrap".to_string(),
        local_mcp_url: PublicMcpEndpoint::new("http://127.0.0.1:1234", "capability").unwrap(),
        public_browser_url: Some(
            "https://shell.example.com/expose#bootstrap=bootstrap".to_string(),
        ),
        public_url: Some(url),
        instance_id: "pdxi_test".to_string(),
        workspace_name: "workspace".to_string(),
        display_name: "workspace".to_string(),
        endpoint: ExposeEndpointMode::QuickTunnel,
        mcp: ExposeMcpEndpoint::new(
            "capability",
            "pdxi_test".to_string(),
            PathBuf::from("/home/test-user/workspace"),
            "workspace".to_string(),
            "workspace".to_string(),
            super_args(),
        ),
    };
    state.apply_engine_event(ExposeLifecycleEvent::Ready(Box::new(ready)));
    assert_eq!(state.phase(), ExposeTuiPhase::Ready);
    state.apply_engine_event(ExposeLifecycleEvent::Stopped);
    assert_eq!(state.phase(), ExposeTuiPhase::Stopped);
}

#[test]
fn openai_ready_status_separates_local_browser_mcp_and_connector_state() {
    let mut state = state();
    state.apply_engine_event(ExposeLifecycleEvent::Ready(Box::new(ExposeReadyState {
        local_url: "http://127.0.0.1:1234/expose#bootstrap=bootstrap".to_string(),
        local_mcp_url: PublicMcpEndpoint::new("http://127.0.0.1:1234", "capability").unwrap(),
        public_browser_url: None,
        public_url: None,
        instance_id: "pdxi_test".to_string(),
        workspace_name: "workspace".to_string(),
        display_name: "workspace".to_string(),
        endpoint: ExposeEndpointMode::OpenAiSecureMcp {
            tunnel_id: "tunnel_0123456789abcdef0123456789abcdef".to_string(),
            client_version: "0.0.13".to_string(),
        },
        mcp: ExposeMcpEndpoint::new(
            "capability",
            "pdxi_test".to_string(),
            PathBuf::from("/home/test-user/workspace"),
            "workspace".to_string(),
            "workspace".to_string(),
            super_args(),
        ),
    })));

    let rendered = ready_body(&state, 120)
        .iter()
        .map(line_text)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("Tunnel runtime ready"), "{rendered}");
    assert!(
        rendered.contains("OpenAI client: 0.0.13 · /healthz and /readyz ready"),
        "{rendered}"
    );
    assert!(
        rendered.contains("ChatGPT connector: not verified"),
        "{rendered}"
    );
    assert!(
        rendered.contains("Browser remains local on loopback"),
        "{rendered}"
    );
    assert!(
        !rendered.contains("Local-only mode stays on loopback"),
        "{rendered}"
    );
}

#[test]
fn clipboard_injection_receives_the_exact_url_bytes_without_a_newline() {
    let url = PublicMcpEndpoint::new("https://shell.example.com", "capability").unwrap();
    let mut copied = Vec::new();
    copy_public_url_to_clipboard_with(&url, |bytes| {
        copied.extend_from_slice(bytes);
        Ok(())
    })
    .unwrap();
    assert_eq!(copied, url.as_str().as_bytes());
    assert!(!format!("{url:?}").contains("capability"));
}

#[test]
fn stop_keys_include_q_ctrl_c_and_resize_only_redraws() {
    let mut state = state();
    assert!(matches!(
        state.handle_event(Event::Key(KeyEvent::new(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
        ))),
        ExposeTuiAction::Stop
    ));
    assert!(matches!(
        state.handle_event(Event::Key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        ))),
        ExposeTuiAction::Stop
    ));
    assert!(matches!(
        state.handle_event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE,))),
        ExposeTuiAction::Stop
    ));
    state.redraw_needed = false;
    assert!(matches!(
        state.handle_event(Event::Resize(120, 40)),
        ExposeTuiAction::None
    ));
    assert!(state.redraw_needed);
}

#[test]
fn ready_copies_with_the_mandated_c_key() {
    let mut state = state();
    let url = PublicMcpEndpoint::new("https://shell.example.com", "capability").unwrap();
    state.apply_engine_event(ExposeLifecycleEvent::Ready(Box::new(ExposeReadyState {
        local_url: "http://127.0.0.1:1234/expose#bootstrap=bootstrap".to_string(),
        local_mcp_url: PublicMcpEndpoint::new("http://127.0.0.1:1234", "capability").unwrap(),
        public_browser_url: Some(
            "https://shell.example.com/expose#bootstrap=bootstrap".to_string(),
        ),
        public_url: Some(url),
        instance_id: "pdxi_test".to_string(),
        workspace_name: "workspace".to_string(),
        display_name: "workspace".to_string(),
        endpoint: ExposeEndpointMode::QuickTunnel,
        mcp: ExposeMcpEndpoint::new(
            "capability",
            "pdxi_test".to_string(),
            PathBuf::from("/home/test-user/workspace"),
            "workspace".to_string(),
            "workspace".to_string(),
            super_args(),
        ),
    })));
    assert!(matches!(
        state.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)),
        ExposeTuiAction::CopyUrl
    ));
}

#[test]
fn long_unbroken_value_wraps_at_display_width_without_mutation() {
    let value = "https://表🙂.example.com/very-long-path/capability";
    let lines = labeled_value_lines(
        "Public MCP URL",
        value,
        32,
        super::tui_primary_style(),
        super::tui_primary_style(),
    );
    assert!(lines.iter().all(|line| line.width() <= 32));
    assert_eq!(
        reconstruct_field(&lines, "Public MCP URL", value, 32),
        value
    );
    assert!(lines.iter().all(|line| !line_text(line).contains("...")));
}

#[test]
fn ready_body_preserves_all_four_long_urls_at_a_narrow_width() {
    let state = long_ready_state();
    let ready = state.ready.as_ref().expect("ready fixture");
    let fields = [
        (
            "Public MCP URL",
            ready.public_url.as_ref().expect("public URL").as_str(),
        ),
        ("MCP URL", ready.local_mcp_url.as_str()),
        (
            "Public Browser URL",
            ready
                .public_browser_url
                .as_deref()
                .expect("public browser URL"),
        ),
        ("Browser URL", ready.local_url.as_str()),
    ];

    for width in [160, 120, 100, 80, 60, 56] {
        let lines = ready_body(&state, width);
        assert!(
            lines
                .iter()
                .all(|line| text_width(&line_text(line)) <= width)
        );
        for (label, value) in fields.iter().copied() {
            assert_eq!(
                reconstruct_field(&lines, label, value, width),
                value,
                "{label} must remain complete at width {width}"
            );
        }
    }
}

#[test]
fn ready_body_scrolls_wrapped_content_and_clamps_after_resize() {
    let mut state = state();
    let long_state = long_ready_state();
    state.ready = long_state.ready;
    state.phase = ExposeTuiPhase::Ready;
    let mut terminal = Terminal::new(TestBackend::new(120, 20)).expect("test terminal");

    terminal
        .draw(|frame| draw_frame(frame, &mut state))
        .expect("wide render");
    terminal.backend_mut().resize(70, 12);
    terminal
        .draw(|frame| draw_frame(frame, &mut state))
        .expect("narrow render");
    assert_eq!(state.body_scroll(), 0);

    state.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
    terminal
        .draw(|frame| draw_frame(frame, &mut state))
        .expect("scrolled render");
    let narrow_body = ready_body(&state, 68);
    assert_eq!(
        state.body_scroll(),
        narrow_body
            .len()
            .saturating_sub(12_usize.saturating_sub(5).saturating_sub(2))
    );

    terminal.backend_mut().resize(140, 60);
    terminal
        .draw(|frame| draw_frame(frame, &mut state))
        .expect("resized render");
    let resized_body = ready_body(&state, 138);
    assert_eq!(
        state.body_scroll(),
        resized_body
            .len()
            .saturating_sub(60_usize.saturating_sub(5).saturating_sub(2))
    );
}

fn long_ready_state() -> ExposeTuiState {
    let mut state = state();
    let capability = "capability_0123456789abcdef0123456789abcdef0123456789abcdef";
    let public_url = PublicMcpEndpoint::new(
        "https://very-long-generated-subdomain.trycloudflare.com",
        capability,
    )
    .expect("public URL");
    let local_mcp_url =
        PublicMcpEndpoint::new("http://127.0.0.1:1234", capability).expect("local MCP URL");
    state.apply_engine_event(ExposeLifecycleEvent::Ready(Box::new(ExposeReadyState {
        local_url: format!(
            "http://127.0.0.1:1234/expose#bootstrap={capability}_local_browser_bootstrap"
        ),
        local_mcp_url,
        public_browser_url: Some(format!(
            "https://very-long-generated-subdomain.trycloudflare.com/expose#bootstrap={capability}_public_browser_bootstrap"
        )),
        public_url: Some(public_url),
        instance_id: "pdxi_test".to_string(),
        workspace_name: "workspace".to_string(),
        display_name: "workspace".to_string(),
        endpoint: ExposeEndpointMode::QuickTunnel,
        mcp: ExposeMcpEndpoint::new(
            "capability",
            "pdxi_test".to_string(),
            PathBuf::from("/home/test-user/workspace"),
            "workspace".to_string(),
            "workspace".to_string(),
            super_args(),
        ),
    })));
    state
}

fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

fn reconstruct_field(lines: &[Line<'_>], label: &str, value: &str, width: usize) -> String {
    let prefix = format!("{label}: ");
    let start = lines
        .iter()
        .position(|line| line_text(line).starts_with(&prefix))
        .expect("field label");
    let value_width = width.saturating_sub(text_width(&prefix));
    let chunks = chunk_token(value, value_width);
    let indent = " ".repeat(text_width(&prefix));
    let mut rendered = line_text(&lines[start])
        .strip_prefix(&prefix)
        .expect("field prefix")
        .to_string();
    for line in lines
        .iter()
        .skip(start + 1)
        .take(chunks.len().saturating_sub(1))
    {
        rendered.push_str(
            line_text(line)
                .strip_prefix(&indent)
                .expect("field continuation indentation"),
        );
    }
    rendered
}
