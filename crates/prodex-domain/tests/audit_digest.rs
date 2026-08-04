use prodex_domain::{
    AuditAction, AuditDigest, AuditEvent, AuditEventId, AuditOutcome, AuditReasonDetail,
    AuditResource, PrincipalId, TenantId, compute_audit_chain_digest,
    normalize_audit_reason_detail,
};
use uuid::Uuid;

fn fixed_event() -> AuditEvent {
    AuditEvent {
        id: AuditEventId::from_uuid(
            Uuid::parse_str("018f0000-0000-7000-8000-000000000001").unwrap(),
        ),
        occurred_at_unix_ms: 1_725_000_000_123,
        tenant_id: TenantId::from_uuid(
            Uuid::parse_str("018f0000-0000-7000-8000-000000000002").unwrap(),
        ),
        principal_id: PrincipalId::from_uuid(
            Uuid::parse_str("018f0000-0000-7000-8000-000000000003").unwrap(),
        ),
        action: AuditAction::new("control_plane.virtual_key.rotate_secret"),
        resource: AuditResource::new(
            "virtual_key",
            Some("key-example"),
            Some(TenantId::from_uuid(
                Uuid::parse_str("018f0000-0000-7000-8000-000000000004").unwrap(),
            )),
        ),
        outcome: AuditOutcome::Success,
        reason_code: Some("mutation_committed".to_string()),
        reason_detail: None,
    }
}

#[test]
fn audit_reason_detail_is_unicode_safe_redacted_and_backward_compatible() {
    let detail = normalize_audit_reason_detail(
        "  incident réponse api_key = top-secret authorization: Bearer other-secret \u{0000} ",
    )
    .unwrap();
    assert_eq!(
        detail,
        "incident réponse api_key=<redacted> authorization: Bearer <redacted>"
    );
    assert_eq!(
        normalize_audit_reason_detail("authorization = Bearer arbitrary-secret").as_deref(),
        Some("authorization=<redacted>")
    );
    assert_eq!(
        normalize_audit_reason_detail("password top-secret").as_deref(),
        Some("password <redacted>")
    );
    assert_eq!(
        normalize_audit_reason_detail("authorization Bearer arbitrary-secret").as_deref(),
        Some("authorization Bearer <redacted>")
    );
    let typed = AuditReasonDetail::new(detail.clone()).unwrap();
    assert!(typed.len() <= AuditEvent::MAX_REASON_DETAIL_BYTES);
    assert!(!format!("{typed:?}").contains("top-secret"));
    assert!(AuditReasonDetail::new("x".repeat(512)).is_ok());
    assert!(AuditReasonDetail::new("x".repeat(513)).is_err());
    assert!(AuditReasonDetail::new("é".repeat(256)).is_ok());
    assert!(AuditReasonDetail::new(format!("{}x", "é".repeat(256))).is_err());

    let event = fixed_event().with_reason_detail(Some(typed));
    assert!(!format!("{event:?}").contains("top-secret"));
    let encoded = serde_json::to_value(&event).unwrap();
    let round_tripped: AuditEvent = serde_json::from_value(encoded).unwrap();
    assert_eq!(round_tripped.reason_detail, event.reason_detail);

    let mut legacy = serde_json::to_value(fixed_event()).unwrap();
    legacy
        .as_object_mut()
        .expect("audit event should encode as an object")
        .remove("reason_detail");
    let decoded: AuditEvent = serde_json::from_value(legacy).unwrap();
    assert_eq!(decoded.reason_detail, None);
}

#[test]
fn audit_chain_digest_matches_canonical_vector() {
    let previous =
        AuditDigest::new("sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .unwrap();

    assert_eq!(
        compute_audit_chain_digest(Some(&previous), &fixed_event()).as_str(),
        "sha256:381c7db58c786897226e0f10f38ae5d32dc7051bafcedc097294aed6a6d13b8d"
    );
}

#[test]
fn audit_chain_digest_binds_previous_digest_and_every_event_field() {
    let previous = AuditDigest::new("sha256:previous").unwrap();
    let baseline_event = fixed_event();
    let baseline = compute_audit_chain_digest(Some(&previous), &baseline_event);
    assert_ne!(baseline, compute_audit_chain_digest(None, &baseline_event));

    let mut variants = Vec::new();
    let mut event = baseline_event.clone();
    event.id = AuditEventId::from_uuid(Uuid::nil());
    variants.push(event);
    let mut event = baseline_event.clone();
    event.occurred_at_unix_ms += 1;
    variants.push(event);
    let mut event = baseline_event.clone();
    event.tenant_id = TenantId::from_uuid(Uuid::nil());
    variants.push(event);
    let mut event = baseline_event.clone();
    event.principal_id = PrincipalId::from_uuid(Uuid::nil());
    variants.push(event);
    let mut event = baseline_event.clone();
    event.action = AuditAction::new("control_plane.virtual_key.delete");
    variants.push(event);
    let mut event = baseline_event.clone();
    event.resource.kind = "user".to_string();
    variants.push(event);
    let mut event = baseline_event.clone();
    event.resource.id = Some("other-key".to_string());
    variants.push(event);
    let mut event = baseline_event.clone();
    event.resource.tenant_id = None;
    variants.push(event);
    let mut event = baseline_event.clone();
    event.outcome = AuditOutcome::Failed;
    variants.push(event);
    let mut event = baseline_event;
    event.reason_code = None;
    variants.push(event);
    let mut event = fixed_event();
    event.reason_detail = Some(AuditReasonDetail::new("break glass incident").unwrap());
    variants.push(event);

    for event in variants {
        assert_ne!(
            baseline,
            compute_audit_chain_digest(Some(&previous), &event)
        );
    }
}
