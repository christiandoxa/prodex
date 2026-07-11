use super::*;
use prodex_runtime_state::RuntimeProfileUsageSnapshot;
use std::collections::BTreeMap;

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
