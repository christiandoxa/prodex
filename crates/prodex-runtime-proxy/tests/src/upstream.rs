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
        "x-responsesapi-include-timing-metrics",
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
