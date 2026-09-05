use super::super::session_prompt_write::{
    ExistingSessionPromptWrite, OpenProcessFile, PromptOutputReadRequest, SessionPromptWriteError,
    SessionPromptWriteService, read_output_events, valid_rollout_path_in_authoritative_open_files,
    valid_rollout_path_in_roots,
};
use super::{FakeProcessInspector, fixture, queue, service};
use std::io::{Seek, SeekFrom, Write};
use std::sync::atomic::AtomicUsize;

const THREAD: &str = "019f3b59-7771-7ea1-a9a1-3cd638f216c4";
const REPORTED_FAILURE_OFFSET: usize = 362_937;

#[test]
fn output_read_uses_exact_rollout_cursor_and_repeated_cursor_is_safe() {
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
            binding_key: "test".to_string(),
        })
        .unwrap();
    assert_eq!(first.thread_id, THREAD);
    assert_eq!(first.events[0].kind, "assistant");
    assert_eq!(first.events[1].kind, "tool");
    let repeated = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: Some(first.next_cursor.clone()),
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "test".to_string(),
        })
        .unwrap();
    let repeated_again = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: Some(first.next_cursor),
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "test".to_string(),
        })
        .unwrap();
    assert_eq!(repeated, repeated_again);
}

#[test]
fn output_cursor_rejects_invalid_values() {
    let fixture = fixture();
    let service = service(&fixture, queue(&fixture, None));
    assert_eq!(
        service.read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: Some("not-a-cursor".to_string()),
            limit: 1,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "test".to_string(),
        }),
        Err(SessionPromptWriteError::InvalidCursor)
    );
}

#[test]
fn output_cursor_rejects_recycled_prodex_process_identity() {
    let fixture = fixture();
    let mut changed_records = fixture.records.clone();
    changed_records[0].start_time = Some(99);
    changed_records[0].birth_identity = Some("test:recycled".to_string());
    let service = SessionPromptWriteService::with_adapters(
        FakeProcessInspector {
            uid: 1000,
            records: fixture.records.clone(),
            details: fixture.writer.clone(),
            changed_records: Some(changed_records),
            lists: AtomicUsize::new(0),
        },
        queue(&fixture, None),
    );
    let first = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: None,
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "recycled-process".to_string(),
        })
        .unwrap();

    assert_eq!(
        service.read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: Some(first.next_cursor),
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "recycled-process".to_string(),
        }),
        Err(SessionPromptWriteError::StaleTarget)
    );
}

#[test]
fn implementation_has_no_direct_queue_payload_or_pty_transport_path() {
    let source = [
        include_str!("session_prompt_write.rs"),
        include_str!("session_prompt_write/output.rs"),
        include_str!("session_prompt_write/process.rs"),
        include_str!("session_prompt_write/queue.rs"),
        include_str!("session_prompt_write/write.rs"),
    ]
    .concat();
    for forbidden in [
        "INSERT INTO queued_items",
        "xdotool",
        "TIOCSTI",
        "/dev/pts/",
        "prodex_super_start",
        "/expose/input",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden mechanism: {forbidden}"
        );
    }
}

#[test]
fn output_read_hides_internal_user_context_but_keeps_real_user_input() {
    let fixture = fixture();
    let path = fixture.root.join("context-filter.jsonl");
    let hidden = serde_json::json!({
        "timestamp": "2026-09-03T10:00:00Z",
        "type": "response_item",
        "payload": {
            "type": "message",
            "role": "user",
            "content": [{"type": "input_text", "text": "hidden instructions"}],
            "internal_chat_message_metadata_passthrough": {
                "content_item_kinds": ["agents_md.instructions"]
            }
        }
    });
    let visible = serde_json::json!({
        "timestamp": "2026-09-03T10:00:01Z",
        "type": "response_item",
        "payload": {
            "type": "message",
            "role": "user",
            "content": [{"type": "input_text", "text": "visible prompt"}],
            "internal_chat_message_metadata_passthrough": {
                "content_item_kinds": ["user.text"]
            }
        }
    });
    std::fs::write(&path, format!("{hidden}\n{visible}\n")).unwrap();

    let batch = read_output_events(&path, 0, 0, 10).unwrap();

    assert_eq!(batch.events.len(), 1);
    assert_eq!(batch.events[0].text, "visible prompt");
}

#[test]
fn session_output_read_cursor_continues_after_oversized_rollout_line() {
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
            binding_key: "cursor-regression".to_string(),
        })
        .unwrap();
    let offset = std::fs::metadata(&fixture.rollout).unwrap().len();
    let first_cursor =
        super::super::session_prompt_write::decode_output_cursor(&first.next_cursor).unwrap();
    assert_eq!(first_cursor.offset, offset);

    let oversized = serde_json::json!({
        "timestamp": "2026-09-03T10:00:03Z",
        "type": "response_item",
        "payload": {
            "type": "function_call_output",
            "output": "x".repeat(200_000)
        }
    });
    let following = serde_json::json!({
        "timestamp": "2026-09-03T10:00:04Z",
        "type": "event_msg",
        "payload": {"type": "agent_message", "message": "after oversized output"}
    });
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&fixture.rollout)
        .unwrap();
    writeln!(file, "{oversized}").unwrap();
    writeln!(file, "{following}").unwrap();
    let skipped = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: Some(first.next_cursor),
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "cursor-regression".to_string(),
        })
        .unwrap();
    assert!(skipped.events.is_empty());
    assert!(skipped.has_more);

    let next = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: Some(skipped.next_cursor),
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "cursor-regression".to_string(),
        })
        .unwrap();
    assert_eq!(next.events.len(), 1);
    assert_eq!(next.events[0].text, "after oversized output");
    assert!(!next.has_more);
}

#[test]
fn session_output_read_cursor_continues_at_reported_oversized_line_offset() {
    let fixture = fixture();
    let service = service(&fixture, queue(&fixture, None));
    let initial_len = std::fs::metadata(&fixture.rollout).unwrap().len() as usize;
    assert!(initial_len + 1 < REPORTED_FAILURE_OFFSET);
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&fixture.rollout)
        .unwrap();
    file.write_all(&vec![b'x'; REPORTED_FAILURE_OFFSET - initial_len - 1])
        .unwrap();
    file.write_all(b"\n").unwrap();

    let first = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: None,
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "reported-offset".to_string(),
        })
        .unwrap();
    let skipped_prefix = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: Some(first.next_cursor),
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "reported-offset".to_string(),
        })
        .unwrap();
    let prefix_cursor =
        super::super::session_prompt_write::decode_output_cursor(&skipped_prefix.next_cursor)
            .unwrap();
    assert_eq!(prefix_cursor.offset, REPORTED_FAILURE_OFFSET as u64);

    let oversized = serde_json::json!({
        "timestamp": "2026-09-03T10:00:03Z",
        "type": "response_item",
        "payload": {
            "type": "function_call_output",
            "output": "x".repeat(150_000)
        }
    });
    let following = serde_json::json!({
        "timestamp": "2026-09-03T10:00:04Z",
        "type": "event_msg",
        "payload": {"type": "agent_message", "message": "after reported offset"}
    });
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&fixture.rollout)
        .unwrap();
    writeln!(file, "{oversized}").unwrap();
    writeln!(file, "{following}").unwrap();

    let skipped_line = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: Some(skipped_prefix.next_cursor),
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "reported-offset".to_string(),
        })
        .unwrap();
    assert!(skipped_line.events.is_empty());
    assert!(skipped_line.has_more);

    let next = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: Some(skipped_line.next_cursor),
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "reported-offset".to_string(),
        })
        .unwrap();
    assert_eq!(next.events.len(), 1);
    assert_eq!(next.events[0].text, "after reported offset");
    assert!(!next.has_more);
}

#[test]
fn session_output_read_rejects_changed_rollout_before_cursor() {
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
            binding_key: "changed-source".to_string(),
        })
        .unwrap();

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(&fixture.rollout)
        .unwrap();
    file.seek(SeekFrom::Start(0)).unwrap();
    file.write_all(b"X").unwrap();

    assert_eq!(
        service.read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: Some(first.next_cursor),
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "changed-source".to_string(),
        }),
        Err(SessionPromptWriteError::OutputSourceChanged)
    );
}

#[test]
fn session_output_read_cursor_continues_after_rollout_append() {
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
            binding_key: "append-source".to_string(),
        })
        .unwrap();
    let appended = serde_json::json!({
        "timestamp": "2026-09-03T10:00:03Z",
        "type": "event_msg",
        "payload": {"type": "agent_message", "message": "after append"}
    });
    writeln!(
        std::fs::OpenOptions::new()
            .append(true)
            .open(&fixture.rollout)
            .unwrap(),
        "{appended}"
    )
    .unwrap();

    let next = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: Some(first.next_cursor),
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "append-source".to_string(),
        })
        .unwrap();
    assert_eq!(next.events.len(), 1);
    assert_eq!(next.events[0].text, "after append");
    assert!(!next.has_more);
}

#[test]
fn output_page_does_not_drop_events_from_one_rollout_line() {
    let fixture = fixture();
    let first = read_output_events(&fixture.rollout, 0, 0, 1).unwrap();
    assert_eq!((first.events.len(), first.next_event_index), (1, 0));
    assert!(first.next_offset > 0);
    let second = read_output_events(
        &fixture.rollout,
        first.next_offset,
        first.next_event_index,
        10,
    )
    .unwrap();
    assert_eq!(second.events.len(), 2);
    assert!(second.next_offset > 0);
}

#[test]
fn output_source_accepts_managed_session_directory_symlink() {
    let fixture = fixture();
    let overlay = fixture.root.join("overlay");
    std::fs::create_dir_all(&overlay).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        fixture.root.join("target-codex-home/sessions"),
        overlay.join("sessions"),
    )
    .unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(
        fixture.root.join("target-codex-home/sessions"),
        overlay.join("sessions"),
    )
    .unwrap();

    let stored = overlay.join("sessions/2026/09/03").join(
        fixture
            .rollout
            .file_name()
            .expect("fixture rollout should have a filename"),
    );
    assert_eq!(
        valid_rollout_path_in_roots(&stored, std::slice::from_ref(&overlay), THREAD),
        None
    );
    assert_eq!(
        valid_rollout_path_in_authoritative_open_files(
            &stored,
            std::slice::from_ref(&overlay),
            &[OpenProcessFile {
                path: fixture.rollout.clone(),
            }],
            THREAD,
        ),
        Some(fixture.rollout.canonicalize().unwrap())
    );
    assert_eq!(
        valid_rollout_path_in_authoritative_open_files(&stored, &[overlay], &[], THREAD),
        None
    );
}

#[test]
fn output_source_rejects_symlink_and_hidden_instruction_rollout() {
    let fixture = fixture();
    let outside = fixture.root.join("outside.jsonl");
    std::fs::write(&outside, "secret\n").unwrap();
    let link = fixture
        .rollout
        .parent()
        .unwrap()
        .join(format!("rollout-{THREAD}-link.jsonl"));
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        assert!(
            valid_rollout_path_in_roots(&link, &[fixture.root.join("target-codex-home")], THREAD,)
                .is_none()
        );
    }
    let hidden = fixture.root.join("hidden.jsonl");
    std::fs::write(
        &hidden,
        "{\"timestamp\":\"2026-09-03T10:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"base_instructions\":{\"text\":\"do not disclose\"}}}\n",
    )
    .unwrap();
    let batch = read_output_events(&hidden, 0, 0, 10).unwrap();
    assert!(batch.events.is_empty());
}
