use super::*;

#[test]
fn dispatch_validation_preserves_request_id_and_param_precedence() {
    use SuperExposeDispatchDecision::*;

    assert_eq!(
        super_expose_dispatch_validation(
            "notifications/initialized",
            false,
            false,
            false,
            false,
            false,
            0,
        )
        .unwrap(),
        NotificationAccepted
    );
    assert_eq!(
        super_expose_dispatch_validation("ping", false, false, false, false, false, 0).unwrap(),
        NotificationUnsupported
    );
    assert_eq!(
        super_expose_dispatch_validation("ping", true, false, false, false, false, 0).unwrap(),
        InvalidRequestId
    );
    assert_eq!(
        super_expose_dispatch_validation("initialize", true, true, false, false, false, 0).unwrap(),
        InitializeParamsRequired
    );
    assert_eq!(
        super_expose_dispatch_validation("initialize", true, true, true, false, false, 0).unwrap(),
        ProtocolVersionRequired
    );
    assert_eq!(
        super_expose_dispatch_validation("tools/call", true, true, false, false, false, 0).unwrap(),
        ToolParamsRequired
    );
    assert_eq!(
        super_expose_dispatch_validation("tools/call", true, true, true, false, false, 0).unwrap(),
        ToolNameRequired
    );
    assert_eq!(
        super_expose_dispatch_validation("tools/call", true, true, true, false, true, 2).unwrap(),
        ToolArgumentsObjectRequired
    );
    assert_eq!(
        super_expose_dispatch_validation("tools/call", true, true, true, false, true, 0).unwrap(),
        Ok
    );
    assert_eq!(
        super_expose_dispatch_validation("unknown/method", true, true, false, false, false, 0)
            .unwrap(),
        Ok
    );
}

#[test]
fn route_labels_are_mojo_owned_and_canonical() {
    for (method, expected_kind, expected_label) in [
        (
            "server/discover",
            SuperExposeMethod::ServerDiscover,
            "server_discover",
        ),
        ("initialize", SuperExposeMethod::Initialize, "initialize"),
        ("ping", SuperExposeMethod::Ping, "ping"),
        ("tools/list", SuperExposeMethod::ToolsList, "tools_list"),
        ("tools/call", SuperExposeMethod::ToolsCall, "tools_call"),
        (
            "notifications/initialized",
            SuperExposeMethod::Notification,
            "notification",
        ),
        ("unknown/method", SuperExposeMethod::Unknown, "unknown"),
    ] {
        let route = super_expose_route(method, None).unwrap();
        assert_eq!(route.method, expected_kind);
        assert_eq!(route.method.label().unwrap(), expected_label);
    }
    for (tool, expected_kind, expected_label) in [
        ("prodex_super_start", SuperExposeTool::Start, "start"),
        ("prodex_super_status", SuperExposeTool::Status, "status"),
        ("prodex_super_events", SuperExposeTool::Events, "events"),
        ("prodex_super_result", SuperExposeTool::Result, "result"),
        ("prodex_super_cancel", SuperExposeTool::Cancel, "cancel"),
        ("prodex_super_list", SuperExposeTool::List, "list"),
        ("prodex_super_exec", SuperExposeTool::Exec, "exec"),
        (
            "prodex_session_prompt_write",
            SuperExposeTool::SessionPromptWrite,
            "session_prompt_write",
        ),
        (
            "prodex_session_preempt",
            SuperExposeTool::SessionPreempt,
            "session_preempt",
        ),
        (
            "prodex_session_output_read",
            SuperExposeTool::SessionOutputRead,
            "session_output_read",
        ),
        ("unknown", SuperExposeTool::Unknown, "unknown"),
    ] {
        let route = super_expose_route("tools/call", Some(tool)).unwrap();
        assert_eq!(route.tool, expected_kind);
        assert_eq!(route.tool.label().unwrap(), expected_label);
    }
}

#[test]
fn protocol_version_and_metadata_policy_preserve_precedence() {
    assert!(super_expose_protocol_version_supported("2026-07-28").unwrap());
    assert!(super_expose_protocol_version_supported("2024-11-05").unwrap());
    assert!(!super_expose_protocol_version_supported(&"x".repeat(512)).unwrap());

    assert_eq!(
        super_expose_protocol_metadata(
            "ping",
            Some("2025-11-25"),
            Some("2025-11-25"),
            None,
            None,
            None,
        )
        .unwrap(),
        SuperExposeProtocolDecision::Ok,
    );
    assert_eq!(
        super_expose_protocol_metadata(
            "ping",
            Some("2099-01-01"),
            Some("2026-07-28"),
            Some("ping"),
            None,
            None,
        )
        .unwrap(),
        SuperExposeProtocolDecision::UnsupportedVersion,
    );
    assert_eq!(
        super_expose_protocol_metadata(
            "ping",
            Some("2025-11-25"),
            Some("2026-07-28"),
            Some("ping"),
            None,
            None,
        )
        .unwrap(),
        SuperExposeProtocolDecision::VersionMismatch,
    );
    assert_eq!(
        super_expose_protocol_metadata("ping", Some("2026-07-28"), None, Some("ping"), None, None,)
            .unwrap(),
        SuperExposeProtocolDecision::MetadataRequired,
    );
    assert_eq!(
        super_expose_protocol_metadata(
            "ping",
            Some("2026-07-28"),
            Some("2026-07-28"),
            Some("tools/list"),
            None,
            None,
        )
        .unwrap(),
        SuperExposeProtocolDecision::MethodMismatch,
    );
    assert_eq!(
        super_expose_protocol_metadata(
            "tools/call",
            Some("2026-07-28"),
            Some("2026-07-28"),
            Some("tools/call"),
            Some("prodex_super_status"),
            Some("prodex_super_result"),
        )
        .unwrap(),
        SuperExposeProtocolDecision::NameMismatch,
    );
}

#[test]
fn media_headers_and_json_nesting_match_rust_contract() {
    assert!(
        super_expose_content_type_allowed(Some("\u{2003}APPLICATION/JSON\u{3000}; charset=utf-8"))
            .unwrap()
    );
    assert!(!super_expose_content_type_allowed(None).unwrap());
    assert!(
        super_expose_accept_allowed(Some("text/plain;q=0.2, \u{2003}application/json ; q=1"))
            .unwrap()
    );
    assert!(!super_expose_accept_allowed(Some("text/plain")).unwrap());

    let body = br#"{"text":"[{}]","items":[{}]}"#;
    assert!(super_expose_json_nesting_within_limit(body, 3).unwrap());
    assert!(!super_expose_json_nesting_within_limit(body, 2).unwrap());
    assert!(!super_expose_json_nesting_within_limit(b"}", 64).unwrap());
    assert!(!super_expose_json_nesting_within_limit(b"{\"x\":\"abc\\", 64).unwrap());
}

#[test]
fn tool_argument_and_identifier_policy_match_public_contract() {
    assert!(super_expose_tool_argument_allowed("prodex_super_events", "run_id").unwrap());
    assert!(super_expose_tool_argument_allowed("prodex_super_events", "after_seq").unwrap());
    assert!(!super_expose_tool_argument_allowed("prodex_super_events", "extra").unwrap());
    assert!(!super_expose_tool_argument_allowed("prodex_super_list", &"x".repeat(1024),).unwrap());
    assert!(super_expose_tool_argument_allowed("unknown-tool", "anything").unwrap());

    assert!(super_expose_run_id_valid("spr_").unwrap());
    assert!(super_expose_run_id_valid("spr_ABC-123").unwrap());
    assert!(!super_expose_run_id_valid("spr_bad:1").unwrap());

    assert!(super_expose_string_valid("model α", 256).unwrap());
    assert!(!super_expose_string_valid("bad\nmodel", 256).unwrap());
    assert!(!super_expose_string_valid("bad\u{0085}model", 256).unwrap());
    assert!(!super_expose_string_valid(&"x".repeat(257), 256).unwrap());
}

#[test]
fn output_cursor_validation_is_mojo_owned_and_sensitive() {
    const { assert!(crate::MOJO_ACTIVE) }

    let base = SuperExposeOutputCursorInput {
        encoded_length: 512,
        version: 1,
        prodex_pid: 42,
        codex_pid: 43,
        prodex_birth_present: true,
        codex_birth_present: true,
        thread_id_present: true,
        source_id_present: true,
        checkpoint_id_present: true,
        event_index: 7,
    };
    assert!(super_expose_output_cursor_valid(base).unwrap());

    let mutators: &[fn(&mut SuperExposeOutputCursorInput)] = &[
        |input| input.encoded_length = 16 * 1024 + 1,
        |input| input.version = 2,
        |input| input.prodex_pid = 0,
        |input| input.codex_pid = 0,
        |input| input.prodex_birth_present = false,
        |input| input.codex_birth_present = false,
        |input| input.thread_id_present = false,
        |input| input.source_id_present = false,
        |input| input.checkpoint_id_present = false,
        |input| input.event_index = 65_537,
    ];
    for mutate in mutators {
        let mut candidate = base;
        mutate(&mut candidate);
        assert!(!super_expose_output_cursor_valid(candidate).unwrap());
    }
}
