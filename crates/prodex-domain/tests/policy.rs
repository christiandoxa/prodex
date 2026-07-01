use prodex_domain::{
    PolicyActivationError, PolicyActivationState, PolicyAuditAction, PolicyAuditRecord,
    PolicyCacheStatus, PolicyDigest, PolicyErrorStatus, PolicyRefreshDecision, PolicyRefreshWindow,
    PolicyRefreshWindowError, PolicyRevisionId, PolicySignature, PolicySnapshot, PolicyValidation,
    evaluate_policy_refresh, plan_policy_activation_error_response,
    plan_policy_refresh_decision_error_response, plan_policy_refresh_window_error_response,
    validate_policy_snapshot,
};

fn snapshot(payload: &str) -> PolicySnapshot<String> {
    PolicySnapshot::new(
        PolicyRevisionId::new(),
        1_725_000_000_000,
        payload.to_string(),
        PolicyDigest::new("sha256:abc123"),
        PolicySignature::new("sig:v1:opaque"),
    )
}

#[test]
fn policy_snapshot_requires_digest_and_signature_validation() {
    let rejected = validate_policy_snapshot(
        snapshot("candidate"),
        PolicyValidation {
            digest_matches: false,
            signature_verified: true,
        },
    )
    .unwrap_err();
    assert_eq!(rejected, PolicyActivationError::DigestMismatch);

    let rejected = validate_policy_snapshot(
        snapshot("candidate"),
        PolicyValidation {
            digest_matches: true,
            signature_verified: false,
        },
    )
    .unwrap_err();
    assert_eq!(rejected, PolicyActivationError::SignatureNotVerified);
}

#[test]
fn policy_validation_debug_output_is_stable_and_redacted() {
    let validation = PolicyValidation {
        digest_matches: false,
        signature_verified: true,
    };

    let rendered = format!("{validation:?}");
    assert!(!rendered.contains("false"));
    assert!(!rendered.contains("true"));
    assert_eq!(
        rendered,
        "PolicyValidation { digest_matches: \"<redacted>\", signature_verified: \"<redacted>\" }"
    );
}

#[test]
fn policy_snapshot_rejects_zero_issued_at_timestamp() {
    let zero_issued_at = PolicySnapshot::new(
        PolicyRevisionId::new(),
        0,
        "candidate".to_string(),
        PolicyDigest::new("sha256:abc123"),
        PolicySignature::new("sig:v1:opaque"),
    );

    assert_eq!(
        validate_policy_snapshot(zero_issued_at, PolicyValidation::verified()).unwrap_err(),
        PolicyActivationError::InvalidIssuedAt
    );
}

#[test]
fn policy_snapshot_rejects_empty_signature_material() {
    let unsigned = PolicySnapshot::new(
        PolicyRevisionId::new(),
        1,
        "candidate".to_string(),
        PolicyDigest::new("sha256:abc123"),
        PolicySignature::new(""),
    );

    assert_eq!(
        validate_policy_snapshot(unsigned, PolicyValidation::verified()).unwrap_err(),
        PolicyActivationError::EmptySignature
    );
}

#[test]
fn policy_snapshot_rejects_malformed_digest_and_signature_material() {
    let blank_signature = PolicySnapshot::new(
        PolicyRevisionId::new(),
        1,
        "candidate".to_string(),
        PolicyDigest::new("sha256:abc123"),
        PolicySignature::new(" "),
    );
    assert_eq!(
        validate_policy_snapshot(blank_signature, PolicyValidation::verified()).unwrap_err(),
        PolicyActivationError::InvalidSignature
    );

    let blank_digest = PolicySnapshot::new(
        PolicyRevisionId::new(),
        1,
        "candidate".to_string(),
        PolicyDigest::new(" "),
        PolicySignature::new("sig:v1:opaque"),
    );
    assert_eq!(
        validate_policy_snapshot(blank_digest, PolicyValidation::verified()).unwrap_err(),
        PolicyActivationError::InvalidDigest
    );

    let bad_digest = PolicySnapshot::new(
        PolicyRevisionId::new(),
        1,
        "candidate".to_string(),
        PolicyDigest::new("sha256:abc 123"),
        PolicySignature::new("sig:v1:opaque"),
    );
    assert_eq!(
        validate_policy_snapshot(bad_digest, PolicyValidation::verified()).unwrap_err(),
        PolicyActivationError::InvalidDigest
    );

    let bad_signature = PolicySnapshot::new(
        PolicyRevisionId::new(),
        1,
        "candidate".to_string(),
        PolicyDigest::new("sha256:abc123"),
        PolicySignature::new("sig:v1:\nopaque"),
    );
    assert_eq!(
        validate_policy_snapshot(bad_signature, PolicyValidation::verified()).unwrap_err(),
        PolicyActivationError::InvalidSignature
    );

    let padded_signature = PolicySnapshot::new(
        PolicyRevisionId::new(),
        1,
        "candidate".to_string(),
        PolicyDigest::new("sha256:abc123"),
        PolicySignature::new(" sig:v1:opaque"),
    );
    assert_eq!(
        validate_policy_snapshot(padded_signature, PolicyValidation::verified()).unwrap_err(),
        PolicyActivationError::InvalidSignature
    );
}

#[test]
fn policy_snapshot_rejects_overlong_integrity_metadata() {
    let overlong_digest = PolicySnapshot::new(
        PolicyRevisionId::new(),
        1,
        "candidate".to_string(),
        PolicyDigest::new("a".repeat(513)),
        PolicySignature::new("sig:v1:opaque"),
    );
    assert_eq!(
        validate_policy_snapshot(overlong_digest, PolicyValidation::verified()).unwrap_err(),
        PolicyActivationError::InvalidDigest
    );

    let overlong_signature = PolicySnapshot::new(
        PolicyRevisionId::new(),
        1,
        "candidate".to_string(),
        PolicyDigest::new("sha256:abc123"),
        PolicySignature::new("a".repeat(513)),
    );
    assert_eq!(
        validate_policy_snapshot(overlong_signature, PolicyValidation::verified()).unwrap_err(),
        PolicyActivationError::InvalidSignature
    );
}

#[test]
fn activation_accepts_only_validated_snapshots_and_tracks_revision() {
    let validated = validate_policy_snapshot(snapshot("active"), PolicyValidation::verified())
        .expect("validated policy");
    let revision = validated.revision_id();

    let state = PolicyActivationState::default().activate(validated);

    assert_eq!(state.active_revision_id(), Some(revision));
    assert_eq!(state.active().unwrap().payload(), "active");
    assert_eq!(state.last_known_good().unwrap().revision_id(), revision);
}

#[test]
fn refresh_failure_keeps_previous_last_known_good_revision() {
    let first = validate_policy_snapshot(snapshot("lkg"), PolicyValidation::verified()).unwrap();
    let first_revision = first.revision_id();
    let state = PolicyActivationState::default().activate(first);

    let failed_refresh_state = state.keep_last_known_good_after_refresh_failure();

    assert_eq!(
        failed_refresh_state.active_revision_id(),
        Some(first_revision)
    );
    assert_eq!(failed_refresh_state.active().unwrap().payload(), "lkg");
    assert_eq!(
        failed_refresh_state
            .last_known_good()
            .unwrap()
            .revision_id(),
        first_revision
    );
}

#[test]
fn policy_signature_debug_redacts_opaque_signature() {
    let digest = PolicyDigest::new("sha256:super-secret-digest");
    let signature = PolicySignature::new("sig:v1:super-secret-material");

    assert!(!format!("{digest:?}").contains("super-secret-digest"));
    assert_eq!(format!("{digest:?}"), "PolicyDigest(\"<redacted>\")");
    assert!(!format!("{signature:?}").contains("super-secret-material"));
}

#[test]
fn policy_snapshot_debug_output_is_stable_and_redacted() {
    let snapshot = snapshot("secret-policy-payload");
    let revision_id = snapshot.revision_id.to_string();
    let validated =
        validate_policy_snapshot(snapshot.clone(), PolicyValidation::verified()).unwrap();

    let rendered = format!("{snapshot:?} {validated:?}");
    for sensitive in [
        revision_id,
        "1725000000000".to_string(),
        "secret-policy-payload".to_string(),
        "sha256:abc123".to_string(),
        "sig:v1:opaque".to_string(),
    ] {
        assert!(
            !rendered.contains(&sensitive),
            "policy snapshot debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("payload: \"<redacted>\""));
    assert!(rendered.contains("PolicyDigest(\"<redacted>\")"));
    assert!(rendered.contains("PolicySignature(\"<redacted>\")"));
}

#[test]
fn policy_activation_state_debug_output_is_stable_and_redacted() {
    let validated = validate_policy_snapshot(
        snapshot("secret-policy-payload"),
        PolicyValidation::verified(),
    )
    .unwrap();
    let revision_id = validated.revision_id().to_string();
    let state = PolicyActivationState::default().activate(validated);

    let rendered = format!("{state:?}");
    for sensitive in [
        revision_id,
        "secret-policy-payload".to_string(),
        "sha256:abc123".to_string(),
        "sig:v1:opaque".to_string(),
    ] {
        assert!(
            !rendered.contains(&sensitive),
            "policy activation state debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert_eq!(
        rendered,
        "PolicyActivationState { has_active: true, has_last_known_good: true }"
    );
}

#[test]
fn policy_audit_record_debug_output_is_stable_and_redacted() {
    let revision_id = PolicyRevisionId::new();
    let record = PolicyAuditRecord {
        revision_id: Some(revision_id),
        action: PolicyAuditAction::Rejected,
        reason: Some("secret-signature-verifier-detail".to_string()),
    };

    let rendered = format!("{record:?}");
    assert!(!rendered.contains(&revision_id.to_string()));
    assert!(!rendered.contains("secret-signature-verifier-detail"));
    assert!(rendered.contains("action: Rejected"));
    assert!(rendered.contains("revision_id: Some(\"<redacted>\")"));
    assert!(rendered.contains("reason: Some(\"<redacted>\")"));
}

#[test]
fn policy_refresh_window_rejects_invalid_ordering() {
    assert_eq!(
        PolicyRefreshWindow::new(20, 10, 30).unwrap_err(),
        PolicyRefreshWindowError::RefreshAfterStale
    );
    assert_eq!(
        PolicyRefreshWindow::new(10, 30, 20).unwrap_err(),
        PolicyRefreshWindowError::StaleAfterExpiry
    );
}

#[test]
fn policy_cache_refresh_decision_tracks_refresh_stale_expiry_and_invalidation() {
    let validated = validate_policy_snapshot(snapshot("active"), PolicyValidation::verified())
        .expect("validated policy");
    let revision = validated.revision_id();
    let state = PolicyActivationState::default().activate(validated);
    let window = PolicyRefreshWindow::new(100, 200, 300).unwrap();
    let status = PolicyCacheStatus::from_state(&state, window);

    assert_eq!(
        evaluate_policy_refresh(&status, 99),
        PolicyRefreshDecision::UseActive
    );
    assert_eq!(
        evaluate_policy_refresh(&status, 100),
        PolicyRefreshDecision::RefreshAsync
    );
    assert_eq!(
        evaluate_policy_refresh(&status, 200),
        PolicyRefreshDecision::UseLastKnownGoodAndRefresh
    );
    assert_eq!(
        evaluate_policy_refresh(&status, 300),
        PolicyRefreshDecision::Expired
    );
    assert_eq!(
        evaluate_policy_refresh(&status.invalidate_revision(revision), 99),
        PolicyRefreshDecision::Invalidated
    );
}

#[test]
fn policy_cache_status_debug_output_is_stable_and_redacted() {
    let validated = validate_policy_snapshot(snapshot("active"), PolicyValidation::verified())
        .expect("validated policy");
    let revision = validated.revision_id();
    let state = PolicyActivationState::default().activate(validated);
    let window = PolicyRefreshWindow::new(100, 200, 300).unwrap();
    let rendered_window = format!("{window:?}");
    let status = PolicyCacheStatus::from_state(&state, window).invalidate_revision(revision);

    let rendered = format!("{rendered_window} {status:?}");
    for sensitive in [
        revision.to_string(),
        "100".into(),
        "200".into(),
        "300".into(),
    ] {
        assert!(
            !rendered.contains(&sensitive),
            "policy cache debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("has_active_revision: true"));
    assert!(rendered.contains("has_last_known_good_revision: true"));
    assert!(rendered.contains("has_invalidated_revision: true"));
    assert!(rendered.contains("refresh_after_unix_ms: \"<redacted>\""));
}

#[test]
fn policy_cache_stale_window_uses_only_usable_last_known_good_revision() {
    let validated = validate_policy_snapshot(snapshot("active"), PolicyValidation::verified())
        .expect("validated policy");
    let revision = validated.revision_id();
    let state = PolicyActivationState::default().activate(validated);
    let window = PolicyRefreshWindow::new(100, 200, 300).unwrap();
    let mut status = PolicyCacheStatus::from_state(&state, window);

    status.last_known_good_revision_id = None;
    assert_eq!(
        evaluate_policy_refresh(&status, 200),
        PolicyRefreshDecision::RefreshAsync
    );

    status.last_known_good_revision_id = Some(revision);
    status.active_revision_id = Some(PolicyRevisionId::new());
    status.invalidated_revision_id = Some(revision);
    assert_eq!(
        evaluate_policy_refresh(&status, 200),
        PolicyRefreshDecision::RefreshAsync
    );
}

#[test]
fn policy_error_responses_are_stable_and_redacted() {
    for error in [
        PolicyActivationError::InvalidIssuedAt,
        PolicyActivationError::EmptyDigest,
        PolicyActivationError::InvalidDigest,
        PolicyActivationError::EmptySignature,
        PolicyActivationError::InvalidSignature,
        PolicyActivationError::DigestMismatch,
        PolicyActivationError::SignatureNotVerified,
    ] {
        let rendered = error.to_string();
        assert_eq!(rendered, "policy snapshot is invalid");
        assert!(!rendered.contains("digest"));
        assert!(!rendered.contains("signature"));
        assert!(!rendered.contains("timestamp"));
    }

    let signature_error =
        plan_policy_activation_error_response(&PolicyActivationError::SignatureNotVerified);
    assert_eq!(signature_error.status, PolicyErrorStatus::InvalidPolicy);
    assert_eq!(signature_error.code, "policy_signature_verification_failed");
    assert_eq!(
        signature_error.message,
        "policy snapshot failed signature verification"
    );

    let digest_shape = plan_policy_activation_error_response(&PolicyActivationError::InvalidDigest);
    assert_eq!(digest_shape.status, PolicyErrorStatus::InvalidPolicy);
    assert_eq!(digest_shape.code, "policy_digest_invalid");
    assert_eq!(
        digest_shape.message,
        "policy snapshot has invalid integrity metadata"
    );

    let signature_shape =
        plan_policy_activation_error_response(&PolicyActivationError::InvalidSignature);
    assert_eq!(signature_shape.status, PolicyErrorStatus::InvalidPolicy);
    assert_eq!(signature_shape.code, "policy_signature_invalid");
    assert_eq!(
        signature_shape.message,
        "policy snapshot has invalid signature metadata"
    );

    let issued_at = plan_policy_activation_error_response(&PolicyActivationError::InvalidIssuedAt);
    assert_eq!(issued_at.status, PolicyErrorStatus::InvalidPolicy);
    assert_eq!(issued_at.code, "policy_issued_at_invalid");
    assert_eq!(issued_at.message, "policy snapshot timestamp is invalid");

    let window_error =
        plan_policy_refresh_window_error_response(&PolicyRefreshWindowError::RefreshAfterStale);
    assert_eq!(window_error.status, PolicyErrorStatus::InvalidRequest);
    assert_eq!(window_error.code, "policy_refresh_window_invalid");
    assert_eq!(window_error.message, "policy refresh window is invalid");

    let expired =
        plan_policy_refresh_decision_error_response(PolicyRefreshDecision::Expired).unwrap();
    assert_eq!(expired.status, PolicyErrorStatus::ServiceUnavailable);
    assert_eq!(expired.code, "policy_cache_expired");
    assert_eq!(expired.message, "policy cache is unavailable");

    let invalidated =
        plan_policy_refresh_decision_error_response(PolicyRefreshDecision::Invalidated).unwrap();
    assert_eq!(invalidated.status, PolicyErrorStatus::ServiceUnavailable);
    assert_eq!(invalidated.code, "policy_cache_invalidated");
    assert_eq!(invalidated.message, "policy cache is unavailable");

    assert!(
        plan_policy_refresh_decision_error_response(PolicyRefreshDecision::RefreshAsync).is_none()
    );

    let rendered = format!(
        "{signature_error:?} {digest_shape:?} {signature_shape:?} {issued_at:?} {window_error:?} {expired:?} {invalidated:?}"
    );
    for sensitive in [
        "sha256:abc123",
        "sig:v1:opaque",
        "1_725_000_000_000",
        "refresh_after",
        "stale_after",
        "expires_after",
        "revision",
        "last_known_good",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "policy response leaked sensitive token {sensitive}: {rendered}"
        );
    }
}
