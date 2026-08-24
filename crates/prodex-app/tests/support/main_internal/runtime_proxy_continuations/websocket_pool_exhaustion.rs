use super::*;

#[test]
fn runtime_proxy_websocket_quota_tries_every_profile_before_final_error() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let (_connect_timeout_guard, _progress_timeout_guard) =
        ci_runtime_proxy_websocket_timeout_guards();
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket_usage_limit_all(),
        "main",
        &["main", "second", "third"],
        &[],
        Vec::new(),
    );
    let mut socket = fixture.connect_websocket("backend-api/prodex/responses");
    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "input": [{"role": "user", "content": "drain every eligible account"}],
        }),
    );

    let (_, failure) = read_runtime_websocket_until(&mut socket, |text| {
        text.contains("insufficient_quota")
    });
    let _ = socket.close(None);

    assert!(failure.contains("insufficient_quota"), "{failure}");
    assert!(!failure.contains("service_unavailable"), "{failure}");
    let accounts = fixture.backend.responses_accounts();
    let mut sorted = accounts.clone();
    sorted.sort();
    assert_eq!(
        sorted,
        ["main-account", "second-account", "third-account"],
        "websocket must exhaust every eligible account before surfacing quota: {accounts:?}"
    );
}
