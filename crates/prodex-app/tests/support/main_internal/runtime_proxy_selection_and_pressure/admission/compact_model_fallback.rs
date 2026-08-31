use super::helpers::*;
use super::*;

#[test]
fn luna_compact_falls_back_to_actual_spark_model_after_luna_capacity_exhausts() {
    let backend = RuntimeProxyBackend::start_http_luna_quota_then_spark();
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "spark-profile",
        "second-account",
        "spark@example.com",
    )
    .upstream_base_url(backend.base_url())
    .profile_usage_snapshot(
        "spark-profile",
        runtime_usage_snapshot(
            quota_window_exhausted(3_600),
            quota_window_exhausted(86_400),
        ),
    )
    .build();
    let now = Local::now().timestamp();
    harness
        .shared()
        .runtime
        .lock()
        .expect("runtime lock")
        .profile_probe_cache
        .insert(
            "spark-profile".to_string(),
            RuntimeProfileProbeCacheEntry {
                checked_at: now,
                auth: AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                },
                result: Ok(exhausted_luna_with_ready_spark_usage()),
            },
        );
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: br#"{"model":"gpt-5.6-luna","input":[],"instructions":"compact"}"#.to_vec(),
    };

    let response = proxy_runtime_standard_request(902, &request, harness.shared())
        .expect("Spark compact fallback should complete the request");
    let (status, body) = tiny_http_response_status_and_body(response);

    assert_eq!(status, 200, "{body}");
    assert_eq!(
        backend.responses_accounts(),
        ["second-account", "second-account"]
    );
    let request_bodies = backend.responses_bodies();
    assert_eq!(request_bodies.len(), 2);
    let request: serde_json::Value = serde_json::from_str(
        request_bodies
            .last()
            .expect("Spark compact fallback should reach upstream"),
    )
    .expect("fallback body should be JSON");
    assert_eq!(request["model"], "gpt-5.3-codex-spark");
}
