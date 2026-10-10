use super::*;

#[test]
fn queue_success_preserves_multiline_message_and_same_thread() {
    let fixture = fixture();
    let message_id = "019f3b59-7771-7ea1-a9a1-3cd638f216c5";
    let mut queue_control = queue(&fixture, Some(message_id));
    queue_control.consumed_message = Some("line one\nline two".to_string());
    let calls = Arc::clone(&queue_control.calls);
    let result = service(&fixture, queue_control).write(request(&fixture, "line one\nline two"));
    assert_eq!(result.unwrap().thread_id, THREAD);
    assert_eq!(
        calls.lock().unwrap()[0],
        (THREAD.to_string(), "line one\nline two".to_string())
    );
}

#[test]
fn queue_success_verifies_exact_message_whitespace_and_escaping() {
    let fixture = fixture();
    let before_len = std::fs::metadata(&fixture.rollout).unwrap().len();
    let message = "  leading\nline with \\\\ and \"quotes\"  \n\n";
    let mut queue_control = queue(&fixture, None);
    queue_control.consumed_message = Some(message.to_string());
    let calls = Arc::clone(&queue_control.calls);

    let service = service(&fixture, queue_control);
    let result = service
        .write(request(&fixture, message))
        .expect("the exact rollout message should verify");

    assert_eq!(result.verification, "rollout_user_event_observed");
    assert_eq!(calls.lock().unwrap()[0].1, message);
    let cursor = result
        .output_cursor
        .expect("existing rollout has an anchor");
    let decoded = super::super::session_prompt_write::decode_output_cursor(&cursor).unwrap();
    assert_eq!(decoded.offset, before_len);
    let output = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: Some(cursor),
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "different-reconnect".to_string(),
            shutdown: None,
        })
        .unwrap();
    assert_eq!(output.events[0].kind, "user");
    assert!(output.events[0].text.starts_with("  leading\nline with"));
}

#[test]
fn accepted_queued_message_does_not_wait_for_busy_turn_completion() {
    let fixture = fixture();
    let mut queue_control = queue(&fixture, None);
    queue_control.invocation.queued = true;

    let result = service(&fixture, queue_control)
        .write(request(&fixture, "queued while busy"))
        .expect("accepted queue submission should be sufficient evidence");

    assert_eq!(result.verification, "queue_pending_observed");
}

#[test]
fn rejected_queue_submission_retries_once_then_fails_closed() {
    let fixture = fixture();
    let mut queue_control = queue(&fixture, None);
    queue_control.invocations = Mutex::new(VecDeque::from([
        QueueInvocation {
            outcome: QueueRequestOutcome::Rejected,
            ..QueueInvocation::default()
        },
        QueueInvocation {
            outcome: QueueRequestOutcome::Rejected,
            ..QueueInvocation::default()
        },
    ]));
    let calls = Arc::clone(&queue_control.calls);

    assert_eq!(
        service(&fixture, queue_control)
            .write(request(&fixture, "rejected twice"))
            .unwrap_err(),
        SessionPromptWriteError::QueueFailed
    );
    assert_eq!(calls.lock().unwrap().len(), 2);
}

#[test]
fn preflight_queue_submission_retries_once_before_acceptance() {
    let fixture = fixture();
    let mut queue_control = queue(&fixture, None);
    queue_control.invocations = Mutex::new(VecDeque::from([
        QueueInvocation {
            outcome: QueueRequestOutcome::Preflight,
            ..QueueInvocation::default()
        },
        QueueInvocation {
            outcome: QueueRequestOutcome::Accepted,
            queued: true,
            ..QueueInvocation::default()
        },
    ]));
    let calls = Arc::clone(&queue_control.calls);

    let result = service(&fixture, queue_control)
        .write(request(&fixture, "retry preflight"))
        .expect("preflight should retry before queue verification");

    assert!(result.last_prompt_requeued);
    assert_eq!(result.verification, "queue_pending_observed");
    assert_eq!(calls.lock().unwrap().len(), 2);
}

#[test]
fn queue_success_verifies_near_limit_escaped_message() {
    let fixture = fixture();
    let message = "\\".repeat(SESSION_PROMPT_WRITE_MAX_MESSAGE_BYTES - 1024);
    let mut queue_control = queue(&fixture, None);
    queue_control.consumed_message = Some(message.clone());

    let result = service(&fixture, queue_control)
        .write(request(&fixture, &message))
        .expect("near-limit rollout message should verify");

    assert_eq!(result.verification, "rollout_user_event_observed");
}

#[test]
fn process_argument_normalization_handles_values_separators_and_unknown_commands() {
    use super::super::session_prompt_write::first_codex_positional_arg;

    assert_eq!(
        first_codex_positional_arg(&[
            "codex".to_string(),
            "--config".to_string(),
            "model_provider=example".to_string(),
            "resume".to_string(),
        ]),
        Some("resume")
    );
    assert_eq!(
        first_codex_positional_arg(&[
            "codex".to_string(),
            "--".to_string(),
            "app-server".to_string()
        ]),
        None
    );
    assert_eq!(
        first_codex_positional_arg(&[
            "codex".to_string(),
            "--model=example".to_string(),
            "app-server".to_string()
        ]),
        Some("app-server")
    );
}

#[test]
fn fresh_idle_thread_uses_live_app_server_before_first_persisted_row() {
    let fixture = fixture();
    let mut queue_control = queue(&fixture, Some("019f3b59-7771-7ea1-a9a1-3cd638f216c5"));
    queue_control.persisted.store(false, Ordering::SeqCst);
    queue_control
        .loaded_addressable
        .store(true, Ordering::SeqCst);
    queue_control.consumed_message = Some("first prompt before manual input".to_string());

    let calls = Arc::clone(&queue_control.calls);
    let result = service(&fixture, queue_control)
        .write(request(&fixture, "first prompt before manual input"))
        .expect("live app-server thread should be queueable before persisted rollout");

    assert_eq!(result.thread_id, THREAD);
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[test]
fn addressability_race_waits_without_becoming_no_session() {
    let fixture = fixture();
    let mut queue_control = queue(&fixture, Some("019f3b59-7771-7ea1-a9a1-3cd638f216c5"));
    queue_control.persisted.store(false, Ordering::SeqCst);
    queue_control.addressable_after = Some(2);
    queue_control.consumed_message = Some("hello".to_string());

    let result = service(&fixture, queue_control).write(request(&fixture, "hello"));

    assert_eq!(result.unwrap().thread_id, THREAD);
}

#[test]
fn queue_success_requires_the_same_rollout_user_event() {
    let fixture = fixture();
    let message_id = "019f3b59-7771-7ea1-a9a1-3cd638f216c5";
    let mut queue_control = queue(&fixture, Some(message_id));
    queue_control.consumed_message = Some("already consumed".to_string());
    let result = service(&fixture, queue_control)
        .write(request(&fixture, "already consumed"))
        .unwrap();
    assert_eq!(result.verification, "rollout_user_event_observed");
}

#[test]
fn app_server_turn_control_requires_the_rollout_user_event_before_success() {
    let fixture = fixture();
    let message = "visible user message";
    let mut queue_control = queue(&fixture, None);
    queue_control.persisted.store(false, Ordering::SeqCst);
    queue_control
        .loaded_addressable
        .store(true, Ordering::SeqCst);
    queue_control.consumed_message = Some(message.to_string());

    let result = service(&fixture, queue_control)
        .write(request(&fixture, message))
        .expect("turn control must wait for the same rollout user event");

    assert_eq!(result.verification, "rollout_user_event_observed");
}

#[test]
fn unpersisted_thread_reproduces_codex_0153_regression_without_queue_mutation() {
    let fixture = fixture();
    let queue_control = queue(&fixture, Some("019f3b59-7771-7ea1-a9a1-3cd638f216c5"));
    queue_control.persisted.store(false, Ordering::SeqCst);
    let calls = Arc::clone(&queue_control.calls);
    let error = service(&fixture, queue_control).write(request(&fixture, "must not send"));
    assert_eq!(
        error.unwrap_err(),
        SessionPromptWriteError::SessionNotQueueAddressable
    );
    assert!(calls.lock().unwrap().is_empty());
}

#[test]
fn missing_main_queue_db_and_wal_only_are_rejected() {
    let root = std::env::temp_dir().join(format!("prodex-db-check-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let files = vec![OpenProcessFile {
        path: root.join("queue_1.sqlite-wal"),
    }];
    assert_eq!(
        exact_open_database(
            &files,
            super::super::session_prompt_write::DatabaseKind::Queue
        )
        .unwrap(),
        None
    );
    let _ = std::fs::remove_dir_all(root);
}
