use super::*;

#[path = "local_bridge.rs"]
mod local_bridge;

#[test]
fn normalizes_prodex_openai_mount_paths() {
    assert_eq!(
        runtime_proxy_normalize_openai_path("/backend-api/prodex/responses?x=1").as_ref(),
        "/backend-api/codex/responses?x=1"
    );
    assert_eq!(
        runtime_proxy_normalize_openai_path("/backend-api/prodex/v1/responses").as_ref(),
        "/backend-api/codex/responses"
    );
    assert_eq!(
        runtime_proxy_normalize_openai_path("/backend-api/codex/responses").as_ref(),
        "/backend-api/codex/responses"
    );
}

#[test]
fn classifies_runtime_proxy_lanes_without_transport_side_effects() {
    assert_eq!(
        runtime_proxy_request_lane("/backend-api/codex/responses", false),
        RuntimeRouteKind::Responses
    );
    assert_eq!(
        runtime_proxy_request_lane("/backend-api/codex/responses/compact", false),
        RuntimeRouteKind::Compact
    );
    assert_eq!(
        runtime_proxy_request_lane("/backend-api/codex/realtime", true),
        RuntimeRouteKind::Websocket
    );
    assert_eq!(
        runtime_proxy_request_lane("/dashboard", false),
        RuntimeRouteKind::Standard
    );
}

#[test]
fn extracts_affinity_request_markers_from_json() {
    let body = br#"{
            "previous_response_id": " resp_123 ",
            "prompt_cache_key": " cache-a "
        }"#;
    let value = serde_json::from_slice::<serde_json::Value>(body).unwrap();
    assert_eq!(
        runtime_request_previous_response_id_from_bytes(body).as_deref(),
        Some("resp_123")
    );
    assert_eq!(
        runtime_request_prompt_cache_key_from_value(&value).as_deref(),
        Some("cache-a")
    );
}

#[test]
fn parses_websocket_request_metadata_without_runtime_state() {
    let metadata = parse_runtime_websocket_request_metadata(
        r#"{
                "previous_response_id": "resp_123",
                "session_id": "sess_456",
                "prompt_cache_key": "cache-a",
                "input": [{"type":"message","content":"continue"}]
            }"#,
    );

    assert_eq!(metadata.previous_response_id.as_deref(), Some("resp_123"));
    assert_eq!(metadata.session_id.as_deref(), Some("sess_456"));
    assert_eq!(metadata.prompt_cache_key.as_deref(), Some("cache-a"));
    assert!(!metadata.requires_previous_response_affinity);
    assert_eq!(
        metadata.previous_response_fresh_fallback_shape,
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation)
    );
}

#[test]
fn locks_previous_response_affinity_for_tool_outputs() {
    let value = serde_json::json!({
        "previous_response_id": "resp_123",
        "input": [{"type": "function_call_output", "call_id": "call_1"}]
    });

    assert!(runtime_request_value_requires_previous_response_affinity(
        &value
    ));
    assert_eq!(
        runtime_request_value_previous_response_fresh_fallback_shape(&value),
        Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly)
    );
}

#[test]
fn explicit_session_header_wins_over_body_session() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("session_id".to_string(), " header-session ".to_string())],
        body: br#"{"session_id":"body-session"}"#.to_vec(),
    };

    assert_eq!(
        runtime_request_explicit_session_id(&request).as_deref(),
        Some("header-session")
    );
    assert_eq!(
        runtime_request_session_id(&request).as_deref(),
        Some("header-session")
    );
}

#[test]
fn codex_session_id_header_wins_over_body_session() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("session-id".to_string(), " codex-session ".to_string())],
        body: br#"{"session_id":"body-session"}"#.to_vec(),
    };

    assert_eq!(
        runtime_request_explicit_session_id(&request).as_deref(),
        Some("codex-session")
    );
    assert_eq!(
        runtime_request_session_id(&request).as_deref(),
        Some("codex-session")
    );
}

#[test]
fn explicit_session_headers_keep_legacy_precedence() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![
            ("x-session-id".to_string(), "x-session".to_string()),
            ("session-id".to_string(), "codex-session".to_string()),
            ("session_id".to_string(), "legacy-session".to_string()),
        ],
        body: Vec::new(),
    };

    assert_eq!(
        runtime_request_explicit_session_id(&request).as_deref(),
        Some("legacy-session")
    );
}

#[test]
fn detects_internal_interactive_origin_case_insensitively() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages".to_string(),
        headers: vec![(
            "x-prodex-internal-request-origin".to_string(),
            " Anthropic_Messages ".to_string(),
        )],
        body: Vec::new(),
    };

    assert!(runtime_proxy_request_prefers_interactive_inflight_wait(
        &request
    ));
    assert!(runtime_proxy_request_prefers_inflight_wait(&request));
}

#[test]
fn structured_log_round_trips_quoted_values() {
    let message = runtime_proxy_structured_log_message(
        "event\nname",
        [
            runtime_proxy_log_field("request", "7"),
            runtime_proxy_log_field("bad key", "skip"),
            runtime_proxy_log_field("profile", "alpha beta"),
            runtime_proxy_log_field("error", "line\rbreak"),
        ],
    );

    assert_eq!(
        message,
        "event name request=7 profile=\"alpha beta\" error=\"line break\""
    );

    let fields = runtime_proxy_log_fields(&message);
    assert_eq!(fields.get("request").map(String::as_str), Some("7"));
    assert_eq!(
        fields.get("profile").map(String::as_str),
        Some("alpha beta")
    );
    assert_eq!(fields.get("error").map(String::as_str), Some("line break"));
    assert!(!fields.contains_key("bad key"));
    assert_eq!(runtime_proxy_log_event(&message), Some("event"));
}

#[test]
fn typed_log_event_preserves_order_and_quoted_fields() {
    let message = runtime_proxy_structured_log_message(
        "stream_read_error",
        [
            runtime_proxy_log_field("profile", "alpha beta"),
            runtime_proxy_log_field("empty", ""),
            runtime_proxy_log_field("error", r#"bad "quote" \ slash"#),
        ],
    );

    assert_eq!(
        message,
        r#"stream_read_error profile="alpha beta" empty="" error="bad \"quote\" \\ slash""#
    );

    let parsed = runtime_proxy_parse_log_event(&message).unwrap();
    assert_eq!(parsed.event(), "stream_read_error");
    assert_eq!(
        parsed
            .fields()
            .iter()
            .map(|field| (field.key(), field.value()))
            .collect::<Vec<_>>(),
        vec![
            ("profile", "alpha beta"),
            ("empty", ""),
            ("error", r#"bad "quote" \ slash"#),
        ]
    );
    assert_eq!(parsed.render_message(), message);
}
