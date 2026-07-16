use super::*;

#[test]
fn runtime_profile_inflight_hard_limit_detects_saturation() {
    let temp_dir = TestDir::isolated();
    let hard_limit = runtime_proxy_profile_inflight_hard_limit();
    let shared = RuntimeProxyFixtureBuilder::new()
        .profile_inflight("main", hard_limit)
        .build_shared(&temp_dir);

    assert!(
        runtime_profile_inflight_hard_limited_for_context(&shared, "main", "standard_http")
            .expect("hard inflight lookup should succeed")
    );
    assert!(
        !runtime_profile_inflight_hard_limited_for_context(&shared, "other", "standard_http")
            .expect("hard inflight lookup should succeed")
    );
}

#[test]
fn runtime_profile_inflight_weight_prioritizes_long_lived_routes() {
    assert_eq!(runtime_profile_inflight_weight("standard_http"), 1);
    assert_eq!(runtime_profile_inflight_weight("compact_http"), 1);
    assert_eq!(runtime_profile_inflight_weight("responses_http"), 2);
    assert_eq!(runtime_profile_inflight_weight("websocket_session"), 2);
}

#[test]
fn acquire_runtime_profile_inflight_guard_uses_weighted_units() {
    let temp_dir = TestDir::isolated();
    let shared = RuntimeProxyFixtureBuilder::new().build_shared(&temp_dir);

    let standard = acquire_runtime_profile_inflight_guard(&shared, "main", "standard_http")
        .expect("standard inflight guard should succeed");
    let responses = acquire_runtime_profile_inflight_guard(&shared, "main", "responses_http")
        .expect("responses inflight guard should succeed");

    assert_eq!(
        shared.lane_admission.profile_inflight_count("main"),
        3,
        "weighted inflight should count long-lived routes heavier than unary routes"
    );

    drop(responses);
    drop(standard);

    assert!(
        shared.lane_admission.profile_inflight_count("main") == 0,
        "weighted inflight release should fully drain the profile count"
    );
}

#[test]
fn runtime_profile_inflight_hard_limit_uses_weighted_admission_cost() {
    let temp_dir = TestDir::isolated();
    let hard_limit = runtime_proxy_profile_inflight_hard_limit();
    let shared = RuntimeProxyFixtureBuilder::new()
        .profile_inflight("main", hard_limit.saturating_sub(1))
        .build_shared(&temp_dir);

    assert!(
        runtime_profile_inflight_hard_limited_for_context(&shared, "main", "responses_http")
            .expect("weighted hard inflight lookup should succeed")
    );
    assert!(
        !runtime_profile_inflight_hard_limited_for_context(&shared, "main", "standard_http")
            .expect("weighted hard inflight lookup should succeed")
    );
}

#[test]
fn runtime_profile_inflight_limits_use_configured_overrides() {
    let _soft_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_SOFT_LIMIT", "9");
    let _hard_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_HARD_LIMIT", "10");

    assert_eq!(
        runtime_profile_inflight_soft_limit(RuntimeRouteKind::Responses, false),
        9
    );
    assert_eq!(
        runtime_profile_inflight_soft_limit(RuntimeRouteKind::Responses, true),
        8
    );
    assert_eq!(
        runtime_profile_inflight_soft_limit(RuntimeRouteKind::Compact, true),
        7
    );

    let temp_dir = TestDir::isolated();
    let shared = RuntimeProxyFixtureBuilder::new()
        .profile_inflight("main", 9)
        .build_shared(&temp_dir);

    assert!(
        runtime_profile_inflight_hard_limited_for_context(&shared, "main", "responses_http")
            .expect("responses inflight lookup should succeed")
    );
    assert!(
        !runtime_profile_inflight_hard_limited_for_context(&shared, "main", "standard_http")
            .expect("standard inflight lookup should succeed")
    );
}
