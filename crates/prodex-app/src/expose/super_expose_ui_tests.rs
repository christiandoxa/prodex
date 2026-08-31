use super::super::mcp::ExposeMcpEndpoint;
use super::{
    ExposeEndpointMode, ExposeLifecycleEvent, ExposeLifecyclePhase, ExposeReadyState,
    ExposeTuiAction, ExposeTuiPhase, ExposeTuiState, PublicMcpEndpoint,
    copy_public_url_to_clipboard_with, visible_url,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use prodex_cli::SuperArgs;
use std::path::PathBuf;

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
    assert_eq!(state.phase(), ExposeTuiPhase::EndpointSelection);
    assert!(matches!(
        state.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE)),
        ExposeTuiAction::None
    ));
    for character in "shell.example.com".chars() {
        state.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
    }
    assert!(matches!(
        state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        ExposeTuiAction::Start(ExposeEndpointMode::ExistingCloudflareTunnel {
            hostname,
            origin_port: 8765,
        }) if hostname == "shell.example.com"
    ));
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
    state.apply_engine_event(ExposeLifecycleEvent::Ready(ready));
    assert_eq!(state.phase(), ExposeTuiPhase::Ready);
    state.apply_engine_event(ExposeLifecycleEvent::Stopped);
    assert_eq!(state.phase(), ExposeTuiPhase::Stopped);
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
    state.apply_engine_event(ExposeLifecycleEvent::Ready(ExposeReadyState {
        local_url: "http://127.0.0.1:1234/expose#bootstrap=bootstrap".to_string(),
        local_mcp_url: PublicMcpEndpoint::new("http://127.0.0.1:1234", "capability").unwrap(),
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
    }));
    assert!(matches!(
        state.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)),
        ExposeTuiAction::CopyUrl
    ));
}

#[test]
fn narrow_url_viewport_is_bounded_and_keeps_the_canonical_value_atomic() {
    let url = "https://example.trycloudflare.com/pdx/v1/abc123/mcp";
    for width in [1, 8, 16, 32, 60, 70, 80, 100, 120, 160, 200] {
        let visible = visible_url(url, 0, width);
        assert!(visible.chars().count() <= width);
        assert!(!visible.contains(['\n', '\r']));
    }
    assert_eq!(visible_url(url, 0, url.chars().count()), url);
    assert!(visible_url(url, 4, 16).starts_with('<'));
    assert!(visible_url(url, url.chars().count(), 16).ends_with("/mcp"));
}

#[test]
fn ready_url_navigation_only_changes_the_viewport_offset() {
    let mut state = state();
    let url = PublicMcpEndpoint::new("https://example.trycloudflare.com", "abc123").unwrap();
    let canonical = url.as_str().to_string();
    state.apply_engine_event(ExposeLifecycleEvent::Ready(ExposeReadyState {
        local_url: "http://127.0.0.1:1234/expose#bootstrap=bootstrap".to_string(),
        local_mcp_url: PublicMcpEndpoint::new("http://127.0.0.1:1234", "capability").unwrap(),
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
    }));
    state.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
    state.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
    state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    state.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    assert_eq!(
        state
            .ready
            .as_ref()
            .unwrap()
            .public_url
            .as_ref()
            .unwrap()
            .as_str(),
        canonical
    );
}
