use super::helpers::*;
use super::*;

#[test]
fn compact_precommit_blocks_retired_spark_with_hard_affinity_even_when_bypass_allowed() {
    let backend = RuntimeProxyBackend::start();

    for plan in ["pro", "prolite"] {
        for model in ["spark", "gpt-5.3-codex-spark", "gpt-5.3-spark"] {
            let mut snapshot = runtime_usage_snapshot(
                quota_window_ready(80, 3_600),
                quota_window_ready(80, 86_400),
            );
            snapshot.plan_type = Some(plan.to_string());
            let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
                "main",
                "main-account",
                "main@example.com",
            )
            .upstream_base_url(backend.base_url())
            .profile_usage_snapshot("main", snapshot)
            .build();
            let request = RuntimeProxyRequest {
                method: "POST".to_string(),
                path_and_query: "/backend-api/codex/responses/compact".to_string(),
                headers: Vec::new(),
                body: format!(
                    r#"{{"model":"{model}","input":[],"instructions":"compact"}}"#
                )
                .into_bytes(),
            };

            assert!(matches!(
                attempt_runtime_standard_request(
                    1,
                    &request,
                    harness.shared(),
                    "main",
                    true,
                    true,
                )
                .expect("compact attempt should succeed"),
                RuntimeStandardAttempt::LocalSelectionBlocked { .. }
            ), "retired {plan} {model} must be blocked before upstream");
        }
    }

    assert!(
        backend.responses_accounts().is_empty(),
        "retired Spark compact attempts must not reach upstream"
    );
}
#[test]
fn compact_precommit_preserves_valid_model_hard_affinity_quota_bypass() {
    let backend = RuntimeProxyBackend::start();
    let mut snapshot = runtime_usage_snapshot(
        quota_window_exhausted(300),
        quota_window_ready(90, 86_400),
    );
    snapshot.plan_type = Some("pro".to_string());
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main",
        "main-account",
        "main@example.com",
    )
    .upstream_base_url(backend.base_url())
    .profile_usage_snapshot("main", snapshot)
    .build();
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: Vec::new(),
        body: br#"{"model":"gpt-5.3-codex","input":[],"instructions":"compact"}"#.to_vec(),
    };

    match attempt_runtime_standard_request(1, &request, harness.shared(), "main", true, true)
        .expect("compact attempt should succeed")
    {
        RuntimeStandardAttempt::Success { profile_name, .. } => assert_eq!(profile_name, "main"),
        _ => panic!("valid-model hard-affinity compact should reach upstream"),
    }
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string()]
    );
}

#[test]
#[cfg(feature = "mojo-quota")]
fn standard_precommit_blocks_retired_model_before_auto_redeem() {
    let backend = RuntimeProxyBackend::start_http_usage_limit_auto_redeem();
    let mut harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main",
        "main-account",
        "main@example.com",
    )
    .upstream_base_url(backend.base_url())
    .build();
    let now = Local::now().timestamp();
    let mut usage: UsageResponse = serde_json::from_str(&runtime_proxy_usage_body_with_remaining(
        "main@example.com",
        0,
        0,
    ))
    .expect("exhausted usage should parse");
    usage.rate_limit_reset_credits = Some(prodex_quota::RateLimitResetCreditsSummary {
        available_count: 1,
    });
    let shared = harness.shared_mut();
    shared.auto_redeem_enabled = true;
    shared
        .runtime
        .lock()
        .expect("runtime lock should succeed")
        .profile_probe_cache
        .insert(
            "main".to_string(),
            RuntimeProfileProbeCacheEntry {
                checked_at: now,
                auth: AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                },
                result: Ok(usage),
            },
        );
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/chat/completions".to_string(),
        headers: Vec::new(),
        body: br#"{"model":"spark","messages":[]}"#.to_vec(),
    };

    assert!(matches!(
        attempt_runtime_noncompact_standard_request(1, &request, shared, "main", false)
            .expect("standard attempt should succeed"),
        RuntimeStandardAttempt::LocalSelectionBlocked { .. }
    ));
    assert!(
        backend.usage_accounts().is_empty(),
        "retired model must not trigger an auto-redeem usage probe"
    );
    assert!(
        backend.reset_credit_consume_accounts().is_empty(),
        "retired model must not consume an auto-redeem credit"
    );
}
