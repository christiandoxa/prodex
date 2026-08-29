use super::helpers::*;
use super::*;

#[test]
fn weekly_exhausted_profile_is_not_a_ready_quota_fallback() {
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .profile_usage_snapshot(
            "main",
            runtime_usage_snapshot(quota_window_ready(80, 3600), quota_window_ready(80, 86_400)),
        )
        .profile_usage_snapshot(
            "second",
            runtime_usage_snapshot(quota_window_ready(80, 3600), quota_window_exhausted(300)),
        )
        .build();

    assert!(
        !runtime_has_route_ready_quota_fallback(
            harness.shared(),
            "main",
            &BTreeSet::new(),
            RuntimeRouteKind::Responses,
        )
        .expect("fallback readiness should resolve")
    );
}

#[test]
fn fresh_live_quota_does_not_reuse_stale_exhausted_snapshot_reset() {
    let live_usage = usage_with_main_windows(20, 3_600, 20, 86_400);
    let now = Local::now().timestamp();
    let mut stale_snapshot = runtime_profile_usage_snapshot_from_usage(&live_usage);
    stale_snapshot.five_hour_status = RuntimeQuotaWindowStatus::Exhausted;
    stale_snapshot.five_hour_remaining_percent = 0;
    stale_snapshot.five_hour_reset_at = now + 300;
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main",
        "main-account",
        "main@example.com",
    )
    .profile_usage_snapshot("main", stale_snapshot)
    .build();

    harness
        .shared()
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
                result: Ok(live_usage),
            },
        );

    let runtime = harness
        .shared()
        .runtime
        .lock()
        .expect("runtime lock should succeed");
    assert_eq!(
        runtime_profile_known_quota_reset_at(&runtime, "main", RuntimeRouteKind::Responses),
        None
    );
}
