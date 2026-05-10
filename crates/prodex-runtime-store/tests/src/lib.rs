use super::*;
use prodex_runtime_state::{RuntimeProfileUsageSnapshot, RuntimeQuotaWindowStatus};
use prodex_state::{ProfileProvider, ResponseProfileBinding};
use std::path::PathBuf;

fn profile() -> ProfileEntry {
    ProfileEntry {
        codex_home: PathBuf::from("/tmp/profile"),
        managed: true,
        email: None,
        provider: ProfileProvider::Openai,
    }
}

#[test]
fn save_merge_plans_state_and_continuation_sidecar_together() {
    let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
    let existing_state = AppState {
        active_profile: Some("alpha".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let state_snapshot = AppState {
        active_profile: Some("alpha".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let lineage_key = format!("{RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX}4:resp:turn");
    let session_binding = ResponseProfileBinding {
        profile_name: "alpha".to_string(),
        bound_at: 20,
    };
    let continuation_snapshot = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([
            (
                "resp".to_string(),
                ResponseProfileBinding {
                    profile_name: "alpha".to_string(),
                    bound_at: 20,
                },
            ),
            (
                lineage_key.clone(),
                ResponseProfileBinding {
                    profile_name: "alpha".to_string(),
                    bound_at: 21,
                },
            ),
        ]),
        session_profile_bindings: BTreeMap::from([(
            "session".to_string(),
            session_binding.clone(),
        )]),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        statuses: RuntimeContinuationStatuses::default(),
    };

    let merged = merge_runtime_state_and_continuations_for_save(
        existing_state,
        &state_snapshot,
        &RuntimeContinuationStore::default(),
        &continuation_snapshot,
        100,
        AppStateCompactionPolicy::default(),
        RuntimeContinuationCompactionPolicy::default(),
    );

    assert_eq!(
        merged
            .state
            .response_profile_bindings
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["resp".to_string()]
    );
    assert!(
        merged
            .continuations
            .response_profile_bindings
            .contains_key(&lineage_key)
    );
    assert_eq!(
        merged.state.session_profile_bindings.get("session"),
        Some(&session_binding)
    );
}

#[test]
fn selected_snapshot_helper_keeps_only_requested_sections() {
    let paths = PathBuf::from("/tmp/state.json");
    let state = AppState {
        active_profile: Some("alpha".to_string()),
        profiles: BTreeMap::from([("alpha".to_string(), profile())]),
        last_run_selected_at: BTreeMap::from([("alpha".to_string(), 10)]),
        response_profile_bindings: BTreeMap::from([(
            "resp".to_string(),
            ResponseProfileBinding {
                profile_name: "alpha".to_string(),
                bound_at: 10,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };
    let continuations: RuntimeContinuationStore<ResponseProfileBinding> =
        RuntimeContinuationStore::default();
    let selected = runtime_state_save_selected_snapshot_from_parts(
        &paths,
        &state,
        &continuations,
        &BTreeMap::<String, RuntimeProfileHealth>::new(),
        &BTreeMap::<String, RuntimeProfileUsageSnapshot>::new(),
        &RuntimeProfileBackoffs::default(),
        RuntimeStateSaveSections {
            state: RuntimeStateSaveStateSection::Core,
            continuations: true,
            profile_scores: false,
            usage_snapshots: false,
            backoffs: false,
        },
    );

    let core_state = selected.state.expect("core state selected");
    assert!(core_state.response_profile_bindings.is_empty());
    assert_eq!(core_state.last_run_selected_at.get("alpha"), Some(&10));
    assert!(selected.profiles.is_none());
    assert!(selected.continuations.is_some());
    assert!(selected.profile_scores.is_none());
}

#[test]
fn selected_snapshot_merge_plan_loads_only_needed_sources() {
    let snapshot = RuntimeStateSaveSelectedSnapshot {
        paths: (),
        state: None::<AppState>,
        profiles: Some(BTreeMap::from([("alpha".to_string(), profile())])),
        continuations: Some(RuntimeContinuationStore::<ResponseProfileBinding>::default()),
        profile_scores: None::<BTreeMap<String, RuntimeProfileHealth>>,
        usage_snapshots: None::<BTreeMap<String, RuntimeProfileUsageSnapshot>>,
        backoffs: None::<RuntimeProfileBackoffs>,
    };

    assert_eq!(
        runtime_state_selected_snapshot_load_plan(&snapshot),
        RuntimeStateSelectedSnapshotLoadPlan {
            needs_existing_state: false,
            needs_existing_continuations: true,
        }
    );
    assert_eq!(
        runtime_state_selected_snapshot_prepare_plan(&snapshot),
        RuntimeStateSelectedSnapshotPreparePlan {
            load: RuntimeStateSelectedSnapshotLoadPlan {
                needs_existing_state: false,
                needs_existing_continuations: true,
            },
            writes_state: false,
            writes_continuations: true,
            writes_profile_scores: false,
            writes_usage_snapshots: false,
            writes_backoffs: false,
        }
    );

    let merged = merge_runtime_state_selected_snapshot_sections(
        &snapshot,
        None,
        Some(&RuntimeContinuationStore::default()),
        100,
        AppStateCompactionPolicy::default(),
        RuntimeContinuationCompactionPolicy::default(),
    )
    .expect("selected snapshot sections merge");

    assert!(merged.state.is_none());
    assert!(merged.continuations.is_some());
    assert_eq!(
        merged.profiles.keys().cloned().collect::<Vec<_>>(),
        vec!["alpha".to_string()]
    );
}

#[test]
fn selected_snapshot_merge_combines_state_and_continuations() {
    let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
    let existing_state = AppState {
        active_profile: Some("alpha".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let state_snapshot = AppState {
        active_profile: Some("alpha".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let snapshot = RuntimeStateSaveSelectedSnapshot {
        paths: (),
        state: Some(state_snapshot),
        profiles: None,
        continuations: Some(RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp".to_string(),
                ResponseProfileBinding {
                    profile_name: "alpha".to_string(),
                    bound_at: 20,
                },
            )]),
            ..RuntimeContinuationStore::default()
        }),
        profile_scores: None::<BTreeMap<String, RuntimeProfileHealth>>,
        usage_snapshots: None::<BTreeMap<String, RuntimeProfileUsageSnapshot>>,
        backoffs: None::<RuntimeProfileBackoffs>,
    };

    assert_eq!(
        runtime_state_selected_snapshot_load_plan(&snapshot),
        RuntimeStateSelectedSnapshotLoadPlan {
            needs_existing_state: true,
            needs_existing_continuations: true,
        }
    );
    assert_eq!(
        runtime_state_selected_snapshot_profiles_for_merge(
            &snapshot,
            Some(&existing_state),
            100,
            AppStateCompactionPolicy::default(),
        )
        .expect("profiles")
        .keys()
        .cloned()
        .collect::<Vec<_>>(),
        vec!["alpha".to_string()]
    );

    let merged = merge_runtime_state_selected_snapshot_sections(
        &snapshot,
        Some(&existing_state),
        Some(&RuntimeContinuationStore::default()),
        100,
        AppStateCompactionPolicy::default(),
        RuntimeContinuationCompactionPolicy::default(),
    )
    .expect("selected snapshot sections merge");

    assert_eq!(
        merged
            .state
            .expect("state")
            .response_profile_bindings
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["resp".to_string()]
    );
    assert!(merged.continuations.is_some());
}

#[test]
fn usage_snapshot_compaction_prunes_missing_and_expired_profiles() {
    let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
    let snapshots = BTreeMap::from([
        (
            "alpha".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: 200,
                five_hour_status: RuntimeQuotaWindowStatus::Ready,
                five_hour_remaining_percent: 100,
                five_hour_reset_at: 0,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 100,
                weekly_reset_at: 0,
            },
        ),
        (
            "missing".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: 200,
                five_hour_status: RuntimeQuotaWindowStatus::Ready,
                five_hour_remaining_percent: 100,
                five_hour_reset_at: 0,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 100,
                weekly_reset_at: 0,
            },
        ),
    ]);

    let compacted = compact_runtime_usage_snapshots(snapshots, &profiles, 250);

    assert_eq!(compacted.len(), 1);
    assert!(compacted.contains_key("alpha"));
}

#[test]
fn profile_score_compaction_supports_route_scoped_keys() {
    let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
    let scores = BTreeMap::from([
        (
            "__route_health__:responses:alpha".to_string(),
            RuntimeProfileHealth {
                score: 2,
                updated_at: 200,
            },
        ),
        (
            "__route_health__:responses:missing".to_string(),
            RuntimeProfileHealth {
                score: 2,
                updated_at: 200,
            },
        ),
    ]);

    let compacted = compact_runtime_profile_scores(scores, &profiles, 250);

    assert_eq!(
        compacted.keys().cloned().collect::<Vec<_>>(),
        vec!["__route_health__:responses:alpha".to_string()]
    );
}

#[test]
fn profile_health_sort_key_includes_route_coupling_and_performance() {
    let scores = BTreeMap::from([
        (
            "alpha".to_string(),
            RuntimeProfileHealth {
                score: 1,
                updated_at: 100,
            },
        ),
        (
            runtime_profile_route_health_key("alpha", RuntimeRouteKind::Responses),
            RuntimeProfileHealth {
                score: 2,
                updated_at: 100,
            },
        ),
        (
            runtime_profile_route_bad_pairing_key("alpha", RuntimeRouteKind::Responses),
            RuntimeProfileHealth {
                score: 3,
                updated_at: 100,
            },
        ),
        (
            runtime_profile_route_health_key("alpha", RuntimeRouteKind::Websocket),
            RuntimeProfileHealth {
                score: 4,
                updated_at: 100,
            },
        ),
        (
            runtime_profile_route_bad_pairing_key("alpha", RuntimeRouteKind::Websocket),
            RuntimeProfileHealth {
                score: 2,
                updated_at: 100,
            },
        ),
        (
            runtime_profile_route_performance_key("alpha", RuntimeRouteKind::Responses),
            RuntimeProfileHealth {
                score: 8,
                updated_at: 100,
            },
        ),
        (
            runtime_profile_route_performance_key("alpha", RuntimeRouteKind::Websocket),
            RuntimeProfileHealth {
                score: 4,
                updated_at: 100,
            },
        ),
    ]);

    assert_eq!(
        runtime_profile_health_sort_key("alpha", &scores, 100, RuntimeRouteKind::Responses),
        19
    );
}

#[test]
fn previous_response_negative_cache_helpers_decay_and_clear_route_keys() {
    let mut scores = BTreeMap::from([
        (
            runtime_previous_response_negative_cache_key(
                "resp",
                "alpha",
                RuntimeRouteKind::Responses,
            ),
            RuntimeProfileHealth {
                score: 3,
                updated_at: 100,
            },
        ),
        (
            runtime_previous_response_negative_cache_key(
                "resp",
                "alpha",
                RuntimeRouteKind::Websocket,
            ),
            RuntimeProfileHealth {
                score: 1,
                updated_at: 100,
            },
        ),
    ]);

    assert_eq!(
        runtime_previous_response_negative_cache_failures(
            &scores,
            "resp",
            "alpha",
            RuntimeRouteKind::Responses,
            102,
            2,
        ),
        2
    );
    assert!(runtime_previous_response_negative_cache_active(
        &scores,
        "resp",
        "alpha",
        RuntimeRouteKind::Websocket,
        100,
        2,
    ));
    assert!(clear_runtime_previous_response_negative_cache(
        &mut scores,
        "resp",
        "alpha",
    ));
    assert!(scores.is_empty());
}

#[test]
fn backoff_compaction_keeps_valid_route_keys_and_future_backoffs() {
    let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
    let backoffs = RuntimeProfileBackoffs {
        retry_backoff_until: BTreeMap::from([
            ("alpha".to_string(), 300),
            ("expired".to_string(), 100),
        ]),
        transport_backoff_until: BTreeMap::from([
            (
                "__route_transport_backoff__:responses:alpha".to_string(),
                300,
            ),
            ("__route_transport_backoff__:unknown:alpha".to_string(), 300),
            ("missing".to_string(), 300),
        ]),
        route_circuit_open_until: BTreeMap::from([
            ("__route_circuit__:responses:alpha".to_string(), 300),
            ("__route_circuit__:responses:missing".to_string(), 300),
        ]),
    };

    let compacted = compact_runtime_profile_backoffs(backoffs, &profiles, 200);

    assert_eq!(
        compacted
            .retry_backoff_until
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["alpha".to_string()]
    );
    assert_eq!(
        compacted
            .transport_backoff_until
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["__route_transport_backoff__:responses:alpha".to_string()]
    );
    assert_eq!(
        compacted
            .route_circuit_open_until
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["__route_circuit__:responses:alpha".to_string()]
    );
}

#[test]
fn transport_backoff_helpers_prefer_route_scoped_future_values() {
    let profile_names = BTreeSet::from(["alpha".to_string()]);
    let route_key = runtime_profile_transport_backoff_key("alpha", RuntimeRouteKind::Responses);
    let backoffs = BTreeMap::from([
        ("alpha".to_string(), 250),
        (route_key.clone(), 300),
        (
            runtime_profile_transport_backoff_key("alpha", RuntimeRouteKind::Compact),
            100,
        ),
    ]);

    assert!(runtime_profile_transport_backoff_key_valid(
        &route_key,
        &profile_names
    ));
    assert_eq!(
        runtime_profile_transport_backoff_until_from_map(
            &backoffs,
            "alpha",
            RuntimeRouteKind::Responses,
            200,
        ),
        Some(300)
    );
    assert_eq!(
        runtime_profile_transport_backoff_max_until(&backoffs, "alpha", 200),
        Some(300)
    );
}

#[test]
fn selection_backoff_helpers_include_retry_transport_and_circuit() {
    let now = 100;
    let retry_backoff_until = BTreeMap::from([("alpha".to_string(), 110)]);
    let transport_backoff_until = BTreeMap::from([(
        runtime_profile_transport_backoff_key("beta", RuntimeRouteKind::Responses),
        120,
    )]);
    let route_circuit_open_until = BTreeMap::from([(
        runtime_profile_route_circuit_key("gamma", RuntimeRouteKind::Responses),
        130,
    )]);

    assert!(runtime_profile_name_in_selection_backoff(
        "alpha",
        &retry_backoff_until,
        &transport_backoff_until,
        &route_circuit_open_until,
        RuntimeRouteKind::Responses,
        now,
    ));
    assert!(runtime_profile_name_in_selection_backoff(
        "beta",
        &retry_backoff_until,
        &transport_backoff_until,
        &route_circuit_open_until,
        RuntimeRouteKind::Responses,
        now,
    ));
    assert!(runtime_profile_name_in_selection_backoff(
        "gamma",
        &retry_backoff_until,
        &transport_backoff_until,
        &route_circuit_open_until,
        RuntimeRouteKind::Responses,
        now,
    ));
    assert!(!runtime_profile_name_in_selection_backoff(
        "delta",
        &retry_backoff_until,
        &transport_backoff_until,
        &route_circuit_open_until,
        RuntimeRouteKind::Responses,
        now,
    ));

    assert_eq!(
        runtime_profile_backoff_sort_key(
            "gamma",
            &retry_backoff_until,
            &transport_backoff_until,
            &route_circuit_open_until,
            RuntimeRouteKind::Responses,
            now,
        ),
        (1, 130, 0, 0)
    );
    assert_eq!(
        runtime_profile_backoff_sort_key(
            "beta",
            &retry_backoff_until,
            &transport_backoff_until,
            &route_circuit_open_until,
            RuntimeRouteKind::Responses,
            now,
        ),
        (2, 120, 0, 0)
    );
    assert_eq!(
        runtime_profile_backoff_sort_key(
            "alpha",
            &retry_backoff_until,
            &transport_backoff_until,
            &route_circuit_open_until,
            RuntimeRouteKind::Responses,
            now,
        ),
        (3, 110, 0, 0)
    );
}

#[test]
fn startup_backoff_softening_prunes_expired_and_caps_future_route_state() {
    let now = 100;
    let transport_key = runtime_profile_transport_backoff_key("alpha", RuntimeRouteKind::Responses);
    let circuit_key = runtime_profile_route_circuit_key("alpha", RuntimeRouteKind::Responses);
    let mut backoffs = RuntimeProfileBackoffs {
        retry_backoff_until: BTreeMap::from([("alpha".to_string(), 500)]),
        transport_backoff_until: BTreeMap::from([
            ("expired".to_string(), 90),
            (transport_key.clone(), 500),
        ]),
        route_circuit_open_until: BTreeMap::from([
            ("expired".to_string(), 90),
            (circuit_key.clone(), 500),
        ]),
    };
    let profile_scores = BTreeMap::from([(
        runtime_profile_route_health_key("alpha", RuntimeRouteKind::Responses),
        RuntimeProfileHealth {
            score: RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD + 2,
            updated_at: now,
        },
    )]);

    assert!(runtime_soften_persisted_backoffs_for_startup(
        &mut backoffs,
        &profile_scores,
        now,
    ));

    assert_eq!(backoffs.retry_backoff_until["alpha"], 500);
    assert_eq!(
        backoffs.transport_backoff_until,
        BTreeMap::from([(
            transport_key,
            now + RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS,
        )])
    );
    assert_eq!(
        backoffs.route_circuit_open_until,
        BTreeMap::from([(
            circuit_key,
            now + runtime_profile_circuit_half_open_probe_seconds(
                RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD + 2,
            ),
        )])
    );
}

#[test]
fn continuation_store_compaction_removes_orphan_lineage_bindings() {
    let profiles = BTreeMap::from([("alpha".to_string(), profile())]);
    let lineage_key = format!(
        "{}{}:{}:{}",
        RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX,
        "missing-parent".len(),
        "missing-parent",
        "turn"
    );
    let keep_lineage_key = format!(
        "{}{}:{}:{}",
        RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX,
        "resp".len(),
        "resp",
        "turn"
    );
    let continuations = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([
            (
                "resp".to_string(),
                ResponseProfileBinding {
                    profile_name: "alpha".to_string(),
                    bound_at: 100,
                },
            ),
            (
                keep_lineage_key.clone(),
                ResponseProfileBinding {
                    profile_name: "alpha".to_string(),
                    bound_at: 100,
                },
            ),
            (
                lineage_key.clone(),
                ResponseProfileBinding {
                    profile_name: "alpha".to_string(),
                    bound_at: 100,
                },
            ),
            (
                "missing-profile".to_string(),
                ResponseProfileBinding {
                    profile_name: "missing".to_string(),
                    bound_at: 100,
                },
            ),
        ]),
        ..RuntimeContinuationStore::default()
    };

    let compacted = compact_runtime_continuation_store(
        continuations,
        &profiles,
        200,
        RuntimeContinuationCompactionPolicy::default(),
    );

    assert!(compacted.response_profile_bindings.contains_key("resp"));
    assert!(
        compacted
            .response_profile_bindings
            .contains_key(&keep_lineage_key)
    );
    assert!(
        !compacted
            .response_profile_bindings
            .contains_key(&lineage_key)
    );
    assert!(
        !compacted
            .response_profile_bindings
            .contains_key("missing-profile")
    );
}

#[test]
fn continuation_status_helpers_touch_verify_and_mark_suspect() {
    let policy = RuntimeContinuationStatusPolicy {
        touch_persist_interval_seconds: 10,
        suspect_grace_seconds: 5,
        suspect_not_found_streak_limit: 2,
        confidence_max: 8,
        verified_confidence_bonus: 2,
        touch_confidence_bonus: 1,
        suspect_confidence_penalty: 1,
    };
    let mut statuses = RuntimeContinuationStatuses::default();

    assert!(runtime_mark_continuation_status_verified(
        &mut statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-1",
        100,
        Some("responses"),
        policy,
    ));
    let status = statuses.response.get("resp-1").expect("verified status");
    assert_eq!(status.state, RuntimeContinuationBindingLifecycle::Verified);
    assert_eq!(status.confidence, 2);
    assert_eq!(status.last_verified_route.as_deref(), Some("responses"));
    assert!(!runtime_continuation_status_should_refresh_verified(
        Some(status),
        105,
        Some("responses"),
        policy,
    ));
    assert!(runtime_continuation_status_should_refresh_verified(
        Some(status),
        111,
        Some("responses"),
        policy,
    ));

    assert!(runtime_mark_continuation_status_suspect(
        &mut statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-1",
        112,
        policy,
    ));
    assert!(runtime_continuation_status_recently_suspect(
        statuses.response.get("resp-1"),
        113,
        policy,
    ));
    assert!(runtime_mark_continuation_status_suspect(
        &mut statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-1",
        114,
        policy,
    ));
    assert_eq!(
        statuses.response.get("resp-1").map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn lineage_key_helpers_round_trip_and_filter_internal_keys() {
    let lineage_key = runtime_response_turn_state_lineage_key("resp:with:colon", "turn");
    assert_eq!(
        runtime_response_turn_state_lineage_parts(&lineage_key),
        Some(("resp:with:colon", "turn"))
    );
    assert_eq!(
        runtime_compact_session_lineage_key("session"),
        "__compact_session__:session"
    );
    assert_eq!(
        runtime_compact_turn_state_lineage_key("turn"),
        "__compact_turn_state__:turn"
    );

    let bindings = BTreeMap::from([
        (
            "external".to_string(),
            ResponseProfileBinding {
                profile_name: "alpha".to_string(),
                bound_at: 100,
            },
        ),
        (
            lineage_key,
            ResponseProfileBinding {
                profile_name: "alpha".to_string(),
                bound_at: 100,
            },
        ),
    ]);

    assert_eq!(
        runtime_external_response_profile_bindings(&bindings)
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["external".to_string()]
    );
}

#[path = "smart_context.rs"]
mod smart_context;
