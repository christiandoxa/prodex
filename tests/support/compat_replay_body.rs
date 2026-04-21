use super::*;
use serde_json::Value;

fn compat_replay_value_scrub(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for key in [
                "request_id",
                "session_id",
                "response_id",
                "trace_id",
                "created_at",
                "updated_at",
                "started_at",
                "timestamp",
                "ts",
                "pid",
            ] {
                if let Some(entry) = map.get_mut(key)
                    && !entry.is_null()
                {
                    *entry = Value::String(format!("<{key}>"));
                }
            }

            for nested in map.values_mut() {
                compat_replay_value_scrub(nested);
            }
        }
        Value::Array(items) => {
            for nested in items {
                compat_replay_value_scrub(nested);
            }
        }
        _ => {}
    }
}

fn compat_replay_normalize_json(bytes: &[u8]) -> String {
    let mut value: Value =
        serde_json::from_slice(bytes).expect("replay payload should be valid JSON");
    compat_replay_value_scrub(&mut value);
    serde_json::to_string(&value).expect("replay payload should reserialize")
}

fn compat_replay_anthropic_request(headers: Vec<(&str, &str)>, body: Value) -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: headers
            .into_iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect(),
        body: body.to_string().into_bytes(),
    }
}

fn compat_replay_translate_anthropic_request(request: &RuntimeProxyRequest) -> String {
    let translated =
        translate_runtime_anthropic_messages_request(request).expect("translation should succeed");
    compat_replay_normalize_json(&translated.translated_request.body)
}

fn compat_replay_websocket_handshake_request(headers: Vec<(&str, &str)>) -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: headers
            .into_iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect(),
        body: Vec::new(),
    }
}

fn compat_replay_anthropic_contract_body() -> Value {
    serde_json::json!({
        "model": "claude-sonnet-4-6",
        "stream": true,
        "mcp_servers": [
            {
                "name": "local_fs",
                "url": "https://mcp.example.com/sse"
            }
        ],
        "tools": [
            {
                "type": "mcp_toolset",
                "mcp_server_name": "local_fs",
                "name": "filesystem"
            },
            {
                "type": "mcp_toolset",
                "mcp_server_name": "local_fs",
                "name": "filesystem_readonly"
            }
        ],
        "messages": [
            {
                "role": "system",
                "content": "You are a compatibility replay harness."
            },
            {
                "role": "user",
                "content": "List the workspace files."
            }
        ]
    })
}

fn compat_replay_anthropic_contract_variants() -> Vec<RuntimeProxyRequest> {
    let body = compat_replay_anthropic_contract_body();
    vec![
        compat_replay_anthropic_request(
            vec![
                ("Content-Type", "application/json"),
                ("x-api-key", "dummy"),
                ("anthropic-version", "2023-06-01"),
            ],
            body.clone(),
        ),
        compat_replay_anthropic_request(
            vec![
                ("anthropic-version", "2023-06-01"),
                ("x-api-key", "dummy"),
                ("Content-Type", "application/json"),
                ("User-Agent", "claude-cli/test"),
            ],
            body.clone(),
        ),
        compat_replay_anthropic_request(
            vec![
                ("x-api-key", "dummy"),
                ("Content-Type", "application/json"),
                ("anthropic-version", "2023-06-01"),
                ("x-claude-code-session-id", "claude-session-42"),
            ],
            body,
        ),
    ]
}

#[test]
fn compat_replay_anthropic_translation_is_stable_across_header_noise() {
    let variants = compat_replay_anthropic_contract_variants();
    let expected = compat_replay_translate_anthropic_request(
        variants.first().expect("at least one replay variant"),
    );

    for request in variants {
        let snapshot = compat_replay_translate_anthropic_request(&request);
        assert_eq!(
            snapshot, expected,
            "Anthropic replay snapshot drifted under header noise"
        );
    }
}

#[test]
fn compat_replay_anthropic_translation_survives_chaos_replay_loops() {
    let variants = compat_replay_anthropic_contract_variants();
    let mut previous_snapshot = None;

    for iteration in 0..48 {
        let request = &variants[iteration % variants.len()];
        let snapshot = compat_replay_translate_anthropic_request(request);
        if let Some(previous_snapshot) = previous_snapshot.as_ref() {
            assert_eq!(
                snapshot, *previous_snapshot,
                "replay snapshot changed during a soak-style replay loop"
            );
        }
        previous_snapshot = Some(snapshot);
    }
}

#[test]
fn compat_replay_claude_session_id_header_is_case_insensitive() {
    let request = compat_replay_anthropic_request(
        vec![
            ("X-Claude-Code-Session-Id", "claude-session-42"),
            ("Content-Type", "application/json"),
        ],
        compat_replay_anthropic_contract_body(),
    );

    assert_eq!(
        runtime_proxy_claude_session_id(&request),
        Some("claude-session-42".to_string())
    );
}

#[test]
fn compat_replay_request_capabilities_detect_claude_code_mcp_stream() {
    let request = compat_replay_anthropic_request(
        vec![
            ("User-Agent", "claude-code/1.2.3"),
            ("x-claude-code-session-id", "claude-session-42"),
            ("Content-Type", "application/json"),
            ("anthropic-version", "2023-06-01"),
        ],
        compat_replay_anthropic_contract_body(),
    );

    let surface = runtime_detect_request_compatibility_surface(&request, "request", "http");

    assert_eq!(surface.family, "claude_code");
    assert_eq!(surface.client, "claude_code");
    assert_eq!(surface.route, "anthropic_messages");
    assert_eq!(surface.transport, "http");
    assert_eq!(surface.stream, "streaming");
    assert_eq!(surface.tool_surface, "mcp");
    assert_eq!(surface.continuation, "none");
    assert!(!surface.approval);
    assert!(surface.warnings.is_empty());
}

#[test]
fn compat_replay_websocket_capabilities_warn_without_turn_state() {
    let handshake_request =
        compat_replay_websocket_handshake_request(vec![("User-Agent", "codex-cli/test-fixture")]);
    let surface = runtime_detect_websocket_message_compatibility_surface(
        &handshake_request,
        r#"{"previous_response_id":"resp_123","input":[]}"#,
    );

    assert_eq!(surface.family, "codex");
    assert_eq!(surface.client, "codex_cli");
    assert_eq!(surface.route, "responses");
    assert_eq!(surface.transport, "websocket");
    assert_eq!(surface.stream, "streaming");
    assert_eq!(surface.continuation, "previous_response");
    assert!(
        surface
            .warnings
            .contains(&"websocket_previous_response_without_turn_state")
    );
}

#[test]
fn compat_replay_doctor_fields_surface_compat_warnings() {
    let tail = [
        "[2026-04-10 00:00:00.000 +07:00] request=1 compat_request_surface stage=request family=claude_code client=claude_code route=anthropic_messages transport=http stream=streaming tool_surface=mcp continuation=none approval=false origin=external user_agent=claude-code/1.2.3",
        "[2026-04-10 00:00:01.000 +07:00] request=1 compat_warning stage=request family=claude_code client=claude_code route=anthropic_messages transport=http warning=anthropic_mcp_servers_without_toolset",
    ]
    .join("\n");
    let summary = summarize_runtime_log_tail(tail.as_bytes());
    let fields = runtime_doctor_fields_for_summary(
        &summary,
        std::path::Path::new("/tmp/prodex-runtime-latest.path"),
    )
    .into_iter()
    .collect::<BTreeMap<_, _>>();
    let value = runtime_doctor_json_value(&summary);

    assert_eq!(summary.compat_warning_count, 1);
    assert_eq!(
        summary.top_client_family.as_deref(),
        Some("claude_code (2)")
    );
    assert_eq!(summary.top_client.as_deref(), Some("claude_code (2)"));
    assert_eq!(summary.top_tool_surface.as_deref(), Some("mcp (1)"));
    assert_eq!(
        summary.top_compat_warning.as_deref(),
        Some("anthropic_mcp_servers_without_toolset (1)")
    );
    assert_eq!(fields.get("Compat samples").map(String::as_str), Some("1"));
    assert_eq!(fields.get("Compat warnings").map(String::as_str), Some("1"));
    assert_eq!(
        fields.get("Client family").map(String::as_str),
        Some("claude_code (2)")
    );
    assert_eq!(
        fields.get("Top client").map(String::as_str),
        Some("claude_code (2)")
    );
    assert_eq!(
        fields.get("Tool surface").map(String::as_str),
        Some("mcp (1)")
    );
    assert_eq!(
        fields.get("Compat warning").map(String::as_str),
        Some("anthropic_mcp_servers_without_toolset (1)")
    );
    assert_eq!(value["compat_warning_count"], 1);
    assert_eq!(value["top_client_family"], "claude_code (2)");
    assert_eq!(value["top_client"], "claude_code (2)");
    assert_eq!(value["top_tool_surface"], "mcp (1)");
    assert_eq!(
        value["top_compat_warning"],
        "anthropic_mcp_servers_without_toolset (1)"
    );
}

#[test]
fn compat_replay_doctor_classifies_context_loss_warning_from_log_text() {
    let tail = [
        "[2026-04-10 00:00:00.000 +07:00] request=7 compat_request_surface stage=message family=codex client=codex_cli route=responses transport=websocket stream=streaming tool_surface=none continuation=previous_response origin=external approval=false user_agent=codex-cli/test-fixture",
        "[2026-04-10 00:00:00.005 +07:00] request=7 compat_warning stage=message family=codex client=codex_cli route=responses transport=websocket warning=websocket_previous_response_without_turn_state",
    ]
    .join("\n");
    let mut summary = summarize_runtime_log_tail(tail.as_bytes());
    summary.pointer_exists = true;
    summary.log_exists = true;
    runtime_doctor_finalize_summary(&mut summary);
    let fields = runtime_doctor_fields_for_summary(
        &summary,
        std::path::Path::new("/tmp/prodex-runtime-latest.path"),
    )
    .into_iter()
    .collect::<BTreeMap<_, _>>();

    assert_eq!(summary.compat_warning_count, 1);
    assert_eq!(summary.top_client_family.as_deref(), Some("codex (2)"));
    assert_eq!(summary.top_client.as_deref(), Some("codex_cli (2)"));
    assert_eq!(
        summary.top_compat_warning.as_deref(),
        Some("websocket_previous_response_without_turn_state (1)")
    );
    assert_eq!(
        summary.diagnosis,
        "Recent compatibility warnings were observed for codex_cli (2): websocket_previous_response_without_turn_state (1)."
    );
    assert_eq!(fields.get("Compat warnings").map(String::as_str), Some("1"));
    assert_eq!(
        fields.get("Compat warning").map(String::as_str),
        Some("websocket_previous_response_without_turn_state (1)")
    );
}
