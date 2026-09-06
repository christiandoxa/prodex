use super::session_prompt_write::{
    ExistingSessionPromptWrite, QueueInvocation, SessionPromptWriteError,
};
use super::session_prompt_write_tests::{fixture, queue, request, service};
use std::sync::Arc;

#[test]
fn definitely_not_accepted_prompt_is_requeued_once() {
    let fixture = fixture();
    let message = "retry this exact\nprompt λ\\\n";
    let mut queue_control = queue(&fixture, None);
    queue_control.consumed_message = Some(message.to_string());
    queue_control.invocations.lock().unwrap().extend([
        QueueInvocation::default(),
        QueueInvocation::accepted(Some(0), Some("message-2".to_string()), None, false),
    ]);
    let calls = Arc::clone(&queue_control.calls);

    let result = service(&fixture, queue_control)
        .write(request(&fixture, message))
        .expect("an explicit rejection may be retried once");

    assert_eq!(result.recovery_generation, 1);
    assert!(result.last_prompt_requeued);
    assert_eq!(result.requeue_reason, Some("definitely_not_accepted"));
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(calls.iter().all(|(_, queued)| queued == message));
    drop(calls);
    let persisted = std::fs::read_to_string(&fixture.rollout).unwrap();
    let persisted_messages = persisted
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|value| {
            value
                .pointer("/payload/content/0/text")
                .and_then(serde_json::Value::as_str)
                == Some(message)
        })
        .count();
    assert_eq!(persisted_messages, 1);
}

#[test]
fn rejected_retry_is_not_requeued_again() {
    let fixture = fixture();
    let queue_control = queue(&fixture, None);
    queue_control
        .invocations
        .lock()
        .unwrap()
        .extend([QueueInvocation::default(), QueueInvocation::default()]);
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
fn ambiguous_prompt_is_never_requeued() {
    let fixture = fixture();
    let mut queue_control = queue(&fixture, None);
    queue_control.invocation = QueueInvocation::ambiguous();
    let calls = Arc::clone(&queue_control.calls);

    assert_eq!(
        service(&fixture, queue_control)
            .write(request(&fixture, "ambiguous"))
            .unwrap_err(),
        SessionPromptWriteError::WriteAmbiguous
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
}
