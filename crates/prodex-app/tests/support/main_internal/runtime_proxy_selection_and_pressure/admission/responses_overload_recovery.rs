use super::helpers::*;
use super::*;
use std::io::Read;

#[test]
fn fresh_responses_keep_recovering_after_multiple_provider_overload_sweeps() {
    let backend = RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
        RuntimeProxyBackendFaultStep::sse_overloaded(
            RuntimeProxyBackendFaultRoute::Responses,
            "main-account",
        ),
        RuntimeProxyBackendFaultStep::sse_overloaded(
            RuntimeProxyBackendFaultRoute::Responses,
            "second-account",
        ),
        RuntimeProxyBackendFaultStep::sse_overloaded(
            RuntimeProxyBackendFaultRoute::Responses,
            "main-account",
        ),
        RuntimeProxyBackendFaultStep::sse_overloaded(
            RuntimeProxyBackendFaultRoute::Responses,
            "second-account",
        ),
        RuntimeProxyBackendFaultStep::sse_success(
            RuntimeProxyBackendFaultRoute::Responses,
            "main-account",
            "recovered-after-outage",
        ),
    ]));
    let ready = runtime_usage_snapshot(
        quota_window_ready(80, 3_600),
        quota_window_ready(80, 86_400),
    );
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready.clone())
        .profile_usage_snapshot("second", ready)
        .build();
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: br#"{"input":[]}"#.to_vec(),
    };

    let response = proxy_runtime_responses_request(903, &request, harness.shared())
        .expect("provider-wide overload should remain recoverable");
    let RuntimeResponsesReply::Streaming(mut response) = response else {
        panic!("recovered response should remain streaming");
    };
    let mut body = String::new();
    response
        .body
        .read_to_string(&mut body)
        .expect("recovered response should be readable");

    assert_eq!(response.status, 200, "{body}");
    assert!(!body.contains("server_is_overloaded"), "{body}");
    assert_eq!(
        backend.responses_accounts(),
        [
            "main-account",
            "second-account",
            "main-account",
            "second-account",
            "main-account"
        ]
    );
}
