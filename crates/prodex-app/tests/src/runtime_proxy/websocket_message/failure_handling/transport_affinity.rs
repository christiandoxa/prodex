use super::*;

fn mixed_quota_pool(shared: &mut RuntimeRotationProxyShared) {
    configure_quota_last_chance_profiles(shared, 0, false);
    let mut runtime = shared.runtime.lock().unwrap();
    runtime.profile_route_circuit_open_until.clear();
    let mut gamma = runtime.state.profiles.get("alpha").unwrap().clone();
    gamma.codex_home = runtime.paths.root.join("gamma-home");
    runtime.state.profiles.insert("gamma".to_string(), gamma);
    // Only beta is Ready: alpha is 5h-blocked and gamma is weekly-blocked.
    for (name, weekly) in [("alpha", false), ("gamma", true)] {
        let mut entry = runtime.profile_probe_cache.get("beta").unwrap().clone();
        let pair = entry.result.as_mut().unwrap().rate_limit.as_mut().unwrap();
        if weekly {
            pair.secondary_window.as_mut().unwrap().used_percent = Some(100);
        } else {
            pair.primary_window.as_mut().unwrap().used_percent = Some(100);
        }
        runtime.profile_probe_cache.insert(name.to_string(), entry);
    }
}

fn turn_state_full_history(flow: &mut RuntimeWebsocketTextMessageFlow<'_>) {
    flow.handshake_request.path_and_query = "/backend-api/prodex/responses".to_string();
    flow.handshake_request
        .headers
        .push(("x-codex-turn-state".to_string(), "turn-owner".to_string()));
    flow.request_text = r#"{"type":"response.create","client_metadata":{"x-codex-turn-state":"turn-owner"},"input":[{"role":"user","content":"Earlier question"},{"role":"assistant","content":"Earlier answer"},{"role":"user","content":"Summarize this conversation"}]}"#.to_string();
    flow.request_turn_state = Some("turn-owner".to_string());
    flow.turn_state_profile = Some("alpha".to_string());
    flow.pinned_profile = Some("alpha".to_string());
    assert!(
        runtime_proxy_crate::runtime_request_text_has_reconstructable_full_history(
            &flow.request_text
        )
    );
}

#[test]
fn websocket_transport_full_history_moves_to_ready_profile_despite_other_quota_blocks() {
    let _guard = acquire_test_runtime_lock();
    let mut shared = test_runtime_shared("transport-full-history-mixed-quota");
    mixed_quota_pool(&mut shared);
    let (mut local, _client) = test_runtime_local_websocket_pair();
    let mut session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local, &shared, &mut session);
    turn_state_full_history(&mut flow);
    let action = flow
        .handle_candidate_attempt(
            RuntimeWebsocketAttempt::TransportFailed {
                profile_name: "alpha".to_string(),
                stage: "precommit_hold_timeout",
            },
            None,
        )
        .unwrap();
    assert!(
        matches!(action, RuntimeWebsocketMessageLoopAction::Continue),
        "a complete request has a Ready alternative and must not fail at the broken owner"
    );
    assert!(flow.request_turn_state.is_none());
    assert!(!flow.request_text.contains("turn-owner"));
    assert!(flow.excluded_profiles.contains("alpha"));
    assert_eq!(flow.select_candidate().unwrap().as_deref(), Some("beta"));
    assert!(flow.saw_transport_failure);
    assert!(!flow.saw_rate_limit_failure);
}

#[test]
fn websocket_transport_delta_never_crosses_affinity_even_with_ready_profiles() {
    let _guard = acquire_test_runtime_lock();
    let mut shared = test_runtime_shared("transport-delta-preserve-owner");
    mixed_quota_pool(&mut shared);
    let (mut local, mut client) = test_runtime_local_websocket_pair();
    let mut session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local, &shared, &mut session);
    turn_state_full_history(&mut flow);
    flow.request_text = r#"{"type":"response.create","input":[{"type":"function_call_output","call_id":"call-owner","output":"delta only"}]}"#.to_string();
    assert!(
        !runtime_proxy_crate::runtime_request_text_has_reconstructable_full_history(
            &flow.request_text
        )
    );
    let action = flow
        .handle_candidate_attempt(
            RuntimeWebsocketAttempt::TransportFailed {
                profile_name: "alpha".to_string(),
                stage: "upstream_close",
            },
            None,
        )
        .unwrap();
    assert!(matches!(
        action,
        RuntimeWebsocketMessageLoopAction::Finished
    ));
    assert_eq!(flow.pinned_profile.as_deref(), Some("alpha"));
    assert_eq!(flow.request_turn_state.as_deref(), Some("turn-owner"));
    assert!(read_runtime_websocket_text(&mut client).contains("service_unavailable"));
}

#[test]
fn websocket_transport_response_owner_requests_full_context_when_only_one_profile_ready() {
    let _guard = acquire_test_runtime_lock();
    let mut shared = test_runtime_shared("transport-response-owner-mixed-quota");
    mixed_quota_pool(&mut shared);
    let (mut local, mut client) = test_runtime_local_websocket_pair();
    let mut session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local, &shared, &mut session);
    configure_hard_affinity_full_context_replay(&mut flow);
    let action = flow
        .handle_candidate_attempt(
            RuntimeWebsocketAttempt::TransportFailed {
                profile_name: "alpha".to_string(),
                stage: "upstream_close",
            },
            None,
        )
        .unwrap();
    assert_full_context_retry_signal(action, &mut client, &flow);
}
