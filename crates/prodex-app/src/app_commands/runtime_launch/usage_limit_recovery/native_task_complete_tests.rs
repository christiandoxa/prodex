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
            "type":"event_msg", "payload":{"type":"task_started", "turn_id":"turn-fixture"}
        }),
    );
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

#[test]
fn native_task_started_prevents_recovery_using_previous_turn_side_effects() {
    let root = fixture_root("native-start-evidence");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let session_path = populate_fixture(&root, &paths, &["a", "b"]);
    blocked_goal(&paths);
    let mut monitor = prepare_goal_usage_limit_monitor(
        &[
            OsString::from("exec"),
            OsString::from("resume"),
            OsString::from(SESSION_ID),
        ],
        false,
    )
    .unwrap()
    .unwrap();
    append_user_event(&session_path);
    append_record(
        &session_path,
        serde_json::json!({
            "type":"response_item","payload":{"type":"custom_tool_call_output","call_id":"previous-completed-tool","output":"already done"}
        }),
    );
    assert_eq!(monitor.take_usage_limit_signal().unwrap(), None);
    assert_eq!(monitor.recovery_evidence().side_effect_state, "observed");
    append_record(
        &session_path,
        serde_json::json!({
            "type":"event_msg","payload":{"type":"task_started","turn_id":"new-unaccepted-turn"}
        }),
    );
    append_record(
        &session_path,
        serde_json::json!({
            "type":"event_msg","payload":{"type":"task_complete","turn_id":"new-unaccepted-turn",
                "error":{"codex_error_info":"server_overloaded","message":"synthetic capacity error"}}
        }),
    );
    assert_eq!(
        monitor.take_usage_limit_signal().unwrap(),
        None,
        "a previous turn's completed tool must not authorize replay of an unaccepted new turn"
    );
    assert_eq!(
        monitor.recovery_evidence(),
        RuntimeWorkflowEvidence::default()
    );
    drop(monitor);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn large_resumed_rollout_recovers_capacity_after_compaction_without_replaying_history() {
    let root = fixture_root("large-rollout-compact-capacity");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let session_path = populate_fixture(&root, &paths, &["a", "b"]);
    blocked_goal(&paths);
    // A long-lived rollout exceeds the whole-document loading limit. Each
    // record stays bounded and valid, as in real persisted tool histories.
    let padding = serde_json::json!({
        "type":"event_msg", "payload":{"type":"historical_padding", "value":"x".repeat(64 * 1024)}
    });
    {
        let mut file = std::io::BufWriter::new(
            fs::OpenOptions::new()
                .append(true)
                .open(&session_path)
                .unwrap(),
        );
        let line = format!("{padding}\n");
        for _ in 0..1025 {
            file.write_all(line.as_bytes()).unwrap();
        }
        file.flush().unwrap();
    }
    append_native_failure(&session_path);
    let baseline_len = fs::metadata(&session_path).unwrap().len();
    assert!(baseline_len > 64 * 1024 * 1024);
    let mut monitor = GoalUsageLimitMonitor::new(
        paths.clone(),
        Some(paths.shared_codex_root.join("goals_1.sqlite")),
        root.join("marker"),
        Some(SESSION_ID.to_string()),
    );
    assert_eq!(monitor.session_scan_offset, baseline_len);
    assert_eq!(monitor.take_usage_limit_signal().unwrap(), None);
    append_record(
        &session_path,
        serde_json::json!({
            "type":"event_msg","payload":{"type":"task_started","turn_id":"new-turn"}
        }),
    );
    append_user_event(&session_path);
    append_record(
        &session_path,
        serde_json::json!({
            "type":"response_item","payload":{"type":"custom_tool_call_output","call_id":"done-before-compact","output":"synthetic completed work"}
        }),
    );
    append_record(
        &session_path,
        serde_json::json!({
            "type":"compacted","payload":{"message":"synthetic compacted history","replacement_history":[]}
        }),
    );
    append_record(
        &session_path,
        serde_json::json!({
            "type":"event_msg","payload":{"type":"context_compacted"}
        }),
    );
    append_user_event(&session_path);
    append_record(
        &session_path,
        serde_json::json!({
            "type":"event_msg","payload":{"type":"task_complete","turn_id":"new-turn","root_turn_id":"new-turn",
            "error":{"message":"Selected model is at capacity. Please try a different model.","codex_error_info":"server_overloaded"}}
        }),
    );
    assert_eq!(
        monitor.take_usage_limit_signal().unwrap().as_deref(),
        Some(SESSION_ID)
    );
    assert_eq!(monitor.recovery_failure_class(), "overload");
    let attempted = BTreeSet::new();
    let plan = next_observed_runtime_recovery_plan(
        &monitor,
        &resume_options(&attempted, false, Some("a")),
    )
    .unwrap()
    .unwrap();
    assert_eq!(plan.profile_name, "b");
    assert_eq!(plan.session_id, SESSION_ID);
    assert!(plan.resume_goal);
    assert!(
        runtime_session_recovery_message(&plan, 1, None, None)
            .contains("last_prompt_requeued=false")
    );
    monitor.prepare_for_resume();
    assert_eq!(monitor.take_usage_limit_signal().unwrap(), None);
    drop(monitor);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_monitor_recovers_after_transient_session_read_failure() {
    let root = fixture_root("transient-rollout-read");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let session_path = populate_fixture(&root, &paths, &["a", "b"]);
    let mut monitor = GoalUsageLimitMonitor::new(
        paths,
        None,
        root.join("marker"),
        Some(SESSION_ID.to_string()),
    );
    assert_eq!(monitor.take_usage_limit_signal().unwrap(), None);
    let original_offset = monitor.session_scan_offset;
    let held = root.join("held-rollout.jsonl");
    fs::rename(&session_path, &held).unwrap();
    fs::create_dir(&session_path).unwrap();
    assert_eq!(monitor.take_usage_limit_signal().unwrap(), None);
    assert!(monitor.workflow_scan_retry_at.is_some());
    assert_eq!(monitor.session_scan_offset, original_offset);
    fs::remove_dir(&session_path).unwrap();
    fs::rename(&held, &session_path).unwrap();
    append_native_failure(&session_path);
    assert_eq!(
        monitor.take_usage_limit_signal().unwrap(),
        None,
        "retry backoff prevents hot polling after an I/O failure"
    );
    monitor.workflow_scan_retry_at = Some(Instant::now());
    assert_eq!(
        monitor.take_usage_limit_signal().unwrap().as_deref(),
        Some(SESSION_ID)
    );
    assert_eq!(monitor.recovery_failure_class(), "overload");
    assert!(monitor.workflow_scan_retry_at.is_none());
    drop(monitor);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_native_retry_notification_does_not_preempt_the_running_frontend() {
    let root = fixture_root("live-native-intermediate-error");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let session = populate_fixture(&root, &paths, &["a", "b"]);
    let mut monitor = GoalUsageLimitMonitor::new(
        paths,
        None,
        root.join("marker"),
        Some(SESSION_ID.to_string()),
    );
    append_user_event(&session);
    append_record(
        &session,
        serde_json::json!({
            "type":"event_msg", "payload":{"type":"error", "codex_error_info":"server_overloaded", "will_retry":true}
        }),
    );
    assert_eq!(monitor.take_usage_limit_signal().unwrap(), None);
    assert!(!monitor.usage_limit_pending);
    drop(monitor);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn live_native_terminal_failure_followed_by_progress_in_same_tail_is_not_replayed() {
    let root = fixture_root("live-native-tail-chronology");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let session = populate_fixture(&root, &paths, &["a", "b"]);
    blocked_goal(&paths);
    let mut monitor = GoalUsageLimitMonitor::new(
        paths.clone(),
        Some(paths.shared_codex_root.join("goals_1.sqlite")),
        root.join("marker"),
        Some(SESSION_ID.to_string()),
    );
    append_native_failure(&session);
    append_record(
        &session,
        serde_json::json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"new-native-attempt"}}),
    );
    append_user_event(&session);
    assert_eq!(monitor.take_usage_limit_signal().unwrap(), None);
    assert!(!monitor.usage_limit_pending);
    assert_eq!(
        monitor.session_scan_offset,
        fs::metadata(&session).unwrap().len()
    );
    drop(monitor);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn live_native_new_activity_clears_latched_failure_and_stale_goal_revision() {
    let root = fixture_root("live-native-stale-pending");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let session = populate_fixture(&root, &paths, &["a", "b"]);
    blocked_goal(&paths);
    let mut monitor = GoalUsageLimitMonitor::new(
        paths.clone(),
        Some(paths.shared_codex_root.join("goals_1.sqlite")),
        root.join("marker"),
        Some(SESSION_ID.to_string()),
    );
    append_native_failure(&session);
    assert_eq!(
        monitor.take_usage_limit_signal().unwrap().as_deref(),
        Some(SESSION_ID)
    );
    let db = rusqlite::Connection::open(paths.shared_codex_root.join("goals_1.sqlite")).unwrap();
    db.execute(
        "UPDATE thread_goals SET status='usage_limited', updated_at_ms=?1",
        [current_unix_time_millis() + 1000],
    )
    .unwrap();
    append_record(
        &session,
        serde_json::json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"native-retry"}}),
    );
    append_user_event(&session);
    monitor.next_retry_at = Instant::now();
    assert_eq!(monitor.take_usage_limit_signal().unwrap(), None);
    monitor.next_retry_at = Instant::now();
    assert_eq!(
        monitor.take_usage_limit_signal().unwrap(),
        None,
        "same stale goal revision must not re-arm"
    );
    drop(monitor);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn in_place_ack_consumes_failure_without_skipping_concurrent_native_events() {
    let root = fixture_root("in-place-ack-cursor");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let session = populate_fixture(&root, &paths, &["a", "b"]);
    let mut monitor = GoalUsageLimitMonitor::new(
        paths,
        None,
        root.join("marker"),
        Some(SESSION_ID.to_string()),
    );
    append_native_failure(&session);
    assert_eq!(
        monitor.take_usage_limit_signal().unwrap().as_deref(),
        Some(SESSION_ID)
    );
    let cursor = monitor.session_scan_offset;
    append_record(
        &session,
        serde_json::json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"native-accepted"}}),
    );
    append_user_event(&session);
    monitor.acknowledge_in_place_recovery();
    assert_eq!(
        monitor.session_scan_offset, cursor,
        "RPC acknowledgement must not seek beyond concurrent events"
    );
    assert_eq!(monitor.take_usage_limit_signal().unwrap(), None);
    assert_eq!(
        monitor.workflow_evidence.acceptance_state,
        "accepted_but_uncommitted"
    );
    assert!(
        !monitor.session_usage_limit_reported,
        "new activity opens a new recovery generation"
    );
    assert_eq!(monitor.take_usage_limit_signal().unwrap(), None);
    drop(monitor);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn foreign_session_activity_cannot_authorize_or_cancel_native_recovery() {
    let root = fixture_root("live-native-other-session");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let session = populate_fixture(&root, &paths, &["a", "b"]);
    let mut monitor = GoalUsageLimitMonitor::new(
        paths,
        None,
        root.join("marker"),
        Some(SESSION_ID.to_string()),
    );
    append_native_failure(&session);
    append_record(
        &session,
        serde_json::json!({"type":"event_msg","session_id":"other-session","payload":{"type":"task_started"}}),
    );
    assert_eq!(
        monitor.take_usage_limit_signal().unwrap().as_deref(),
        Some(SESSION_ID)
    );
    assert_eq!(monitor.workflow_evidence.side_effect_state, "observed");
    drop(monitor);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn paused_goal_cancels_a_pending_live_recovery_before_another_poll() {
    let root = fixture_root("native-recovery-explicit-pause");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let session = populate_fixture(&root, &paths, &["a", "b"]);
    blocked_goal(&paths);
    let db_path = paths.shared_codex_root.join("goals_1.sqlite");
    let mut monitor = GoalUsageLimitMonitor::new(
        paths,
        Some(db_path.clone()),
        root.join("marker"),
        Some(SESSION_ID.to_string()),
    );
    append_native_failure(&session);
    assert_eq!(
        monitor.take_usage_limit_signal().unwrap().as_deref(),
        Some(SESSION_ID)
    );
    let db = rusqlite::Connection::open(db_path).unwrap();
    db.execute(
        "UPDATE thread_goals SET status='paused', updated_at_ms=100",
        [],
    )
    .unwrap();
    monitor.next_retry_at = Instant::now();
    assert_eq!(monitor.take_usage_limit_signal().unwrap(), None);
    assert!(!monitor.usage_limit_pending);
    drop(monitor);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn terminal_timeout_after_compaction_arms_one_in_place_recovery_without_replaying_work() {
    let root = fixture_root("native-compact-timeout");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let session_path = populate_fixture(&root, &paths, &["a", "b"]);
    blocked_goal(&paths);
    let mut monitor = GoalUsageLimitMonitor::new(
        paths.clone(),
        Some(paths.shared_codex_root.join("goals_1.sqlite")),
        root.join("marker"),
        Some(SESSION_ID.to_string()),
    );
    append_user_event(&session_path);
    append_record(
        &session_path,
        serde_json::json!({"type":"response_item","payload":{
            "type":"custom_tool_call_output","call_id":"finished-before-compact","output":"done"
        }}),
    );
    append_record(
        &session_path,
        serde_json::json!({"type":"compacted","payload":{
            "message":"synthetic summary", "replacement_history":[]
        }}),
    );
    append_record(
        &session_path,
        serde_json::json!({"type":"event_msg","payload":{
            "type":"error","message":"request timed out","codex_error_info":"other"
        }}),
    );
    assert_eq!(
        monitor.take_usage_limit_signal().unwrap(),
        None,
        "a retryable notice is not a completed turn and must not race native compaction"
    );
    append_record(
        &session_path,
        serde_json::json!({"type":"event_msg","payload":{
            "type":"task_complete", "error":{
                "message":"request timed out", "codex_error_info":"other"
            }
        }}),
    );
    assert_eq!(
        monitor.take_usage_limit_signal().unwrap().as_deref(),
        Some(SESSION_ID)
    );
    assert_eq!(monitor.recovery_failure_class(), "transport");
    assert!(monitor.recovery_retries_after_pool_round());
    assert_eq!(monitor.recovery_evidence().side_effect_state, "observed");
    monitor.acknowledge_in_place_recovery();
    assert_eq!(
        monitor.take_usage_limit_signal().unwrap(),
        None,
        "acknowledged timeout must not cause duplicate recovery or repeat completed tool calls"
    );
    drop(monitor);
    let _ = fs::remove_dir_all(root);
}

#[path = "session_marker_tests.rs"]
mod session_marker_tests;
