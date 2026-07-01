use prodex_domain::{
    AuditEventId, CallId, CorrelationContext, RequestId, TenantId, TraceId, TraceIdError,
    TraceIdErrorStatus, plan_trace_id_error_response,
};

#[test]
fn trace_id_rejects_empty_overlong_and_invalid_values() {
    assert_eq!(TraceId::new(""), Err(TraceIdError::Empty));
    assert_eq!(TraceId::new(" "), Err(TraceIdError::InvalidCharacter));
    assert_eq!(
        TraceId::new("a".repeat(129)),
        Err(TraceIdError::TooLong { length: 129 })
    );
    assert_eq!(
        TraceId::new("trace id with spaces"),
        Err(TraceIdError::InvalidCharacter)
    );
    assert_eq!(
        TraceId::new(" abcdef0123456789"),
        Err(TraceIdError::InvalidCharacter)
    );
}

#[test]
fn trace_id_normalizes_to_lowercase_without_accepting_secret_like_text() {
    let trace_id = TraceId::new("ABCDEF0123456789").unwrap();

    assert_eq!(trace_id.as_str(), "abcdef0123456789");
}

#[test]
fn w3c_trace_id_requires_32_hex_characters_and_rejects_all_zero() {
    assert_eq!(TraceId::new_w3c(""), Err(TraceIdError::Empty));
    assert_eq!(TraceId::new_w3c(" "), Err(TraceIdError::InvalidCharacter));
    assert_eq!(
        TraceId::new_w3c("abcdef0123456789"),
        Err(TraceIdError::InvalidCharacter)
    );
    assert_eq!(
        TraceId::new_w3c("abcdef0123456789abcdef01234567_"),
        Err(TraceIdError::InvalidCharacter)
    );
    assert_eq!(
        TraceId::new_w3c("00000000000000000000000000000000"),
        Err(TraceIdError::InvalidCharacter)
    );
    assert_eq!(
        TraceId::new_w3c(" abcdef0123456789abcdef0123456789"),
        Err(TraceIdError::InvalidCharacter)
    );

    let trace_id = TraceId::new_w3c("ABCDEF0123456789ABCDEF0123456789").unwrap();
    assert_eq!(trace_id.as_str(), "abcdef0123456789abcdef0123456789");
}

#[test]
fn trace_id_debug_output_is_stable_and_redacted() {
    let trace_id = TraceId::new("abcdef0123456789").unwrap();

    let rendered = format!("{trace_id:?}");
    assert!(!rendered.contains("abcdef0123456789"));
    assert_eq!(rendered, "TraceId(\"<redacted>\")");
}

#[test]
fn correlation_context_chains_request_call_trace_tenant_and_audit_ids() {
    let request_id = RequestId::new();
    let call_id = CallId::new();
    let trace_id = TraceId::new("abcdef0123456789").unwrap();
    let tenant_id = TenantId::new();
    let audit_event_id = AuditEventId::new();

    let context = CorrelationContext::new(request_id)
        .with_call_id(call_id)
        .with_trace_id(trace_id.clone())
        .with_tenant_id(tenant_id)
        .with_audit_event_id(audit_event_id);

    assert_eq!(context.request_id, request_id);
    assert_eq!(context.call_id, Some(call_id));
    assert_eq!(context.trace_id, Some(trace_id));
    assert_eq!(context.tenant_id, Some(tenant_id));
    assert_eq!(context.audit_event_id, Some(audit_event_id));
}

#[test]
fn correlation_context_debug_output_is_stable_and_redacted() {
    let request_id = RequestId::new();
    let call_id = CallId::new();
    let trace_id = TraceId::new("abcdef0123456789").unwrap();
    let tenant_id = TenantId::new();
    let audit_event_id = AuditEventId::new();
    let context = CorrelationContext::new(request_id)
        .with_call_id(call_id)
        .with_trace_id(trace_id.clone())
        .with_tenant_id(tenant_id)
        .with_audit_event_id(audit_event_id);

    let rendered = format!("{context:?}");
    for sensitive in [
        request_id.to_string(),
        call_id.to_string(),
        trace_id.as_str().to_string(),
        tenant_id.to_string(),
        audit_event_id.to_string(),
    ] {
        assert!(
            !rendered.contains(&sensitive),
            "correlation context debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("\"<redacted>\""));
}

#[test]
fn correlation_context_serializes_for_log_or_trace_propagation() {
    let request_id = RequestId::new();
    let context = CorrelationContext::new(request_id)
        .with_trace_id(TraceId::new("abcdef0123456789").unwrap());

    let encoded = serde_json::to_string(&context).unwrap();

    assert!(encoded.contains(&request_id.to_string()));
    assert!(encoded.contains("abcdef0123456789"));
    assert!(!encoded.contains("Bearer"));
    assert!(!encoded.contains("Authorization"));
}

#[test]
fn trace_id_error_responses_are_stable_and_redacted() {
    let too_long = TraceIdError::TooLong { length: 256 };
    assert_eq!(too_long.to_string(), "trace id is invalid");
    assert!(!too_long.to_string().contains("256"));
    assert_eq!(
        format!("{too_long:?}"),
        "TooLong { length: \"<redacted>\" }"
    );
    let response = plan_trace_id_error_response(&too_long);

    assert_eq!(response.status, TraceIdErrorStatus::InvalidRequest);
    assert_eq!(response.code, "trace_id_invalid");
    assert_eq!(response.message, "trace id is invalid");

    let empty = plan_trace_id_error_response(&TraceIdError::Empty);
    assert_eq!(empty.status, TraceIdErrorStatus::InvalidRequest);
    assert_eq!(empty.code, "trace_id_required");

    let rendered = format!("{response:?} {empty:?}");
    for sensitive in [
        "256",
        "abcdef0123456789",
        "request_id",
        "call_id",
        "tenant_id",
        "audit_event_id",
        "traceparent",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "trace id response leaked sensitive token {sensitive}: {rendered}"
        );
    }
}
