use super::*;

#[test]
fn upstream_url_preserves_backend_api_mount() {
    assert_eq!(
        runtime_proxy_upstream_url(
            "https://chatgpt.com/backend-api",
            "/backend-api/prodex/responses?x=1",
        ),
        "https://chatgpt.com/backend-api/codex/responses?x=1"
    );
}

#[test]
fn upstream_url_only_strips_backend_api_for_exact_mount_suffix() {
    assert_eq!(
        runtime_proxy_upstream_url(
            "https://example.test/backend-api-v2",
            "/backend-api/prodex/responses?x=1",
        ),
        "https://example.test/backend-api-v2/backend-api/codex/responses?x=1"
    );
    assert_eq!(
        runtime_proxy_upstream_url(
            "https://chatgpt.com/backend-api",
            "/backend-api-v2/prodex/responses?x=1",
        ),
        "https://chatgpt.com/backend-api/backend-api-v2/prodex/responses?x=1"
    );
}

#[test]
fn upstream_url_joins_plain_base_url() {
    assert_eq!(
        runtime_proxy_upstream_url("https://example.test/", "responses"),
        "https://example.test/responses"
    );
    assert_eq!(
        runtime_proxy_upstream_url("https://example.test", "/responses"),
        "https://example.test/responses"
    );
}

#[test]
fn upstream_url_neutralizes_dot_segments_before_url_parsing_can_escape_mount() {
    assert_eq!(
        runtime_proxy_upstream_url(
            "https://chatgpt.com/backend-api",
            "/backend-api/prodex/../wham/usage?x=1",
        ),
        "https://chatgpt.com/backend-api/codex/%252e%252e/wham/usage?x=1"
    );
    assert_eq!(
        runtime_proxy_upstream_url(
            "https://chatgpt.com/backend-api",
            "/backend-api/prodex/%2e%2e/wham/usage",
        ),
        "https://chatgpt.com/backend-api/codex/%252e%252e/wham/usage"
    );
}

#[test]
fn request_header_skip_list_preserves_codex_metadata_headers() {
    for header in [
        "session_id",
        "x-openai-subagent",
        "x-openai-memgen-request",
        "x-codex-installation-id",
        "x-codex-turn-state",
        "x-codex-turn-metadata",
        "x-codex-parent-thread-id",
        "x-codex-window-id",
        "x-client-request-id",
        "x-codex-beta-features",
        "x-oai-attestation",
        "x-responsesapi-include-timing-metrics",
        "x-openai-internal-codex-responses-lite",
        "x-codex-ws-stream-request-start-ms",
        "OpenAI-Beta",
        "User-Agent",
    ] {
        assert!(
            !should_skip_runtime_request_header(header),
            "runtime proxy should preserve upstream Codex metadata header {header}"
        );
    }
}

#[test]
fn request_header_skip_list_preserves_codex_rust_0_131_and_newer_passthrough_headers() {
    for header in [
        "session-id",
        "thread-id",
        "x-codex-parent-thread-id",
        "x-codex-window-id",
        "x-codex-inference-call-id",
        "X-OpenAI-Product-Sku",
        "x-codex-ws-stream-request-start-ms",
        "ws_request_header_x_openai_internal_codex_responses_lite",
    ] {
        assert!(
            !should_skip_runtime_request_header(header),
            "runtime proxy should preserve Codex passthrough header {header}"
        );
    }
}

#[test]
fn request_header_skip_list_replaces_auth_and_transport_headers() {
    for header in [
        "Authorization",
        "ChatGPT-Account-Id",
        "Connection",
        "Content-Length",
        "Host",
        "Keep-Alive",
        "Proxy-Authenticate",
        "Proxy-Authorization",
        "TE",
        "Trailer",
        "Transfer-Encoding",
        "Upgrade",
        "sec-websocket-key",
        "x-prodex-internal-request-origin",
    ] {
        assert!(
            should_skip_runtime_request_header(header),
            "runtime proxy should not forward local/auth header {header}"
        );
    }
}

#[test]
fn request_header_forwarding_strips_connection_named_headers() {
    let headers = runtime_forward_request_headers([
        ("Connection", "keep-alive, X-Local-Hop"),
        ("X-Local-Hop", "strip-me"),
        ("x-codex-turn-state", "keep-me"),
        ("User-Agent", "codex-cli-test"),
    ]);

    assert_eq!(
        headers,
        vec![
            ("x-codex-turn-state", "keep-me"),
            ("User-Agent", "codex-cli-test"),
        ]
    );
}

#[test]
fn effective_user_agent_ignores_empty_values() {
    assert_eq!(
        runtime_proxy_effective_user_agent(&[
            ("User-Agent".to_string(), String::new()),
            ("x-test".to_string(), "value".to_string()),
        ]),
        None
    );
    assert_eq!(
        runtime_proxy_effective_user_agent(&[(
            "user-agent".to_string(),
            "codex-cli-test".to_string(),
        )]),
        Some("codex-cli-test")
    );
}
