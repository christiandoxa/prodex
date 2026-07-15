use super::*;
use std::path::Path;

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
    write_runtime_launch_auth(
        secret_store::auth_json_path(&main_home),
        r#"{"tokens":{"access_token":"main-token","account_id":"main-account"}}"#,
    )
    .unwrap();
    write_runtime_launch_auth(
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
            "{\"timestamp\":\"2026-06-05T01:00:00Z\",\"type\":\"thread\",\"payload\":{\"thread_id\":\"019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9\"}}\n",
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
        rusqlite::params![session_id],
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
                        codex_home: main_home.clone(),
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
    assert_eq!(analysis.thread_id.as_deref(), Some(session_id));
    assert!(analysis.saw_usage_limit);
    assert!(shared_goal_needs_resume(&paths.shared_codex_root, session_id).unwrap());

    let plan = strategy.plan_goal_resume_relaunch(&exit_status(1)).unwrap();
    assert_eq!(
        plan,
        Some(GoalResumeRelaunchPlan {
            session_id: session_id.to_string(),
            profile_name: "second".to_string(),
        })
    );

    let mut fresh_strategy = RunCommandStrategy::new(RunArgs {
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
        codex_args: Vec::new(),
    })
    .unwrap();
    let marker_path = fresh_strategy
        .goal_usage_limit_monitor
        .as_ref()
        .unwrap()
        .marker_path
        .clone();
    write_runtime_goal_session_marker(
        &marker_path,
        std::ffi::OsStr::new(&format!(r#"{{"thread-id":"{session_id}"}}"#)),
    )
    .unwrap();
    let mut notify_args = Vec::new();
    add_runtime_goal_session_notify(&main_home, None, &mut notify_args, &marker_path);
    assert_eq!(notify_args.first(), Some(&OsString::from("-c")));
    assert!(notify_args.get(1).is_some_and(|arg| {
        arg.to_string_lossy()
            .contains(RUNTIME_GOAL_SESSION_NOTIFY_COMMAND)
    }));
    conn.execute(
        "UPDATE thread_goals SET status = 'active', updated_at_ms = 2 WHERE thread_id = ?1",
        [session_id],
    )
    .unwrap();
    assert!(!fresh_strategy.child_exit_requested());
    conn.execute(
        "UPDATE thread_goals SET status = 'usage_limited', updated_at_ms = 3 WHERE thread_id = ?1",
        [session_id],
    )
    .unwrap();
    assert!(fresh_strategy.child_exit_requested());
    assert!(!fresh_strategy.child_exit_requested());
    assert_eq!(
        fresh_strategy.pending_goal_resume_plan,
        Some(GoalResumeRelaunchPlan {
            session_id: session_id.to_string(),
            profile_name: "second".to_string(),
        })
    );
    assert!(
        fresh_strategy
            .relaunch_after_child_exit(&exit_status(0))
            .unwrap()
    );
    assert_eq!(fresh_strategy.resume_session_id(), Some(session_id));
    assert_eq!(fresh_strategy.session_affinity_release(), Some(session_id));
    assert!(codex_args_include_goal_resume(&fresh_strategy.codex_args));
    assert_eq!(
        fresh_strategy.codex_args.last(),
        Some(&OsString::from("/goal resume"))
    );
}
