use super::*;

#[path = "profile_selection/commands.rs"]
mod commands;

#[test]
fn runtime_probe_refresh_wait_spans_multiple_short_slices_until_progress() {
    let probe_refresh = RuntimeProbeRefreshTestGuard::new();
    let observed_revision = probe_refresh.observed_revision();
    let notify = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(30));
        note_runtime_probe_refresh_progress();
    });

    assert!(
        wait_for_runtime_probe_refresh_progress(
            std::time::Duration::from_millis(200),
            std::time::Duration::from_millis(5),
            observed_revision,
        ),
        "probe refresh wait should survive several short timeout slices before progress"
    );
    notify.join().expect("probe refresh notifier should join");
}

fn selection_trace(
    shared: &RuntimeRotationProxyShared,
) -> runtime_proxy_crate::RuntimeRouteDecisionTrace {
    runtime_proxy_flush_logs_for_path(&shared.log_path).expect("runtime log should flush");
    let log = read_runtime_proxy_test_log(&shared.log_path);
    let trace_line = log
        .lines()
        .find(|line| line.contains(" route_decision "))
        .expect("selection should emit a route decision trace");
    let trace_json = runtime_proxy_crate::runtime_proxy_log_fields(trace_line)
        .remove("trace")
        .expect("route decision trace should contain typed JSON");
    serde_json::from_str(&trace_json).expect("route decision trace should be valid")
}

#[test]
fn selection_applies_route_scoped_affinity_conflict_policy() {
    for (route_kind, turn_state_profile, session_profile, previous_response_id, expected_selected, expected_outcome) in [
        (
            RuntimeRouteKind::Responses,
            Some("second"),
            None,
            None,
            None,
            runtime_proxy_crate::RuntimeRouteDecisionTerminalOutcome::AffinityExhausted,
        ),
        (
            RuntimeRouteKind::Compact,
            None,
            Some("second"),
            None,
            None,
            runtime_proxy_crate::RuntimeRouteDecisionTerminalOutcome::AffinityExhausted,
        ),
        (
            RuntimeRouteKind::Responses,
            None,
            Some("second"),
            Some("resp-pinned"),
            Some("main"),
            runtime_proxy_crate::RuntimeRouteDecisionTerminalOutcome::Selected,
        ),
    ] {
        let temp_dir = TestDir::isolated();
        let bindings = previous_response_id.map_or_else(BTreeMap::new, |id| {
            BTreeMap::from([(
                id.to_string(),
                ResponseProfileBinding {
                    binding_identity: None,
                    profile_name: "main".to_string(),
                    bound_at: Local::now().timestamp(),
                },
            )])
        });
        let shared = runtime_shared_for_affinity_selection(&temp_dir, bindings);
        let selected = select_runtime_response_candidate_for_route(
            &shared,
            RuntimeResponseCandidateSelection {
                pinned_profile: Some("main"),
                previous_response_id,
                turn_state_profile,
                session_profile,
                ..RuntimeResponseCandidateSelection::fresh(&BTreeSet::new(), route_kind)
            },
        )
        .expect("selection should succeed without an executable owner");

        assert_eq!(selected.as_deref(), expected_selected);
        assert_eq!(
            selection_trace(&shared).terminal_outcome,
            expected_outcome,
            "{route_kind:?} affinity selection had the wrong terminal outcome"
        );
    }
}
