use super::*;
use serde_json::Value;

fn compat_replay_fixture_text(name: &str) -> &'static str {
    match name {
        "claude_mcp_expected_surface.json" => {
            include_str!("../fixtures/compat_replay/claude_mcp_expected_surface.json")
        }
        "claude_mcp_expected_translation.json" => {
            include_str!("../fixtures/compat_replay/claude_mcp_expected_translation.json")
        }
        "claude_mcp_request.json" => {
            include_str!("../fixtures/compat_replay/claude_mcp_request.json")
        }
        "codex_sse_expected_inspection.json" => {
            include_str!("../fixtures/compat_replay/codex_sse_expected_inspection.json")
        }
        "codex_sse_expected_payloads.json" => {
            include_str!("../fixtures/compat_replay/codex_sse_expected_payloads.json")
        }
        "codex_sse_expected_surface.json" => {
            include_str!("../fixtures/compat_replay/codex_sse_expected_surface.json")
        }
        "codex_sse_request.json" => {
            include_str!("../fixtures/compat_replay/codex_sse_request.json")
        }
        "codex_sse_stream.txt" => include_str!("../fixtures/compat_replay/codex_sse_stream.txt"),
        "codex_websocket_expected_metadata.json" => {
            include_str!("../fixtures/compat_replay/codex_websocket_expected_metadata.json")
        }
        "codex_websocket_expected_surface.json" => {
            include_str!("../fixtures/compat_replay/codex_websocket_expected_surface.json")
        }
        "codex_websocket_replay.json" => {
            include_str!("../fixtures/compat_replay/codex_websocket_replay.json")
        }
        "sample_capture_replay.json" => {
            include_str!("../fixtures/compat_replay/sample_capture_replay.json")
        }
        other => panic!("unknown compat replay fixture {other}"),
    }
}

fn compat_replay_fixture_json(name: &str) -> Value {
    serde_json::from_str(compat_replay_fixture_text(name))
        .unwrap_or_else(|err| panic!("compat replay fixture {name} should be valid JSON: {err}"))
}

fn compat_replay_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key == "authorization"
        || key == "authorization_token"
        || key == "api_key"
        || key == "x-api-key"
        || key == "cookie"
        || key.contains("token")
        || key.contains("secret")
        || key.contains("password")
        || key.contains("credential")
}

fn compat_replay_header_placeholder(header_name: &str) -> Option<&'static str> {
    let header_name = header_name.to_ascii_lowercase();
    if compat_replay_sensitive_key(&header_name) {
        return Some("<redacted>");
    }
    match header_name.as_str() {
        "chatgpt-account-id" => Some("<account_id>"),
        "session_id" | "x-session-id" | "x-claude-code-session-id" => Some("<session_id>"),
        "x-codex-turn-state" => Some("<turn_state>"),
        _ => None,
    }
}

fn compat_replay_scrub_string_array(value: &mut Value, placeholder: &str) {
    if let Value::Array(items) = value {
        for item in items {
            if !item.is_null() {
                *item = Value::String(placeholder.to_string());
            }
        }
    }
}

fn compat_replay_value_scrub(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for key in [
                "id",
                "request_id",
                "session_id",
                "response_id",
                "previous_response_id",
                "trace_id",
                "created_at",
                "updated_at",
                "started_at",
                "timestamp",
                "ts",
                "pid",
                "account_id",
                "call_id",
                "approval_request_id",
            ] {
                if let Some(entry) = map.get_mut(key)
                    && !entry.is_null()
                {
                    *entry = Value::String(format!("<{key}>"));
                }
            }
            if let Some(entry) = map.get_mut("response_ids") {
                compat_replay_scrub_string_array(entry, "<response_id>");
            }
            if let Some(entry) = map.get_mut("allowed_tools") {
                compat_replay_scrub_string_array(entry, "<tool_name>");
            }
            for (key, entry) in map.iter_mut() {
                if compat_replay_sensitive_key(key) && !entry.is_null() {
                    *entry = Value::String("<redacted>".to_string());
                }
            }
            if let Some(placeholder) = map
                .get("name")
                .and_then(Value::as_str)
                .and_then(compat_replay_header_placeholder)
                && let Some(entry) = map.get_mut("value")
                && !entry.is_null()
            {
                *entry = Value::String(placeholder.to_string());
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

fn compat_replay_normalized_value(mut value: Value) -> Value {
    compat_replay_value_scrub(&mut value);
    value
}

fn compat_replay_assert_golden(actual: Value, expected_fixture: &str) {
    assert_eq!(
        compat_replay_normalized_value(actual),
        compat_replay_normalized_value(compat_replay_fixture_json(expected_fixture)),
        "compat replay fixture {expected_fixture} drifted"
    );
}

fn compat_replay_request_from_fixture(value: &Value) -> RuntimeProxyRequest {
    let headers = value
        .get("headers")
        .and_then(Value::as_array)
        .expect("request fixture should include headers array")
        .iter()
        .map(|entry| {
            let entry = entry
                .as_object()
                .expect("header fixture entry should be an object");
            let name = entry
                .get("name")
                .and_then(Value::as_str)
                .expect("header fixture entry should include name");
            let value = entry
                .get("value")
                .and_then(Value::as_str)
                .expect("header fixture entry should include value");
            (name.to_string(), value.to_string())
        })
        .collect();
    let body = value.get("body").cloned().unwrap_or(Value::Null);

    RuntimeProxyRequest {
        method: value
            .get("method")
            .and_then(Value::as_str)
            .expect("request fixture should include method")
            .to_string(),
        path_and_query: value
            .get("path_and_query")
            .and_then(Value::as_str)
            .expect("request fixture should include path_and_query")
            .to_string(),
        headers,
        body: if body.is_null() {
            Vec::new()
        } else {
            serde_json::to_vec(&body).expect("request fixture body should serialize")
        },
    }
}

fn compat_replay_surface_value(surface: RuntimeRequestCompatibilitySurface) -> Value {
    serde_json::json!({
        "stage": surface.stage,
        "family": surface.family,
        "client": surface.client,
        "route": surface.route,
        "transport": surface.transport,
        "stream": surface.stream,
        "tool_surface": surface.tool_surface,
        "continuation": surface.continuation,
        "request_origin": surface.request_origin,
        "approval": surface.approval,
        "user_agent": surface.user_agent,
        "warnings": surface.warnings,
    })
}

fn compat_replay_sse_payload_values(text: &str) -> Value {
    let mut data_lines = Vec::new();
    let mut payloads = Vec::new();
    let mut flush = |data_lines: &mut Vec<String>| {
        if let Some(value) = parse_runtime_sse_payload(data_lines) {
            payloads.push(value);
        }
        data_lines.clear();
    };

    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            flush(&mut data_lines);
            continue;
        }
        if let Some(data) = line.strip_prefix("data:") {
            data_lines.push(data.strip_prefix(' ').unwrap_or(data).to_string());
        }
    }
    flush(&mut data_lines);

    Value::Array(payloads)
}

fn compat_replay_sse_inspection_value(progress: RuntimeSseInspectionProgress) -> Value {
    match progress {
        RuntimeSseInspectionProgress::Hold {
            response_ids,
            turn_state,
        } => serde_json::json!({
            "kind": "hold",
            "response_ids": response_ids,
            "turn_state": turn_state,
        }),
        RuntimeSseInspectionProgress::Commit {
            response_ids,
            turn_state,
        } => serde_json::json!({
            "kind": "commit",
            "response_ids": response_ids,
            "turn_state": turn_state,
        }),
        RuntimeSseInspectionProgress::QuotaBlocked => serde_json::json!({
            "kind": "quota_blocked",
        }),
        RuntimeSseInspectionProgress::PreviousResponseNotFound => serde_json::json!({
            "kind": "previous_response_not_found",
        }),
    }
}

fn compat_replay_translated_request_value(request: &RuntimeProxyRequest) -> Value {
    serde_json::json!({
        "method": request.method.as_str(),
        "path_and_query": request.path_and_query.as_str(),
        "headers": request
            .headers
            .iter()
            .map(|(name, value)| serde_json::json!({
                "name": name.as_str(),
                "value": value.as_str(),
            }))
            .collect::<Vec<_>>(),
        "body": serde_json::from_slice::<Value>(&request.body)
            .expect("translated request body should be valid JSON"),
    })
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
fn compat_replay_codex_sse_fixture_matches_golden_surface_and_stream_parse() {
    let request =
        compat_replay_request_from_fixture(&compat_replay_fixture_json("codex_sse_request.json"));
    let surface = runtime_detect_request_compatibility_surface(&request, "request", "http");
    compat_replay_assert_golden(
        compat_replay_surface_value(surface),
        "codex_sse_expected_surface.json",
    );

    let stream = compat_replay_fixture_text("codex_sse_stream.txt");
    compat_replay_assert_golden(
        compat_replay_sse_payload_values(stream),
        "codex_sse_expected_payloads.json",
    );
    let inspection =
        inspect_runtime_sse_buffer(stream.as_bytes()).expect("Codex SSE fixture should inspect");
    compat_replay_assert_golden(
        compat_replay_sse_inspection_value(inspection),
        "codex_sse_expected_inspection.json",
    );
}

#[test]
fn compat_replay_codex_websocket_fixture_matches_golden_surface_and_metadata() {
    let replay = compat_replay_fixture_json("codex_websocket_replay.json");
    let handshake = compat_replay_request_from_fixture(
        replay
            .get("handshake")
            .expect("websocket replay fixture should include handshake"),
    );
    let message = replay
        .get("message")
        .expect("websocket replay fixture should include message")
        .to_string();

    let surface =
        runtime_detect_websocket_message_compatibility_surface(&handshake, message.as_str());
    compat_replay_assert_golden(
        compat_replay_surface_value(surface),
        "codex_websocket_expected_surface.json",
    );

    let metadata = parse_runtime_websocket_request_metadata(message.as_str());
    compat_replay_assert_golden(
        serde_json::json!({
            "previous_response_id": metadata.previous_response_id,
            "session_id": metadata.session_id,
            "requires_previous_response_affinity": metadata.requires_previous_response_affinity,
            "fresh_fallback_shape": runtime_previous_response_fresh_fallback_shape_label(
                metadata.previous_response_fresh_fallback_shape,
            ),
        }),
        "codex_websocket_expected_metadata.json",
    );
}

#[test]
fn compat_replay_claude_mcp_fixture_matches_golden_surface_and_translation() {
    let request =
        compat_replay_request_from_fixture(&compat_replay_fixture_json("claude_mcp_request.json"));
    let surface = runtime_detect_request_compatibility_surface(&request, "request", "http");
    compat_replay_assert_golden(
        compat_replay_surface_value(surface),
        "claude_mcp_expected_surface.json",
    );

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("fixture should translate");
    assert!(translated.stream);
    assert!(translated.server_tools.mcp);
    compat_replay_assert_golden(
        compat_replay_translated_request_value(&translated.translated_request),
        "claude_mcp_expected_translation.json",
    );
}

#[test]
fn compat_replay_capture_sample_fixture_is_scrubbed() {
    let text = compat_replay_fixture_text("sample_capture_replay.json");
    serde_json::from_str::<Value>(text).expect("sample capture replay fixture should be JSON");

    for needle in [
        "live_authorization_secret",
        "sk-live-secret",
        "body_api_key_secret",
        "sse_secret_token",
        "resp_parent_live",
        "sess_body_live",
        "turn_live_20260428",
        "call_ws_live",
        "2026-04-28T01:02:03Z",
    ] {
        assert!(
            !text.contains(needle),
            "sample replay fixture should not contain raw capture value {needle}"
        );
    }
    for placeholder in [
        "<redacted>",
        "<session_id>",
        "<previous_response_id>",
        "<response_id>",
        "<turn_state>",
    ] {
        assert!(
            text.contains(placeholder),
            "sample replay fixture should contain placeholder {placeholder}"
        );
    }
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
