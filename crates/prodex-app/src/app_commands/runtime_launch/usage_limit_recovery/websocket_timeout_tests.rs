use super::*;

const MESSAGE: &str = "stream disconnected before completion: idle timeout waiting for websocket";

fn append(path: &Path, record: serde_json::Value) {
    writeln!(
        fs::OpenOptions::new().append(true).open(path).unwrap(),
        "{record}"
    )
    .unwrap();
}

#[test]
fn terminal_websocket_timeout_recovers_once_but_never_replays_intermediate_or_paused_turns() {
    for status in ["blocked", "paused", "complete", "budget_limited"] {
        let root = fixture_root("websocket-idle-terminal");
        let (_home, _shared) = configured_env(&root);
        let paths = AppPaths::discover().unwrap();
        let session = populate_fixture(&root, &paths, &["a", "b"]);
        let db_path = paths.shared_codex_root.join("goals_1.sqlite");
        let db = rusqlite::Connection::open(&db_path).unwrap();
        db.execute_batch("CREATE TABLE thread_goals (thread_id TEXT PRIMARY KEY, status TEXT NOT NULL, updated_at_ms INTEGER NOT NULL);").unwrap();
        db.execute(
            "INSERT INTO thread_goals VALUES (?1, ?2, 1)",
            [SESSION_ID, status],
        )
        .unwrap();
        drop(db);
        let mut monitor = GoalUsageLimitMonitor::new(
            paths,
            Some(db_path),
            root.join("owned.id"),
            Some(SESSION_ID.to_string()),
        );
        append_user_event(&session);
        append(
            &session,
            serde_json::json!({"type":"response_item", "payload":{
                "type":"custom_tool_call_output", "call_id":"done-once", "output":"synthetic completed work"
            }}),
        );
        append(
            &session,
            serde_json::json!({"type":"event_msg", "payload":{
                "type":"error", "message":MESSAGE, "codex_error_info":"other", "will_retry":true
            }}),
        );
        assert_eq!(
            monitor.take_usage_limit_signal().unwrap(),
            None,
            "never interrupt an intermediate native retry"
        );
        append(
            &session,
            serde_json::json!({"type":"event_msg", "payload":{
                "type":"task_complete", "turn_id":"synthetic-turn", "error":{
                    "message":MESSAGE, "codex_error_info":"other"
                }
            }}),
        );
        let signal = monitor.take_usage_limit_signal().unwrap();
        assert_eq!(
            signal.as_deref(),
            (status == "blocked").then_some(SESSION_ID),
            "{status}"
        );
        if status == "blocked" {
            assert_eq!(monitor.recovery_failure_class(), "transport");
            assert_eq!(monitor.recovery_evidence().side_effect_state, "observed");
            monitor.acknowledge_in_place_recovery();
            assert_eq!(
                monitor.take_usage_limit_signal().unwrap(),
                None,
                "acknowledged failures are not replayed"
            );
        }
        drop(monitor);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn websocket_timeout_envelopes_reject_cancellation_foreign_identity_and_quoted_user_text() {
    let error = serde_json::json!({"message":MESSAGE, "codex_error_info":"other"});
    for record in [
        serde_json::json!({"type":"task_complete","status":"cancelled","error":error}),
        serde_json::json!({"type":"task_complete","status":"completed","error":error}),
        serde_json::json!({"type":"task_complete","session_id":"foreign","error":error}),
        serde_json::json!({"type":"response_item","payload":{"type":"message","role":"user","content":MESSAGE}}),
    ] {
        assert_eq!(runtime_workflow_recovery_class(&record, SESSION_ID), None);
    }
    let completed = serde_json::json!({"method":"turn/completed","params":{"threadId":SESSION_ID,"turn":{
        "status":"failed","error":{"message":MESSAGE,"codexErrorInfo":"other"}
    }}});
    assert_eq!(
        runtime_workflow_recovery_class(&completed, SESSION_ID),
        Some(RuntimeWorkflowRecoveryClass::Transport)
    );
}
