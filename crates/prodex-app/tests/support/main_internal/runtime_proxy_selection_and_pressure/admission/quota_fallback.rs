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
            runtime_usage_snapshot(
                quota_window_ready(80, 3600),
                quota_window_ready(80, 86_400),
            ),
        )
        .profile_usage_snapshot(
            "second",
            runtime_usage_snapshot(quota_window_ready(80, 3600), quota_window_exhausted(300)),
        )
        .build();

    assert!(!runtime_has_route_ready_quota_fallback(
        harness.shared(),
        "main",
        &BTreeSet::new(),
        RuntimeRouteKind::Responses,
    )
    .expect("fallback readiness should resolve"));
}
