use std::str::FromStr;

use prodex_domain::{
    AuditEventId, CallId, IdParseErrorStatus, PolicyRevisionId, PrincipalId, RequestId,
    ReservationId, TenantId, VirtualKeyId, plan_id_parse_error_response,
};

#[test]
fn generated_domain_ids_are_uuid_v7_and_unique_per_type() {
    let request = RequestId::new();
    let call = CallId::new();
    let reservation = ReservationId::new();

    assert_eq!(request.as_uuid().get_version_num(), 7);
    assert_eq!(call.as_uuid().get_version_num(), 7);
    assert_eq!(reservation.as_uuid().get_version_num(), 7);
    assert_ne!(request.to_string(), call.to_string());
    assert_ne!(request.to_string(), reservation.to_string());
}

#[test]
fn domain_ids_round_trip_as_strings() {
    let tenant = TenantId::new();
    let encoded = serde_json::to_string(&tenant).unwrap();
    let decoded: TenantId = serde_json::from_str(&encoded).unwrap();

    assert_eq!(tenant, decoded);
    assert_eq!(TenantId::from_str(&tenant.to_string()).unwrap(), tenant);
}

#[test]
fn domain_id_debug_output_is_stable_and_redacted() {
    let tenant = TenantId::new();
    let request = RequestId::new();
    let call = CallId::new();

    for (rendered, raw) in [
        (format!("{tenant:?}"), tenant.to_string()),
        (format!("{request:?}"), request.to_string()),
        (format!("{call:?}"), call.to_string()),
    ] {
        assert!(
            !rendered.contains(&raw),
            "domain id debug output leaked raw ID {raw}: {rendered}"
        );
        assert!(rendered.contains("\"<redacted>\""));
    }
}

#[test]
fn parse_errors_preserve_identifier_kind() {
    let err = PrincipalId::from_str("not-a-uuid").unwrap_err();

    assert_eq!(err.kind(), "principal_id");
}

#[test]
fn id_parse_error_debug_output_is_stable_and_redacted() {
    let err = TenantId::from_str("not-a-uuid").unwrap_err();
    let rendered = format!("{err:?}");

    assert!(!rendered.contains("tenant_id"));
    assert_eq!(rendered, "IdParseError { kind: \"<redacted>\" }");
}

#[test]
fn id_parse_error_display_output_is_stable_and_redacted() {
    let err = TenantId::from_str("not-a-uuid").unwrap_err();

    assert!(!err.to_string().contains("tenant_id"));
    assert_eq!(err.to_string(), "identifier is invalid");
}

#[test]
fn enterprise_identifier_set_is_available() {
    let _tenant = TenantId::new();
    let _principal = PrincipalId::new();
    let _request = RequestId::new();
    let _call = CallId::new();
    let _reservation = ReservationId::new();
    let _virtual_key = VirtualKeyId::new();
    let _policy_revision = PolicyRevisionId::new();
    let _audit_event = AuditEventId::new();
}

#[test]
fn id_parse_error_response_is_stable_and_redacted() {
    let err = PrincipalId::from_str("principal:secret:not-a-uuid").unwrap_err();
    assert!(!err.to_string().contains("principal:secret:not-a-uuid"));
    assert!(!err.to_string().contains("secret"));
    assert!(!format!("{err:?}").contains("principal:secret:not-a-uuid"));
    assert!(!format!("{err:?}").contains("secret"));
    let response = plan_id_parse_error_response(&err);

    assert_eq!(response.status, IdParseErrorStatus::InvalidRequest);
    assert_eq!(response.code, "principal_id_invalid");
    assert_eq!(response.message, "identifier is invalid");

    let tenant_err = TenantId::from_str("tenant-secret").unwrap_err();
    assert!(!tenant_err.to_string().contains("tenant-secret"));
    let tenant_response = plan_id_parse_error_response(&tenant_err);
    assert_eq!(tenant_response.code, "tenant_id_invalid");

    let rendered = format!("{response:?} {tenant_response:?}");
    for sensitive in [
        "principal:secret:not-a-uuid",
        "tenant-secret",
        "not-a-uuid",
        "secret",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "ID parse response leaked sensitive token {sensitive}: {rendered}"
        );
    }
}
