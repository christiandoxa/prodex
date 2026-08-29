use super::*;

#[path = "run_command_strategy/live_goal_resume.rs"]
mod live_goal_resume;
#[path = "run_command_strategy/model_resume.rs"]
mod model_resume;
#[path = "run_command_strategy/session_binding.rs"]
mod session_binding;

fn exit_status(code: i32) -> std::process::ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        std::process::ExitStatus::from_raw(code << 8)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::ExitStatusExt as _;
        std::process::ExitStatus::from_raw(code as u32)
    }
}

fn assert_repaired_session_meta_line(line: &str, session_id: &str) {
    let value: serde_json::Value =
        serde_json::from_str(line).expect("repaired metadata should be valid JSON");
    assert_eq!(value["type"], "session_meta");
    assert_eq!(value["payload"]["id"], session_id);
    assert!(value["timestamp"].as_str().is_some());
    assert!(value["payload"]["timestamp"].as_str().is_some());
    assert!(value["payload"]["cwd"].as_str().is_some());
    assert_eq!(value["payload"]["originator"], "prodex-repair");
    assert_eq!(value["payload"]["source"], "cli");
}

#[test]
fn run_strategy_auto_routes_gemini_resume_sessions_to_provider_bridge() {
    let root = temp_dir("auto-route-gemini-resume");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_codex_home = root.join("shared-codex-home");
    let _shared_env = TestEnvVarGuard::set(
        "PRODEX_SHARED_CODEX_HOME",
        shared_codex_home.to_str().unwrap(),
    );
    let paths = AppPaths::discover().unwrap();
    let session_id = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9";
    let sessions = paths.shared_codex_root.join("sessions/2026/06/05");
    fs::create_dir_all(&sessions).unwrap();
    fs::write(
        sessions.join("rollout.jsonl"),
        session_meta_line(session_id, &root, Some("prodex-gemini")),
    )
    .unwrap();

    let strategy = RunCommandStrategy::new(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: false,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![OsString::from(session_id)],
    })
    .unwrap();
    let request = strategy.runtime_request();
    let codex_args = strategy
        .codex_args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(request.external_provider, Some("gemini"));
    assert_eq!(
        request.model_provider_override,
        Some(SUPER_GEMINI_PROVIDER_ID)
    );
    assert_eq!(request.base_url, Some(SUPER_GEMINI_DEFAULT_BASE_URL));
    assert!(request.smart_context_enabled);
    assert!(codex_args.contains(&"model_provider=\"prodex-gemini\"".to_string()));
    assert!(codex_args.contains(&"resume".to_string()));
    assert!(codex_args.contains(&session_id.to_string()));

    let mut child = ChildProcessPlan {
        binary: OsString::from("codex"),
        args: Vec::new(),
        codex_home: root.join("profile"),
        extra_env: vec![(
            OsString::from("UNRELATED_CHILD_ENV"),
            OsString::from("keep-me"),
        )],
        removed_env: vec![OsString::from("EXISTING_REMOVED_ENV")],
        reset_terminal_keyboard_enhancement: false,
    };
    isolate_auto_external_provider_child_env(strategy.auto_external_provider, &mut child);

    for key in PROVIDER_SECRET_ENV_KEYS {
        assert!(
            child.removed_env.contains(&OsString::from(key)),
            "provider secret env {key} should be removed"
        );
    }
    assert!(
        child
            .removed_env
            .contains(&OsString::from("EXISTING_REMOVED_ENV"))
    );
    assert!(
        child
            .extra_env
            .iter()
            .any(|(key, value)| { key == "UNRELATED_CHILD_ENV" && value == "keep-me" })
    );
}

#[test]
fn run_strategy_auto_routes_kiro_resume_sessions_to_provider_bridge() {
    let root = temp_dir("auto-route-kiro-resume");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_codex_home = root.join("shared-codex-home");
    let _shared_env = TestEnvVarGuard::set(
        "PRODEX_SHARED_CODEX_HOME",
        shared_codex_home.to_str().unwrap(),
    );
    let paths = AppPaths::discover().unwrap();
    let session_id = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44fa";
    let sessions = paths.shared_codex_root.join("sessions/2026/06/05");
    fs::create_dir_all(&sessions).unwrap();
    fs::write(
        sessions.join("rollout.jsonl"),
        session_meta_line(session_id, &root, Some("prodex-kiro")),
    )
    .unwrap();

    let strategy = RunCommandStrategy::new(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: false,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![OsString::from(session_id)],
    })
    .unwrap();
    let request = strategy.runtime_request();
    let codex_args = strategy
        .codex_args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(request.external_provider, Some("kiro"));
    assert_eq!(
        request.model_provider_override,
        Some(SUPER_KIRO_PROVIDER_ID)
    );
    assert_eq!(request.base_url, Some("https://kiro.dev"));
    assert!(request.smart_context_enabled);
    assert!(codex_args.contains(&"model_provider=\"prodex-kiro\"".to_string()));
    assert!(codex_args.contains(&"resume".to_string()));
    assert!(codex_args.contains(&session_id.to_string()));
}

#[cfg(unix)]
#[test]
fn run_strategy_exact_resume_provider_detection_skips_unreadable_unrelated_files() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_dir("auto-route-gemini-resume-skip-unreadable");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_codex_home = root.join("shared-codex-home");
    let _shared_env = TestEnvVarGuard::set(
        "PRODEX_SHARED_CODEX_HOME",
        shared_codex_home.to_str().unwrap(),
    );
    let paths = AppPaths::discover().unwrap();
    let session_id = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9";
    let sessions = paths.shared_codex_root.join("sessions/2026/06/05");
    fs::create_dir_all(&sessions).unwrap();
    let target = sessions.join(format!("rollout-2026-06-05T01-00-00-{session_id}.jsonl"));
    let unrelated = sessions.join("rollout-2026-06-05T01-00-00-other-session.jsonl");
    fs::write(
        &target,
        session_meta_line(session_id, &root, Some("prodex-gemini")),
    )
    .unwrap();
    fs::write(
        &unrelated,
        "{\"timestamp\":\"2026-06-05T01:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"other-session\",\"cwd\":\"/tmp/unrelated\",\"model_provider\":\"openai\"}}\n",
    )
    .unwrap();
    let mut perms = fs::metadata(&unrelated).unwrap().permissions();
    perms.set_mode(0o0);
    fs::set_permissions(&unrelated, perms).unwrap();

    let strategy = RunCommandStrategy::new(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: false,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![OsString::from(session_id)],
    })
    .unwrap();

    assert_eq!(strategy.runtime_request().external_provider, Some("gemini"));

    let mut perms = fs::metadata(&unrelated).unwrap().permissions();
    perms.set_mode(0o644);
    let _ = fs::set_permissions(&unrelated, perms);
}

#[test]
fn run_strategy_repairs_resume_session_metadata_prefix_before_provider_detection() {
    let root = temp_dir("repair-resume-prefix-before-provider-detection");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_codex_home = root.join("shared-codex-home");
    let _shared_env = TestEnvVarGuard::set(
        "PRODEX_SHARED_CODEX_HOME",
        shared_codex_home.to_str().unwrap(),
    );
    let paths = AppPaths::discover().unwrap();
    let session_id = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9";
    let sessions = paths.shared_codex_root.join("sessions/2026/06/05");
    fs::create_dir_all(&sessions).unwrap();
    let session_path = sessions.join("rollout.jsonl");
    fs::write(
        &session_path,
        format!(
            "{}\n{}",
            serde_json::json!({
                "timestamp": "2026-06-05T00:59:00Z",
                "type": "event",
                "payload": {"message": "partial"},
            }),
            session_meta_line(session_id, &root, Some("prodex-gemini"))
        ),
    )
    .unwrap();

    let strategy = RunCommandStrategy::new(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: false,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![OsString::from(session_id)],
    })
    .unwrap();

    let repaired = fs::read_to_string(session_path).unwrap();
    assert!(
        repaired
            .lines()
            .next()
            .unwrap()
            .contains(r#""type":"session_meta""#)
    );
    assert_eq!(strategy.runtime_request().external_provider, Some("gemini"));
}

#[test]
fn run_strategy_repairs_resume_session_missing_metadata_before_codex_launch() {
    let root = temp_dir("repair-missing-resume-session-metadata");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_codex_home = root.join("shared-codex-home");
    let _shared_env = TestEnvVarGuard::set(
        "PRODEX_SHARED_CODEX_HOME",
        shared_codex_home.to_str().unwrap(),
    );
    let paths = AppPaths::discover().unwrap();
    let session_id = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9";
    let sessions = paths.shared_codex_root.join("sessions/2026/06/05");
    fs::create_dir_all(&sessions).unwrap();
    fs::write(
        sessions.join(format!("rollout-2026-06-05T01-00-00-{session_id}.jsonl")),
        "{\"timestamp\":\"2026-06-05T00:59:00Z\",\"type\":\"event\",\"payload\":{\"message\":\"partial only\"}}\n",
    )
    .unwrap();

    let strategy = RunCommandStrategy::new(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: false,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![OsString::from(session_id)],
    })
    .unwrap();

    let codex_args = strategy
        .codex_args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(codex_args.contains(&"resume".to_string()));
    assert!(codex_args.contains(&session_id.to_string()));

    let repaired = fs::read_to_string(
        sessions.join(format!("rollout-2026-06-05T01-00-00-{session_id}.jsonl")),
    )
    .unwrap();
    assert!(
        repaired
            .lines()
            .next()
            .unwrap()
            .contains(r#""type":"session_meta""#)
    );
}

#[test]
fn run_strategy_repairs_resume_session_in_selected_profile_home_before_codex_launch() {
    let root = temp_dir("repair-resume-selected-profile-home");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_codex_home = root.join("shared-codex-home");
    let _shared_env = TestEnvVarGuard::set(
        "PRODEX_SHARED_CODEX_HOME",
        shared_codex_home.to_str().unwrap(),
    );
    let profile_home = root.join("profiles").join("em2");
    let other_profile_home = root.join("profiles").join("157");
    let orphan_profile_home = root.join("profiles").join("orphan");
    let sessions = profile_home.join("sessions/2026/06/13");
    let other_sessions = other_profile_home.join("sessions/2026/06/13");
    let orphan_sessions = orphan_profile_home.join("sessions/2026/06/13");
    fs::create_dir_all(&sessions).unwrap();
    fs::create_dir_all(&other_sessions).unwrap();
    fs::create_dir_all(&orphan_sessions).unwrap();
    write_runtime_launch_auth(
        secret_store::auth_json_path(&profile_home),
        r#"{"tokens":{"access_token":"profile-token"}}"#,
    )
    .unwrap();
    let session_id = "019ebd01-c881-74c0-b01d-7fdf5bd4dd32";
    let session_path = sessions.join(format!("rollout-2026-06-13T02-04-31-{session_id}.jsonl"));
    fs::write(
        &session_path,
        "{\"timestamp\":\"2026-06-13T02:04:31Z\",\"type\":\"event\",\"payload\":{\"message\":\"partial only\"}}\n",
    )
    .unwrap();
    let other_session_path =
        other_sessions.join(format!("rollout-2026-06-13T02-04-31-{session_id}.jsonl"));
    fs::write(
        &other_session_path,
        "{\"timestamp\":\"2026-06-13T02:04:31Z\",\"type\":\"event\",\"payload\":{\"message\":\"partial in other profile\"}}\n",
    )
    .unwrap();
    let orphan_session_path =
        orphan_sessions.join(format!("rollout-2026-06-13T02-04-31-{session_id}.jsonl"));
    fs::write(
        &orphan_session_path,
        "{\"timestamp\":\"2026-06-13T02:04:31Z\",\"type\":\"event\",\"payload\":{\"message\":\"partial in orphan profile\"}}\n",
    )
    .unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("em2".to_string()),
            profiles: BTreeMap::from([
                (
                    "157".to_string(),
                    ProfileEntry {
                        codex_home: other_profile_home,
                        managed: false,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                ),
                (
                    "em2".to_string(),
                    ProfileEntry {
                        codex_home: profile_home.clone(),
                        managed: false,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                ),
            ]),
            ..AppState::default()
        },
    );

    let mut strategy = RunCommandStrategy::new(RunArgs {
        profile: Some("em2".to_string()),
        auto_rotate: false,
        no_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: true,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![OsString::from(session_id)],
    })
    .unwrap();
    let prepared = prepare_runtime_launch(strategy.runtime_request()).unwrap();

    strategy
        .build_plan(&prepared, prepared.runtime_proxy.as_ref())
        .unwrap();

    let repaired = fs::read_to_string(session_path).unwrap();
    assert_repaired_session_meta_line(
        repaired.lines().next().unwrap(),
        "019ebd01-c881-74c0-b01d-7fdf5bd4dd32",
    );
    let other_repaired = fs::read_to_string(other_session_path).unwrap();
    assert_repaired_session_meta_line(
        other_repaired.lines().next().unwrap(),
        "019ebd01-c881-74c0-b01d-7fdf5bd4dd32",
    );
    let orphan_repaired = fs::read_to_string(orphan_session_path).unwrap();
    assert_repaired_session_meta_line(
        orphan_repaired.lines().next().unwrap(),
        "019ebd01-c881-74c0-b01d-7fdf5bd4dd32",
    );
}

#[cfg(unix)]
#[test]
fn run_strategy_skips_symlink_managed_profile_home_during_resume_repair() {
    let root = temp_dir("repair-resume-skip-symlink-profile-home");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_codex_home = root.join("shared-codex-home");
    let _shared_env = TestEnvVarGuard::set(
        "PRODEX_SHARED_CODEX_HOME",
        shared_codex_home.to_str().unwrap(),
    );
    let primary_home = root.join("profiles").join("primary");
    let outside_home = root.join("outside-profile-home");
    let outside_sessions = outside_home.join("sessions/2026/06/13");
    fs::create_dir_all(&primary_home).unwrap();
    fs::create_dir_all(&outside_sessions).unwrap();
    write_runtime_launch_auth(
        secret_store::auth_json_path(&primary_home),
        r#"{"tokens":{"access_token":"profile-token"}}"#,
    )
    .unwrap();
    let session_id = "019ebd01-c881-74c0-b01d-7fdf5bd4dd32";
    let outside_session_path =
        outside_sessions.join(format!("rollout-2026-06-13T02-04-31-{session_id}.jsonl"));
    fs::write(
        &outside_session_path,
        "{\"timestamp\":\"2026-06-13T02:04:31Z\",\"type\":\"event\",\"payload\":{\"message\":\"outside should stay untouched\"}}\n",
    )
    .unwrap();
    std::os::unix::fs::symlink(&outside_home, root.join("profiles").join("linked")).unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("primary".to_string()),
            profiles: BTreeMap::from([(
                "primary".to_string(),
                ProfileEntry {
                    codex_home: primary_home.clone(),
                    managed: true,
                    email: Some("primary@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        },
    );

    let mut strategy = RunCommandStrategy::new(RunArgs {
        profile: Some("primary".to_string()),
        auto_rotate: false,
        no_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: true,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![OsString::from(session_id)],
    })
    .unwrap();
    let prepared = prepare_runtime_launch(strategy.runtime_request()).unwrap();

    strategy
        .build_plan(&prepared, prepared.runtime_proxy.as_ref())
        .unwrap();

    let untouched = fs::read_to_string(&outside_session_path).unwrap();
    assert_eq!(
        untouched,
        "{\"timestamp\":\"2026-06-13T02:04:31Z\",\"type\":\"event\",\"payload\":{\"message\":\"outside should stay untouched\"}}\n"
    );
    assert!(!outside_sessions.join(".prodex-repair-bak").exists());
}

#[cfg(unix)]
#[test]
fn run_strategy_refuses_managed_profile_home_under_symlink_parent() {
    let root = temp_dir("launch-reject-symlink-parent-profile-home");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_codex_home = root.join("shared-codex-home");
    let _shared_env = TestEnvVarGuard::set(
        "PRODEX_SHARED_CODEX_HOME",
        shared_codex_home.to_str().unwrap(),
    );
    let profiles = root.join("profiles");
    let outside = root.join("outside-profile-parent");
    let link = profiles.join("link");
    let codex_home = link.join("main");
    fs::create_dir_all(&profiles).unwrap();
    fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("primary".to_string()),
            profiles: BTreeMap::from([(
                "primary".to_string(),
                ProfileEntry {
                    codex_home,
                    managed: true,
                    email: Some("primary@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        },
    );

    let strategy = RunCommandStrategy::new(RunArgs {
        profile: Some("primary".to_string()),
        auto_rotate: false,
        no_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: true,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: Vec::new(),
    })
    .unwrap();

    let err = match prepare_runtime_launch(strategy.runtime_request()) {
        Ok(_) => panic!("symlink parent managed profile home should reject"),
        Err(err) => err,
    };

    assert!(
        err.to_string().contains("is outside"),
        "unexpected error: {err:#}"
    );
    assert!(
        !outside.join("main").exists(),
        "launch must not create profile homes through a symlinked parent"
    );
}

#[test]
fn run_strategy_repairs_resume_session_in_managed_profile_after_shared_migration() {
    let root = temp_dir("repair-resume-managed-profile-after-migration");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_codex_home = root.join("shared-codex-home");
    let _shared_env = TestEnvVarGuard::set(
        "PRODEX_SHARED_CODEX_HOME",
        shared_codex_home.to_str().unwrap(),
    );
    let profile_home = root.join("profiles").join("em2015-139.com");
    let sessions = profile_home.join("sessions/2026/06/14");
    fs::create_dir_all(&sessions).unwrap();
    write_runtime_launch_auth(
        secret_store::auth_json_path(&profile_home),
        r#"{"tokens":{"access_token":"profile-token"}}"#,
    )
    .unwrap();
    let session_id = "019ec6c3-28a4-79f0-91f9-74a2f34b0928";
    let session_path = sessions.join(format!("rollout-2026-06-14T23-32-19-{session_id}.jsonl"));
    fs::write(
        &session_path,
        "{\"timestamp\":\"2026-06-14T23:32:19Z\",\"type\":\"event\",\"payload\":{\"message\":\"partial only\"}}\n",
    )
    .unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("em2015-139.com".to_string()),
            profiles: BTreeMap::from([(
                "em2015-139.com".to_string(),
                ProfileEntry {
                    codex_home: profile_home.clone(),
                    managed: true,
                    email: Some("em2015-139.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        },
    );

    let mut strategy = RunCommandStrategy::new(RunArgs {
        profile: Some("em2015-139.com".to_string()),
        auto_rotate: false,
        no_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: true,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![OsString::from(session_id)],
    })
    .unwrap();
    let prepared = prepare_runtime_launch(strategy.runtime_request()).unwrap();

    strategy
        .build_plan(&prepared, prepared.runtime_proxy.as_ref())
        .unwrap();

    let repaired_path = shared_codex_home.join(format!(
        "sessions/2026/06/14/rollout-2026-06-14T23-32-19-{session_id}.jsonl"
    ));
    let repaired = fs::read_to_string(repaired_path).unwrap();
    assert_repaired_session_meta_line(
        repaired.lines().next().unwrap(),
        "019ec6c3-28a4-79f0-91f9-74a2f34b0928",
    );
}

#[test]
fn run_strategy_auto_routes_explicit_exec_gemini_resume_sessions_to_provider_bridge() {
    let root = temp_dir("auto-route-explicit-exec-gemini-resume");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_codex_home = root.join("shared-codex-home");
    let _shared_env = TestEnvVarGuard::set(
        "PRODEX_SHARED_CODEX_HOME",
        shared_codex_home.to_str().unwrap(),
    );
    let paths = AppPaths::discover().unwrap();
    let session_id = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9";
    let sessions = paths.shared_codex_root.join("sessions/2026/06/05");
    fs::create_dir_all(&sessions).unwrap();
    fs::write(
        sessions.join("rollout.jsonl"),
        session_meta_line(session_id, &root, Some("prodex-gemini")),
    )
    .unwrap();

    let strategy = RunCommandStrategy::new(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: false,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![
            OsString::from("exec"),
            OsString::from("resume"),
            OsString::from(session_id),
            OsString::from("continue"),
        ],
    })
    .unwrap();
    let request = strategy.runtime_request();
    let codex_args = strategy
        .codex_args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(request.external_provider, Some("gemini"));
    assert_eq!(
        request.model_provider_override,
        Some(SUPER_GEMINI_PROVIDER_ID)
    );
    assert_eq!(request.base_url, Some(SUPER_GEMINI_DEFAULT_BASE_URL));
    assert!(request.smart_context_enabled);
    assert!(codex_args.contains(&"model_provider=\"prodex-gemini\"".to_string()));
    assert_eq!(
        codex_args
            .iter()
            .filter(|arg| arg.as_str() == "exec")
            .count(),
        1
    );
    assert_eq!(
        codex_args
            .iter()
            .filter(|arg| arg.as_str() == "resume")
            .count(),
        1
    );
    assert!(codex_args.contains(&session_id.to_string()));
}

#[test]
fn run_command_strategy_keeps_smart_context_autopilot_disabled() {
    let strategy = RunCommandStrategy::new(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: false,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![OsString::from("exec"), OsString::from("hello")],
    })
    .unwrap();

    assert!(!strategy.runtime_request().smart_context_enabled);
}

#[test]
fn run_command_strategy_carries_profile_v2_name() {
    let strategy = RunCommandStrategy::new(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: false,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![
            OsString::from("exec"),
            OsString::from("--profile"),
            OsString::from("bedrock"),
            OsString::from("hello"),
        ],
    })
    .unwrap();

    assert_eq!(strategy.runtime_request().profile_v2_name, Some("bedrock"));
}
