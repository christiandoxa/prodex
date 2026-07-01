use prodex_authz::BoundaryKind;
use prodex_domain::{
    BudgetLimit, BudgetSnapshot, CallId, CredentialScope, IdempotencyKey, Principal, PrincipalId,
    PrincipalKind, RequestId, ReservationId, ReservationReconciliationReason, ReservationRecord,
    ReservationRequest, Role, SecretRef, TenantContext, TenantId, TenantScopedResource,
    UsageAmount,
};
use prodex_gateway_core::{
    GatewayAdmissionError, GatewayAdmissionErrorStatus, GatewayAdmissionRequest,
    GatewayAuthorizationBoundaryError, GatewayExpiredReservationRecoveryError,
    GatewayExpiredReservationRecoveryErrorStatus, GatewayExpiredReservationRecoveryRequest,
    GatewayQuotaReadAuthorizationError, GatewayQuotaReadAuthorizationErrorStatus,
    GatewayQuotaReadAuthorizationRequest, GatewayUsageReconciliationError,
    GatewayUsageReconciliationErrorStatus, GatewayUsageReconciliationRequest,
    plan_data_plane_admission, plan_gateway_admission_error_response,
    plan_gateway_expired_reservation_recovery,
    plan_gateway_expired_reservation_recovery_error_response,
    plan_gateway_quota_read_authorization, plan_gateway_quota_read_authorization_error_response,
    plan_gateway_usage_reconciliation, plan_gateway_usage_reconciliation_error_response,
};
use prodex_observability::TraceContext;
use prodex_provider_core::{ProviderEndpoint, ProviderId, ProviderWireFormat};
use prodex_provider_spi::{ProviderInvocation, ProviderRoute, ProviderStreamMode};
use prodex_storage::{
    AtomicReservationCommand, ExpiredReservationRecoveryCommand, TenantStorageKey,
    UsageReconciliationCommand,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Resource {
    tenant_id: TenantId,
}

impl TenantScopedResource for Resource {
    fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }
}

fn principal(tenant_id: TenantId, scope: CredentialScope, role: Role) -> Principal {
    Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::VirtualKey,
        role,
        scope,
    )
}

fn request(tenant_id: TenantId, principal: Principal) -> GatewayAdmissionRequest<Resource> {
    let call_id = CallId::new();
    let reservation_id = ReservationId::new();
    let request_id = RequestId::new();
    GatewayAdmissionRequest {
        tenant: TenantContext { tenant_id },
        principal: principal.clone(),
        resource: Resource { tenant_id },
        reservation: AtomicReservationCommand {
            storage_key: TenantStorageKey::tenant(tenant_id),
            idempotency_key: IdempotencyKey::from_call_reservation(call_id, reservation_id),
            snapshot: BudgetSnapshot::default(),
            limit: BudgetLimit::new(1_000, 10_000),
            request: ReservationRequest {
                tenant_id,
                call_id,
                reservation_id,
                estimate: UsageAmount::new(25, 250),
            },
            created_at_unix_ms: 1_000,
            ttl_ms: 60_000,
        },
        provider_invocation: ProviderInvocation {
            tenant: TenantContext { tenant_id },
            principal,
            request_id,
            call_id,
            route: ProviderRoute::new(
                ProviderId::OpenAi,
                ProviderEndpoint::Responses,
                ProviderWireFormat::OpenAiResponses,
                "gpt-5.3-codex",
            )
            .unwrap(),
            credential_ref: SecretRef::new("external-secret", "openai-prod", Some("v1")),
            stream_mode: ProviderStreamMode::Streaming,
            estimated_usage: UsageAmount::new(25, 250),
        },
        trace_context: TraceContext::parse_traceparent(
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
        )
        .unwrap(),
    }
}

fn quota_authorization_request(
    tenant_id: TenantId,
    principal: Principal,
) -> GatewayQuotaReadAuthorizationRequest<Resource> {
    GatewayQuotaReadAuthorizationRequest {
        tenant: TenantContext { tenant_id },
        principal,
        resource: Resource { tenant_id },
        request_id: RequestId::new(),
        trace_context: TraceContext::parse_traceparent(
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
        )
        .unwrap(),
    }
}

fn reconciliation_request(
    tenant_id: TenantId,
    reserved: UsageAmount,
    actual: UsageAmount,
) -> GatewayUsageReconciliationRequest {
    let reservation = ReservationRequest {
        tenant_id,
        call_id: CallId::new(),
        reservation_id: ReservationId::new(),
        estimate: reserved,
    };
    GatewayUsageReconciliationRequest {
        tenant: TenantContext { tenant_id },
        request_id: RequestId::new(),
        reconciliation: UsageReconciliationCommand {
            storage_key: TenantStorageKey::tenant(tenant_id),
            snapshot: BudgetSnapshot {
                reserved,
                committed: UsageAmount::new(1, 10),
            },
            record: ReservationRecord::from_request(reservation, 1_000, 60_000).unwrap(),
            actual,
            reason: ReservationReconciliationReason::Completed,
        },
        trace_context: TraceContext::parse_traceparent(
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
        )
        .unwrap(),
    }
}

fn expired_recovery_request(
    tenant_id: TenantId,
    reserved: UsageAmount,
    now_unix_ms: u64,
) -> GatewayExpiredReservationRecoveryRequest {
    let reservation = ReservationRequest {
        tenant_id,
        call_id: CallId::new(),
        reservation_id: ReservationId::new(),
        estimate: reserved,
    };
    GatewayExpiredReservationRecoveryRequest {
        tenant: TenantContext { tenant_id },
        request_id: RequestId::new(),
        recovery: ExpiredReservationRecoveryCommand {
            storage_key: TenantStorageKey::tenant(tenant_id),
            snapshot: BudgetSnapshot {
                reserved,
                committed: UsageAmount::new(1, 10),
            },
            record: ReservationRecord::from_request(reservation, 1_000, 500).unwrap(),
            now_unix_ms,
        },
        trace_context: TraceContext::parse_traceparent(
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
        )
        .unwrap(),
    }
}

#[test]
fn gateway_admission_plans_authorized_reservation_provider_and_spans() {
    let tenant_id = TenantId::new();
    let request = request(
        tenant_id,
        principal(tenant_id, CredentialScope::DataPlane, Role::Operator),
    );

    let plan = plan_data_plane_admission(request).unwrap();

    assert_eq!(plan.tenant.tenant_id, tenant_id);
    assert_eq!(
        plan.authorization_boundary,
        BoundaryKind::DataPlaneInference
    );
    assert_eq!(plan.reservation.storage_key.tenant_id, tenant_id);
    assert_eq!(
        plan.reservation.ledger_event.amount,
        UsageAmount::new(25, 250)
    );
    assert_eq!(plan.provider_invocation.tenant.tenant_id, tenant_id);
    assert_eq!(plan.spans.len(), 3);
}

#[test]
fn gateway_authorization_boundary_error_display_is_stable_and_redacted() {
    assert_eq!(
        GatewayAuthorizationBoundaryError::Unavailable.to_string(),
        "gateway authorization is temporarily unavailable"
    );
}

#[test]
fn gateway_admission_rejects_control_plane_scope_for_inference() {
    let tenant_id = TenantId::new();
    let request = request(
        tenant_id,
        principal(tenant_id, CredentialScope::ControlPlane, Role::Admin),
    );

    assert!(matches!(
        plan_data_plane_admission(request),
        Err(GatewayAdmissionError::Authorization(_))
    ));
}

#[test]
fn gateway_admission_rejects_cross_tenant_storage_or_provider_context() {
    let tenant_id = TenantId::new();
    let mut storage_request = request(
        tenant_id,
        principal(tenant_id, CredentialScope::DataPlane, Role::Operator),
    );
    storage_request.reservation.storage_key = TenantStorageKey::tenant(TenantId::new());

    assert_eq!(
        plan_data_plane_admission(storage_request),
        Err(GatewayAdmissionError::TenantMismatch)
    );

    let mut provider_request = request(
        tenant_id,
        principal(tenant_id, CredentialScope::DataPlane, Role::Operator),
    );
    provider_request.provider_invocation.tenant = TenantContext {
        tenant_id: TenantId::new(),
    };
    assert_eq!(
        plan_data_plane_admission(provider_request),
        Err(GatewayAdmissionError::TenantMismatch)
    );
}

#[test]
fn gateway_admission_error_responses_are_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let other_tenant = TenantId::new();

    let auth_error = plan_data_plane_admission(request(
        tenant_id,
        principal(tenant_id, CredentialScope::ControlPlane, Role::Admin),
    ))
    .unwrap_err();
    let auth_response = plan_gateway_admission_error_response(&auth_error);
    assert_eq!(auth_response.status, GatewayAdmissionErrorStatus::Forbidden);
    assert_eq!(auth_response.code, "credential_scope_not_allowed");
    assert!(!auth_response.message.contains("ControlPlane"));
    assert!(!auth_response.message.contains("DataPlane"));

    let mut reservation_request = request(
        tenant_id,
        principal(tenant_id, CredentialScope::DataPlane, Role::Operator),
    );
    reservation_request.reservation.created_at_unix_ms = u64::MAX;
    reservation_request.reservation.ttl_ms = 1;
    let reservation_error = plan_data_plane_admission(reservation_request).unwrap_err();
    let reservation_response = plan_gateway_admission_error_response(&reservation_error);
    assert_eq!(
        reservation_response.status,
        GatewayAdmissionErrorStatus::BadRequest
    );
    assert_eq!(reservation_response.code, "gateway_admission_rejected");
    assert_eq!(
        reservation_response.message,
        "gateway admission request is invalid"
    );
    assert!(
        !reservation_response
            .message
            .contains(&tenant_id.to_string())
    );
    assert!(!reservation_response.message.contains("overflow"));

    let mut provider_request = request(
        tenant_id,
        principal(tenant_id, CredentialScope::DataPlane, Role::Operator),
    );
    provider_request.provider_invocation.principal =
        principal(tenant_id, CredentialScope::ControlPlane, Role::Admin);
    let provider_error = plan_data_plane_admission(provider_request).unwrap_err();
    let provider_response = plan_gateway_admission_error_response(&provider_error);
    assert_eq!(
        provider_response.status,
        GatewayAdmissionErrorStatus::Forbidden
    );
    assert_eq!(provider_response.code, "provider_invocation_not_authorized");
    assert!(!provider_response.message.contains("ControlPlane"));
    assert!(!provider_response.message.contains("openai-prod"));

    let tenant_error = GatewayAdmissionError::TenantMismatch;
    assert_eq!(
        tenant_error.to_string(),
        "gateway admission request is invalid"
    );
    let tenant_response = plan_gateway_admission_error_response(&tenant_error);
    assert_eq!(tenant_response.code, "gateway_admission_rejected");
    assert!(!tenant_response.message.contains(&tenant_id.to_string()));
    assert!(!tenant_response.message.contains(&other_tenant.to_string()));

    assert_eq!(
        GatewayAdmissionError::AuthorizationBoundaryUnavailable.to_string(),
        "gateway admission is temporarily unavailable"
    );
    let unavailable_response = plan_gateway_admission_error_response(
        &GatewayAdmissionError::AuthorizationBoundaryUnavailable,
    );
    assert_eq!(
        unavailable_response.status,
        GatewayAdmissionErrorStatus::ServiceUnavailable
    );
    assert_eq!(
        unavailable_response.code,
        "gateway_authorization_unavailable"
    );
    assert_eq!(
        unavailable_response.message,
        "gateway authorization is temporarily unavailable"
    );
    assert!(!unavailable_response.message.contains("DataPlaneInference"));
}

#[test]
fn gateway_quota_read_authorization_uses_data_plane_quota_boundary() {
    let tenant_id = TenantId::new();
    let plan = plan_gateway_quota_read_authorization(quota_authorization_request(
        tenant_id,
        principal(tenant_id, CredentialScope::DataPlane, Role::Operator),
    ))
    .unwrap();

    assert_eq!(plan.tenant.tenant_id, tenant_id);
    assert_eq!(plan.authorization_boundary, BoundaryKind::DataPlaneQuota);
    assert_eq!(plan.spans.len(), 1);
    assert_eq!(
        plan.spans[0].descriptor.name,
        "prodex.gateway.quota_authorization"
    );
    assert_eq!(
        plan.spans[0].trace_context.as_ref().unwrap().traceparent(),
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
    );
}

#[test]
fn gateway_quota_read_authorization_rejects_control_plane_scope_or_wrong_tenant() {
    let tenant_id = TenantId::new();
    let control_plane = quota_authorization_request(
        tenant_id,
        principal(tenant_id, CredentialScope::ControlPlane, Role::Admin),
    );

    assert!(matches!(
        plan_gateway_quota_read_authorization(control_plane),
        Err(GatewayQuotaReadAuthorizationError::Authorization(_))
    ));

    let other_tenant = TenantId::new();
    let mut wrong_tenant = quota_authorization_request(
        tenant_id,
        principal(other_tenant, CredentialScope::DataPlane, Role::Operator),
    );
    wrong_tenant.resource = Resource {
        tenant_id: other_tenant,
    };
    assert_eq!(
        plan_gateway_quota_read_authorization(wrong_tenant),
        Err(GatewayQuotaReadAuthorizationError::TenantMismatch)
    );
}

#[test]
fn gateway_quota_read_authorization_error_responses_are_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let auth_error = plan_gateway_quota_read_authorization(quota_authorization_request(
        tenant_id,
        principal(tenant_id, CredentialScope::ControlPlane, Role::Admin),
    ))
    .unwrap_err();
    let auth_response = plan_gateway_quota_read_authorization_error_response(&auth_error);
    assert_eq!(
        auth_response.status,
        GatewayQuotaReadAuthorizationErrorStatus::Forbidden
    );
    assert_eq!(auth_response.code, "credential_scope_not_allowed");
    assert!(!auth_response.message.contains("ControlPlane"));
    assert!(!auth_response.message.contains("DataPlaneQuota"));

    assert_eq!(
        GatewayQuotaReadAuthorizationError::AuthorizationBoundaryUnavailable.to_string(),
        "gateway quota authorization is temporarily unavailable"
    );
    let unavailable_response = plan_gateway_quota_read_authorization_error_response(
        &GatewayQuotaReadAuthorizationError::AuthorizationBoundaryUnavailable,
    );
    assert_eq!(
        unavailable_response.status,
        GatewayQuotaReadAuthorizationErrorStatus::ServiceUnavailable
    );
    assert_eq!(
        unavailable_response.code,
        "gateway_quota_authorization_unavailable"
    );
    assert_eq!(
        unavailable_response.message,
        "gateway quota authorization is temporarily unavailable"
    );
    assert!(!unavailable_response.message.contains("DataPlaneQuota"));

    assert_eq!(
        GatewayQuotaReadAuthorizationError::TenantMismatch.to_string(),
        "gateway quota authorization request is invalid"
    );
    let tenant_response = plan_gateway_quota_read_authorization_error_response(
        &GatewayQuotaReadAuthorizationError::TenantMismatch,
    );
    assert_eq!(
        tenant_response.status,
        GatewayQuotaReadAuthorizationErrorStatus::BadRequest
    );
    assert_eq!(tenant_response.code, "gateway_quota_authorization_rejected");
    assert!(!tenant_response.message.contains(&tenant_id.to_string()));
}

#[test]
fn gateway_usage_reconciliation_plans_commit_release_and_trace_span() {
    let tenant_id = TenantId::new();
    let plan = plan_gateway_usage_reconciliation(reconciliation_request(
        tenant_id,
        UsageAmount::new(100, 1_000),
        UsageAmount::new(40, 400),
    ))
    .unwrap();

    assert_eq!(plan.tenant.tenant_id, tenant_id);
    assert_eq!(plan.reconciliation.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.reconciliation.ledger_events.len(), 2);
    assert_eq!(
        plan.reconciliation.updated_snapshot.reserved,
        UsageAmount::ZERO
    );
    assert_eq!(
        plan.spans[0].descriptor.name,
        "prodex.gateway.usage_reconciliation"
    );
    assert_eq!(
        plan.spans[0].trace_context.as_ref().unwrap().traceparent(),
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
    );
}

#[test]
fn gateway_usage_reconciliation_rejects_cross_tenant_storage_or_record() {
    let tenant_id = TenantId::new();
    let mut request = reconciliation_request(
        tenant_id,
        UsageAmount::new(10, 100),
        UsageAmount::new(1, 10),
    );
    request.reconciliation.storage_key = TenantStorageKey::tenant(TenantId::new());

    assert_eq!(
        plan_gateway_usage_reconciliation(request),
        Err(GatewayUsageReconciliationError::TenantMismatch)
    );
}

#[test]
fn gateway_usage_reconciliation_rejects_actual_usage_above_reserved() {
    let tenant_id = TenantId::new();

    assert!(matches!(
        plan_gateway_usage_reconciliation(reconciliation_request(
            tenant_id,
            UsageAmount::new(10, 100),
            UsageAmount::new(11, 90),
        )),
        Err(GatewayUsageReconciliationError::Reconciliation(_))
    ));
}

#[test]
fn gateway_usage_reconciliation_error_responses_are_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let other_tenant = TenantId::new();

    let mut cross_tenant = reconciliation_request(
        tenant_id,
        UsageAmount::new(10, 100),
        UsageAmount::new(1, 10),
    );
    cross_tenant.reconciliation.storage_key = TenantStorageKey::tenant(other_tenant);
    let tenant_error = plan_gateway_usage_reconciliation(cross_tenant).unwrap_err();
    assert_eq!(
        tenant_error.to_string(),
        "usage reconciliation request is invalid"
    );
    let tenant_response = plan_gateway_usage_reconciliation_error_response(&tenant_error);
    assert_eq!(
        tenant_response.status,
        GatewayUsageReconciliationErrorStatus::BadRequest
    );
    assert_eq!(tenant_response.code, "usage_reconciliation_rejected");
    assert_eq!(
        tenant_response.message,
        "usage reconciliation request is invalid"
    );
    assert!(!tenant_response.message.contains(&tenant_id.to_string()));
    assert!(!tenant_response.message.contains(&other_tenant.to_string()));

    let usage_error = plan_gateway_usage_reconciliation(reconciliation_request(
        tenant_id,
        UsageAmount::new(10, 100),
        UsageAmount::new(11, 90),
    ))
    .unwrap_err();
    let usage_response = plan_gateway_usage_reconciliation_error_response(&usage_error);
    assert_eq!(usage_response.code, "usage_reconciliation_rejected");
    assert!(!usage_response.message.contains("tokens"));
    assert!(!usage_response.message.contains("cost"));
    assert!(!usage_response.message.contains("11"));
}

#[test]
fn gateway_expired_reservation_recovery_plans_release_and_trace_span() {
    let tenant_id = TenantId::new();
    let plan = plan_gateway_expired_reservation_recovery(expired_recovery_request(
        tenant_id,
        UsageAmount::new(10, 100),
        1_500,
    ))
    .unwrap();

    assert_eq!(plan.tenant.tenant_id, tenant_id);
    assert_eq!(plan.recovery.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.recovery.ledger_event.tenant_id, tenant_id);
    assert_eq!(plan.recovery.updated_snapshot.reserved, UsageAmount::ZERO);
    assert_eq!(
        plan.spans[0].descriptor.name,
        "prodex.gateway.expired_reservation_recovery"
    );
    assert_eq!(
        plan.spans[0].trace_context.as_ref().unwrap().traceparent(),
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
    );
}

#[test]
fn gateway_expired_reservation_recovery_rejects_cross_tenant_or_not_expired() {
    let tenant_id = TenantId::new();
    let mut cross_tenant = expired_recovery_request(tenant_id, UsageAmount::new(10, 100), 1_500);
    cross_tenant.recovery.storage_key = TenantStorageKey::tenant(TenantId::new());

    assert_eq!(
        plan_gateway_expired_reservation_recovery(cross_tenant),
        Err(GatewayExpiredReservationRecoveryError::TenantMismatch)
    );

    assert!(matches!(
        plan_gateway_expired_reservation_recovery(expired_recovery_request(
            tenant_id,
            UsageAmount::new(10, 100),
            1_499,
        )),
        Err(GatewayExpiredReservationRecoveryError::Recovery(_))
    ));
}

#[test]
fn gateway_expired_reservation_recovery_error_responses_are_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let other_tenant = TenantId::new();

    let mut cross_tenant = expired_recovery_request(tenant_id, UsageAmount::new(10, 100), 1_500);
    cross_tenant.recovery.storage_key = TenantStorageKey::tenant(other_tenant);
    let tenant_error = plan_gateway_expired_reservation_recovery(cross_tenant).unwrap_err();
    assert_eq!(
        tenant_error.to_string(),
        "expired reservation recovery request is invalid"
    );
    let tenant_response = plan_gateway_expired_reservation_recovery_error_response(&tenant_error);
    assert_eq!(
        tenant_response.status,
        GatewayExpiredReservationRecoveryErrorStatus::BadRequest
    );
    assert_eq!(
        tenant_response.code,
        "expired_reservation_recovery_rejected"
    );
    assert_eq!(
        tenant_response.message,
        "expired reservation recovery request is invalid"
    );
    assert!(!tenant_response.message.contains(&tenant_id.to_string()));
    assert!(!tenant_response.message.contains(&other_tenant.to_string()));

    let not_expired_error = plan_gateway_expired_reservation_recovery(expired_recovery_request(
        tenant_id,
        UsageAmount::new(10, 100),
        1_499,
    ))
    .unwrap_err();
    let not_expired_response =
        plan_gateway_expired_reservation_recovery_error_response(&not_expired_error);
    assert_eq!(
        not_expired_response.code,
        "expired_reservation_recovery_rejected"
    );
    assert!(!not_expired_response.message.contains("tokens"));
    assert!(!not_expired_response.message.contains("100"));
    assert!(!not_expired_response.message.contains("1499"));
}
