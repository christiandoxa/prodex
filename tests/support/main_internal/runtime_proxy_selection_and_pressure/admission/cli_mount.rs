use super::*;

#[test]
fn section_headers_use_cli_width() {
    assert_eq!(
        text_width(&section_header_with_width("Quota Overview", CLI_WIDTH)),
        CLI_WIDTH
    );
}

#[test]
fn field_lines_do_not_exceed_cli_width() {
    let label_width = panel_label_width(
            &[(
                "Path".to_string(),
                "/tmp/some/really/long/path/that/should/still/stay/inside/the/configured/cli/width/when/rendered"
                    .to_string(),
            )],
            CLI_WIDTH,
        );
    let fields = format_field_lines_with_layout(
        "Path",
        "/tmp/some/really/long/path/that/should/still/stay/inside/the/configured/cli/width/when/rendered",
        CLI_WIDTH,
        label_width,
    );

    assert!(fields.iter().all(|line| text_width(line) <= CLI_WIDTH));
}

#[test]
fn section_headers_expand_to_requested_width() {
    assert_eq!(text_width(&section_header_with_width("Doctor", 72)), 72);
}

#[test]
fn field_lines_respect_requested_width() {
    let width = 72;
    let fields = vec![(
        "Profiles root".to_string(),
        "/tmp/some/really/long/path/that/needs/to/wrap/narrower".to_string(),
    )];
    let label_width = panel_label_width(&fields, width);
    let lines = format_field_lines_with_layout(
        "Profiles root",
        "/tmp/some/really/long/path/that/needs/to/wrap/narrower",
        width,
        label_width,
    );

    assert!(lines.iter().all(|line| text_width(line) <= width));
}

#[test]
fn shared_panel_builder_matches_render_panel_output() {
    let mut panel = PanelBuilder::new("Doctor");
    panel.push("Profile", "main");
    panel.extend([("Status".to_string(), "Ready".to_string())]);

    let expected = render_panel(
        "Doctor",
        &[
            ("Profile".to_string(), "main".to_string()),
            ("Status".to_string(), "Ready".to_string()),
        ],
    );

    assert_eq!(panel.render(), expected);
}

#[test]
fn runtime_proxy_injects_codex_backend_overrides() {
    let args = runtime_proxy_codex_args(
        "127.0.0.1:4455".parse().expect("socket addr"),
        &[OsString::from("exec"), OsString::from("hello")],
    );
    let rendered = args
        .iter()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    assert_eq!(rendered[0], "-c");
    assert!(rendered[1].contains("chatgpt_base_url=\"http://127.0.0.1:4455/backend-api\""));
    assert_eq!(rendered[2], "-c");
    assert_eq!(
        rendered[3],
        format!(
            "openai_base_url=\"http://127.0.0.1:4455{}\"",
            RUNTIME_PROXY_OPENAI_MOUNT_PATH
        )
    );
    assert_eq!(&rendered[4..], ["exec", "hello"]);
}

#[test]
fn runtime_proxy_maps_openai_prefix_to_upstream_backend_api() {
    assert_eq!(
        runtime_proxy_upstream_url(
            "https://chatgpt.com/backend-api",
            &format!("{}/responses", RUNTIME_PROXY_OPENAI_MOUNT_PATH)
        ),
        "https://chatgpt.com/backend-api/codex/responses"
    );
}

#[test]
fn runtime_proxy_injects_custom_openai_mount_path_overrides() {
    let args = runtime_proxy_codex_args_with_mount_path(
        "127.0.0.1:4455".parse().expect("socket addr"),
        "/backend-api/prodex/v0.2.99",
        &[OsString::from("exec")],
    );
    let rendered = args
        .iter()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    assert_eq!(
        rendered[3],
        "openai_base_url=\"http://127.0.0.1:4455/backend-api/prodex/v0.2.99\""
    );
}

#[test]
fn runtime_proxy_passthrough_args_preserve_user_args_without_proxy() {
    let user_args = vec![OsString::from("exec"), OsString::from("hello")];
    assert_eq!(
        runtime_proxy_codex_passthrough_args(None, &user_args),
        user_args
    );
}

#[test]
fn runtime_proxy_passthrough_args_follow_endpoint_mount_path() {
    let temp_dir = TestDir::isolated();
    let endpoint = RuntimeProxyEndpoint {
        listen_addr: "127.0.0.1:4455".parse().expect("listen addr"),
        openai_mount_path: "/backend-api/prodex/v0.2.99".to_string(),
        lease_dir: temp_dir.path.join("leases"),
        _lease: None,
    };

    let rendered = runtime_proxy_codex_passthrough_args(
        Some(&endpoint),
        &[OsString::from("exec"), OsString::from("hello")],
    )
    .into_iter()
    .map(|arg| arg.to_string_lossy().into_owned())
    .collect::<Vec<_>>();

    assert_eq!(rendered[0], "-c");
    assert_eq!(
        rendered[3],
        "openai_base_url=\"http://127.0.0.1:4455/backend-api/prodex/v0.2.99\""
    );
    assert_eq!(&rendered[4..], ["exec", "hello"]);
}

#[test]
fn runtime_proxy_maps_legacy_versioned_openai_prefix_to_upstream_backend_api() {
    assert_eq!(
        runtime_proxy_upstream_url(
            "https://chatgpt.com/backend-api",
            "/backend-api/prodex/v0.2.99/responses"
        ),
        "https://chatgpt.com/backend-api/codex/responses"
    );
}

#[test]
fn runtime_proxy_accepts_legacy_openai_prefix() {
    assert!(is_runtime_responses_path("/backend-api/codex/responses"));
    assert!(is_runtime_compact_path(
        "/backend-api/codex/responses/compact"
    ));
    assert!(is_runtime_realtime_call_path(
        "/backend-api/codex/realtime/calls"
    ));
    assert!(is_runtime_realtime_websocket_path(
        "/backend-api/codex/realtime?call_id=call-123"
    ));
    assert!(is_runtime_responses_path(
        "/backend-api/prodex/v0.2.99/responses"
    ));
    assert!(is_runtime_compact_path(
        "/backend-api/prodex/v0.2.99/responses/compact"
    ));
    assert!(is_runtime_realtime_call_path(
        "/backend-api/prodex/v0.2.99/realtime/calls"
    ));
    assert!(is_runtime_realtime_websocket_path(
        "/backend-api/prodex/v0.2.99/realtime?call_id=call-123"
    ));
    assert!(!is_runtime_realtime_websocket_path(
        "/backend-api/prodex/v0.2.99/realtime/calls"
    ));
    assert_eq!(
        runtime_proxy_upstream_url(
            "https://chatgpt.com/backend-api",
            "/backend-api/codex/responses"
        ),
        "https://chatgpt.com/backend-api/codex/responses"
    );
    assert_eq!(
        runtime_proxy_upstream_url(
            "https://chatgpt.com/backend-api",
            "/backend-api/prodex/realtime/calls"
        ),
        "https://chatgpt.com/backend-api/codex/realtime/calls"
    );
}

#[test]
fn runtime_request_session_id_accepts_realtime_header() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/prodex/realtime/calls".to_string(),
        headers: vec![("x-session-id".to_string(), " sess-realtime ".to_string())],
        body: Vec::new(),
    };

    assert_eq!(
        runtime_request_session_id(&request),
        Some("sess-realtime".to_string())
    );
}

#[test]
fn runtime_request_explicit_session_id_ignores_body_session_id() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: br#"{"session_id":"sess-body","input":[]}"#.to_vec(),
    };

    assert_eq!(runtime_request_explicit_session_id(&request), None);
    assert_eq!(
        runtime_request_session_id(&request),
        Some("sess-body".to_string())
    );
}

#[test]
fn runtime_request_explicit_session_id_ignores_turn_metadata_session_id() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: vec![(
            "x-codex-turn-metadata".to_string(),
            r#"{"source":"resume","session_id":"sess-metadata"}"#.to_string(),
        )],
        body: Vec::new(),
    };

    assert_eq!(runtime_request_explicit_session_id(&request), None);
    assert_eq!(
        runtime_request_session_id(&request),
        Some("sess-metadata".to_string())
    );
}

#[test]
fn runtime_proxy_broker_key_uses_stable_mount_path() {
    let current_key = runtime_broker_key("https://chatgpt.com/backend-api", false);

    let stable_key = runtime_broker_key_for_binary_identity(
        "https://chatgpt.com/backend-api",
        false,
        &runtime_broker_current_binary_identity_key(),
    );
    let previous_version_key = runtime_broker_key_for_binary_identity(
        "https://chatgpt.com/backend-api",
        false,
        "version=0.29.0",
    );

    let versioned_key = {
        let mut hasher = DefaultHasher::new();
        "https://chatgpt.com/backend-api".hash(&mut hasher);
        false.hash(&mut hasher);
        "/backend-api/prodex/v0.2.99".hash(&mut hasher);
        runtime_broker_current_binary_identity_key().hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    };

    assert_eq!(current_key, stable_key);
    assert_ne!(current_key, previous_version_key);
    assert_ne!(current_key, versioned_key);
}
