use super::*;

fn test_paths(temp_dir: &TestDir) -> AppPaths {
    AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    }
}

fn state_with_profiles(temp_dir: &TestDir, names: &[&str], selected_at: i64) -> AppState {
    let profiles = names
        .iter()
        .map(|name| {
            (
                (*name).to_string(),
                ProfileEntry {
                    codex_home: temp_dir.path.join("homes").join(name),
                    managed: true,
                    email: Some(format!("{name}@example.com")),
                    provider: ProfileProvider::Openai,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let bindings = names
        .iter()
        .map(|name| {
            (
                format!("response-{name}"),
                ResponseProfileBinding {
                    binding_identity: None,
                    profile_name: (*name).to_string(),
                    bound_at: selected_at,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let session_bindings = names
        .iter()
        .map(|name| {
            (
                format!("session-{name}"),
                ResponseProfileBinding {
                    binding_identity: None,
                    profile_name: (*name).to_string(),
                    bound_at: selected_at,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    AppState {
        active_profile: names.first().map(|name| (*name).to_string()),
        last_run_selected_at: names
            .iter()
            .map(|name| ((*name).to_string(), selected_at))
            .collect(),
        profiles,
        response_profile_bindings: bindings,
        session_profile_bindings: session_bindings,
    }
}

fn save_full_runtime_snapshot(paths: &AppPaths, snapshot: &AppState) {
    let continuations = runtime_continuation_store_from_app_state(snapshot);
    let revision = AtomicU64::new(1);
    assert!(
        save_runtime_state_snapshot_if_latest(RuntimeStateSnapshotSaveInput {
            paths,
            snapshot,
            continuations: &continuations,
            profile_scores: &BTreeMap::new(),
            usage_snapshots: &BTreeMap::new(),
            backoffs: &RuntimeProfileBackoffs::default(),
            revision: 1,
            latest_revision: &revision,
        })
        .expect("runtime snapshot save should succeed")
    );
}

fn assert_no_profile_targets(continuations: &RuntimeContinuationStore, profile_name: &str) {
    for bindings in [
        &continuations.response_profile_bindings,
        &continuations.session_profile_bindings,
        &continuations.turn_state_bindings,
        &continuations.session_id_bindings,
    ] {
        assert!(
            bindings
                .values()
                .all(|binding| binding.profile_name != profile_name),
            "continuation binding still targets deleted profile {profile_name}"
        );
    }
}

#[test]
fn runtime_snapshot_save_initializes_profiles_on_first_run() {
    let temp_dir = TestDir::isolated();
    let paths = test_paths(&temp_dir);
    let snapshot = state_with_profiles(&temp_dir, &["main"], 100);

    save_full_runtime_snapshot(&paths, &snapshot);

    let loaded = AppState::load(&paths).expect("first-run state should reload");
    assert_eq!(loaded.profiles, snapshot.profiles);
    assert_eq!(loaded.active_profile.as_deref(), Some("main"));
}

#[test]
fn stale_runtime_snapshot_does_not_restore_a_deleted_profile() {
    let temp_dir = TestDir::isolated();
    let paths = test_paths(&temp_dir);
    let initial = state_with_profiles(&temp_dir, &["main"], 1000);
    initial.save(&paths).expect("initial state should save");

    AppState::default()
        .save_with_removed_profiles(&paths, &["main".to_string()])
        .expect("profile deletion should save an empty durable state");

    let stale = state_with_profiles(&temp_dir, &["main"], 10);
    let stale_continuations = runtime_continuation_store_from_app_state(&stale);
    save_runtime_continuation_journal_for_profiles(
        &paths,
        &stale_continuations,
        &stale.profiles,
        10,
    )
    .expect("stale journal save should complete");
    save_full_runtime_snapshot(&paths, &stale);

    let loaded = AppState::load(&paths).expect("deleted state should reload");
    assert!(loaded.profiles.is_empty());
    assert!(loaded.active_profile.is_none());
    assert_no_profile_targets(&runtime_continuation_store_from_app_state(&loaded), "main");
    let journal = load_runtime_continuation_journal_with_recovery(&paths, &BTreeMap::new())
        .expect("journal should reload")
        .value;
    assert_no_profile_targets(&journal.continuations, "main");
}

#[test]
fn stale_runtime_snapshot_preserves_survivors_and_unrelated_fields() {
    let temp_dir = TestDir::isolated();
    let paths = test_paths(&temp_dir);
    let current_selected_at = Local::now().timestamp();
    let current = state_with_profiles(&temp_dir, &["survivor"], current_selected_at);
    current.save(&paths).expect("current state should save");

    let stale = state_with_profiles(
        &temp_dir,
        &["deleted", "survivor"],
        current_selected_at.saturating_sub(1),
    );
    save_full_runtime_snapshot(&paths, &stale);

    let loaded = AppState::load(&paths).expect("merged state should reload");
    assert_eq!(loaded.profiles, current.profiles);
    assert_eq!(loaded.active_profile.as_deref(), Some("survivor"));
    assert_eq!(
        loaded.last_run_selected_at.get("survivor"),
        Some(&current_selected_at)
    );
    assert_eq!(
        loaded
            .response_profile_bindings
            .get("response-survivor")
            .map(|binding| binding.profile_name.as_str()),
        Some("survivor")
    );
    assert!(
        !loaded
            .response_profile_bindings
            .values()
            .any(|binding| binding.profile_name == "deleted")
    );
}

#[test]
fn selected_stale_runtime_snapshot_does_not_restore_a_deleted_profile() {
    let temp_dir = TestDir::isolated();
    let paths = test_paths(&temp_dir);
    let initial = state_with_profiles(&temp_dir, &["main"], 1000);
    initial.save(&paths).expect("initial state should save");
    AppState::default()
        .save_with_removed_profiles(&paths, &["main".to_string()])
        .expect("profile deletion should save an empty durable state");

    let stale = state_with_profiles(&temp_dir, &["main"], 10);
    let selected = RuntimeStateSaveSelectedSnapshot {
        paths: paths.clone(),
        state: Some(stale.clone()),
        profiles: None,
        continuations: Some(runtime_continuation_store_from_app_state(&stale)),
        profile_scores: None,
        usage_snapshots: None,
        backoffs: None,
    };
    let revision = AtomicU64::new(1);
    assert!(
        save_runtime_state_selected_snapshot_if_latest(&selected, 1, &revision)
            .expect("selected runtime snapshot save should succeed")
    );

    let loaded = AppState::load(&paths).expect("selected state should reload");
    assert!(loaded.profiles.is_empty());
    let continuations = load_runtime_continuations_with_recovery(&paths, &BTreeMap::new())
        .expect("selected continuations should reload")
        .value;
    assert_no_profile_targets(&continuations, "main");
}
