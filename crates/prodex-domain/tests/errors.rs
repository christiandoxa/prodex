use prodex_domain::{
    AuditEventId, ErrorCategory, ErrorCode, ErrorCodeError, ErrorCodeErrorStatus, ErrorEnvelope,
    ErrorMetadata, RequestId, TenantId, plan_error_code_error_response,
};

#[test]
fn error_envelope_serializes_stable_machine_readable_shape() {
    let request_id = RequestId::new();
    let tenant_id = TenantId::new();
    let audit_event_id = AuditEventId::new();
    let envelope = ErrorEnvelope::new(
        ErrorCode::new("tenant.cross_tenant_access_denied"),
        ErrorCategory::TenantIsolation,
        "cross-tenant access denied",
        ErrorMetadata::new(
            Some(request_id),
            Some(tenant_id),
            Some(audit_event_id),
            false,
        ),
    );

    let encoded = serde_json::to_value(&envelope).unwrap();

    assert_eq!(encoded["version"], 1);
    assert_eq!(encoded["code"], "tenant.cross_tenant_access_denied");
    assert_eq!(encoded["category"], "tenant_isolation");
    assert_eq!(encoded["metadata"]["request_id"], request_id.to_string());
    assert_eq!(encoded["metadata"]["tenant_id"], tenant_id.to_string());
    assert_eq!(
        encoded["metadata"]["audit_event_id"],
        audit_event_id.to_string()
    );
    assert_eq!(encoded["metadata"]["retryable"], false);
}

#[test]
fn error_metadata_debug_output_is_stable_and_redacted() {
    let request_id = RequestId::new();
    let tenant_id = TenantId::new();
    let audit_event_id = AuditEventId::new();
    let metadata = ErrorMetadata::new(
        Some(request_id),
        Some(tenant_id),
        Some(audit_event_id),
        true,
    );

    let rendered = format!("{metadata:?}");
    assert!(!rendered.contains(&request_id.to_string()));
    assert!(!rendered.contains(&tenant_id.to_string()));
    assert!(!rendered.contains(&audit_event_id.to_string()));
    assert!(rendered.contains("\"<redacted>\""));
    assert!(rendered.contains("retryable: true"));
}

#[test]
fn error_envelope_debug_output_is_stable_and_redacted() {
    let request_id = RequestId::new();
    let tenant_id = TenantId::new();
    let audit_event_id = AuditEventId::new();
    let envelope = ErrorEnvelope::new(
        ErrorCode::new("tenant.cross_tenant_access_denied"),
        ErrorCategory::TenantIsolation,
        "cross-tenant access denied",
        ErrorMetadata::new(
            Some(request_id),
            Some(tenant_id),
            Some(audit_event_id),
            false,
        ),
    );

    let rendered = format!("{envelope:?}");
    let request_id = request_id.to_string();
    let tenant_id = tenant_id.to_string();
    let audit_event_id = audit_event_id.to_string();
    for sensitive in [
        "tenant.cross_tenant_access_denied",
        "cross-tenant access denied",
        request_id.as_str(),
        tenant_id.as_str(),
        audit_event_id.as_str(),
    ] {
        assert!(
            !rendered.contains(sensitive),
            "error envelope debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("category: TenantIsolation"));
    assert!(rendered.contains("\"<redacted>\""));
}

#[test]
fn error_envelope_redacts_secret_like_messages() {
    for message in [
        "Authorization: Bearer secret-token leaked",
        "api key abc123 failed",
        "x-api-key abc123 failed",
        "chatgpt-account-id account-secret failed",
        "password hunter2 failed",
    ] {
        let envelope = ErrorEnvelope::new(
            ErrorCode::new("auth.invalid_credential"),
            ErrorCategory::Authentication,
            message,
            ErrorMetadata::new(None, None, None, false),
        );
        let encoded = serde_json::to_string(&envelope).unwrap();

        assert_eq!(envelope.message, "request failed");
        assert!(!encoded.contains("Bearer"));
        assert!(!encoded.contains("secret-token"));
        assert!(!encoded.contains("abc123"));
        assert!(!encoded.contains("hunter2"));
        assert!(!encoded.contains("Authorization"));
    }
}

#[test]
fn error_envelope_replaces_unstable_public_messages() {
    for message in [
        String::new(),
        "   ".to_string(),
        "upstream\nunavailable".to_string(),
        "upstream\tunavailable".to_string(),
        "echec upstream: accès refusé".to_string(),
        "x".repeat(513),
    ] {
        let envelope = ErrorEnvelope::new(
            ErrorCode::new("upstream.unavailable"),
            ErrorCategory::Unavailable,
            message,
            ErrorMetadata::new(None, None, None, false),
        );

        assert_eq!(envelope.message, "request failed");
    }
}

#[test]
fn error_envelope_keeps_printable_public_messages() {
    let envelope = ErrorEnvelope::new(
        ErrorCode::new("upstream.unavailable"),
        ErrorCategory::Unavailable,
        "upstream unavailable",
        ErrorMetadata::new(None, None, None, false),
    );

    assert_eq!(envelope.message, "upstream unavailable");
}

#[test]
fn error_envelope_replaces_invalid_machine_readable_codes() {
    let envelope = ErrorEnvelope::new(
        ErrorCode::new("Tenant/Bad Code"),
        ErrorCategory::Internal,
        "internal error",
        ErrorMetadata::new(None, None, None, false),
    );
    let encoded = serde_json::to_string(&envelope).unwrap();

    assert_eq!(envelope.code.as_str(), "internal.error");
    assert!(!encoded.contains("Tenant/Bad Code"));
}

#[test]
fn error_envelope_marks_retryable_unavailable_errors() {
    let envelope = ErrorEnvelope::new(
        ErrorCode::new("upstream.unavailable"),
        ErrorCategory::Unavailable,
        "upstream unavailable",
        ErrorMetadata::new(Some(RequestId::new()), None, None, true),
    );

    assert!(envelope.metadata.retryable);
    assert_eq!(envelope.version, ErrorEnvelope::CURRENT_VERSION);
}

#[test]
fn error_code_try_new_accepts_stable_machine_readable_codes() {
    let code = ErrorCode::try_new("tenant.cross_tenant_access_denied").unwrap();

    assert_eq!(code.as_str(), "tenant.cross_tenant_access_denied");
    assert_eq!(code.validate(), Ok(()));
}

#[test]
fn error_code_debug_output_is_stable_and_redacted() {
    let code = ErrorCode::new("tenant.cross_tenant_access_denied");

    let rendered = format!("{code:?}");
    assert!(!rendered.contains("tenant.cross_tenant_access_denied"));
    assert_eq!(rendered, "ErrorCode(\"<redacted>\")");
}

#[test]
fn error_code_try_new_rejects_unstable_or_path_like_codes() {
    assert_eq!(ErrorCode::try_new(" "), Err(ErrorCodeError::Empty));
    assert_eq!(
        ErrorCode::try_new("tenant..cross_tenant_access_denied"),
        Err(ErrorCodeError::EmptySegment)
    );
    assert_eq!(
        ErrorCode::try_new("Tenant.CrossTenantAccessDenied"),
        Err(ErrorCodeError::InvalidCharacter)
    );
    assert_eq!(
        ErrorCode::try_new("tenant/cross-tenant-access-denied"),
        Err(ErrorCodeError::InvalidCharacter)
    );
    assert_eq!(
        ErrorCode::try_new("a".repeat(129)),
        Err(ErrorCodeError::TooLong { length: 129 })
    );
}

#[test]
fn error_code_error_responses_are_stable_and_redacted() {
    let too_long_error = ErrorCodeError::TooLong { length: 1_000 };
    assert_eq!(too_long_error.to_string(), "error code is invalid");
    assert!(!too_long_error.to_string().contains("1000"));
    assert_eq!(
        format!("{too_long_error:?}"),
        "TooLong { length: \"<redacted>\" }"
    );
    let too_long = plan_error_code_error_response(&too_long_error);

    assert_eq!(too_long.status, ErrorCodeErrorStatus::InvalidRequest);
    assert_eq!(too_long.code, "error_code_invalid");
    assert_eq!(too_long.message, "error code is invalid");

    let empty = plan_error_code_error_response(&ErrorCodeError::Empty);
    assert_eq!(empty.status, ErrorCodeErrorStatus::InvalidRequest);
    assert_eq!(empty.code, "error_code_required");

    let rendered = format!("{too_long:?} {empty:?}");
    for sensitive in [
        "1000",
        "tenant.cross_tenant_access_denied",
        "Tenant.CrossTenantAccessDenied",
        "tenant/cross-tenant-access-denied",
        "api key",
        "Bearer",
        "secret-token",
        "request_id",
        "tenant_id",
        "audit_event_id",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "error-code response leaked sensitive token {sensitive}: {rendered}"
        );
    }
}
