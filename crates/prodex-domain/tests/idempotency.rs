use prodex_domain::{
    CallId, IdempotencyConflict, IdempotencyConflictStatus, IdempotencyDecision, IdempotencyEntry,
    IdempotencyKey, IdempotencyKeyError, IdempotencyKeyErrorStatus, IdempotencyRecord,
    IdempotencyReplayDecision, IdempotentOperation, IdempotentOperationError, ReservationId,
    TenantId, decide_idempotency, decide_idempotency_replay, plan_idempotency_conflict_response,
    plan_idempotency_key_error_response,
};

#[test]
fn idempotency_key_rejects_empty_and_overlong_values() {
    assert_eq!(IdempotencyKey::new(""), Err(IdempotencyKeyError::Empty));
    assert_eq!(
        IdempotencyKey::new("  "),
        Err(IdempotencyKeyError::InvalidCharacter {
            index: 0,
            character: ' ',
        })
    );
    assert_eq!(
        IdempotencyKey::new("x".repeat(257)),
        Err(IdempotencyKeyError::TooLong { length: 257 })
    );

    assert_eq!(
        IdempotencyKey::new("tenant replay"),
        Err(IdempotencyKeyError::InvalidCharacter {
            index: 6,
            character: ' ',
        })
    );
    assert_eq!(
        IdempotencyKey::new(" tenant-replay"),
        Err(IdempotencyKeyError::InvalidCharacter {
            index: 0,
            character: ' ',
        })
    );
    assert_eq!(
        IdempotencyKey::new("tenant\nreplay"),
        Err(IdempotencyKeyError::InvalidCharacter {
            index: 6,
            character: '\n',
        })
    );
    assert_eq!(
        IdempotencyKey::new("tenant-é"),
        Err(IdempotencyKeyError::InvalidCharacter {
            index: 7,
            character: 'é',
        })
    );
}

#[test]
fn idempotency_key_error_response_is_stable_and_redacted() {
    let empty_response = plan_idempotency_key_error_response(&IdempotencyKeyError::Empty);

    assert_eq!(empty_response.status, IdempotencyKeyErrorStatus::BadRequest);
    assert_eq!(empty_response.code, "idempotency_key_invalid");
    assert_eq!(empty_response.message, "idempotency key is invalid");

    let long_response =
        plan_idempotency_key_error_response(&IdempotencyKeyError::TooLong { length: 257 });
    assert_eq!(
        (IdempotencyKeyError::TooLong { length: 257 }).to_string(),
        "idempotency key is invalid"
    );
    assert!(
        !(IdempotencyKeyError::TooLong { length: 257 })
            .to_string()
            .contains("257")
    );
    assert_eq!(
        format!("{:?}", IdempotencyKeyError::TooLong { length: 257 }),
        "TooLong { length: \"<redacted>\" }"
    );
    assert_eq!(long_response, empty_response);
    assert!(!long_response.message.contains("257"));

    let invalid = IdempotencyKeyError::InvalidCharacter {
        index: 6,
        character: '\n',
    };
    assert_eq!(invalid.to_string(), "idempotency key is invalid");
    assert!(!invalid.to_string().contains("6"));
    assert!(!invalid.to_string().contains("\\n"));
    let rendered = format!("{invalid:?}");
    assert!(!rendered.contains("6"));
    assert!(!rendered.contains("\\n"));
    assert!(rendered.contains("\"<redacted>\""));
    let invalid_response = plan_idempotency_key_error_response(&invalid);
    assert_eq!(invalid_response, empty_response);
    assert!(!invalid_response.message.contains("6"));
    assert!(!invalid_response.message.contains("\\n"));
}

#[test]
fn call_reservation_key_is_stable_for_billing_retries() {
    let call_id = CallId::new();
    let reservation_id = ReservationId::new();

    let key = IdempotencyKey::from_call_reservation(call_id, reservation_id);

    assert!(key.as_str().contains(&call_id.to_string()));
    assert!(key.as_str().contains(&reservation_id.to_string()));
}

#[test]
fn first_idempotent_operation_should_execute() {
    let operation = IdempotentOperation::new(
        TenantId::new(),
        IdempotencyKey::new("admin-mutation-1").unwrap(),
        "sha256:request-a",
    )
    .unwrap();

    assert_eq!(
        decide_idempotency::<String>(&operation, None).unwrap(),
        IdempotencyDecision::Execute
    );
}

#[test]
fn idempotent_operation_rejects_empty_request_fingerprint() {
    assert_eq!(
        IdempotentOperation::new(
            TenantId::new(),
            IdempotencyKey::new("admin-mutation-1").unwrap(),
            "",
        ),
        Err(IdempotentOperationError::EmptyRequestFingerprint)
    );
}

#[test]
fn idempotent_operation_rejects_malformed_request_fingerprint() {
    let key = IdempotencyKey::new("admin-mutation-1").unwrap();
    assert_eq!(
        IdempotentOperation::new(TenantId::new(), key.clone(), " "),
        Err(
            IdempotentOperationError::InvalidRequestFingerprintCharacter {
                index: 0,
                character: ' ',
            }
        )
    );
    assert_eq!(
        IdempotentOperation::new(TenantId::new(), key.clone(), "sha256:bad fingerprint"),
        Err(
            IdempotentOperationError::InvalidRequestFingerprintCharacter {
                index: 10,
                character: ' ',
            }
        )
    );
    assert_eq!(
        IdempotentOperation::new(TenantId::new(), key.clone(), "sha256:bad\nfingerprint"),
        Err(
            IdempotentOperationError::InvalidRequestFingerprintCharacter {
                index: 10,
                character: '\n',
            }
        )
    );
    assert_eq!(
        IdempotentOperation::new(TenantId::new(), key, "sha256:bad-é"),
        Err(
            IdempotentOperationError::InvalidRequestFingerprintCharacter {
                index: 11,
                character: 'é',
            }
        )
    );
}

#[test]
fn idempotent_operation_error_debug_output_is_stable_and_redacted() {
    let error = IdempotentOperationError::InvalidRequestFingerprintCharacter {
        index: 10,
        character: '\n',
    };

    assert_eq!(error.to_string(), "request fingerprint is invalid");

    let rendered = format!("{error:?}");
    assert!(!rendered.contains("10"));
    assert!(!rendered.contains("\\n"));
    assert_eq!(
        rendered,
        "InvalidRequestFingerprintCharacter { index: \"<redacted>\", character: \"<redacted>\" }"
    );
}

#[test]
fn same_tenant_same_fingerprint_replays_stored_response() {
    let operation = IdempotentOperation::new(
        TenantId::new(),
        IdempotencyKey::new("admin-mutation-1").unwrap(),
        "sha256:request-a",
    )
    .unwrap();
    let existing = IdempotencyRecord {
        operation: operation.clone(),
        response: "created".to_string(),
    };

    assert_eq!(
        decide_idempotency(&operation, Some(&existing)).unwrap(),
        IdempotencyDecision::Replay("created".to_string())
    );
}

#[test]
fn same_key_different_fingerprint_is_conflict_not_replay() {
    let tenant_id = TenantId::new();
    let key = IdempotencyKey::new("admin-mutation-1").unwrap();
    let operation = IdempotentOperation::new(tenant_id, key.clone(), "sha256:request-b").unwrap();
    let existing = IdempotencyRecord {
        operation: IdempotentOperation::new(tenant_id, key, "sha256:request-a").unwrap(),
        response: "created".to_string(),
    };

    assert_eq!(
        decide_idempotency(&operation, Some(&existing)),
        Err(IdempotencyConflict::RequestFingerprintMismatch)
    );
}

#[test]
fn same_key_different_tenant_is_conflict_not_replay() {
    let key = IdempotencyKey::new("admin-mutation-1").unwrap();
    let operation =
        IdempotentOperation::new(TenantId::new(), key.clone(), "sha256:request-a").unwrap();
    let existing = IdempotencyRecord {
        operation: IdempotentOperation::new(TenantId::new(), key, "sha256:request-a").unwrap(),
        response: "created".to_string(),
    };

    assert_eq!(
        decide_idempotency(&operation, Some(&existing)),
        Err(IdempotencyConflict::TenantMismatch)
    );
}

#[test]
fn defensive_record_key_mismatch_is_conflict_not_replay() {
    let tenant_id = TenantId::new();
    let operation = IdempotentOperation::new(
        tenant_id,
        IdempotencyKey::new("admin-mutation-1").unwrap(),
        "sha256:request-a",
    )
    .unwrap();
    let existing = IdempotencyRecord {
        operation: IdempotentOperation::new(
            tenant_id,
            IdempotencyKey::new("admin-mutation-2").unwrap(),
            "sha256:request-a",
        )
        .unwrap(),
        response: "created".to_string(),
    };

    assert_eq!(
        decide_idempotency(&operation, Some(&existing)),
        Err(IdempotencyConflict::KeyMismatch)
    );
}

#[test]
fn idempotency_conflict_responses_are_stable_and_redacted() {
    assert_eq!(
        IdempotencyConflict::TenantMismatch.to_string(),
        "idempotency replay conflict"
    );
    assert_eq!(
        IdempotencyConflict::KeyMismatch.to_string(),
        "idempotency replay conflict"
    );
    assert!(
        !IdempotencyConflict::TenantMismatch
            .to_string()
            .contains("tenant")
    );
    assert!(!IdempotencyConflict::KeyMismatch.to_string().contains("key"));

    let tenant = plan_idempotency_conflict_response(&IdempotencyConflict::TenantMismatch);

    assert_eq!(tenant.status, IdempotencyConflictStatus::Conflict);
    assert_eq!(tenant.code, "idempotency_replay_conflict");
    assert_eq!(tenant.message, "idempotency replay conflict");
    assert!(!tenant.message.contains("tenant"));

    let key = plan_idempotency_conflict_response(&IdempotencyConflict::KeyMismatch);
    assert_eq!(key, tenant);
    assert!(!key.message.contains("key"));

    let fingerprint =
        plan_idempotency_conflict_response(&IdempotencyConflict::RequestFingerprintMismatch);
    assert_eq!(fingerprint.status, IdempotencyConflictStatus::Conflict);
    assert_eq!(fingerprint.code, "idempotency_key_reused");
    assert_eq!(
        fingerprint.message,
        "idempotency key was reused for a different request"
    );
    assert!(!fingerprint.message.contains("sha256"));
}

#[test]
fn idempotency_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let key = IdempotencyKey::new("admin-mutation-secret-key").unwrap();
    let operation =
        IdempotentOperation::new(tenant_id, key.clone(), "sha256:reserve-sensitive-tokens")
            .unwrap();
    let record = IdempotencyRecord {
        operation: operation.clone(),
        response: "stored-secret-response-body".to_string(),
    };
    let pending = IdempotencyEntry::<String>::pending(operation.clone(), 1_700_000_000_000);
    let completed = IdempotencyEntry::completed(record);
    let replay = IdempotencyDecision::Replay("stored-secret-response-body".to_string());
    let replay_in_progress = IdempotencyReplayDecision::<String>::AlreadyInProgress {
        started_at_unix_ms: 1_700_000_000_000,
    };
    let replay_guard = IdempotencyReplayDecision::Replay("stored-secret-response-body".to_string());

    let rendered = format!(
        "{key:?} {operation:?} {pending:?} {completed:?} {replay:?} {replay_in_progress:?} {replay_guard:?}",
    );

    for sensitive in [
        &tenant_id.to_string(),
        "admin-mutation-secret-key",
        "sha256:reserve-sensitive-tokens",
        "stored-secret-response-body",
        "1700000000000",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "idempotency debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert_eq!(
        format!("{replay_in_progress:?}"),
        "AlreadyInProgress { started_at_unix_ms: \"<redacted>\" }"
    );
}

#[test]
fn replay_guard_requires_pending_record_before_executing_mutation() {
    let operation = IdempotentOperation::new(
        TenantId::new(),
        IdempotencyKey::new("billing-reservation-1").unwrap(),
        "sha256:reserve-10-tokens",
    )
    .unwrap();

    assert_eq!(
        decide_idempotency_replay::<String>(&operation, None).unwrap(),
        IdempotencyReplayDecision::ExecuteAndRecordPending
    );
}

#[test]
fn replay_guard_blocks_concurrent_duplicate_while_first_attempt_is_pending() {
    let operation = IdempotentOperation::new(
        TenantId::new(),
        IdempotencyKey::new("billing-reservation-1").unwrap(),
        "sha256:reserve-10-tokens",
    )
    .unwrap();
    let existing = IdempotencyEntry::<String>::pending(operation.clone(), 1_700_000_000_000);

    assert_eq!(
        decide_idempotency_replay(&operation, Some(&existing)).unwrap(),
        IdempotencyReplayDecision::AlreadyInProgress {
            started_at_unix_ms: 1_700_000_000_000,
        }
    );
}

#[test]
fn replay_guard_replays_completed_response_after_retry() {
    let operation = IdempotentOperation::new(
        TenantId::new(),
        IdempotencyKey::new("billing-reservation-1").unwrap(),
        "sha256:reserve-10-tokens",
    )
    .unwrap();
    let existing = IdempotencyEntry::completed(IdempotencyRecord {
        operation: operation.clone(),
        response: "reserved".to_string(),
    });

    assert_eq!(
        decide_idempotency_replay(&operation, Some(&existing)).unwrap(),
        IdempotencyReplayDecision::Replay("reserved".to_string())
    );
}

#[test]
fn replay_guard_rejects_pending_record_with_different_fingerprint() {
    let tenant_id = TenantId::new();
    let key = IdempotencyKey::new("billing-reservation-1").unwrap();
    let operation =
        IdempotentOperation::new(tenant_id, key.clone(), "sha256:reserve-10-tokens").unwrap();
    let existing = IdempotencyEntry::<String>::pending(
        IdempotentOperation::new(tenant_id, key, "sha256:reserve-20-tokens").unwrap(),
        1_700_000_000_000,
    );

    assert_eq!(
        decide_idempotency_replay(&operation, Some(&existing)),
        Err(IdempotencyConflict::RequestFingerprintMismatch)
    );
}

#[test]
fn replay_guard_rejects_pending_record_with_wrong_tenant_or_key() {
    let tenant_id = TenantId::new();
    let key = IdempotencyKey::new("billing-reservation-1").unwrap();
    let operation =
        IdempotentOperation::new(tenant_id, key.clone(), "sha256:reserve-10-tokens").unwrap();

    let wrong_tenant = IdempotencyEntry::<String>::pending(
        IdempotentOperation::new(TenantId::new(), key.clone(), "sha256:reserve-10-tokens").unwrap(),
        1_700_000_000_000,
    );
    assert_eq!(
        decide_idempotency_replay(&operation, Some(&wrong_tenant)),
        Err(IdempotencyConflict::TenantMismatch)
    );

    let wrong_key = IdempotencyEntry::<String>::pending(
        IdempotentOperation::new(
            tenant_id,
            IdempotencyKey::new("billing-reservation-2").unwrap(),
            "sha256:reserve-10-tokens",
        )
        .unwrap(),
        1_700_000_000_000,
    );
    assert_eq!(
        decide_idempotency_replay(&operation, Some(&wrong_key)),
        Err(IdempotencyConflict::KeyMismatch)
    );
}
