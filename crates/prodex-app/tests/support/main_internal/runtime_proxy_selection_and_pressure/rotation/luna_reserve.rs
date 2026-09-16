use super::*;

#[test]
fn runtime_luna_reserve_request_uses_hidden_upstream_model_without_reset_claim() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start();
    let codex_home = temp_dir.path.join("homes/main");
    write_auth_json(&codex_home.join("auth.json"), "main-account");
    let reserve_usage: UsageResponse = serde_json::from_value(serde_json::json!({
        "ordinaryUsageAllowed": false,
        "rateLimitResetCredits": { "availableCount": 2 },
        "rateLimitsByLimitId": {
            "codex": {
                "limitId": "codex",
                "primary": { "usedPercent": 100, "windowDurationMins": 300 },
                "secondary": { "usedPercent": 100, "windowDurationMins": 10080 }
            },
            "base_model_inference": {
                "limitId": "base_model_inference",
                "limitName": "gpt-reserve",
                "normalModelSlug": "gpt-5.6-luna",
                "primary": { "usedPercent": 0, "windowDurationMins": 300 },
                "secondary": { "usedPercent": 0, "windowDurationMins": 10080 }
            }
        }
    }))
    .expect("reserve usage should parse");
    assert_eq!(
        prodex_quota::openai_effective_model_for_usage(
            &reserve_usage,
            Some("gpt-5.6-luna"),
            Some("main-account"),
            true,
        ),
        Some("gpt-reserve")
    );

    let now = Local::now().timestamp();
    let shared = runtime_rotation_proxy_shared_with_auto_redeem(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
                active_profile: Some("main".to_string()),
                profiles: BTreeMap::from([(
                    "main".to_string(),
                    ProfileEntry {
                        codex_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: backend.base_url(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::from([(
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(reserve_usage),
                },
            )]),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_backoff_updated_at: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
        true,
    );
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: br#"{"model":"gpt-5.6-luna","input":"hello"}"#.to_vec(),
    };
    let response = proxy_runtime_responses_request(41, &request, &shared)
        .expect("Reserve-backed Luna request should succeed");
    match response {
        RuntimeResponsesReply::Buffered(parts) => assert_eq!(parts.status, 200),
        RuntimeResponsesReply::Streaming(response) => assert_eq!(response.status, 200),
    }

    let upstream_bodies = backend.responses_bodies();
    let upstream_body: serde_json::Value = serde_json::from_str(
        upstream_bodies
            .last()
            .expect("backend should capture the upstream request body"),
    )
    .expect("captured upstream body should parse");
    assert_eq!(upstream_body["model"], "gpt-reserve");
    assert!(
        backend.reset_credit_consume_accounts().is_empty(),
        "Luna Reserve routing must not claim a reset credit"
    );
}
