use super::*;

#[test]
fn responses_parent_loop_mojo_plan_drives_candidate_exhaustion_with_nonzero_upstream_count() {
    let backend = RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
        RuntimeProxyBackendFaultStep::explicit_quota_429(
            RuntimeProxyBackendFaultRoute::Responses,
            "main-account",
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
        .profile_usage_snapshot(
            "second",
            runtime_usage_snapshot(
                quota_window_exhausted(300),
                quota_window_ready(80, 86_400),
            ),
        )
        .build();

    let now = Local::now().timestamp();
    let mut runtime = harness.shared().runtime.lock().expect("runtime lock");
    runtime.profile_probe_cache.insert(
        "main".to_string(),
        RuntimeProfileProbeCacheEntry {
            checked_at: now,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(80, 3_600, 80, 86_400)),
        },
    );
    runtime.profile_probe_cache.insert(
        "second".to_string(),
        RuntimeProfileProbeCacheEntry {
            checked_at: now,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(0, 300, 80, 86_400)),
        },
    );
    drop(runtime);

    let response = proxy_runtime_responses_request(7401, &responses_request(br#"{"input":[]}"#), harness.shared())
        .expect("candidate exhaustion should produce a bounded response");
    let (status, _body, _profile) = consume_responses_reply(response);
    let accounts = backend.responses_accounts();

    assert!(matches!(status, 429 | 503));
    assert_eq!(accounts, ["main-account"]);
    assert!(!accounts.is_empty(), "consumer must make a real upstream attempt");
}
