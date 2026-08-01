use super::*;

#[test]
fn profile_lifecycle_lock_serializes_recovery_until_owner_drops() {
    use fs2::FileExt as _;
    use std::fs::OpenOptions;
    use std::sync::mpsc;

    let sandbox_dir = ProfileCommandsTestDir::new("lifecycle-lock");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let paths = AppPaths::discover().expect("app paths should resolve");
    let profile_home = paths.managed_profiles_root.join("main");
    create_codex_home_if_missing(&profile_home).expect("profile home should exist");
    let old_auth =
        profile_commands_auth_json_with_email("main@example.com", "old-token", "main-account");
    write_secret_text_file(&profile_home.join("auth.json"), &old_auth)
        .expect("old auth should be written");
    let profile = ProfileEntry {
        codex_home: profile_home.clone(),
        managed: true,
        email: Some("main@example.com".to_string()),
        provider: ProfileProvider::Openai,
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([("main".to_string(), profile.clone())]),
        ..AppState::default()
    };
    state.save(&paths).expect("initial state should save");

    let temporary_home = paths.managed_profiles_root.join(".login-test");
    create_codex_home_if_missing(&temporary_home).expect("temporary home should exist");
    let mut desired = profile;
    desired.email = Some("new@example.com".to_string());
    let new_auth =
        profile_commands_auth_json_with_email("new@example.com", "new-token", "main-account");
    let owner = crate::profile_commands::import_export::acquire_profile_lifecycle_lock(&paths)
        .expect("owner should acquire lifecycle lock");
    let (lifecycle_path, auth_path) =
        crate::profile_commands::import_export::prepare_existing_profile_lifecycle(
            &paths,
            "login",
            &state,
            "main",
            &desired,
            Some("main".to_string()),
            crate::profile_commands::import_export::ProfileAuthUpdate {
                next_auth_json: Some(new_auth.clone()),
                next_provider_json: Some(serde_json::to_string(&desired.provider).unwrap()),
                next_secret_files: Vec::new(),
                previous_secret_file_paths: &[],
                temporary_home: Some(&temporary_home),
            },
        )
        .expect("lifecycle should be journaled");
    write_secret_text_file(&profile_home.join("auth.json"), &new_auth)
        .expect("partial auth should be written");

    let lock_path = crate::profile_commands::import_export::profile_lifecycle_lock_path(&paths);
    let (ready_tx, ready_rx) = mpsc::channel();
    let recovery = std::thread::spawn(move || {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(lock_path)
            .expect("lifecycle lock should exist");
        let error = file
            .try_lock_exclusive()
            .expect_err("owner should block a second lifecycle writer");
        assert_eq!(error.kind(), fs2::lock_contended_error().kind());
        ready_tx
            .send(())
            .expect("lock contention should be reported");
        drop(file);
        handle_current_profile().expect("current profile should recover after owner drops");
    });

    ready_rx.recv().expect("second writer should reach lock");
    assert_eq!(
        profile_commands_read_access_token(&profile_home),
        "new-token"
    );
    assert!(lifecycle_path.exists());
    assert!(auth_path.exists());
    assert!(temporary_home.exists());

    drop(owner);
    recovery.join().expect("recovery thread should finish");

    assert_eq!(
        profile_commands_read_access_token(&profile_home),
        "old-token"
    );
    assert_eq!(
        AppState::load(&paths).unwrap().profiles["main"]
            .email
            .as_deref(),
        Some("main@example.com")
    );
    assert!(!lifecycle_path.exists());
    assert!(!auth_path.exists());
    assert!(!temporary_home.exists());

    let released = crate::profile_commands::import_export::acquire_profile_lifecycle_lock(&paths)
        .expect("lifecycle lock should release after recovery");
    drop(released);
}

#[test]
fn ordinary_command_repairs_pending_profile_lifecycle_before_consuming_state() {
    let root = ProfileCommandsTestDir::new("dispatch-lifecycle-recovery");
    let _env = ProfileCommandsTestEnv::new(&root.path);
    let paths = AppPaths::discover().expect("paths should resolve");
    let profile_home = paths.managed_profiles_root.join("main");
    create_codex_home_if_missing(&profile_home).expect("profile home should exist");
    write_secret_text_file(
        &profile_home.join("auth.json"),
        &profile_commands_auth_json_with_email("old@example.com", "old-token", "main-account"),
    )
    .expect("old credential should be written");
    let profile = ProfileEntry {
        codex_home: profile_home.clone(),
        managed: true,
        email: Some("old@example.com".to_string()),
        provider: ProfileProvider::Openai,
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([("main".to_string(), profile.clone())]),
        ..AppState::default()
    };
    state.save(&paths).expect("initial state should save");

    let temporary_home = paths.managed_profiles_root.join(".login-test");
    create_codex_home_if_missing(&temporary_home).expect("temporary home should exist");
    let mut desired = profile.clone();
    desired.email = Some("new@example.com".to_string());
    let new_auth =
        profile_commands_auth_json_with_email("new@example.com", "new-token", "main-account");
    let (lifecycle_path, auth_path) =
        crate::profile_commands::import_export::prepare_existing_profile_lifecycle(
            &paths,
            "login",
            &state,
            "main",
            &desired,
            Some("main".to_string()),
            crate::profile_commands::import_export::ProfileAuthUpdate {
                next_auth_json: Some(new_auth.clone()),
                next_provider_json: Some(serde_json::to_string(&desired.provider).unwrap()),
                next_secret_files: Vec::new(),
                previous_secret_file_paths: &[],
                temporary_home: Some(&temporary_home),
            },
        )
        .expect("pending lifecycle should be journaled");
    write_secret_text_file(&profile_home.join("auth.json"), &new_auth)
        .expect("partial credential update should be written");

    crate::command_dispatch::execute_command(crate::Commands::Current)
        .expect("ordinary command should repair before loading profile state");

    assert_eq!(
        profile_commands_read_access_token(&profile_home),
        "old-token"
    );
    assert_eq!(
        AppState::load(&paths)
            .expect("state should remain readable")
            .profiles["main"],
        profile
    );
    assert!(!temporary_home.exists());
    assert!(!lifecycle_path.exists());
    assert!(!auth_path.exists());
}

#[test]
fn native_dry_run_leaves_pending_profile_lifecycle_untouched() {
    let root = ProfileCommandsTestDir::new("native-dry-run-lifecycle");
    let _env = ProfileCommandsTestEnv::new(&root.path);
    let paths = AppPaths::discover().expect("paths should resolve");
    let lifecycle_path = crate::profile_commands::import_export::write_profile_lifecycle_plan(
        &paths,
        "login",
        &crate::profile_commands::import_export::ProfileLifecyclePlan {
            profile_states: Vec::new(),
            previous_active_profile: None,
            next_active_profile: None,
            home_actions: Vec::new(),
            auth_journal_paths: Vec::new(),
        },
    )
    .expect("pending lifecycle should be journaled");
    let command = parse_cli_command_from([
        "prodex",
        "super",
        "--cli",
        "gemini",
        "--provider",
        "gemini",
        "--dry-run",
    ])
    .expect("native dry-run should parse");

    crate::command_dispatch::execute_command(command).expect("native dry-run should succeed");

    assert!(
        lifecycle_path.exists(),
        "dry-run must not recover lifecycle state"
    );
    assert!(
        !paths.state_file.exists(),
        "dry-run must not write recovered state"
    );
    assert!(
        !crate::profile_commands::import_export::profile_lifecycle_lock_path(&paths).exists(),
        "dry-run must not acquire lifecycle lock"
    );
}
