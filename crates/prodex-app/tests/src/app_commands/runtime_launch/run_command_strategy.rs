use super::*;
use std::path::Path;
use std::process::Command;

fn exit_status(code: i32) -> std::process::ExitStatus {
    Command::new("/bin/sh")
        .args(["-c", &format!("exit {code}")])
        .status()
        .expect("exit status should run")
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
fn run_strategy_resolves_codex_delete_partial_selector_before_launch() {
    let root = temp_dir("delete-partial-selector");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let paths = AppPaths::discover().unwrap();
    let session_id = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9";
    let sessions = paths.shared_codex_root.join("sessions/2026/06/05");
    fs::create_dir_all(&sessions).unwrap();
    fs::write(
        sessions.join("rollout.jsonl"),
        format!(
            "{{\"timestamp\":\"2026-06-05T01:00:00Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"{session_id}\",\"cwd\":\"{}\"}}}}\n",
            root.display()
        ),
    )
    .unwrap();

    let strategy = RunCommandStrategy::new(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: true,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![OsString::from("delete"), OsString::from("019c9e3d")],
    })
    .unwrap();

    assert_eq!(strategy.delete_session_id.as_deref(), Some(session_id));
}

#[test]
fn run_strategy_plans_goal_resume_relaunch_after_usage_limit_with_active_goal() {
    let root = temp_dir("goal-resume-relaunch-plan");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_codex_home = root.join("shared-codex-home");
    let _shared_env = TestEnvVarGuard::set(
        "PRODEX_SHARED_CODEX_HOME",
        shared_codex_home.to_str().unwrap(),
    );
    let paths = AppPaths::discover().unwrap();
    let main_home = root.join("profiles").join("main");
    let second_home = root.join("profiles").join("second");
    fs::create_dir_all(&main_home).unwrap();
    fs::create_dir_all(&second_home).unwrap();
    fs::write(
        secret_store::auth_json_path(&main_home),
        r#"{"tokens":{"access_token":"main-token","account_id":"main-account"}}"#,
    )
    .unwrap();
    fs::write(
        secret_store::auth_json_path(&second_home),
        r#"{"tokens":{"access_token":"second-token","account_id":"second-account"}}"#,
    )
    .unwrap();

    let session_id = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9";
    let sessions = paths.shared_codex_root.join("sessions/2026/06/05");
    fs::create_dir_all(&sessions).unwrap();
    fs::write(
        sessions.join(format!("rollout-2026-06-05T01-00-00-{session_id}.jsonl")),
        concat!(
            "{\"timestamp\":\"2026-06-05T01:00:00Z\",\"type\":\"session_meta\",\"payload\":",
            "{\"id\":\"019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9\",\"cwd\":\"/tmp/test\"}}\n",
            "{\"timestamp\":\"2026-06-05T01:00:00Z\",\"type\":\"thread\",\"payload\":{\"thread_id\":\"thread-1\"}}\n",
            "{\"timestamp\":\"2026-06-05T01:00:01Z\",\"type\":\"message\",\"payload\":{\"role\":\"assistant\",\"content\":\"partial answer\"}}\n",
            "{\"timestamp\":\"2026-06-05T01:00:02Z\",\"type\":\"error\",\"payload\":{\"message\":\"Your workspace is out of credits. Ask your workspace owner to refill in order to continue.\"}}\n"
        ),
    )
    .unwrap();

    let goals_db = paths.shared_codex_root.join("goals_1.sqlite");
    let conn = rusqlite::Connection::open(&goals_db).unwrap();
    conn.execute_batch(
        "CREATE TABLE thread_goals (
            thread_id TEXT PRIMARY KEY,
            goal_id TEXT NOT NULL,
            objective TEXT NOT NULL,
            status TEXT NOT NULL,
            token_budget INTEGER,
            tokens_used INTEGER NOT NULL DEFAULT 0,
            time_used_seconds INTEGER NOT NULL DEFAULT 0,
            created_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL
        );",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO thread_goals (thread_id, goal_id, objective, status, created_at_ms, updated_at_ms) VALUES (?1, 'goal-1', 'finish work', 'paused', 1, 1)",
        rusqlite::params!["thread-1"],
    )
    .unwrap();

    let now = chrono::Local::now().timestamp();
    write_state(
        &root,
        AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([
                (
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: false,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: false,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                ),
            ]),
            session_profile_bindings: BTreeMap::from([(
                session_id.to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            )]),
            ..AppState::default()
        },
    );

    let strategy = RunCommandStrategy::new(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: true,
        full_access: false,
        base_url: None,
        no_proxy: true,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![OsString::from(session_id)],
    })
    .unwrap();

    let analysis = analyze_goal_resume_session(Path::new(&format!(
        "{}/sessions/2026/06/05/rollout-2026-06-05T01-00-00-{session_id}.jsonl",
        paths.shared_codex_root.display()
    )))
    .unwrap();
    assert_eq!(analysis.thread_id.as_deref(), Some("thread-1"));
    assert!(analysis.saw_usage_limit);
    assert!(shared_goal_needs_resume(&paths.shared_codex_root, "thread-1").unwrap());

    let plan = strategy.plan_goal_resume_relaunch(&exit_status(1)).unwrap();
    assert_eq!(
        plan,
        Some(GoalResumeRelaunchPlan {
            session_id: session_id.to_string(),
            profile_name: "second".to_string(),
        })
    );
}

#[test]
fn run_strategy_skips_goal_resume_relaunch_when_goal_is_complete() {
    let root = temp_dir("goal-resume-relaunch-complete");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_codex_home = root.join("shared-codex-home");
    let _shared_env = TestEnvVarGuard::set(
        "PRODEX_SHARED_CODEX_HOME",
        shared_codex_home.to_str().unwrap(),
    );
    let paths = AppPaths::discover().unwrap();
    let main_home = root.join("profiles").join("main");
    let second_home = root.join("profiles").join("second");
    fs::create_dir_all(&main_home).unwrap();
    fs::create_dir_all(&second_home).unwrap();
    fs::write(
        secret_store::auth_json_path(&main_home),
        r#"{"tokens":{"access_token":"main-token","account_id":"main-account"}}"#,
    )
    .unwrap();
    fs::write(
        secret_store::auth_json_path(&second_home),
        r#"{"tokens":{"access_token":"second-token","account_id":"second-account"}}"#,
    )
    .unwrap();
    let session_id = "019c9e3d-45a0-7ad0-a6ee-b194ac2d4500";
    let sessions = paths.shared_codex_root.join("sessions/2026/06/05");
    fs::create_dir_all(&sessions).unwrap();
    fs::write(
        sessions.join(format!("rollout-2026-06-05T01-00-00-{session_id}.jsonl")),
        concat!(
            "{\"timestamp\":\"2026-06-05T01:00:00Z\",\"type\":\"session_meta\",\"payload\":",
            "{\"id\":\"019c9e3d-45a0-7ad0-a6ee-b194ac2d4500\",\"cwd\":\"/tmp/test\"}}\n",
            "{\"timestamp\":\"2026-06-05T01:00:00Z\",\"type\":\"thread\",\"payload\":{\"thread_id\":\"thread-1\"}}\n",
            "{\"timestamp\":\"2026-06-05T01:00:02Z\",\"type\":\"error\",\"payload\":{\"message\":\"You've hit your usage limit. Try again later.\"}}\n"
        ),
    )
    .unwrap();
    let goals_db = paths.shared_codex_root.join("goals_1.sqlite");
    let conn = rusqlite::Connection::open(&goals_db).unwrap();
    conn.execute_batch(
        "CREATE TABLE thread_goals (
            thread_id TEXT PRIMARY KEY,
            goal_id TEXT NOT NULL,
            objective TEXT NOT NULL,
            status TEXT NOT NULL,
            token_budget INTEGER,
            tokens_used INTEGER NOT NULL DEFAULT 0,
            time_used_seconds INTEGER NOT NULL DEFAULT 0,
            created_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL
        );",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO thread_goals (thread_id, goal_id, objective, status, created_at_ms, updated_at_ms) VALUES (?1, 'goal-1', 'finish work', 'complete', 1, 1)",
        rusqlite::params!["thread-1"],
    )
    .unwrap();
    let now = chrono::Local::now().timestamp();
    write_state(
        &root,
        AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([
                (
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: false,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: false,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                ),
            ]),
            session_profile_bindings: BTreeMap::from([(
                session_id.to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            )]),
            ..AppState::default()
        },
    );

    let strategy = RunCommandStrategy::new(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: true,
        full_access: false,
        base_url: None,
        no_proxy: true,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![OsString::from(session_id)],
    })
    .unwrap();

    assert!(
        strategy
            .plan_goal_resume_relaunch(&exit_status(1))
            .unwrap()
            .is_none()
    );
}

#[test]
fn run_strategy_relaunch_after_child_exit_appends_goal_resume_and_releases_binding() {
    let root = temp_dir("goal-resume-relaunch-apply");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_codex_home = root.join("shared-codex-home");
    let _shared_env = TestEnvVarGuard::set(
        "PRODEX_SHARED_CODEX_HOME",
        shared_codex_home.to_str().unwrap(),
    );
    let paths = AppPaths::discover().unwrap();
    let main_home = root.join("profiles").join("main");
    let second_home = root.join("profiles").join("second");
    fs::create_dir_all(&main_home).unwrap();
    fs::create_dir_all(&second_home).unwrap();
    fs::write(
        secret_store::auth_json_path(&main_home),
        r#"{"tokens":{"access_token":"main-token","account_id":"main-account"}}"#,
    )
    .unwrap();
    fs::write(
        secret_store::auth_json_path(&second_home),
        r#"{"tokens":{"access_token":"second-token","account_id":"second-account"}}"#,
    )
    .unwrap();
    let session_id = "019c9e3d-45a0-7ad0-a6ee-b194ac2d4501";
    let sessions = paths.shared_codex_root.join("sessions/2026/06/05");
    fs::create_dir_all(&sessions).unwrap();
    fs::write(
        sessions.join(format!("rollout-2026-06-05T01-00-00-{session_id}.jsonl")),
        concat!(
            "{\"timestamp\":\"2026-06-05T01:00:00Z\",\"type\":\"session_meta\",\"payload\":",
            "{\"id\":\"019c9e3d-45a0-7ad0-a6ee-b194ac2d4501\",\"cwd\":\"/tmp/test\"}}\n",
            "{\"timestamp\":\"2026-06-05T01:00:00Z\",\"type\":\"thread\",\"payload\":{\"thread_id\":\"thread-1\"}}\n",
            "{\"timestamp\":\"2026-06-05T01:00:02Z\",\"type\":\"error\",\"payload\":{\"message\":\"You've hit your usage limit. Try again later.\"}}\n"
        ),
    )
    .unwrap();
    let goals_db = paths.shared_codex_root.join("goals_1.sqlite");
    let conn = rusqlite::Connection::open(&goals_db).unwrap();
    conn.execute_batch(
        "CREATE TABLE thread_goals (
            thread_id TEXT PRIMARY KEY,
            goal_id TEXT NOT NULL,
            objective TEXT NOT NULL,
            status TEXT NOT NULL,
            token_budget INTEGER,
            tokens_used INTEGER NOT NULL DEFAULT 0,
            time_used_seconds INTEGER NOT NULL DEFAULT 0,
            created_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL
        );",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO thread_goals (thread_id, goal_id, objective, status, created_at_ms, updated_at_ms) VALUES (?1, 'goal-1', 'finish work', 'paused', 1, 1)",
        rusqlite::params!["thread-1"],
    )
    .unwrap();
    let now = chrono::Local::now().timestamp();
    write_state(
        &root,
        AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([
                (
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: false,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: false,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                ),
            ]),
            session_profile_bindings: BTreeMap::from([(
                session_id.to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            )]),
            ..AppState::default()
        },
    );

    let mut strategy = RunCommandStrategy::new(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: true,
        full_access: false,
        base_url: None,
        no_proxy: true,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![OsString::from(session_id)],
    })
    .unwrap();

    assert!(strategy.relaunch_after_child_exit(&exit_status(1)).unwrap());
    assert_eq!(strategy.args.profile.as_deref(), Some("second"));
    assert!(codex_args_include_goal_resume(&strategy.codex_args));

    let state = AppState::load(&paths).unwrap();
    assert!(!state.session_profile_bindings.contains_key(session_id));
}

#[test]
fn codex_delete_cleanup_prunes_session_and_compact_bindings() {
    let root = temp_dir("delete-prune-bindings");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let paths = AppPaths::discover().unwrap();
    let session_id = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9";
    let compact_key = prodex_runtime_store::runtime_compact_session_lineage_key(session_id);
    let now = chrono::Local::now().timestamp();
    write_state(
        &root,
        AppState {
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: root.join("main-home"),
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            session_profile_bindings: BTreeMap::from([
                (
                    session_id.to_string(),
                    ResponseProfileBinding {
                        profile_name: "main".to_string(),
                        bound_at: now,
                    },
                ),
                (
                    compact_key.clone(),
                    ResponseProfileBinding {
                        profile_name: "main".to_string(),
                        bound_at: now,
                    },
                ),
            ]),
            ..AppState::default()
        },
    );

    cleanup_codex_deleted_session_binding(Some(session_id)).unwrap();

    let state = AppState::load(&paths).unwrap();
    assert!(!state.session_profile_bindings.contains_key(session_id));
    assert!(!state.session_profile_bindings.contains_key(&compact_key));
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
        format!(
            "{{\"timestamp\":\"2026-06-05T01:00:00Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"{session_id}\",\"cwd\":\"{}\",\"model_provider\":\"prodex-gemini\"}}}}\n",
            root.display()
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
        format!(
            "{{\"timestamp\":\"2026-06-05T01:00:00Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"{session_id}\",\"cwd\":\"{}\",\"model_provider\":\"prodex-kiro\"}}}}\n",
            root.display()
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
        format!(
            "{{\"timestamp\":\"2026-06-05T01:00:00Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"{session_id}\",\"cwd\":\"{}\",\"model_provider\":\"prodex-gemini\"}}}}\n",
            root.display()
        ),
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
            "{{\"timestamp\":\"2026-06-05T00:59:00Z\",\"type\":\"event\",\"payload\":{{\"message\":\"partial\"}}}}\n{{\"timestamp\":\"2026-06-05T01:00:00Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"{session_id}\",\"cwd\":\"{}\",\"model_provider\":\"prodex-gemini\"}}}}\n",
            root.display()
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
    fs::write(
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

    let strategy = RunCommandStrategy::new(RunArgs {
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
    fs::write(
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

    let strategy = RunCommandStrategy::new(RunArgs {
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
        format!(
            "{{\"timestamp\":\"2026-06-05T01:00:00Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"{session_id}\",\"cwd\":\"{}\",\"model_provider\":\"prodex-gemini\"}}}}\n",
            root.display()
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

#[test]
fn runtime_launch_parses_model_context_window_override() {
    assert_eq!(
        runtime_launch_cli_model_context_window_tokens(&[
            OsString::from("-c"),
            OsString::from("model_context_window=65536"),
        ]),
        Some(65_536)
    );
}

#[test]
fn runtime_launch_reads_profile_v2_model_context_window_overlay() {
    let root = temp_dir("profile-v2-context-window");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("config.toml"), "model_context_window = 8192\n").unwrap();
    fs::write(
        root.join("local.config.toml"),
        "model_context_window = 65536\n",
    )
    .unwrap();

    assert!(
        codex_profile_v2_config_path(&root, "local")
            .unwrap()
            .exists()
    );
    assert_eq!(
        runtime_launch_config_model_context_window_tokens(&root),
        Some(8192)
    );
    assert_eq!(
        runtime_launch_config_model_context_window_tokens_with_profile_v2(&root, Some("local")),
        Some(65_536)
    );
    assert_eq!(
        runtime_launch_config_model_context_window_tokens_with_profile_v2(&root, Some("missing")),
        Some(8192)
    );
}
