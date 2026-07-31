use super::tests::{dashboard_json_request, dashboard_test_paths};
use super::{AppState, DashboardServer};
use crate::{
    AppStateIoExt, ProfileEntry, ProfileLifecyclePlan, ProfileProvider, ResponseProfileBinding,
    RuntimeContinuationStore, lifecycle_profile_state, runtime_continuations_file_path,
    save_runtime_continuations_for_profiles, write_profile_lifecycle_plan,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;

#[test]
fn dashboard_profile_removal_prunes_state_and_runtime_continuations() {
    let paths = dashboard_test_paths("profile-removal-runtime-prune");
    let dashboard = DashboardServer {
        paths: paths.clone(),
        base_url: None,
    };
    let main_home = paths.managed_profiles_root.join("main");
    let removed_home = paths.managed_profiles_root.join("second");
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: removed_home,
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        response_profile_bindings: BTreeMap::from([(
            "response-1".to_string(),
            ResponseProfileBinding {
                profile_name: "second".to_string(),
                bound_at: 1,
            },
        )]),
        session_profile_bindings: BTreeMap::from([(
            "session-1".to_string(),
            ResponseProfileBinding {
                profile_name: "second".to_string(),
                bound_at: 1,
            },
        )]),
        ..AppState::default()
    };
    state.save(&paths).expect("dashboard state should save");
    save_runtime_continuations_for_profiles(
        &paths,
        &RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "response-1".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: 1,
                },
            )]),
            session_profile_bindings: BTreeMap::from([(
                "session-1".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: 1,
                },
            )]),
            turn_state_bindings: BTreeMap::from([(
                "turn-1".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: 1,
                },
            )]),
            session_id_bindings: BTreeMap::from([(
                "session-id-1".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: 1,
                },
            )]),
            ..RuntimeContinuationStore::default()
        },
        &state.profiles,
    )
    .expect("runtime continuations should save");

    let (status, _) = dashboard_json_request(
        &dashboard,
        reqwest::Method::DELETE,
        "/api/profile/second",
        None,
    );
    assert_eq!(status, 200, "dashboard profile removal should succeed");
    let state = AppState::load(&paths).expect("dashboard state should reload");
    assert!(!state.profiles.contains_key("second"));
    assert!(state.response_profile_bindings.is_empty());
    assert!(state.session_profile_bindings.is_empty());
    assert!(
        !fs::read_to_string(runtime_continuations_file_path(&paths))
            .expect("runtime continuations should remain readable")
            .contains("second")
    );
    fs::remove_dir_all(paths.root).expect("test root should be removed");
}

#[test]
fn dashboard_mutation_finalizes_recovered_profile_removal() {
    let paths = dashboard_test_paths("profile-removal-recovery");
    let dashboard = DashboardServer {
        paths: paths.clone(),
        base_url: None,
    };
    let main_home = paths.managed_profiles_root.join("main");
    let removed_home = paths.managed_profiles_root.join("second");
    let previous_state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: removed_home,
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        ..AppState::default()
    };
    previous_state
        .save(&paths)
        .expect("initial dashboard state should save");
    save_runtime_continuations_for_profiles(
        &paths,
        &RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "response-1".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: 1,
                },
            )]),
            ..RuntimeContinuationStore::default()
        },
        &previous_state.profiles,
    )
    .expect("runtime continuations should save");

    let mut committed_state = previous_state.clone();
    committed_state.profiles.remove("second");
    committed_state.active_profile = Some("main".to_string());
    let lifecycle_path = write_profile_lifecycle_plan(
        &paths,
        "remove",
        &ProfileLifecyclePlan {
            profile_states: vec![
                lifecycle_profile_state("second", previous_state.profiles.get("second"), None)
                    .expect("removal lifecycle state should build"),
            ],
            previous_active_profile: previous_state.active_profile.clone(),
            next_active_profile: committed_state.active_profile.clone(),
            home_actions: Vec::new(),
            auth_journal_paths: Vec::new(),
        },
    )
    .expect("removal lifecycle should be journaled");
    committed_state
        .save_with_removed_profiles(&paths, &["second".to_string()])
        .expect("committed removal state should save");

    let (status, _) = dashboard_json_request(
        &dashboard,
        reqwest::Method::POST,
        "/api/profile/active",
        Some(json!({ "profile": "main" })),
    );
    assert_eq!(status, 200, "dashboard mutation should recover removal");
    assert!(!lifecycle_path.exists());
    assert!(
        prodex_profile_export::profile_lifecycle_journal_paths(&paths.root)
            .expect("lifecycle journals should be readable")
            .is_empty()
    );
    assert!(
        !fs::read_to_string(runtime_continuations_file_path(&paths))
            .expect("runtime continuations should remain readable")
            .contains("second")
    );
    fs::remove_dir_all(paths.root).expect("test root should be removed");
}
