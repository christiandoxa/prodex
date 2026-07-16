use super::super::test_support::{
    test_runtime_local_websocket_pair, test_runtime_shared, test_runtime_websocket_flow,
};
use super::*;

fn ready_usage() -> prodex_quota::UsageResponse {
    let now = chrono::Local::now().timestamp();
    prodex_quota::UsageResponse {
        email: None,
        plan_type: Some("plus".to_string()),
        rate_limit: Some(prodex_quota::WindowPair {
            primary_window: Some(prodex_quota::UsageWindow {
                used_percent: Some(0),
                reset_at: Some(now + 18_000),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: Some(prodex_quota::UsageWindow {
                used_percent: Some(50),
                reset_at: Some(now + 604_800),
                limit_window_seconds: Some(604_800),
            }),
        }),
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    }
}

fn configure_quota_last_chance_profiles(
    shared: &mut RuntimeRotationProxyShared,
    beta_inflight: usize,
    transport_backoff: bool,
) {
    let config = std::sync::Arc::make_mut(&mut shared.runtime_config);
    config.tuning.profile_inflight_soft_limit = 4;
    config.tuning.profile_inflight_hard_limit = 8;
    let now = chrono::Local::now().timestamp();
    shared
        .lane_admission
        .set_profile_inflight("beta", beta_inflight);
    let mut runtime = shared
        .runtime
        .lock()
        .expect("runtime state should not be poisoned");
    let root = runtime.paths.root.clone();
    for name in ["alpha", "beta"] {
        runtime.state.profiles.insert(
            name.to_string(),
            prodex_state::ProfileEntry {
                codex_home: root.join(format!("{name}-home")),
                managed: true,
                email: None,
                provider: prodex_state::ProfileProvider::Openai,
            },
        );
    }
    runtime.profile_probe_cache.insert(
        "beta".to_string(),
        prodex_shared_types::RuntimeProfileProbeCacheEntry {
            checked_at: now,
            auth: prodex_quota::AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(ready_usage()),
        },
    );
    runtime.profile_route_circuit_open_until.insert(
        crate::runtime_proxy::runtime_profile_route_circuit_key(
            "beta",
            RuntimeRouteKind::Websocket,
        ),
        now + 60,
    );
    if transport_backoff {
        runtime.profile_transport_backoff_until.insert(
            crate::runtime_proxy::runtime_profile_transport_backoff_key(
                "beta",
                RuntimeRouteKind::Websocket,
            ),
            now + 60,
        );
    }
}

#[test]
fn quota_blocked_uses_one_last_chance_ready_profile_past_soft_load_and_circuit() {
    let _guard = acquire_test_runtime_lock();
    let mut shared = test_runtime_shared("failure-quota-last-chance");
    configure_quota_last_chance_profiles(&mut shared, 6, false);
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);
    assert_eq!(
        crate::runtime_quota_last_chance_profile_for_route(
            &shared,
            &BTreeSet::from(["alpha".to_string()]),
            RuntimeRouteKind::Websocket,
            None,
        )
        .unwrap(),
        Some("beta".to_string())
    );

    let action = flow
        .handle_candidate_quota_blocked(
            "alpha".to_string(),
            RuntimeWebsocketErrorPayload::Text(runtime_proxy_websocket_error_payload_text(
                429,
                "usage_limit_reached",
                "The usage limit has been reached.",
            )),
        )
        .expect("quota-ready circuit fallback should remain retryable");

    assert!(matches!(
        action,
        RuntimeWebsocketMessageLoopAction::Continue
    ));
    assert_eq!(flow.select_candidate().unwrap(), Some("beta".to_string()));
    assert!(flow.quota_last_chance_profile.is_none());
}

#[test]
fn quota_last_chance_bypasses_active_transport_backoff_once() {
    let _guard = acquire_test_runtime_lock();
    let mut shared = test_runtime_shared("failure-quota-last-chance-transport-backoff");
    configure_quota_last_chance_profiles(&mut shared, 6, true);
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);

    let action = flow
        .handle_candidate_quota_blocked(
            "alpha".to_string(),
            RuntimeWebsocketErrorPayload::Text(runtime_proxy_websocket_error_payload_text(
                429,
                "usage_limit_reached",
                "The usage limit has been reached.",
            )),
        )
        .expect("quota-ready transport fallback should remain retryable");

    assert!(matches!(
        action,
        RuntimeWebsocketMessageLoopAction::Continue
    ));
    assert_eq!(flow.select_candidate().unwrap(), Some("beta".to_string()));
    assert!(flow.quota_last_chance_profile.is_none());
}

#[test]
fn quota_last_chance_respects_hard_inflight_cap() {
    let _guard = acquire_test_runtime_lock();
    let mut shared = test_runtime_shared("failure-quota-last-chance-hard-cap");
    configure_quota_last_chance_profiles(&mut shared, 7, false);

    assert_eq!(
        crate::runtime_quota_last_chance_profile_for_route(
            &shared,
            &BTreeSet::from(["alpha".to_string()]),
            RuntimeRouteKind::Websocket,
            None,
        )
        .unwrap(),
        None
    );
}

#[test]
fn candidate_usage_not_included_with_ready_fallback_rotates() {
    let _guard = acquire_test_runtime_lock();
    let shared = test_runtime_shared("failure-usage-not-included");
    {
        let mut runtime = shared
            .runtime
            .lock()
            .expect("runtime state should not be poisoned");
        let root = runtime.paths.root.clone();
        for name in ["alpha", "beta"] {
            runtime.state.profiles.insert(
                name.to_string(),
                prodex_state::ProfileEntry {
                    codex_home: root.join(format!("{name}-home")),
                    managed: true,
                    email: None,
                    provider: prodex_state::ProfileProvider::Openai,
                },
            );
        }
        runtime.profile_probe_cache.insert(
            "beta".to_string(),
            prodex_shared_types::RuntimeProfileProbeCacheEntry {
                checked_at: chrono::Local::now().timestamp(),
                auth: prodex_quota::AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                },
                result: Err("cached-ready-profile".to_string()),
            },
        );
    }
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);

    let action = flow
        .handle_candidate_attempt(
            RuntimeWebsocketAttempt::QuotaBlocked {
                profile_name: "alpha".to_string(),
                payload: RuntimeWebsocketErrorPayload::Text(
                    runtime_proxy_websocket_error_payload_text(
                        429,
                        "usage_not_included",
                        "Your workspace is out of credits.",
                    ),
                ),
            },
            None,
        )
        .expect("usage_not_included should rotate when another profile is ready");

    assert!(matches!(
        action,
        RuntimeWebsocketMessageLoopAction::Continue
    ));
    assert!(flow.excluded_profiles.contains("alpha"));
    assert!(matches!(
        flow.last_failure,
        Some((RuntimeUpstreamFailureResponse::Websocket(_), true))
    ));
}

#[test]
fn workspace_credit_continuation_uses_fresh_retry_when_ready_profile_exists() {
    let _guard = acquire_test_runtime_lock();
    let shared = test_runtime_shared("failure-workspace-credit-fresh-retry");
    {
        let mut runtime = shared
            .runtime
            .lock()
            .expect("runtime state should not be poisoned");
        let root = runtime.paths.root.clone();
        for name in ["alpha", "beta"] {
            runtime.state.profiles.insert(
                name.to_string(),
                prodex_state::ProfileEntry {
                    codex_home: root.join(format!("{name}-home")),
                    managed: true,
                    email: None,
                    provider: prodex_state::ProfileProvider::Openai,
                },
            );
        }
        runtime.profile_probe_cache.insert(
            "beta".to_string(),
            prodex_shared_types::RuntimeProfileProbeCacheEntry {
                checked_at: chrono::Local::now().timestamp(),
                auth: prodex_quota::AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                },
                result: Err("cached-ready-profile".to_string()),
            },
        );
    }
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);
    flow.request_text = r#"{"type":"response.create","previous_response_id":"resp_alpha","input":[{"type":"message","content":"again"}]}"#.to_string();
    flow.previous_response_id = Some("resp_alpha".to_string());
    flow.previous_response_fresh_fallback_shape =
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation);
    flow.pinned_profile = Some("alpha".to_string());
    flow.bound_profile = Some("alpha".to_string());
    flow.trusted_previous_response_affinity = true;

    let action = flow
        .handle_candidate_quota_blocked(
            "alpha".to_string(),
            RuntimeWebsocketErrorPayload::Text(
                r#"{"type":"error","status_code":429,"headers":{"X-Codex-Rate-Limit-Reached-Type":"workspace_member_credits_depleted"},"error":{"type":"usage_limit_reached","message":"The usage limit has been reached"}}"#.to_string(),
            ),
        )
        .expect("workspace-credit continuation should become a fresh retry");

    assert!(matches!(
        action,
        RuntimeWebsocketMessageLoopAction::Continue
    ));
    assert!(flow.excluded_profiles.contains("alpha"));
    assert!(flow.previous_response_id.is_none());
    assert!(flow.previous_response_fresh_fallback_shape.is_none());
    assert!(flow.pinned_profile.is_none());
    assert!(flow.bound_profile.is_none());
    assert!(!flow.trusted_previous_response_affinity);
    assert!(!flow.request_text.contains("previous_response_id"));
    assert!(matches!(
        &flow.last_failure,
        Some((RuntimeUpstreamFailureResponse::Websocket(_), true))
    ));
}

#[test]
fn candidate_attempt_reuse_watchdog_tripped_excludes_profile_and_continues() {
    let _guard = acquire_test_runtime_lock();
    let shared = test_runtime_shared("failure-reuse-watchdog");
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);

    let action = flow
        .handle_candidate_attempt(
            RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                profile_name: "alpha".to_string(),
                event: "timeout",
            },
            None,
        )
        .expect("reuse watchdog handling should succeed");

    assert!(matches!(
        action,
        RuntimeWebsocketMessageLoopAction::Continue
    ));
    assert!(flow.excluded_profiles.contains("alpha"));
}

#[test]
fn candidate_attempt_transport_failed_excludes_profile_and_continues() {
    let _guard = acquire_test_runtime_lock();
    let shared = test_runtime_shared("failure-transport");
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);

    let action = flow
        .handle_candidate_attempt(
            RuntimeWebsocketAttempt::TransportFailed {
                profile_name: "alpha".to_string(),
                stage: "connect",
            },
            None,
        )
        .expect("transport failure handling should succeed");

    assert!(matches!(
        action,
        RuntimeWebsocketMessageLoopAction::Continue
    ));
    assert!(flow.excluded_profiles.contains("alpha"));
    assert!(matches!(
        flow.last_failure,
        Some((RuntimeUpstreamFailureResponse::Websocket(_), true))
    ));
}

#[test]
fn candidate_overloaded_releasable_affinity_rotates() {
    let _guard = acquire_test_runtime_lock();
    let shared = test_runtime_shared("failure-overloaded");
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);

    let action = flow
        .handle_candidate_overloaded(
            "alpha".to_string(),
            RuntimeWebsocketErrorPayload::Text(runtime_proxy_websocket_error_payload_text(
                503,
                "server_is_overloaded",
                "Upstream Codex backend is currently overloaded.",
            )),
        )
        .expect("overload handling should succeed");

    assert!(matches!(
        action,
        RuntimeWebsocketMessageLoopAction::Continue
    ));
    assert!(flow.excluded_profiles.contains("alpha"));
    assert!(matches!(
        flow.last_failure,
        Some((RuntimeUpstreamFailureResponse::Websocket(_), false))
    ));
}

#[test]
fn candidate_local_selection_blocked_releasable_affinity_rotates() {
    let _guard = acquire_test_runtime_lock();
    let shared = test_runtime_shared("failure-local-selection");
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);

    let action = flow
        .handle_candidate_local_selection_blocked(
            "alpha".to_string(),
            "quota_windows_unavailable_after_reprobe",
        )
        .expect("local selection block handling should succeed");

    assert!(matches!(
        action,
        RuntimeWebsocketMessageLoopAction::Continue
    ));
    assert!(flow.excluded_profiles.contains("alpha"));
}
