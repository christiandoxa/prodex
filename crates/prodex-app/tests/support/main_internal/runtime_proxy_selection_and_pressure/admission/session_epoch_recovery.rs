use super::helpers::*;
use super::*;

fn snapshot(harness: &RuntimeProxyProfileHarness) -> RuntimeContinuationStore {
    let runtime = harness.shared().runtime.lock().unwrap();
    RuntimeContinuationStore {
        response_profile_bindings: runtime.state.response_profile_bindings.clone(),
        session_profile_bindings: runtime.state.session_profile_bindings.clone(),
        turn_state_bindings: runtime.turn_state_bindings.clone(),
        session_id_bindings: runtime.session_id_bindings.clone(),
        statuses: runtime.continuation_statuses.clone(),
    }
}

#[test]
fn explicit_session_recovery_then_same_second_rebind_survives_stale_persistence() {
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .build();
    remember_runtime_session_id(
        harness.shared(),
        "main",
        Some("recover-session"),
        RuntimeRouteKind::Responses,
    )
    .unwrap();
    remember_runtime_response_ids(
        harness.shared(),
        "main",
        &["immutable-response".into()],
        RuntimeRouteKind::Responses,
    )
    .unwrap();
    let stale = snapshot(&harness);
    assert!(
        release_runtime_session_affinity(harness.shared(), "recover-session", "goal_resume")
            .unwrap()
    );
    remember_runtime_session_id(
        harness.shared(),
        "second",
        Some("recover-session"),
        RuntimeRouteKind::Responses,
    )
    .unwrap();
    let live = snapshot(&harness);
    assert!(
        live.session_id_bindings["recover-session"].bound_at
            > live.statuses.session_release_epochs["recover-session"]
    );
    let encoded = serde_json::to_vec(&live).unwrap();
    let decoded = serde_json::from_slice(&encoded).unwrap();
    let profiles = harness
        .shared()
        .runtime
        .lock()
        .unwrap()
        .state
        .profiles
        .clone();
    for (left, right) in [(&stale, &decoded), (&decoded, &stale)] {
        let merged = merge_runtime_continuation_store(left, right, &profiles);
        assert_eq!(
            merged.session_id_bindings["recover-session"].profile_name,
            "second"
        );
        assert_eq!(
            merged.session_profile_bindings["recover-session"].profile_name,
            "second"
        );
        assert_eq!(
            merged.response_profile_bindings["immutable-response"].profile_name,
            "main"
        );
    }
}

#[test]
fn full_history_request_repairs_legacy_session_conflict_and_uses_ready_account() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([]));
    let ready = runtime_usage_snapshot(
        quota_window_ready(80, 3_600),
        quota_window_ready(80, 86_400),
    );
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("second")
        .current_profile("second")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready.clone())
        .profile_usage_snapshot("second", ready)
        .build();
    {
        let mut runtime = harness.shared().runtime.lock().unwrap();
        let binding = ResponseProfileBinding {
            profile_name: prodex_runtime_state::RUNTIME_HARD_BINDING_CONFLICT_PROFILE.into(),
            bound_at: Local::now().timestamp() - 60,
            binding_identity: None,
        };
        runtime
            .session_id_bindings
            .insert("legacy-conflict".into(), binding.clone());
        runtime
            .state
            .session_profile_bindings
            .insert("legacy-conflict".into(), binding);
    }
    let stale = snapshot(&harness);
    let request = RuntimeProxyRequest {
        method: "POST".into(), path_and_query: "/backend-api/codex/responses".into(),
        headers: vec![("Content-Type".into(), "application/json".into())],
        body: br#"{"model":"gpt-6-luna","session_id":"legacy-conflict","input":[{"role":"user","content":"persisted goal and context"},{"role":"assistant","content":"completed earlier work"},{"role":"user","content":"continue"}]}"#.to_vec(),
    };
    assert!(runtime_proxy_crate::runtime_request_has_reconstructable_full_history(&request));
    let reply = proxy_runtime_responses_request(911, &request, harness.shared()).unwrap();
    let (status, body) = match reply {
        RuntimeResponsesReply::Buffered(parts) => (
            parts.status,
            String::from_utf8_lossy(&parts.body).into_owned(),
        ),
        RuntimeResponsesReply::Streaming(mut response) => {
            let mut body = String::new();
            response.body.read_to_string(&mut body).unwrap();
            (response.status, body)
        }
    };
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_owned()]
    );
    let live = snapshot(&harness);
    let profiles = harness
        .shared()
        .runtime
        .lock()
        .unwrap()
        .state
        .profiles
        .clone();
    let merged = merge_runtime_continuation_store(&stale, &live, &profiles);
    assert_eq!(
        merged.session_id_bindings["legacy-conflict"].profile_name,
        "second"
    );
    assert_eq!(
        merged.session_profile_bindings["legacy-conflict"].profile_name,
        "second"
    );
    let log = read_runtime_proxy_test_log(&harness.shared().log_path);
    assert!(
        log.contains("session_conflict_recovered full_history=true"),
        "{log}"
    );
}

#[test]
fn newer_live_session_conflict_is_not_erased_by_an_older_release_epoch() {
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .build();
    let now = Local::now().timestamp();
    let mut stale_release = RuntimeContinuationStore::default();
    runtime_mark_continuation_status_dead(
        &mut stale_release.statuses,
        RuntimeContinuationBindingKind::SessionId,
        "concurrent",
        now - 30,
    );
    let live = RuntimeContinuationStore {
        session_id_bindings: BTreeMap::from([(
            "concurrent".into(),
            ResponseProfileBinding {
                profile_name: prodex_runtime_state::RUNTIME_HARD_BINDING_CONFLICT_PROFILE.into(),
                bound_at: now,
                binding_identity: None,
            },
        )]),
        ..Default::default()
    };
    let profiles = harness
        .shared()
        .runtime
        .lock()
        .unwrap()
        .state
        .profiles
        .clone();
    let merged = merge_runtime_continuation_store(&stale_release, &live, &profiles);
    assert_eq!(
        merged.session_id_bindings["concurrent"].profile_name,
        prodex_runtime_state::RUNTIME_HARD_BINDING_CONFLICT_PROFILE
    );
}

#[test]
fn self_contained_session_request_rotates_away_from_known_exhausted_quota() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([]));
    let ready = runtime_usage_snapshot(
        quota_window_ready(80, 3_600),
        quota_window_ready(80, 86_400),
    );
    let exhausted =
        runtime_usage_snapshot(quota_window_ready(0, 3_600), quota_window_ready(80, 86_400));
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", exhausted)
        .profile_usage_snapshot("second", ready)
        .build();
    remember_runtime_session_id(
        harness.shared(),
        "main",
        Some("exhausted-session"),
        RuntimeRouteKind::Responses,
    )
    .unwrap();
    let request = RuntimeProxyRequest { method: "POST".into(), path_and_query: "/backend-api/codex/responses".into(),
        headers: vec![("Content-Type".into(), "application/json".into())],
        body: br#"{"session_id":"exhausted-session","input":[{"role":"user","content":"original goal"},{"role":"assistant","content":"prior work"},{"role":"user","content":"continue"}]}"#.to_vec(),
    };
    let response = proxy_runtime_responses_request(912, &request, harness.shared()).unwrap();
    let (status, body) = match response {
        RuntimeResponsesReply::Buffered(parts) => (
            parts.status,
            String::from_utf8_lossy(&parts.body).into_owned(),
        ),
        RuntimeResponsesReply::Streaming(mut response) => {
            let mut body = String::new();
            response.body.read_to_string(&mut body).unwrap();
            (response.status, body)
        }
    };
    assert_eq!(status, 200, "{body}");
    let attempts = backend.responses_accounts();
    // A single authoritative last-chance probe is allowed; the request must
    // complete on an eligible account instead of repeating the failed owner.
    assert_eq!(attempts.last().map(String::as_str), Some("second-account"));
    assert!(attempts.len() <= 2, "unexpected retry storm: {attempts:?}");
}
