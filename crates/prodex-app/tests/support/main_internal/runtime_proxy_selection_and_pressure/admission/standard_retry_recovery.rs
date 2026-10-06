use super::helpers::*;
use super::*;

#[test]
fn scripted_noncompact_pool_overload_recovers_after_every_profile_temporarily_fails() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::overloaded_503(
                RuntimeProxyBackendFaultRoute::Status,
                "main-account",
            ),
            RuntimeProxyBackendFaultStep::overloaded_503(
                RuntimeProxyBackendFaultRoute::Status,
                "second-account",
            ),
            RuntimeProxyBackendFaultStep::success(
                RuntimeProxyBackendFaultRoute::Status,
                "main-account",
            ),
        ]));
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .build();
    let request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/status".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let response = proxy_runtime_standard_request(151, &request, harness.shared())
        .expect("pool-wide temporary 503 should retry until a quota-usable profile recovers");
    let (status, body) = tiny_http_response_status_and_body(response);

    assert_eq!(
        status, 200,
        "temporary upstream 503 must not terminate the workflow while a usable account remains: {body}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec![
            "main-account".to_string(),
            "second-account".to_string(),
            "main-account".to_string(),
        ],
        "standard route must retry the pool after transient overload backoff"
    );
    let log = read_runtime_proxy_test_log(&harness.shared().log_path);
    assert!(
        log.contains("rotation_waiting_for_recovery") || log.contains("rotation_sweep_start"),
        "pool-wide standard 503 should enter recovery instead of returning 503: {log}"
    );
}
