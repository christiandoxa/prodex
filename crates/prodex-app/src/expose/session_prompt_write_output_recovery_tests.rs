use super::super::session_prompt_write::{
    ExistingSessionPromptWrite, OpenProcessFile, ProcessDetails, PromptOutputReadRequest,
    QueueRequestOutcome, SessionPromptWriteError, SessionPromptWriteService, output_source_id,
};
use super::{FakeProcessInspector, fixture, process, queue, request, service};
use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

#[test]
fn incomplete_final_append_waits_for_newline_before_advancing() {
    let fixture = fixture();
    let service = service(&fixture, queue(&fixture, None));
    let first = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: None,
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "partial".to_string(),
            shutdown: None,
        })
        .unwrap();
    let partial = serde_json::json!({
        "timestamp": "2026-09-03T10:00:04Z",
        "type": "event_msg",
        "payload": {"type": "agent_message", "message": "partial"}
    })
    .to_string();
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&fixture.rollout)
        .unwrap();
    file.write_all(partial.as_bytes()).unwrap();

    let pending = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: Some(first.next_cursor.clone()),
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "partial".to_string(),
            shutdown: None,
        })
        .unwrap();
    assert!(pending.events.is_empty());
    assert!(pending.has_more);
    assert_eq!(pending.next_cursor, first.next_cursor);

    file.write_all(b"\n").unwrap();
    let completed = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: Some(pending.next_cursor),
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "partial".to_string(),
            shutdown: None,
        })
        .unwrap();
    assert_eq!(completed.events[0].text, "partial");
}

#[test]
fn accepted_write_omits_cursor_when_the_rollout_source_was_replaced() {
    let fixture = fixture();
    let source_id_before = output_source_id(&fixture.rollout, super::THREAD).unwrap();
    let mut queue_control = queue(&fixture, None);
    queue_control.invocation.queued = true;
    queue_control.replace_rollout_on_queue = true;

    let result = service(&fixture, queue_control)
        .write(request(&fixture, "accepted before source replacement"))
        .expect("an already accepted write must not fail with its optional cursor");

    assert_eq!(result.verification, "queue_pending_observed");
    let source_id_after = output_source_id(&fixture.rollout, super::THREAD).unwrap();
    assert_ne!(source_id_before, source_id_after);
    assert!(result.output_cursor.is_none());
}

#[test]
fn ambiguous_queue_submission_is_reported_without_replay() {
    let fixture = fixture();
    let mut queue_control = queue(&fixture, None);
    queue_control.invocation.outcome = QueueRequestOutcome::Ambiguous;
    let calls = Arc::clone(&queue_control.calls);

    assert_eq!(
        service(&fixture, queue_control)
            .write(request(&fixture, "may have been accepted"))
            .unwrap_err(),
        SessionPromptWriteError::WriteAmbiguous
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[test]
fn cursor_read_does_not_replace_the_implicit_target_binding() {
    const OTHER_THREAD: &str = "019f3b59-7771-7ea1-a9a1-3cd638f216d0";
    let fixture = fixture();
    let other_home = fixture.root.join("other-codex-home");
    let other_sqlite = fixture.root.join("other-sqlite-home");
    let other_rollout = other_home
        .join("sessions/2026/09/03")
        .join(format!("rollout-{OTHER_THREAD}.jsonl"));
    std::fs::create_dir_all(other_rollout.parent().unwrap()).unwrap();
    std::fs::create_dir_all(&other_sqlite).unwrap();
    std::fs::write(
        &other_rollout,
        "{\"timestamp\":\"2026-09-03T10:00:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"agent_message\",\"message\":\"other output\"}}\n",
    )
    .unwrap();
    let other_queue = other_sqlite.join("queue_1.sqlite");
    let other_state = other_sqlite.join("state_5.sqlite");
    std::fs::write(&other_queue, []).unwrap();
    std::fs::write(&other_state, []).unwrap();
    let other_prodex = process(
        101,
        1,
        "/usr/bin/prodex",
        vec!["prodex", "s"],
        &fixture.workspace,
        11,
    );
    let other_writer = process(
        201,
        101,
        "/usr/bin/codex",
        vec!["codex"],
        &fixture.workspace,
        21,
    );
    let other_details = ProcessDetails {
        record: other_writer.clone(),
        environment: BTreeMap::from([
            ("HOME".to_string(), "/home/test-user".to_string()),
            ("CODEX_HOME".to_string(), other_home.display().to_string()),
            (
                "CODEX_SQLITE_HOME".to_string(),
                other_sqlite.display().to_string(),
            ),
            ("PWD".to_string(), fixture.workspace.display().to_string()),
        ]),
        open_files: vec![
            OpenProcessFile {
                path: other_home
                    .join("thread-writer-locks")
                    .join(format!("{OTHER_THREAD}.lock")),
            },
            OpenProcessFile { path: other_queue },
            OpenProcessFile { path: other_state },
        ],
    };
    let mut records = fixture.records.clone();
    records.extend([other_prodex, other_writer]);
    let process = FakeProcessInspector {
        uid: 1000,
        records,
        details: fixture.writer.clone(),
        details_by_pid: HashMap::from([(201, other_details)]),
        changed_records: None,
        lists: AtomicUsize::new(0),
    };
    let mut queue_control = queue(&fixture, None);
    queue_control.rollout = None;
    let service = SessionPromptWriteService::with_adapters(process, queue_control);

    let first = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: None,
            limit: 10,
            wait_ms: 0,
            prodex_pid: Some(100),
            thread_id: None,
            binding_key: "implicit".to_string(),
            shutdown: None,
        })
        .unwrap();
    let other = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: None,
            limit: 10,
            wait_ms: 0,
            prodex_pid: Some(101),
            thread_id: Some(OTHER_THREAD.to_string()),
            binding_key: "other".to_string(),
            shutdown: None,
        })
        .unwrap();
    service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: Some(other.next_cursor),
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "implicit".to_string(),
            shutdown: None,
        })
        .unwrap();
    let implicit = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: None,
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "implicit".to_string(),
            shutdown: None,
        })
        .unwrap();

    assert_eq!(first.thread_id, super::THREAD);
    assert_eq!(implicit.thread_id, first.thread_id);
}
