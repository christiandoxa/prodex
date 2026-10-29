use super::*;

#[test]
#[ignore = "requires the isolated TLS fixture in scripts/compat/guardian-tls-smoke.mjs"]
fn runtime_proxy_guardian_tls_parent_pin_end_to_end() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start(),
        "second",
        &["second"],
        &[],
        Vec::new(),
    )
    .restart_with_upstream(crate::DEFAULT_CHATGPT_BASE_URL.to_owned());
    for index in 1..=3 {
        let headers = if index == 3 {
            vec![runtime_continuation_header("x-codex-guardian", "reviewer")]
        } else {
            Vec::new()
        };
        let mut socket =
            fixture.connect_websocket_with_headers("backend-api/prodex/responses", &headers);
        set_test_websocket_io_timeout(&mut socket, ci_timing_upper_bound_ms(5000, 15000));
        let body = if index == 3 {
            serde_json::json!({"input": [], "client_metadata": {"parent_response_id": "resp-parent-tls"}})
        } else {
            serde_json::json!({"input": []})
        };
        send_runtime_websocket_json(&mut socket, body);
        let (_, completed) = read_runtime_websocket_until(&mut socket, |text| {
            text.contains("\"type\":\"response.completed\"")
        });
        if index == 1 {
            assert!(completed.contains("resp-parent-tls"));
        }
        let _ = socket.close(None);
    }
}

#[test]
fn runtime_proxy_guardian_websocket_quota_does_not_rotate_parent() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket_usage_limit_all(),
        "main",
        &["main", "second"],
        &[("resp-parent", "second")],
        Vec::new(),
    );
    let headers = [runtime_continuation_header("x-codex-guardian", "reviewer")];
    let mut socket =
        fixture.connect_websocket_with_headers("backend-api/prodex/responses", &headers);
    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "input": [], "client_metadata": {"parent_response_id": "resp-parent"}
        }),
    );
    let (_, response) = read_runtime_websocket_until(&mut socket, |text| {
        text.contains("usage_limit_reached") || text.contains("insufficient_quota")
    });
    let _ = socket.close(None);
    assert!(response.contains("usage_limit_reached") || response.contains("insufficient_quota"));
    assert_eq!(fixture.backend.responses_accounts(), ["second-account"]);
}

#[test]
fn runtime_proxy_guardian_reviewer_prefers_parent_response_profile() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket(),
        "main",
        &["main", "second"],
        &[("resp-parent", "second")],
        Vec::new(),
    );
    let headers = [runtime_continuation_header("x-codex-guardian", "reviewer")];
    let mut socket =
        fixture.connect_websocket_with_headers("backend-api/prodex/responses", &headers);
    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "input": [{"type": "message", "role": "user", "content": "review"}],
            "client_metadata": {"parent_response_id": "resp-parent"}
        }),
    );
    let _ = read_runtime_websocket_until(&mut socket, |text| {
        text.contains("\"type\":\"response.completed\"")
    });
    let _ = socket.close(None);
    let observed = fixture.backend.responses_headers();
    assert_eq!(
        observed
            .first()
            .and_then(|headers| headers.get("chatgpt-account-id"))
            .map(String::as_str),
        Some("second-account"),
        "Guardian should prefer the parent response's profile"
    );
}

#[test]
fn runtime_proxy_non_guardian_parent_metadata_does_not_change_selection() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket(),
        "main",
        &["main", "second"],
        &[("resp-parent", "second")],
        Vec::new(),
    );
    let mut socket = fixture.connect_websocket("backend-api/prodex/responses");
    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "input": [{"type": "message", "role": "user", "content": "hello"}],
            "client_metadata": {"parent_response_id": "resp-parent"}
        }),
    );
    let _ = read_runtime_websocket_until(&mut socket, |text| {
        text.contains("\"type\":\"response.completed\"")
    });
    let _ = socket.close(None);
    let observed = fixture.backend.responses_headers();
    assert_eq!(
        observed
            .first()
            .and_then(|headers| headers.get("chatgpt-account-id"))
            .map(String::as_str),
        Some("main-account")
    );
}

#[test]
fn runtime_proxy_guardian_previous_response_keeps_its_owner() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket(),
        "main",
        &["main", "second"],
        &[("resp-parent", "main"), ("resp-second", "second")],
        Vec::new(),
    );
    let headers = [runtime_continuation_header("x-codex-guardian", "reviewer")];
    let mut socket =
        fixture.connect_websocket_with_headers("backend-api/prodex/responses", &headers);
    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "previous_response_id": "resp-second",
            "input": [{"type": "message", "role": "user", "content": "continue"}],
            "client_metadata": {"parent_response_id": "resp-parent"}
        }),
    );
    let _ = read_runtime_websocket_until(&mut socket, |text| {
        text.contains("\"type\":\"response.completed\"")
    });
    let _ = socket.close(None);
    let observed = fixture.backend.responses_headers();
    assert_eq!(
        observed
            .first()
            .and_then(|headers| headers.get("chatgpt-account-id"))
            .map(String::as_str),
        Some("second-account")
    );
}
