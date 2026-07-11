use prodex_domain::{
    BackupId, BackupIdError, BackupIdErrorStatus, BackupSnapshot, BackupStatus, RestorePlan,
    RestorePlanError, RestorePlanErrorStatus, plan_backup_id_error_response,
    plan_restore_error_response,
};

fn completed_backup() -> BackupSnapshot {
    BackupSnapshot {
        id: BackupId::new("backup-20260701").unwrap(),
        created_at_unix_ms: 1_000,
        expires_at_unix_ms: 10_000,
        status: BackupStatus::Completed,
        checksum: Some("sha256:ok".to_string()),
    }
}

#[test]
fn backup_id_rejects_empty_values() {
    assert_eq!(BackupId::new(""), Err(BackupIdError::Empty));
    assert_eq!(
        BackupId::new(" "),
        Err(BackupIdError::InvalidCharacter {
            index: 0,
            character: ' ',
        })
    );
    assert_eq!(
        BackupId::new("backup 20260701"),
        Err(BackupIdError::InvalidCharacter {
            index: 6,
            character: ' ',
        })
    );
    assert_eq!(
        BackupId::new(" backup-20260701"),
        Err(BackupIdError::InvalidCharacter {
            index: 0,
            character: ' ',
        })
    );
    assert_eq!(
        BackupId::new("backup\n20260701"),
        Err(BackupIdError::InvalidCharacter {
            index: 6,
            character: '\n',
        })
    );
    assert_eq!(
        BackupId::new("backup-é"),
        Err(BackupIdError::InvalidCharacter {
            index: 7,
            character: 'é',
        })
    );
}

#[test]
fn backup_id_error_response_is_stable_and_redacted() {
    let response = plan_backup_id_error_response(&BackupIdError::Empty);

    assert_eq!(response.status, BackupIdErrorStatus::BadRequest);
    assert_eq!(response.code, "backup_id_invalid");
    assert_eq!(response.message, "backup id is invalid");
    assert!(!response.message.contains("empty"));

    let invalid = BackupIdError::InvalidCharacter {
        index: 6,
        character: '\n',
    };
    assert_eq!(invalid.to_string(), "backup id is invalid");
    assert!(!invalid.to_string().contains("6"));
    assert!(!invalid.to_string().contains("\\n"));
    let rendered = format!("{invalid:?}");
    assert!(!rendered.contains("6"));
    assert!(!rendered.contains("\\n"));
    assert!(rendered.contains("\"<redacted>\""));
    let invalid_response = plan_backup_id_error_response(&invalid);
    assert_eq!(invalid_response, response);
    assert!(!invalid_response.message.contains("6"));
    assert!(!invalid_response.message.contains("\\n"));
}

#[test]
fn backup_id_debug_output_is_stable_and_redacted() {
    let backup_id = BackupId::new("backup-secret-20260701").unwrap();

    let rendered = format!("{backup_id:?}");
    assert!(!rendered.contains("backup-secret-20260701"));
    assert_eq!(rendered, "BackupId(\"<redacted>\")");
}

#[test]
fn backup_snapshot_debug_output_is_stable_and_redacted() {
    let backup = completed_backup();

    let rendered = format!("{backup:?}");
    for sensitive in ["backup-20260701", "1000", "10000", "sha256:ok"] {
        assert!(
            !rendered.contains(sensitive),
            "backup snapshot debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("status: Completed"));
    assert!(rendered.contains("\"<redacted>\""));
}

#[test]
fn restore_plan_debug_output_is_stable_and_redacted() {
    let plan = RestorePlan {
        backup: completed_backup(),
        expected_checksum: Some("sha256:ok".to_string()),
        now_unix_ms: 2_000,
    };

    let rendered = format!("{plan:?}");
    for sensitive in ["backup-20260701", "1000", "10000", "2000", "sha256:ok"] {
        assert!(
            !rendered.contains(sensitive),
            "restore plan debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("expected_checksum: Some(\"<redacted>\")"));
    assert!(rendered.contains("now_unix_ms: \"<redacted>\""));
}

#[test]
fn completed_unexpired_backup_with_matching_checksum_is_restorable() {
    let plan = RestorePlan {
        backup: completed_backup(),
        expected_checksum: Some("sha256:ok".to_string()),
        now_unix_ms: 2_000,
    };

    assert_eq!(plan.validate(), Ok(()));
}

#[test]
fn failed_backup_is_not_restorable() {
    let mut backup = completed_backup();
    backup.status = BackupStatus::Failed;
    let plan = RestorePlan {
        backup,
        expected_checksum: Some("sha256:ok".to_string()),
        now_unix_ms: 2_000,
    };

    assert_eq!(plan.validate(), Err(RestorePlanError::BackupNotCompleted));
}

#[test]
fn expired_backup_is_not_restorable() {
    let plan = RestorePlan {
        backup: completed_backup(),
        expected_checksum: Some("sha256:ok".to_string()),
        now_unix_ms: 10_000,
    };

    assert_eq!(plan.validate(), Err(RestorePlanError::BackupExpired));
}

#[test]
fn backup_with_invalid_expiry_window_is_not_restorable() {
    let mut backup = completed_backup();
    backup.expires_at_unix_ms = backup.created_at_unix_ms;
    let plan = RestorePlan {
        backup,
        expected_checksum: Some("sha256:ok".to_string()),
        now_unix_ms: 999,
    };

    assert_eq!(plan.validate(), Err(RestorePlanError::InvalidExpiry));
}

#[test]
fn backup_without_checksum_is_not_restorable() {
    let mut backup = completed_backup();
    backup.checksum = None;
    let plan = RestorePlan {
        backup,
        expected_checksum: None,
        now_unix_ms: 2_000,
    };

    assert_eq!(plan.validate(), Err(RestorePlanError::MissingChecksum));
}

#[test]
fn blank_backup_or_expected_checksum_is_not_restorable() {
    let mut blank_backup_checksum = completed_backup();
    blank_backup_checksum.checksum = Some(" ".to_string());
    let plan = RestorePlan {
        backup: blank_backup_checksum,
        expected_checksum: Some(" ".to_string()),
        now_unix_ms: 2_000,
    };
    assert_eq!(plan.validate(), Err(RestorePlanError::MalformedChecksum));

    let plan = RestorePlan {
        backup: completed_backup(),
        expected_checksum: Some(" ".to_string()),
        now_unix_ms: 2_000,
    };
    assert_eq!(plan.validate(), Err(RestorePlanError::MalformedChecksum));
}

#[test]
fn malformed_backup_or_expected_checksum_is_not_restorable() {
    let mut malformed_backup_checksum = completed_backup();
    malformed_backup_checksum.checksum = Some("sha256:not ok".to_string());
    let plan = RestorePlan {
        backup: malformed_backup_checksum,
        expected_checksum: None,
        now_unix_ms: 2_000,
    };
    assert_eq!(plan.validate(), Err(RestorePlanError::MalformedChecksum));

    let mut padded_backup_checksum = completed_backup();
    padded_backup_checksum.checksum = Some(" sha256:ok ".to_string());
    let plan = RestorePlan {
        backup: padded_backup_checksum,
        expected_checksum: Some(" sha256:ok ".to_string()),
        now_unix_ms: 2_000,
    };
    assert_eq!(plan.validate(), Err(RestorePlanError::MalformedChecksum));

    let plan = RestorePlan {
        backup: completed_backup(),
        expected_checksum: Some("sha256:\nok".to_string()),
        now_unix_ms: 2_000,
    };
    assert_eq!(plan.validate(), Err(RestorePlanError::MalformedChecksum));

    let plan = RestorePlan {
        backup: completed_backup(),
        expected_checksum: Some("sha256:ö".to_string()),
        now_unix_ms: 2_000,
    };
    assert_eq!(plan.validate(), Err(RestorePlanError::MalformedChecksum));

    let plan = RestorePlan {
        backup: completed_backup(),
        expected_checksum: Some("x".repeat(513)),
        now_unix_ms: 2_000,
    };
    assert_eq!(plan.validate(), Err(RestorePlanError::MalformedChecksum));
}

#[test]
fn checksum_mismatch_is_not_restorable() {
    let plan = RestorePlan {
        backup: completed_backup(),
        expected_checksum: Some("sha256:different".to_string()),
        now_unix_ms: 2_000,
    };

    assert_eq!(plan.validate(), Err(RestorePlanError::ChecksumMismatch));
}

#[test]
fn restore_error_responses_are_stable_and_redacted() {
    assert_eq!(
        RestorePlanError::BackupNotCompleted.to_string(),
        "backup is not restorable"
    );
    assert_eq!(
        RestorePlanError::BackupExpired.to_string(),
        "backup is not restorable"
    );
    assert_eq!(
        RestorePlanError::InvalidExpiry.to_string(),
        "backup is not restorable"
    );
    assert_eq!(
        RestorePlanError::MissingChecksum.to_string(),
        "backup integrity cannot be verified"
    );
    assert_eq!(
        RestorePlanError::MalformedChecksum.to_string(),
        "backup integrity cannot be verified"
    );
    assert_eq!(
        RestorePlanError::ChecksumMismatch.to_string(),
        "backup integrity verification failed"
    );

    let not_completed = plan_restore_error_response(&RestorePlanError::BackupNotCompleted);

    assert_eq!(not_completed.status, RestorePlanErrorStatus::Conflict);
    assert_eq!(not_completed.code, "restore_backup_not_completed");
    assert_eq!(not_completed.message, "backup is not restorable");
    assert!(!not_completed.message.contains("Failed"));

    let expired = plan_restore_error_response(&RestorePlanError::BackupExpired);
    assert_eq!(expired.status, RestorePlanErrorStatus::Conflict);
    assert_eq!(expired.code, "restore_backup_expired");
    assert_eq!(expired.message, "backup is not restorable");
    assert!(!expired.message.contains("10000"));

    let invalid_expiry = plan_restore_error_response(&RestorePlanError::InvalidExpiry);
    assert_eq!(invalid_expiry.status, RestorePlanErrorStatus::Conflict);
    assert_eq!(invalid_expiry.code, "restore_backup_expiry_invalid");
    assert_eq!(invalid_expiry.message, "backup is not restorable");
    assert!(!invalid_expiry.message.contains("1000"));

    let missing = plan_restore_error_response(&RestorePlanError::MissingChecksum);
    assert_eq!(missing.status, RestorePlanErrorStatus::Conflict);
    assert_eq!(missing.code, "restore_checksum_unavailable");
    assert_eq!(missing.message, "backup integrity cannot be verified");
    assert!(!missing.message.contains("sha256"));

    let malformed = plan_restore_error_response(&RestorePlanError::MalformedChecksum);
    assert_eq!(malformed.status, RestorePlanErrorStatus::Conflict);
    assert_eq!(malformed.code, "restore_checksum_invalid");
    assert_eq!(malformed.message, "backup integrity cannot be verified");
    assert!(!malformed.message.contains("sha256"));

    let mismatch = plan_restore_error_response(&RestorePlanError::ChecksumMismatch);
    assert_eq!(mismatch.status, RestorePlanErrorStatus::Conflict);
    assert_eq!(mismatch.code, "restore_checksum_mismatch");
    assert_eq!(mismatch.message, "backup integrity verification failed");
    assert!(!mismatch.message.contains("sha256"));
}
