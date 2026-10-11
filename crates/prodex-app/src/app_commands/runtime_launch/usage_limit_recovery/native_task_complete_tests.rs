use super::*;

fn append_record(path: &Path, value: serde_json::Value) {
    writeln!(
        fs::OpenOptions::new().append(true).open(path).unwrap(),
        "{value}"
    )
    .unwrap();
}

fn blocked_goal(paths: &AppPaths) {
    let conn = rusqlite::Connection::open(paths.shared_codex_root.join("goals_1.sqlite")).unwrap();
    conn.execute_batch("CREATE TABLE thread_goals (thread_id TEXT PRIMARY KEY, status TEXT NOT NULL, updated_at_ms INTEGER NOT NULL);").unwrap();
    conn.execute(
        "INSERT INTO thread_goals VALUES (?1, 'blocked', 1)",
        [SESSION_ID],
    )
    .unwrap();
}

fn append_native_failure(path: &Path) {
    append_record(
        path,
        serde_json::json!({
            "type":"turn_context", "payload":{"model":"gpt-6-luna"}
        }),
    );
    append_user_event(path);
    append_record(
        path,
        serde_json::json!({
            "type":"response_item", "payload":{"type":"custom_tool_call_output", "call_id":"already-done", "output":"synthetic completed work"}
        }),
    );
    append_record(
        path,
        serde_json::json!({
            "type":"event_msg", "payload":{
                "type":"task_complete", "root_turn_id":"turn-fixture", "turn_id":"turn-fixture",
                "last_agent_message":null,
                "error":{"message":"Selected model is at capacity. Please try a different model.", "codex_error_info":"server_overloaded"}
            }
        }),
    );
}

#[test]
fn native_task_complete_unblocks_live_goal_recovery_and_preserves_completed_work() {
    let root = fixture_root("native-completion-live");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let session_path = populate_fixture(&root, &paths, &["a", "b", "c"]);
    blocked_goal(&paths);
    let mut monitor =
        prepare_goal_usage_limit_monitor(&[OsString::from("exec"), OsString::from("work")], false)
            .unwrap()
            .unwrap();
    super::super::super::goal_resume::write_runtime_goal_session_marker(
        &monitor.marker_path,
        std::ffi::OsStr::new(&format!(r#"{{"session_id":"{SESSION_ID}"}}"#)),
    )
    .unwrap();
    assert_eq!(monitor.take_usage_limit_signal().unwrap(), None);
    append_native_failure(&session_path);
    assert_eq!(
        monitor.take_usage_limit_signal().unwrap().as_deref(),
        Some(SESSION_ID),
        "a native terminal overload must arm recovery even when the goal is blocked"
    );
    assert_eq!(monitor.recovery_failure_class(), "overload");
    assert!(monitor.recovery_retries_after_pool_round());
    assert_eq!(monitor.recovery_model(), Some("gpt-6-luna"));
    let attempted = BTreeSet::new();
    let plan = next_observed_runtime_recovery_plan(
        &monitor,
        &resume_options(&attempted, false, Some("a")),
    )
    .unwrap()
    .unwrap();
    assert_eq!(plan.failed_profile_name, "a");
    assert_eq!(plan.profile_name, "b");
    assert_eq!(plan.session_id, SESSION_ID);
    assert_eq!(plan.failure_class, "overload");
    assert!(plan.resume_goal);
    assert_eq!(plan.evidence.side_effect_state, "observed");
    assert_eq!(plan.evidence.stream_committed, Some(true));
    let state = AppState::load_and_repair(&paths).unwrap();
    assert_eq!(
        state.active_profile.as_deref(),
        Some("a"),
        "planning must not mutate accounts"
    );
    let telemetry =
        runtime_session_recovery_message(&plan, 1, Some("gpt-6-luna"), monitor.recovery_model());
    assert!(telemetry.contains("last_prompt_requeued=false"));
    monitor.prepare_for_resume();
    assert_eq!(
        monitor.take_usage_limit_signal().unwrap(),
        None,
        "old terminal record must not trigger another relaunch"
    );
    drop(monitor);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn native_task_complete_post_exit_recovery_rotates_but_respects_cancellation_and_opt_out() {
    for (label, code, disabled, expected) in [
        ("failed", 1, false, Some("b")),
        ("cancelled", 130, false, None),
        ("disabled", 1, true, None),
        ("user-exit", 0, false, None),
    ] {
        let root = fixture_root(label);
        let (_home, _shared) = configured_env(&root);
        let paths = AppPaths::discover().unwrap();
        let session_path = populate_fixture(&root, &paths, &["a", "b"]);
        let mut monitor = GoalUsageLimitMonitor::new(
            paths.clone(),
            None,
            root.join("marker"),
            Some(SESSION_ID.to_string()),
        );
        append_native_failure(&session_path);
        let attempted = BTreeSet::new();
        let plan = plan_runtime_usage_limit_relaunch(
            &mut monitor,
            &exit_status(code),
            &resume_options(&attempted, disabled, Some("a")),
        )
        .unwrap();
        assert_eq!(
            plan.as_ref().map(|p| p.profile_name.as_str()),
            expected,
            "{label}"
        );
        drop(monitor);
        let _ = fs::remove_dir_all(root);
    }
}
