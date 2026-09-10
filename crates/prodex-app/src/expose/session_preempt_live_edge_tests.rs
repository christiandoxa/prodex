use super::super::session_prompt_write::{QueueControl, SessionPromptWriteError};
use super::{FakeAppServer, LiveThread, SystemQueueControl, THREAD_A, TURN_A};
use std::sync::atomic::Ordering;

#[test]
fn app_server_preempt_fails_closed_on_malformed_queue_payload() {
    let server = FakeAppServer::new([(
        THREAD_A,
        LiveThread {
            active_turn: Some(TURN_A.to_string()),
            queue: vec!["queued-a".to_string()],
        },
    )]);
    server.state.malformed_queue.store(true, Ordering::SeqCst);

    assert!(matches!(
        SystemQueueControl.preempt(&server.target(THREAD_A)),
        Err(SessionPromptWriteError::VerificationInconclusive)
    ));
    assert!(
        !server
            .state
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|method| method == "turn/interrupt")
    );
}

#[test]
fn app_server_preempt_fails_closed_when_exact_turn_interrupt_is_rejected() {
    let server = FakeAppServer::new([(
        THREAD_A,
        LiveThread {
            active_turn: Some(TURN_A.to_string()),
            queue: Vec::new(),
        },
    )]);
    server
        .state
        .interrupt_rejected
        .store(true, Ordering::SeqCst);

    assert!(matches!(
        SystemQueueControl.preempt(&server.target(THREAD_A)),
        Err(SessionPromptWriteError::VerificationInconclusive)
    ));
    assert_eq!(
        server
            .state
            .threads
            .lock()
            .unwrap()
            .get(THREAD_A)
            .and_then(|thread| thread.active_turn.as_deref()),
        Some(TURN_A)
    );
}
