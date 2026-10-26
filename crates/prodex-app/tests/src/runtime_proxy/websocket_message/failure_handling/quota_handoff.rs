use super::*;

#[test]
fn websocket_known_exhausted_owner_recovers_before_opening_upstream_04392() {
    let _guard = acquire_test_runtime_lock();
    for busy in [false, true] {
        let mut shared = test_runtime_shared(if busy {
            "quota-handoff-busy"
        } else {
            "quota-handoff-ready"
        });
        configure_quota_last_chance_profiles(
            &mut shared,
            if busy { usize::MAX / 2 } else { 0 },
            false,
        );
        {
            let mut runtime = shared.runtime.lock().unwrap();
            runtime.profile_route_circuit_open_until.clear();
            let mut usage = ready_usage();
            usage
                .rate_limit
                .as_mut()
                .unwrap()
                .primary_window
                .as_mut()
                .unwrap()
                .used_percent = Some(100);
            runtime.profile_probe_cache.insert(
                "alpha".into(),
                prodex_shared_types::RuntimeProfileProbeCacheEntry {
                    checked_at: chrono::Local::now().timestamp(),
                    auth: prodex_quota::AuthSummary {
                        label: "chatgpt".into(),
                        quota_compatible: true,
                    },
                    result: Ok(usage),
                },
            );
        }
        let (mut local, mut client) = test_runtime_local_websocket_pair();
        let mut session = RuntimeWebsocketSessionState::default();
        let mut flow = test_runtime_websocket_flow(&mut local, &shared, &mut session);
        configure_hard_affinity_full_context_replay(&mut flow);
        // No credentials or upstream listener are provided: admission must
        // resolve this known quota failure before opening any upstream transport.
        let attempt = flow
            .attempt_profile("alpha", None)
            .expect("known exhausted continuation must use local recovery");
        assert!(
            matches!(&attempt, RuntimeWebsocketAttempt::LocalSelectionBlocked {
            profile_name, reason: "quota_exhausted_before_send",
        } if profile_name == "alpha")
        );
        let action = flow.handle_candidate_attempt(attempt, None).unwrap();
        assert_full_context_retry_signal(action, &mut client, &flow);
        assert!(flow.websocket_session.profile_name.is_none());
        shared.lane_admission.set_profile_inflight("beta", 0);
        shared.lane_admission.record_inflight_release();
    }
}
