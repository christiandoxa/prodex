use super::super::test_support::{
    test_runtime_local_websocket_pair, test_runtime_shared, test_runtime_websocket_flow,
};
use super::*;
use crate::{ProfileEntry, ProfileProvider, RuntimeRotationProxyShared};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[test]
fn direct_current_profile_fallback_requires_fresh_request_context() {
    let _guard = acquire_test_runtime_lock();
    let cases = [
        (Some("resp-1"), None, None, None, None, false, false, false),
        (None, Some("alpha"), None, None, None, false, false, false),
        (None, None, Some("ts-1"), None, None, false, false, false),
        (None, None, None, Some("alpha"), None, false, false, false),
        (None, None, None, None, Some("alpha"), false, false, false),
        (None, None, None, None, None, true, false, false),
        (None, None, None, None, None, false, true, false),
        (None, None, None, None, None, false, false, true),
    ];

    for (
        previous_response_id,
        pinned_profile,
        request_turn_state,
        turn_state_profile,
        session_profile,
        saw_inflight_saturation,
        saw_failure,
        expected,
    ) in cases
    {
        let shared = test_runtime_shared("loop-direct-current");
        let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
        let mut websocket_session = RuntimeWebsocketSessionState::default();
        let mut flow =
            test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);
        flow.previous_response_id = previous_response_id.map(str::to_string);
        flow.pinned_profile = pinned_profile.map(str::to_string);
        flow.request_turn_state = request_turn_state.map(str::to_string);
        flow.turn_state_profile = turn_state_profile.map(str::to_string);
        flow.session_profile = session_profile.map(str::to_string);
        flow.saw_inflight_saturation = saw_inflight_saturation;
        if saw_failure {
            flow.last_failure = Some((
                RuntimeUpstreamFailureResponse::Websocket(RuntimeWebsocketErrorPayload::Empty),
                false,
            ));
        }

        assert_eq!(flow.allows_direct_current_profile_fallback(), expected);
    }
}

#[test]
fn pending_websocket_reuse_retry_bypasses_expired_budget_once() {
    let _guard = acquire_test_runtime_lock();
    let shared = test_runtime_shared("loop-reuse-retry-budget");
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);
    flow.websocket_reuse_fresh_retry_pending = true;
    let expired = Instant::now() - std::time::Duration::from_secs(60);

    assert!(
        !flow
            .precommit_budget_exhausted(expired, usize::MAX, false)
            .expect("scheduled fresh retry should bypass the expired budget")
    );
    assert!(
        flow.precommit_budget_exhausted(expired, usize::MAX, false)
            .expect("budget should apply again after the one retry")
    );
}

#[test]
fn transport_failure_cannot_bypass_expired_budget() {
    let _guard = acquire_test_runtime_lock();
    let shared = test_runtime_shared("loop-transport-failure-budget");
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);
    flow.saw_transport_failure = true;
    let expired = Instant::now() - std::time::Duration::from_secs(60);

    assert!(
        flow.precommit_budget_exhausted(expired, 1, false)
            .expect("expired budget should remain authoritative after transport failure")
    );
}

#[test]
fn retryable_pool_extends_fresh_budget_but_not_continuation_budget() {
    let _guard = acquire_test_runtime_lock();
    let shared = capacity_ready_shared("loop-retryable-transport-budget");
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);
    flow.saw_transport_failure = true;
    let expired = Instant::now() - Duration::from_secs(60);

    assert!(
        !flow
            .precommit_budget_exhausted(expired, 1, false)
            .expect("fresh retryable pool should defer exhaustion")
    );
    flow.previous_response_id = Some("resp-1".to_string());
    assert!(
        flow.precommit_budget_exhausted(expired, 1, false)
            .expect("continuation affinity should keep the retry budget authoritative")
    );
}

#[test]
fn quota_recovery_keeps_busy_positive_quota_account_eligible_for_full_context_retry() {
    let _guard = acquire_test_runtime_lock();
    let shared = capacity_ready_shared("quota-busy-alternative");
    shared.lane_admission.set_profile_inflight("main", 2);
    assert!(
        crate::runtime_has_route_recoverable_quota_fallback_for_model(
            &shared,
            "exhausted",
            &BTreeSet::new(),
            RuntimeRouteKind::Websocket,
            None
        )
        .unwrap(),
        "temporary local load must not leak exhausted-owner quota to the client"
    );
    assert!(
        crate::runtime_profile_inflight_hard_limited_for_context(
            &shared,
            "main",
            "websocket_session"
        )
        .unwrap(),
        "eligibility must not bypass actual admission limits"
    );
    assert!(
        !crate::runtime_has_route_ready_quota_fallback_for_model(
            &shared,
            "exhausted",
            &BTreeSet::new(),
            RuntimeRouteKind::Websocket,
            None
        )
        .unwrap(),
        "ready-now and recoverable eligibility are different facts"
    );
}

#[test]
fn quota_recovery_keeps_temporary_transport_backoff_distinct_from_quota_exhaustion() {
    let _guard = acquire_test_runtime_lock();
    let shared = capacity_ready_shared("quota-backoff-alternative");
    let now = chrono::Local::now().timestamp();
    {
        let mut runtime = shared.runtime.lock().unwrap();
        runtime.profile_transport_backoff_until.insert(
            crate::runtime_profile_transport_backoff_key("main", RuntimeRouteKind::Websocket),
            now + 10,
        );
    }
    assert!(
        crate::runtime_has_route_recoverable_quota_fallback_for_model(
            &shared,
            "exhausted",
            &BTreeSet::new(),
            RuntimeRouteKind::Websocket,
            None
        )
        .unwrap(),
        "recoverable transport cooldown is not a terminal empty quota pool"
    );
    {
        let mut runtime = shared.runtime.lock().unwrap();
        runtime
            .profile_retry_backoff_until
            .insert("main".to_string(), now + 86400);
    }
    assert!(
        !crate::runtime_has_route_recoverable_quota_fallback_for_model(
            &shared,
            "exhausted",
            &BTreeSet::new(),
            RuntimeRouteKind::Websocket,
            None
        )
        .unwrap(),
        "confirmed account quota quarantine must not be bypassed"
    );
}

#[test]
fn quota_replay_soft_session_can_wait_for_other_accounts_transport_recovery() {
    let _guard = acquire_test_runtime_lock();
    let mut shared = capacity_ready_shared("quota-replay-transport-recovery");
    shared.async_runtime = std::sync::Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("running timer executor for transient recovery"),
    );
    let now = chrono::Local::now().timestamp();
    shared
        .runtime
        .lock()
        .unwrap()
        .profile_transport_backoff_until
        .insert(
            crate::runtime_profile_transport_backoff_key("main", RuntimeRouteKind::Websocket),
            now + 1,
        );
    let (mut socket, _peer) = test_runtime_local_websocket_pair();
    let mut session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut socket, &shared, &mut session);
    flow.session_profile = Some("exhausted-owner".to_string());
    flow.request_session_id = Some("existing-conversation".to_string());
    assert!(
        flow.has_continuation_priority(),
        "conversation keeps scheduling priority"
    );
    assert!(
        flow.wait_for_transient_recovery().unwrap(),
        "a soft session preference must not strand full-context replay behind a recoverable cooldown"
    );
    flow.previous_response_id = Some("resp-hard-owner".to_string());
    assert!(
        !flow.wait_for_transient_recovery().unwrap(),
        "opaque previous-response ownership still fails closed before replay"
    );
}

#[test]
fn quota_recovery_does_not_reuse_excluded_or_auth_incompatible_accounts() {
    let _guard = acquire_test_runtime_lock();
    let shared = capacity_ready_shared("quota-recovery-negative-controls");
    assert!(
        !crate::runtime_has_route_recoverable_quota_fallback_for_model(
            &shared,
            "exhausted",
            &BTreeSet::from(["main".to_string()]),
            RuntimeRouteKind::Websocket,
            None,
        )
        .unwrap()
    );
    {
        let mut runtime = shared.runtime.lock().unwrap();
        runtime.profile_usage_auth.clear();
        runtime
            .profile_probe_cache
            .get_mut("main")
            .unwrap()
            .auth
            .quota_compatible = false;
    }
    assert!(
        !crate::runtime_has_route_recoverable_quota_fallback_for_model(
            &shared,
            "exhausted",
            &BTreeSet::new(),
            RuntimeRouteKind::Websocket,
            None,
        )
        .unwrap()
    );
}

#[test]
fn quota_recovery_rejects_zero_quota_even_without_a_transport_backoff() {
    let _guard = acquire_test_runtime_lock();
    let shared = capacity_ready_shared("quota-recovery-empty-quota");
    {
        let mut runtime = shared.runtime.lock().unwrap();
        let probe = runtime.profile_probe_cache.get_mut("main").unwrap();
        let mut usage = serde_json::to_value(probe.result.as_ref().unwrap()).unwrap();
        usage["rate_limit"]["primary_window"]["used_percent"] = serde_json::json!(100);
        usage["rate_limit"]["secondary_window"]["used_percent"] = serde_json::json!(100);
        probe.result = Ok(serde_json::from_value(usage).unwrap());
    }
    assert!(
        !crate::runtime_has_route_recoverable_quota_fallback_for_model(
            &shared,
            "exhausted",
            &BTreeSet::new(),
            RuntimeRouteKind::Websocket,
            None,
        )
        .unwrap(),
        "zero-quota accounts must not manufacture an alternate destination"
    );
}

#[test]
fn cold_start_probe_wait_is_one_shot() {
    let _guard = acquire_test_runtime_lock();
    let shared = test_runtime_shared("loop-cold-start");
    shared
        .runtime
        .lock()
        .expect("runtime lock should succeed")
        .state
        .profiles
        .insert(
            "second".to_string(),
            ProfileEntry {
                codex_home: PathBuf::from("/home/test-user/codex-second"),
                managed: true,
                email: Some("second@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        );
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);

    assert!(matches!(
        flow.handle_candidate_exhausted(
            &mut Instant::now(),
            crate::runtime_profile_inflight_release_revision(&shared)
        )
        .expect("first cold-start probe wait should succeed"),
        RuntimeWebsocketMessageLoopAction::Continue
    ));
    assert!(flow.cold_start_probe_waited);
    assert!(matches!(
        flow.handle_candidate_exhausted(
            &mut Instant::now(),
            crate::runtime_profile_inflight_release_revision(&shared)
        )
        .expect("second candidate exhaustion should fail closed"),
        RuntimeWebsocketMessageLoopAction::Finished
    ));
}

fn capacity_ready_shared(name: &str) -> RuntimeRotationProxyShared {
    let mut shared = test_runtime_shared(name);
    let config = std::sync::Arc::make_mut(&mut shared.runtime_config);
    config.tuning.profile_inflight_soft_limit = 2;
    config.tuning.profile_inflight_hard_limit = 2;
    let now = chrono::Local::now().timestamp();
    let mut runtime = shared.runtime.lock().unwrap();
    let home = runtime.paths.root.join("main-home");
    runtime.state.profiles.insert(
        "main".to_string(),
        ProfileEntry {
            codex_home: home,
            managed: true,
            email: None,
            provider: ProfileProvider::Openai,
        },
    );
    runtime.profile_probe_cache.insert(
        "main".to_string(),
        prodex_shared_types::RuntimeProfileProbeCacheEntry {
            checked_at: now,
            auth: prodex_quota::AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(serde_json::from_value(serde_json::json!({
                "plan_type": "plus",
                "rate_limit": {
                    "primary_window": {
                        "used_percent": 1,
                        "reset_at": now + 18_000,
                        "limit_window_seconds": 18_000
                    },
                    "secondary_window": {
                        "used_percent": 20,
                        "reset_at": now + 604_800,
                        "limit_window_seconds": 604_800
                    }
                }
            }))
            .unwrap()),
        },
    );
    drop(runtime);
    shared
}

fn assert_websocket_capacity_wait_preserves_budget(atomic_admission_race: bool) {
    let _guard = acquire_test_runtime_lock();
    let shared = capacity_ready_shared(if atomic_admission_race {
        "capacity-atomic-budget"
    } else {
        "capacity-candidate-budget"
    });
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);
    flow.excluded_profiles.insert("already-failed".to_string());
    let occupied =
        crate::acquire_runtime_profile_inflight_guard(&shared, "main", "websocket_session")
            .expect("another session should occupy the profile");
    let release = std::thread::spawn(move || {
        // Longer than the real production pressure budget, not just the test budget.
        std::thread::sleep(Duration::from_millis(1_050));
        drop(occupied);
    });
    let mut selection_started_at = Instant::now() - Duration::from_millis(50);
    let blocked = if atomic_admission_race {
        flow.handle_inflight_saturation(
            &RuntimeWebsocketAttempt::LocalSelectionBlocked {
                profile_name: "main".to_string(),
                reason: "profile_inflight_saturated",
            },
            &mut selection_started_at,
        )
    } else {
        flow.candidate_inflight_saturated("main", &mut selection_started_at)
    }
    .expect("capacity wait should complete when another session releases its permit");
    release.join().unwrap();
    assert!(blocked);
    assert!(flow.saw_inflight_saturation);
    assert!(
        !flow
            .precommit_budget_exhausted(selection_started_at, 1, true)
            .unwrap(),
        "local queue time must not exhaust the upstream retry budget"
    );
    assert!(
        flow.precommit_budget_exhausted(selection_started_at, usize::MAX, true)
            .unwrap(),
        "capacity relief must not reset actual upstream attempt limits"
    );
    assert!(
        flow.excluded_profiles.contains("already-failed"),
        "capacity relief must not retry an already rejected profile"
    );
}

#[test]
fn websocket_candidate_capacity_wait_preserves_precommit_budget() {
    assert_websocket_capacity_wait_preserves_budget(false);
}

#[test]
fn websocket_atomic_capacity_wait_preserves_precommit_budget() {
    assert_websocket_capacity_wait_preserves_budget(true);
}

#[test]
fn websocket_capacity_wait_rechecks_readiness_after_missed_notification() {
    let _guard = acquire_test_runtime_lock();
    let shared = capacity_ready_shared("capacity-missed-notification");
    shared.lane_admission.set_profile_inflight("main", 2);
    let admission = shared.lane_admission.clone();
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        // Model a release crossing waiter registration: readiness changes but
        // no later notification is available to this waiter.
        admission.set_profile_inflight("main", 0);
    });
    let mut selection_started_at = Instant::now();
    let excluded_profiles = std::collections::BTreeSet::new();
    let result =
        runtime_proxy_maybe_wait_for_interactive_inflight_relief(RuntimeInflightReliefWait {
            observed_release_revision: None,
            request_id: 19,
            shared: &shared,
            excluded_profiles: &excluded_profiles,
            route_kind: RuntimeRouteKind::Websocket,
            selection_started_at: &mut selection_started_at,
            continuation: false,
            wait_affinity_owner: None,
            selected_profile: None,
        })
        .expect("capacity wait should recheck readiness");
    release.join().unwrap();
    assert_eq!(
        result,
        RuntimeInflightReliefWaitResult::Relieved,
        "a missed notification must cause reselection, not terminal pool exhaustion"
    );
}

#[test]
fn websocket_selection_pressure_cannot_expire_before_first_upstream_attempt() {
    let _guard = acquire_test_runtime_lock();
    let shared = capacity_ready_shared("capacity-unsent-budget");
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut session);
    let expired = Instant::now() - Duration::from_secs(60);
    for pressure in [false, true] {
        assert!(
            !flow
                .precommit_budget_exhausted(expired, 0, pressure)
                .unwrap()
        );
        assert!(
            flow.precommit_budget_exhausted(expired, 1, pressure)
                .unwrap()
        );
    }
}

#[test]
fn websocket_capacity_release_between_selection_and_wait_reselects() {
    let _guard = acquire_test_runtime_lock();
    let shared = capacity_ready_shared("capacity-release-before-wait");
    let occupied =
        crate::acquire_runtime_profile_inflight_guard(&shared, "main", "websocket_session")
            .unwrap();
    let observed_revision = crate::runtime_profile_inflight_release_revision(&shared);
    drop(occupied);
    let mut selection_started_at = Instant::now();
    let excluded_profiles = std::collections::BTreeSet::new();
    let result =
        runtime_proxy_maybe_wait_for_interactive_inflight_relief(RuntimeInflightReliefWait {
            observed_release_revision: Some(observed_revision),
            request_id: 23,
            shared: &shared,
            excluded_profiles: &excluded_profiles,
            route_kind: RuntimeRouteKind::Websocket,
            selection_started_at: &mut selection_started_at,
            continuation: false,
            wait_affinity_owner: None,
            selected_profile: None,
        })
        .unwrap();
    assert_eq!(
        result,
        RuntimeInflightReliefWaitResult::Relieved,
        "a profile released after selection is not an exhausted pool"
    );
}
