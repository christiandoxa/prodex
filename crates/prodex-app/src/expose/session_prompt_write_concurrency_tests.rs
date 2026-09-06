use super::session_prompt_write::{
    ExistingSessionPromptWrite, PromptOutputReadRequest, SessionPromptWriteError,
};
use super::session_prompt_write_tests::{fixture, queue, request, service};
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn simultaneous_cursor_reads_are_independent_and_equal() {
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
            binding_key: "read-owner".to_string(),
            shutdown: None,
        })
        .unwrap();
    let cursor = first.next_cursor;
    let service = Arc::new(service);
    let reads = (0..2)
        .map(|index| {
            let service = Arc::clone(&service);
            let workspace_root = fixture.workspace.clone();
            let cursor = cursor.clone();
            thread::spawn(move || {
                service.read_output(PromptOutputReadRequest {
                    workspace_root,
                    cursor: Some(cursor),
                    limit: 10,
                    wait_ms: 0,
                    prodex_pid: None,
                    thread_id: None,
                    binding_key: format!("read-{index}"),
                    shutdown: None,
                })
            })
        })
        .collect::<Vec<_>>();
    let mut reads = reads.into_iter();
    let first = reads.next().unwrap().join().unwrap().unwrap();
    let second = reads.next().unwrap().join().unwrap().unwrap();
    assert_eq!(first, second);
}

#[test]
fn simultaneous_writes_are_each_accepted_once_without_cross_session_leakage() {
    let fixture = fixture();
    let mut queue_control = queue(&fixture, None);
    queue_control.invocation.queued = true;
    let calls = Arc::clone(&queue_control.calls);
    let service = Arc::new(service(&fixture, queue_control));
    let writes = ["session-a", "session-b"]
        .into_iter()
        .map(|message| {
            let service = Arc::clone(&service);
            let mut request = request(&fixture, message);
            request.binding_key = message.to_string();
            thread::spawn(move || service.write(request))
        })
        .collect::<Vec<_>>();
    for write in writes {
        assert_eq!(
            write.join().unwrap().unwrap().verification,
            "queue_pending_observed"
        );
    }
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(calls.iter().any(|(_, message)| message == "session-a"));
    assert!(calls.iter().any(|(_, message)| message == "session-b"));
}

#[test]
fn prompt_write_stays_responsive_and_append_wakes_long_poll() {
    let fixture = fixture();
    let mut queue_control = queue(&fixture, None);
    queue_control.invocation.queued = true;
    let service = Arc::new(service(&fixture, queue_control));
    let first = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: None,
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "poll".to_string(),
            shutdown: None,
        })
        .unwrap();
    let shutdown = Arc::new(AtomicBool::new(false));
    let waiter_service = Arc::clone(&service);
    let waiter_shutdown = Arc::clone(&shutdown);
    let waiter_workspace = fixture.workspace.clone();
    let waiter_cursor = first.next_cursor;
    let waiter = thread::spawn(move || {
        waiter_service.read_output(PromptOutputReadRequest {
            workspace_root: waiter_workspace,
            cursor: Some(waiter_cursor),
            limit: 10,
            wait_ms: 2_000,
            prodex_pid: None,
            thread_id: None,
            binding_key: "poll".to_string(),
            shutdown: Some(waiter_shutdown),
        })
    });
    thread::sleep(Duration::from_millis(20));

    let started = Instant::now();
    let mut write_request = request(&fixture, "write while reading");
    write_request.binding_key = "writer".to_string();
    assert_eq!(
        service.write(write_request).unwrap().verification,
        "queue_pending_observed"
    );
    assert!(started.elapsed() < Duration::from_millis(250));

    let event = serde_json::json!({
        "timestamp": "2026-09-03T10:00:04Z",
        "type": "event_msg",
        "payload": {"type": "agent_message", "message": "wakeup"}
    });
    writeln!(
        std::fs::OpenOptions::new()
            .append(true)
            .open(&fixture.rollout)
            .unwrap(),
        "{event}"
    )
    .unwrap();
    let result = waiter.join().unwrap().unwrap();
    assert_eq!(result.events[0].text, "wakeup");
    shutdown.store(true, std::sync::atomic::Ordering::SeqCst);
    assert_ne!(result.events[0].kind, "error");
}

#[test]
fn long_poll_shutdown_returns_without_replay_or_error_leak() {
    let fixture = fixture();
    let service = Arc::new(service(&fixture, queue(&fixture, None)));
    let first = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: None,
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "shutdown".to_string(),
            shutdown: None,
        })
        .unwrap();
    let shutdown = Arc::new(AtomicBool::new(false));
    let worker_shutdown = Arc::clone(&shutdown);
    let worker = {
        let service = Arc::clone(&service);
        let workspace_root = fixture.workspace.clone();
        thread::spawn(move || {
            service.read_output(PromptOutputReadRequest {
                workspace_root,
                cursor: Some(first.next_cursor),
                limit: 10,
                wait_ms: 2_000,
                prodex_pid: None,
                thread_id: None,
                binding_key: "shutdown".to_string(),
                shutdown: Some(worker_shutdown),
            })
        })
    };
    thread::sleep(Duration::from_millis(20));
    shutdown.store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        worker.join().unwrap(),
        Err(SessionPromptWriteError::OutputReadFailed)
    );
}
