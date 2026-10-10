use prodex_mojo_core::deepseek_attempt_policy::{
    DeepSeekAttemptAction, DeepSeekAttemptInput, DeepSeekAttemptKind, attempt_action,
    first_event_retry_allowed,
};

#[test]
fn compiled_mojo_owns_first_event_budget_and_commit_boundary() {
    assert!(first_event_retry_allowed(0, false).unwrap());
    assert!(!first_event_retry_allowed(1, false).unwrap());
    assert!(!first_event_retry_allowed(0, true).unwrap());
}

#[test]
fn compiled_mojo_prioritizes_model_then_credential_for_native_first_event() {
    let next_model = attempt_action(DeepSeekAttemptInput {
        kind: DeepSeekAttemptKind::NativeFirstEvent,
        attempted_first_event_retries: 0,
        first_event_committed: false,
        model_index: 0,
        model_count: 2,
        credential_index: 0,
        credential_count: 2,
        model_retry_allowed: true,
        credential_retry_allowed: true,
    })
    .unwrap();
    assert_eq!(next_model, DeepSeekAttemptAction::NextModel);

    let next_credential = attempt_action(DeepSeekAttemptInput {
        kind: DeepSeekAttemptKind::NativeFirstEvent,
        attempted_first_event_retries: 0,
        first_event_committed: false,
        model_index: 1,
        model_count: 2,
        credential_index: 0,
        credential_count: 2,
        model_retry_allowed: true,
        credential_retry_allowed: true,
    })
    .unwrap();
    assert_eq!(next_credential, DeepSeekAttemptAction::NextCredential);

    let terminal = attempt_action(DeepSeekAttemptInput {
        kind: DeepSeekAttemptKind::NativeFirstEvent,
        attempted_first_event_retries: 1,
        first_event_committed: false,
        model_index: 0,
        model_count: 2,
        credential_index: 0,
        credential_count: 2,
        model_retry_allowed: true,
        credential_retry_allowed: true,
    })
    .unwrap();
    assert_eq!(terminal, DeepSeekAttemptAction::Return);
}

#[test]
fn compiled_mojo_keeps_error_precedence_independent_of_first_event_budget() {
    assert_eq!(
        attempt_action(DeepSeekAttemptInput {
            kind: DeepSeekAttemptKind::Error,
            attempted_first_event_retries: 1,
            first_event_committed: false,
            model_index: 0,
            model_count: 2,
            credential_index: 0,
            credential_count: 2,
            model_retry_allowed: true,
            credential_retry_allowed: true,
        })
        .unwrap(),
        DeepSeekAttemptAction::NextModel
    );
    assert_eq!(
        attempt_action(DeepSeekAttemptInput {
            kind: DeepSeekAttemptKind::Error,
            attempted_first_event_retries: 0,
            first_event_committed: true,
            model_index: 1,
            model_count: 2,
            credential_index: 1,
            credential_count: 2,
            model_retry_allowed: true,
            credential_retry_allowed: true,
        })
        .unwrap(),
        DeepSeekAttemptAction::Return
    );
}
