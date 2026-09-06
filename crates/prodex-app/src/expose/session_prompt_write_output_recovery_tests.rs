use super::super::session_prompt_write::{ExistingSessionPromptWrite, PromptOutputReadRequest};
use super::{fixture, queue, service};
use std::io::Write;

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
