#[path = "support/governance.rs"]
mod governance_support;

use prodex_application::{
    ApplicationAtomicReservationError, ApplicationAtomicReservationErrorStatus,
    ApplicationAtomicReservationRequest, ApplicationAtomicReservationStoragePlan,
    ApplicationAuditExportError, ApplicationAuditExportErrorStatus,
    ApplicationAuditExportQueryStoragePlan, ApplicationAuditExportRequest,
    ApplicationAuditRetentionPurgeError, ApplicationAuditRetentionPurgeErrorStatus,
    ApplicationAuditRetentionPurgeRequest, ApplicationAuditRetentionPurgeStoragePlan,
    ApplicationBillingLedgerStoragePlan, ApplicationBillingReadError,
    ApplicationBillingReadErrorStatus, ApplicationBillingReadRequest,
    ApplicationBreakGlassAuditRequest, ApplicationBudgetPolicyLifecycleError,
    ApplicationBudgetPolicyLifecycleErrorStatus, ApplicationBudgetPolicyLifecycleRequest,
    ApplicationBudgetPolicyUpdateError, ApplicationBudgetPolicyUpdateErrorStatus,
    ApplicationBudgetPolicyUpdateRequest, ApplicationBudgetPolicyUpdateStoragePlan,
    ApplicationConfigurationActivationError, ApplicationConfigurationActivationErrorStatus,
    ApplicationConfigurationActivationRequest, ApplicationConfigurationPublicationAuditStoragePlan,
    ApplicationConfigurationPublicationError, ApplicationConfigurationPublicationErrorStatus,
    ApplicationConfigurationPublicationRequest, ApplicationConfigurationReadinessRequest,
    ApplicationControlPlaneAuditCorrelationErrorStatus,
    ApplicationControlPlaneAuditCorrelationRequest,
    ApplicationControlPlaneAuditEmissionSpanErrorStatus,
    ApplicationControlPlaneAuditEmissionSpanRequest, ApplicationControlPlaneAuditError,
    ApplicationControlPlaneAuditErrorStatus,
    ApplicationControlPlaneAuditPersistenceSpanErrorStatus,
    ApplicationControlPlaneAuditPersistenceSpanRequest, ApplicationControlPlaneAuditRequest,
    ApplicationControlPlaneAuditStoragePlan, ApplicationControlPlaneHttpRouteError,
    ApplicationControlPlaneHttpRouteErrorStatus, ApplicationControlPlaneHttpRoutePlan,
    ApplicationControlPlaneIdempotencyCompletedStoragePlan,
    ApplicationControlPlaneIdempotencyError, ApplicationControlPlaneIdempotencyErrorStatus,
    ApplicationControlPlaneIdempotencyLookupStoragePlan,
    ApplicationControlPlaneIdempotencyPendingStoragePlan,
    ApplicationControlPlaneIdempotencyRequest,
    ApplicationControlPlaneIdempotencyStorageCompleteRequest,
    ApplicationControlPlaneIdempotencyStorageError,
    ApplicationControlPlaneIdempotencyStorageErrorStatus,
    ApplicationControlPlaneIdempotencyStoragePrepareRequest,
    ApplicationControlPlanePageRequestError, ApplicationControlPlanePageRequestErrorStatus,
    ApplicationControlPlanePreconditionError, ApplicationControlPlanePreconditionErrorStatus,
    ApplicationDataPlaneError, ApplicationDataPlaneRequest,
    ApplicationExpiredReservationRecoveryCoordinationPlan,
    ApplicationExpiredReservationRecoveryCoordinationRequest,
    ApplicationExpiredReservationRecoveryError, ApplicationExpiredReservationRecoveryErrorStatus,
    ApplicationExpiredReservationRecoveryRequest, ApplicationExpiredReservationRecoveryStoragePlan,
    ApplicationMigrationMode, ApplicationProviderCapabilityError,
    ApplicationProviderCapabilityRequest, ApplicationProviderCircuitBreakerEventRequest,
    ApplicationProviderCircuitBreakerRequest, ApplicationProviderCredentialReferenceError,
    ApplicationProviderCredentialReferenceErrorStatus,
    ApplicationProviderCredentialReferenceRequest,
    ApplicationProviderCredentialReferenceStoragePlan, ApplicationProviderCredentialRotationError,
    ApplicationProviderCredentialRotationErrorStatus, ApplicationProviderCredentialRotationRequest,
    ApplicationProviderRetryRequest, ApplicationQuotaReadError, ApplicationQuotaReadRequest,
    ApplicationRecoveryLeaseReleaseError, ApplicationRecoveryLeaseReleaseErrorStatus,
    ApplicationRecoveryLeaseReleaseRequest, ApplicationRequestAuthenticationError,
    ApplicationRequestAuthenticationRequest, ApplicationRoleBindingLifecycleError,
    ApplicationRoleBindingLifecycleErrorStatus, ApplicationRoleBindingLifecycleRequest,
    ApplicationRoleBindingMutationError, ApplicationRoleBindingMutationErrorStatus,
    ApplicationRoleBindingMutationRequest, ApplicationRoleBindingMutationStoragePlan,
    ApplicationRuntimeAccountingVerificationError, ApplicationRuntimeAccountingVerificationRequest,
    ApplicationRuntimePlanError, ApplicationRuntimePlanErrorStatus, ApplicationRuntimeTopology,
    ApplicationServiceIdentityCreateError, ApplicationServiceIdentityCreateErrorStatus,
    ApplicationServiceIdentityCreateRequest, ApplicationServiceIdentityCreateStoragePlan,
    ApplicationServiceIdentityLifecycleError, ApplicationServiceIdentityLifecycleErrorStatus,
    ApplicationServiceIdentityLifecycleRequest, ApplicationTenantLifecycleError,
    ApplicationTenantLifecycleErrorStatus, ApplicationTenantLifecycleRequest,
    ApplicationTenantLifecycleStoragePlan, ApplicationUsageReconciliationError,
    ApplicationUsageReconciliationErrorStatus, ApplicationUsageReconciliationRequest,
    ApplicationUsageReconciliationStoragePlan, ApplicationUserLifecycleError,
    ApplicationUserLifecycleErrorStatus, ApplicationUserLifecycleRequest,
    ApplicationUserLifecycleStoragePlan, ApplicationVirtualKeyLifecycleError,
    ApplicationVirtualKeyLifecycleErrorStatus, ApplicationVirtualKeyLifecycleRequest,
    ApplicationVirtualKeySecretReferenceError, ApplicationVirtualKeySecretReferenceErrorStatus,
    ApplicationVirtualKeySecretReferenceRequest, ApplicationVirtualKeySecretReferenceStoragePlan,
    application_control_plane_principal_scoped_idempotency_key,
    plan_application_atomic_reservation, plan_application_atomic_reservation_error_response,
    plan_application_audit_export, plan_application_audit_export_error_response,
    plan_application_audit_retention_purge, plan_application_audit_retention_purge_error_response,
    plan_application_billing_read, plan_application_billing_read_error_response,
    plan_application_break_glass_with_audit_storage, plan_application_budget_policy_lifecycle,
    plan_application_budget_policy_lifecycle_error_response, plan_application_budget_policy_update,
    plan_application_budget_policy_update_error_response,
    plan_application_configuration_activation,
    plan_application_configuration_activation_error_response,
    plan_application_configuration_publication,
    plan_application_configuration_publication_decision_error_response,
    plan_application_configuration_publication_error_response,
    plan_application_configuration_readiness_snapshot, plan_application_control_plane,
    plan_application_control_plane_audit_correlation_error_response,
    plan_application_control_plane_audit_correlation_from_http,
    plan_application_control_plane_audit_emission_span,
    plan_application_control_plane_audit_emission_span_error_response,
    plan_application_control_plane_audit_error_response,
    plan_application_control_plane_audit_from_http,
    plan_application_control_plane_audit_persistence_span,
    plan_application_control_plane_audit_persistence_span_error_response,
    plan_application_control_plane_http_route,
    plan_application_control_plane_http_route_error_response,
    plan_application_control_plane_idempotency,
    plan_application_control_plane_idempotency_error_response,
    plan_application_control_plane_idempotency_from_http,
    plan_application_control_plane_idempotency_from_http_digest,
    plan_application_control_plane_idempotency_replay,
    plan_application_control_plane_idempotency_replay_from_lookup_row,
    plan_application_control_plane_idempotency_storage_complete,
    plan_application_control_plane_idempotency_storage_error_response,
    plan_application_control_plane_idempotency_storage_prepare,
    plan_application_control_plane_page_request_error_response,
    plan_application_control_plane_page_request_from_http_query,
    plan_application_control_plane_precondition_error_response,
    plan_application_control_plane_precondition_from_http,
    plan_application_control_plane_with_audit_storage,
    plan_application_control_plane_with_audit_storage_from_http, plan_application_data_plane,
    plan_application_data_plane_error_response, plan_application_expired_reservation_recovery,
    plan_application_expired_reservation_recovery_error_response,
    plan_application_provider_capability, plan_application_provider_capability_error_response,
    plan_application_provider_circuit_breaker, plan_application_provider_circuit_breaker_event,
    plan_application_provider_credential_reference,
    plan_application_provider_credential_reference_error_response,
    plan_application_provider_credential_rotation,
    plan_application_provider_credential_rotation_error_response, plan_application_provider_retry,
    plan_application_quota_read, plan_application_quota_read_error_response,
    plan_application_recovery_lease_release,
    plan_application_recovery_lease_release_error_response,
    plan_application_request_authentication,
    plan_application_request_authentication_error_response,
    plan_application_role_binding_lifecycle,
    plan_application_role_binding_lifecycle_error_response, plan_application_role_binding_mutation,
    plan_application_role_binding_mutation_error_response, plan_application_runtime,
    plan_application_runtime_accounting_verification,
    plan_application_runtime_accounting_verification_error_response,
    plan_application_runtime_accounting_verification_required_response,
    plan_application_runtime_error_response, plan_application_service_identity_create,
    plan_application_service_identity_create_error_response,
    plan_application_service_identity_lifecycle,
    plan_application_service_identity_lifecycle_error_response, plan_application_tenant_lifecycle,
    plan_application_tenant_lifecycle_error_response, plan_application_usage_reconciliation,
    plan_application_usage_reconciliation_error_response, plan_application_user_lifecycle,
    plan_application_user_lifecycle_error_response, plan_application_virtual_key_lifecycle,
    plan_application_virtual_key_lifecycle_error_response,
    plan_application_virtual_key_secret_reference,
    plan_application_virtual_key_secret_reference_error_response,
};
use prodex_authn::{AuthenticationErrorStatus, TokenClaims};
use prodex_config::{
    ConfigCacheState, ConfigCacheWindow, ConfigPublicationEventDelivery,
    ConfigPublicationEventTarget, ConfigRevision,
};
use prodex_control_plane::{
    BreakGlassAuthorization, ConfigurationPublicationDecision, ConfigurationPublicationRequest,
    ControlPlaneActionRequest, ControlPlaneDecision, ControlPlaneOperation,
    ControlPlaneResourceRef,
};
use prodex_domain::{
    Audience, AuditDigest, AuditExportFormat, AuditExportPlan, AuditPageLimit, AuditQueryPlan,
    AuditQueryScope, AuditRetentionBatchLimit, AuditRetentionPurgeBatch, AuditRetentionPurgeKey,
    AuditSortOrder, AuditTimeRange, AuditTimestamp, BudgetLimit, BudgetSnapshot, CallId,
    CapabilityErrorStatus, CapabilityRequest, CapabilitySet, CredentialScope, ExplicitRoleMapper,
    HealthCheck, HealthState, IdempotencyConflict, IdempotencyEntry, IdempotencyKey,
    IdempotencyRecord, IdempotencyReplayDecision, Issuer, JwksCacheSnapshot, JwtAlgorithm,
    ModelCapability, OidcValidationPolicy, PolicyRevisionId, Principal, PrincipalId, PrincipalKind,
    ProviderCredentialId, RequestId, ReservationId, ReservationReconciliationReason,
    ReservationRecord, ReservationRequest, ResourceKind, Role, RoleBindingId, SecretRef,
    TenantContext, TenantId, TenantScopedResource, UsageAmount, VirtualKeyId,
};
use prodex_gateway_core::{
    GatewayAdmissionErrorStatus, GatewayAdmissionRequest, GatewayExpiredReservationRecoveryRequest,
    GatewayQuotaReadAuthorizationErrorStatus, GatewayQuotaReadAuthorizationRequest,
    GatewayUsageReconciliationRequest,
};
use prodex_gateway_http::{
    GatewayControlPlaneOperation, GatewayHttpErrorStatus, GatewayHttpHeader, GatewayHttpMethod,
    GatewayHttpPolicy, GatewayHttpRequestMeta, GatewayHttpRouteKind,
};
use prodex_provider_core::{ProviderEndpoint, ProviderErrorClass, ProviderId, ProviderWireFormat};
use prodex_provider_spi::{
    ProviderCircuitBreakerDecision, ProviderCircuitBreakerEvent, ProviderCircuitBreakerPolicy,
    ProviderCircuitBreakerState, ProviderInvocation, ProviderRetryCause, ProviderRetryDecision,
    ProviderRetryPolicy, ProviderRetryStage, ProviderRoute, ProviderRouteCapabilityCandidate,
    ProviderStreamMode,
};
use prodex_storage::{
    AtomicReservationCommand, AuditExportQueryCommand, AuditRetentionPurgeCommand,
    BillingLedgerPageLimit, BillingLedgerQueryCommand, BillingLedgerSortOrder,
    BudgetPolicyUpdateCommand, CacheStoreKind, DurableStoreKind, ExpiredReservationRecoveryCommand,
    IdempotencyRecordLookupRow, IdempotencyRecordLookupRowStatus, MigrationRuntimePolicy,
    MultiReplicaAccountingCheck, MultiReplicaAccountingEvidence,
    ProviderCredentialReferenceCommand, RoleBindingMutationCommand, RoleBindingMutationKind,
    ServiceIdentityCreateCommand, StorageTopology, TenantLifecycleCommand, TenantLifecycleKind,
    TenantStorageKey, UsageReconciliationCommand, UserLifecycleCommand, UserLifecycleKind,
    VirtualKeySecretReferenceCommand, VirtualKeySecretReferenceKind,
};
use prodex_storage_redis::RedisCachePurpose;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Resource {
    tenant_id: TenantId,
}

impl TenantScopedResource for Resource {
    fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }
}

fn principal(tenant_id: TenantId) -> Principal {
    Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::VirtualKey,
        Role::Operator,
        CredentialScope::DataPlane,
    )
}

fn traceparent() -> GatewayHttpHeader {
    GatewayHttpHeader::new(
        "traceparent",
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
    )
}

fn oidc_policy() -> OidcValidationPolicy {
    OidcValidationPolicy::new(
        Issuer::new("https://issuer.example.com").unwrap(),
        vec![Audience::new("prodex").unwrap()],
        vec![JwtAlgorithm::Rs256],
    )
    .unwrap()
}

fn jwks_snapshot() -> JwksCacheSnapshot {
    JwksCacheSnapshot {
        fetched_at_unix_ms: 1_000,
        expires_at_unix_ms: 10_000,
        stale_until_unix_ms: 20_000,
        key_count: 2,
        last_refresh_error_at_unix_ms: None,
        retry_after_unix_ms: None,
    }
}

fn role_mapper() -> ExplicitRoleMapper {
    ExplicitRoleMapper::new([("operator", Role::Operator), ("admin", Role::Admin)])
}

fn token_claims(tenant_id: TenantId, scope: CredentialScope) -> TokenClaims {
    TokenClaims {
        issuer: Issuer::new("https://issuer.example.com").unwrap(),
        audience: Audience::new("prodex").unwrap(),
        algorithm: JwtAlgorithm::Rs256,
        key_id: "kid-1".to_string(),
        key_known: true,
        signature_verified: true,
        principal_id: PrincipalId::new(),
        tenant_id: Some(tenant_id),
        principal_kind: PrincipalKind::User,
        credential_scope: scope,
        role_claim: Some("operator".to_string()),
        expires_at_unix_ms: 9_000,
        not_before_unix_ms: Some(1_000),
    }
}

fn application_authentication_request(
    path: &str,
    method: GatewayHttpMethod,
    tenant_id: TenantId,
    scope: CredentialScope,
) -> ApplicationRequestAuthenticationRequest<'static> {
    let policy = Box::leak(Box::new(oidc_policy()));
    let jwks = Box::leak(Box::new(jwks_snapshot()));
    let mapper = Box::leak(Box::new(role_mapper()));
    ApplicationRequestAuthenticationRequest {
        http: GatewayHttpRequestMeta {
            method,
            path: path.to_string(),
            body_len: 128,
            headers: vec![traceparent()],
        },
        oidc_policy: policy,
        jwks_snapshot: Some(jwks),
        role_mapper: mapper,
        claims: token_claims(tenant_id, scope),
        now_unix_ms: 2_000,
    }
}

fn control_plane_http_request(path: &str) -> GatewayHttpRequestMeta {
    GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Post,
        path: path.to_string(),
        body_len: 128,
        headers: vec![traceparent()],
    }
}

fn data_plane_request(tenant_id: TenantId) -> ApplicationDataPlaneRequest<Resource> {
    let call_id = CallId::new();
    let reservation_id = ReservationId::new();
    let request_id = RequestId::new();
    let principal = principal(tenant_id);
    let inspection = prodex_application::plan_application_request_inspection(
        prodex_application::ApplicationInspectionRequest {
            sources: Vec::new(),
            default_classification: prodex_domain::DataClassification::Internal,
            trusted_label: None,
            prior_classification: None,
            detector_revision: prodex_domain::DetectorRevisionId::new("test-v1").unwrap(),
            limits: prodex_domain::InspectionLimits::default(),
        },
    )
    .unwrap();
    let governance = governance_support::test_governance_plan(tenant_id, &principal, &inspection);
    ApplicationDataPlaneRequest {
        http: GatewayHttpRequestMeta {
            method: GatewayHttpMethod::Post,
            path: "/v1/responses".to_string(),
            body_len: 128,
            headers: vec![
                traceparent(),
                GatewayHttpHeader::new("session_id", "sess-1"),
                GatewayHttpHeader::new("Authorization", "Bearer should-strip"),
            ],
        },
        inspection,
        governance,
        admission: GatewayAdmissionRequest {
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
                    estimate: UsageAmount::new(20, 200),
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
                estimated_usage: UsageAmount::new(20, 200),
            },
            trace_context: prodex_observability::TraceContext::parse_traceparent(
                "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
            )
            .unwrap(),
        },
    }
}

fn atomic_reservation_request(
    durable_store: DurableStoreKind,
    tenant_id: TenantId,
) -> ApplicationAtomicReservationRequest {
    ApplicationAtomicReservationRequest {
        durable_store,
        reservation: data_plane_request(tenant_id).admission.reservation,
    }
}

fn gateway_usage_reconciliation_request(
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
            reason: ReservationReconciliationReason::StreamInterrupted,
        },
        trace_context: prodex_observability::TraceContext::parse_traceparent(
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
        )
        .unwrap(),
    }
}

fn quota_read_request(tenant_id: TenantId) -> ApplicationQuotaReadRequest<Resource> {
    ApplicationQuotaReadRequest {
        http: GatewayHttpRequestMeta {
            method: GatewayHttpMethod::Get,
            path: "/v1/quota".to_string(),
            body_len: 0,
            headers: vec![
                traceparent(),
                GatewayHttpHeader::new("session_id", "sess-quota"),
                GatewayHttpHeader::new("Authorization", "Bearer should-strip"),
            ],
        },
        authorization: GatewayQuotaReadAuthorizationRequest {
            tenant: TenantContext { tenant_id },
            principal: principal(tenant_id),
            resource: Resource { tenant_id },
            request_id: RequestId::new(),
            trace_context: prodex_observability::TraceContext::parse_traceparent(
                "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
            )
            .unwrap(),
        },
    }
}

fn usage_reconciliation_request(
    durable_store: DurableStoreKind,
    tenant_id: TenantId,
    reserved: UsageAmount,
    actual: UsageAmount,
) -> ApplicationUsageReconciliationRequest {
    ApplicationUsageReconciliationRequest {
        durable_store,
        gateway: gateway_usage_reconciliation_request(tenant_id, reserved, actual),
    }
}

fn gateway_expired_recovery_request(
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
        trace_context: prodex_observability::TraceContext::parse_traceparent(
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
        )
        .unwrap(),
    }
}

fn expired_recovery_request(
    durable_store: DurableStoreKind,
    tenant_id: TenantId,
    reserved: UsageAmount,
    now_unix_ms: u64,
) -> ApplicationExpiredReservationRecoveryRequest {
    ApplicationExpiredReservationRecoveryRequest {
        durable_store,
        gateway: gateway_expired_recovery_request(tenant_id, reserved, now_unix_ms),
        coordination: None,
    }
}

fn control_plane_principal(tenant_id: TenantId, role: Role, scope: CredentialScope) -> Principal {
    Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::User,
        role,
        scope,
    )
}

fn control_plane_action(
    tenant_id: TenantId,
    principal: Principal,
    operation: ControlPlaneOperation,
    resource_kind: ResourceKind,
) -> ControlPlaneActionRequest {
    ControlPlaneActionRequest {
        principal,
        operation,
        resource: ControlPlaneResourceRef::new(tenant_id, resource_kind, Some("resource-1")),
        occurred_at_unix_ms: 10_000,
    }
}

fn control_plane_audit_request(
    durable_store: DurableStoreKind,
    tenant_id: TenantId,
    principal: Principal,
    operation: ControlPlaneOperation,
    resource_kind: ResourceKind,
) -> ApplicationControlPlaneAuditRequest {
    ApplicationControlPlaneAuditRequest {
        durable_store,
        action: control_plane_action(tenant_id, principal, operation, resource_kind),
        previous_digest: Some(AuditDigest::new("sha256:previous").unwrap()),
        event_digest: AuditDigest::new("sha256:current").unwrap(),
    }
}

fn audit_retention_purge_request(
    durable_store: DurableStoreKind,
    tenant_id: TenantId,
) -> ApplicationAuditRetentionPurgeRequest {
    let batch = AuditRetentionPurgeBatch::new(
        AuditQueryScope::tenant(TenantContext { tenant_id }),
        [
            AuditRetentionPurgeKey {
                tenant_id,
                event_id: prodex_domain::AuditEventId::new(),
            },
            AuditRetentionPurgeKey {
                tenant_id,
                event_id: prodex_domain::AuditEventId::new(),
            },
        ],
        AuditRetentionBatchLimit::new(Some(10)).unwrap(),
    )
    .unwrap();
    ApplicationAuditRetentionPurgeRequest {
        durable_store,
        purge: AuditRetentionPurgeCommand {
            storage_key: TenantStorageKey::tenant(tenant_id),
            batch,
        },
    }
}

fn audit_export_request(
    durable_store: DurableStoreKind,
    tenant_id: TenantId,
    principal: Principal,
) -> ApplicationAuditExportRequest {
    let query = AuditQueryPlan::new(
        AuditQueryScope::tenant(TenantContext { tenant_id }),
        AuditTimeRange::new(
            Some(AuditTimestamp::new(1_800_000_000_000).unwrap()),
            Some(AuditTimestamp::new(1_800_000_999_999).unwrap()),
        )
        .unwrap(),
        AuditPageLimit::new(Some(250)).unwrap(),
        AuditSortOrder::OccurredAtDesc,
    );
    ApplicationAuditExportRequest {
        durable_store,
        action: control_plane_action(
            tenant_id,
            principal,
            ControlPlaneOperation::AuditExport,
            ResourceKind::AuditLog,
        ),
        export: AuditExportQueryCommand {
            storage_key: TenantStorageKey::tenant(tenant_id),
            export: AuditExportPlan::new(query, AuditExportFormat::Jsonl),
        },
        previous_digest: Some(AuditDigest::new("sha256:previous").unwrap()),
        event_digest: AuditDigest::new("sha256:current").unwrap(),
    }
}

fn billing_read_request(
    durable_store: DurableStoreKind,
    tenant_id: TenantId,
    principal: Principal,
) -> ApplicationBillingReadRequest {
    ApplicationBillingReadRequest {
        durable_store,
        action: control_plane_action(
            tenant_id,
            principal,
            ControlPlaneOperation::BillingRead,
            ResourceKind::Billing,
        ),
        query: BillingLedgerQueryCommand {
            storage_key: TenantStorageKey::tenant(tenant_id),
            tenant_id,
            start_unix_ms: Some(1_800_000_000_000),
            end_unix_ms: Some(1_800_000_999_999),
            page_limit: BillingLedgerPageLimit::new(Some(250)).unwrap(),
            sort_order: BillingLedgerSortOrder::OccurredAtDesc,
        },
        previous_digest: Some(AuditDigest::new("sha256:previous").unwrap()),
        event_digest: AuditDigest::new("sha256:current").unwrap(),
    }
}

fn service_identity_create_request(
    durable_store: DurableStoreKind,
    tenant_id: TenantId,
) -> ApplicationServiceIdentityCreateRequest {
    ApplicationServiceIdentityCreateRequest {
        durable_store,
        identity: ServiceIdentityCreateCommand {
            storage_key: TenantStorageKey::tenant(tenant_id),
            tenant_id,
            principal_id: PrincipalId::new(),
            display_name: "svc-ingest".to_string(),
            created_at_unix_ms: 1_800_000_002_250,
        },
    }
}

fn service_identity_lifecycle_request(
    durable_store: DurableStoreKind,
    tenant_id: TenantId,
    principal: Principal,
) -> ApplicationServiceIdentityLifecycleRequest {
    ApplicationServiceIdentityLifecycleRequest {
        durable_store,
        action: control_plane_action(
            tenant_id,
            principal,
            ControlPlaneOperation::ServiceIdentityCreate,
            ResourceKind::ServiceIdentity,
        ),
        identity: service_identity_create_request(durable_store, tenant_id).identity,
        previous_digest: Some(AuditDigest::new("sha256:previous").unwrap()),
        event_digest: AuditDigest::new("sha256:current").unwrap(),
    }
}

fn budget_policy_update_request(
    durable_store: DurableStoreKind,
    tenant_id: TenantId,
) -> ApplicationBudgetPolicyUpdateRequest {
    ApplicationBudgetPolicyUpdateRequest {
        durable_store,
        policy: BudgetPolicyUpdateCommand {
            storage_key: TenantStorageKey::tenant(tenant_id),
            tenant_id,
            budget_scope: "tenant-default".to_string(),
            limit: BudgetLimit::new(10_000, 1_000_000),
            updated_at_unix_ms: 1_800_000_002_750,
        },
    }
}

fn budget_policy_lifecycle_request(
    durable_store: DurableStoreKind,
    tenant_id: TenantId,
    principal: Principal,
) -> ApplicationBudgetPolicyLifecycleRequest {
    ApplicationBudgetPolicyLifecycleRequest {
        durable_store,
        action: control_plane_action(
            tenant_id,
            principal,
            ControlPlaneOperation::BudgetUpdate,
            ResourceKind::Budget,
        ),
        policy: budget_policy_update_request(durable_store, tenant_id).policy,
        previous_digest: Some(AuditDigest::new("sha256:previous").unwrap()),
        event_digest: AuditDigest::new("sha256:current").unwrap(),
    }
}

fn tenant_lifecycle_request(
    durable_store: DurableStoreKind,
    tenant_id: TenantId,
    principal: Principal,
    kind: TenantLifecycleKind,
) -> ApplicationTenantLifecycleRequest {
    let operation = match kind {
        TenantLifecycleKind::Create => ControlPlaneOperation::TenantCreate,
        TenantLifecycleKind::Update => ControlPlaneOperation::TenantUpdate,
    };
    ApplicationTenantLifecycleRequest {
        durable_store,
        action: control_plane_action(tenant_id, principal, operation, ResourceKind::Tenant),
        tenant: TenantLifecycleCommand {
            storage_key: TenantStorageKey::tenant(tenant_id),
            tenant_id,
            display_name: "acme-prod".to_string(),
            kind,
            occurred_at_unix_ms: 1_800_000_002_900,
        },
        previous_digest: Some(AuditDigest::new("sha256:previous").unwrap()),
        event_digest: AuditDigest::new("sha256:current").unwrap(),
    }
}

fn user_lifecycle_request(
    durable_store: DurableStoreKind,
    tenant_id: TenantId,
    principal: Principal,
    operation: ControlPlaneOperation,
    kind: UserLifecycleKind,
) -> ApplicationUserLifecycleRequest {
    ApplicationUserLifecycleRequest {
        durable_store,
        action: control_plane_action(tenant_id, principal, operation, ResourceKind::User),
        user: UserLifecycleCommand {
            storage_key: TenantStorageKey::tenant(tenant_id),
            tenant_id,
            principal_id: PrincipalId::new(),
            external_id: "scim-user-1".to_string(),
            display_name: "Ada Lovelace".to_string(),
            kind,
            occurred_at_unix_ms: 1_800_000_002_400,
        },
        previous_digest: Some(AuditDigest::new("sha256:previous").unwrap()),
        event_digest: AuditDigest::new("sha256:current").unwrap(),
    }
}

fn role_binding_mutation_request(
    durable_store: DurableStoreKind,
    tenant_id: TenantId,
    kind: RoleBindingMutationKind,
) -> ApplicationRoleBindingMutationRequest {
    ApplicationRoleBindingMutationRequest {
        durable_store,
        mutation: RoleBindingMutationCommand {
            storage_key: TenantStorageKey::tenant(tenant_id),
            tenant_id,
            role_binding_id: RoleBindingId::new(),
            principal_id: PrincipalId::new(),
            role: Role::Operator,
            kind,
            occurred_at_unix_ms: 1_800_000_002_000,
        },
    }
}

fn role_binding_lifecycle_request(
    durable_store: DurableStoreKind,
    tenant_id: TenantId,
    principal: Principal,
    kind: RoleBindingMutationKind,
) -> ApplicationRoleBindingLifecycleRequest {
    let operation = match kind {
        RoleBindingMutationKind::Grant => ControlPlaneOperation::RoleBindingGrant,
        RoleBindingMutationKind::Revoke => ControlPlaneOperation::RoleBindingRevoke,
    };
    ApplicationRoleBindingLifecycleRequest {
        durable_store,
        action: control_plane_action(tenant_id, principal, operation, ResourceKind::RoleBinding),
        mutation: role_binding_mutation_request(durable_store, tenant_id, kind).mutation,
        previous_digest: Some(AuditDigest::new("sha256:previous").unwrap()),
        event_digest: AuditDigest::new("sha256:current").unwrap(),
    }
}

fn virtual_key_secret_reference_request(
    durable_store: DurableStoreKind,
    tenant_id: TenantId,
    kind: VirtualKeySecretReferenceKind,
) -> ApplicationVirtualKeySecretReferenceRequest {
    let virtual_key_id = VirtualKeyId::new();
    ApplicationVirtualKeySecretReferenceRequest {
        durable_store,
        reference: VirtualKeySecretReferenceCommand {
            storage_key: TenantStorageKey::virtual_key(tenant_id, virtual_key_id),
            tenant_id,
            virtual_key_id,
            principal_id: PrincipalId::new(),
            display_name: "prod-api".to_string(),
            secret_ref: SecretRef::new("vault", "virtual-keys/prod-api", Some("v4")),
            kind,
            occurred_at_unix_ms: 1_800_000_002_500,
        },
    }
}

fn virtual_key_lifecycle_request(
    durable_store: DurableStoreKind,
    tenant_id: TenantId,
    principal: Principal,
    kind: VirtualKeySecretReferenceKind,
) -> ApplicationVirtualKeyLifecycleRequest {
    let operation = match kind {
        VirtualKeySecretReferenceKind::Create => ControlPlaneOperation::VirtualKeyCreate,
        VirtualKeySecretReferenceKind::Rotate => ControlPlaneOperation::VirtualKeyRotateSecret,
    };
    ApplicationVirtualKeyLifecycleRequest {
        durable_store,
        action: control_plane_action(tenant_id, principal, operation, ResourceKind::VirtualKey),
        reference: virtual_key_secret_reference_request(durable_store, tenant_id, kind).reference,
        previous_digest: Some(AuditDigest::new("sha256:previous").unwrap()),
        event_digest: AuditDigest::new("sha256:current").unwrap(),
    }
}

fn provider_credential_reference_request(
    durable_store: DurableStoreKind,
    tenant_id: TenantId,
) -> ApplicationProviderCredentialReferenceRequest {
    ApplicationProviderCredentialReferenceRequest {
        durable_store,
        reference: ProviderCredentialReferenceCommand {
            storage_key: TenantStorageKey::tenant(tenant_id),
            tenant_id,
            provider_credential_id: ProviderCredentialId::new(),
            provider_name: "openai".to_string(),
            secret_ref: SecretRef::new("vault", "providers/openai", Some("v3")),
            rotated_at_unix_ms: 1_800_000_003_000,
        },
    }
}

fn provider_credential_rotation_request(
    durable_store: DurableStoreKind,
    tenant_id: TenantId,
    principal: Principal,
) -> ApplicationProviderCredentialRotationRequest {
    ApplicationProviderCredentialRotationRequest {
        durable_store,
        action: control_plane_action(
            tenant_id,
            principal,
            ControlPlaneOperation::ProviderCredentialRotate,
            ResourceKind::ProviderCredential,
        ),
        reference: provider_credential_reference_request(durable_store, tenant_id).reference,
        previous_digest: Some(AuditDigest::new("sha256:previous").unwrap()),
        event_digest: AuditDigest::new("sha256:current").unwrap(),
    }
}

fn configuration_publication_request(
    durable_store: DurableStoreKind,
    tenant_id: TenantId,
    candidate_tenant_id: TenantId,
) -> ApplicationConfigurationPublicationRequest<&'static str> {
    ApplicationConfigurationPublicationRequest {
        durable_store,
        publication: ConfigurationPublicationRequest {
            action: control_plane_action(
                tenant_id,
                control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
                ControlPlaneOperation::ConfigurationPublish,
                ResourceKind::Configuration,
            ),
            current_revision_id: None,
            candidate: ConfigRevision {
                tenant_id: candidate_tenant_id,
                revision_id: PolicyRevisionId::new(),
                published_at_unix_ms: 10_000,
                payload: "config-payload",
            },
        },
        previous_digest: Some(AuditDigest::new("sha256:previous").unwrap()),
        event_digest: AuditDigest::new("sha256:current").unwrap(),
    }
}

#[test]
fn application_request_authentication_accepts_data_plane_scope_for_data_plane_routes() {
    let tenant_id = TenantId::new();
    let request = application_authentication_request(
        "/v1/responses",
        GatewayHttpMethod::Post,
        tenant_id,
        CredentialScope::DataPlane,
    );
    let request_debug = format!("{request:?}");
    let plan =
        plan_application_request_authentication(GatewayHttpPolicy::production_default(), request)
            .unwrap();
    let plan_debug = format!("{plan:?}");

    assert_eq!(plan.http.route, GatewayHttpRouteKind::DataPlaneResponses);
    assert_eq!(plan.required_scope, CredentialScope::DataPlane);
    assert_eq!(plan.principal.tenant_id, Some(tenant_id));
    assert_eq!(plan.principal.role, Role::Operator);
    assert!(!request_debug.contains(&tenant_id.to_string()));
    assert!(!request_debug.contains("/v1/responses"));
    assert!(!request_debug.contains("DataPlane"));
    assert!(!plan_debug.contains(&tenant_id.to_string()));
    assert!(!plan_debug.contains("DataPlane"));
}

#[test]
fn application_request_authentication_rejects_scope_bypass_between_data_and_control_planes() {
    let tenant_id = TenantId::new();
    let wrong_route =
        ApplicationRequestAuthenticationError::WrongRoute(GatewayHttpRouteKind::Unknown);
    assert_eq!(wrong_route.to_string(), "request authentication is denied");
    assert!(!wrong_route.to_string().contains("Unknown"));

    for (path, method, route) in [
        (
            "/v1/responses",
            GatewayHttpMethod::Post,
            GatewayHttpRouteKind::DataPlaneResponses,
        ),
        (
            "/v1/responses/compact",
            GatewayHttpMethod::Post,
            GatewayHttpRouteKind::DataPlaneCompact,
        ),
        (
            "/v1/realtime",
            GatewayHttpMethod::Get,
            GatewayHttpRouteKind::DataPlaneWebSocket,
        ),
        (
            "/v1/quota",
            GatewayHttpMethod::Get,
            GatewayHttpRouteKind::DataPlaneQuota,
        ),
    ] {
        let control_on_data = plan_application_request_authentication(
            GatewayHttpPolicy::production_default(),
            application_authentication_request(
                path,
                method,
                tenant_id,
                CredentialScope::ControlPlane,
            ),
        )
        .unwrap_err();
        assert_eq!(
            control_on_data,
            ApplicationRequestAuthenticationError::CredentialScopeMismatch {
                route,
                actual: CredentialScope::ControlPlane,
                required: CredentialScope::DataPlane,
            }
        );
        assert_eq!(
            control_on_data.to_string(),
            "request authentication is denied"
        );
        let control_on_data_debug = format!("{control_on_data:?}");
        assert!(!control_on_data_debug.contains(&tenant_id.to_string()));
        assert!(!control_on_data_debug.contains("ControlPlane"));
        assert!(!control_on_data_debug.contains("DataPlane"));
        assert!(!control_on_data_debug.contains("DataPlaneResponses"));
    }

    let mut data_on_control = None;
    for path in [
        "/admin/keys",
        "/v1/admin/keys",
        "/scim/v2/Users",
        "/v1/scim/v2/Users",
    ] {
        let error = plan_application_request_authentication(
            GatewayHttpPolicy::production_default(),
            application_authentication_request(
                path,
                GatewayHttpMethod::Get,
                tenant_id,
                CredentialScope::DataPlane,
            ),
        )
        .unwrap_err();
        assert_eq!(
            error,
            ApplicationRequestAuthenticationError::CredentialScopeMismatch {
                route: GatewayHttpRouteKind::ControlPlane,
                actual: CredentialScope::DataPlane,
                required: CredentialScope::ControlPlane,
            }
        );
        assert_eq!(error.to_string(), "request authentication is denied");
        data_on_control = Some(error);
    }

    let response =
        plan_application_request_authentication_error_response(&data_on_control.unwrap());
    assert_eq!(response.http, None);
    let authentication = response
        .authentication
        .expect("expected authentication response");
    assert_eq!(
        authentication.status,
        AuthenticationErrorStatus::Unauthorized
    );
    assert_eq!(authentication.code, "credential_scope_not_allowed");
    assert!(!authentication.message.contains("ControlPlane"));
    assert!(!authentication.message.contains("DataPlane"));
    assert!(!authentication.message.contains(&tenant_id.to_string()));
}

#[test]
fn application_request_authentication_rejects_missing_role_without_admin_fallback() {
    let tenant_id = TenantId::new();
    let mut request = application_authentication_request(
        "/admin/keys",
        GatewayHttpMethod::Get,
        tenant_id,
        CredentialScope::ControlPlane,
    );
    request.claims.role_claim = None;

    let error =
        plan_application_request_authentication(GatewayHttpPolicy::production_default(), request)
            .unwrap_err();
    let response = plan_application_request_authentication_error_response(&error);
    let authentication = response
        .authentication
        .expect("expected authentication response");
    assert_eq!(
        authentication.status,
        AuthenticationErrorStatus::Unauthorized
    );
    assert_eq!(authentication.code, "role_not_authorized");
    assert!(!authentication.message.contains("Admin"));
    assert!(!authentication.message.contains(&tenant_id.to_string()));
}

#[test]
fn application_provider_capability_selects_compatible_route_before_admission() {
    let request = ApplicationProviderCapabilityRequest {
        request: CapabilityRequest::new(CapabilitySet::new(vec![
            ModelCapability::ResponsesApi,
            ModelCapability::Streaming,
            ModelCapability::Tools,
        ])),
        candidates: vec![
            ProviderRouteCapabilityCandidate::new(
                ProviderRoute::new(
                    ProviderId::OpenAi,
                    ProviderEndpoint::Responses,
                    ProviderWireFormat::OpenAiResponses,
                    "gpt-base",
                )
                .unwrap(),
                CapabilitySet::new(vec![ModelCapability::ResponsesApi]),
            ),
            ProviderRouteCapabilityCandidate::new(
                ProviderRoute::new(
                    ProviderId::Copilot,
                    ProviderEndpoint::Responses,
                    ProviderWireFormat::OpenAiResponses,
                    "gpt-5.3-codex",
                )
                .unwrap(),
                CapabilitySet::new(vec![
                    ModelCapability::ResponsesApi,
                    ModelCapability::Streaming,
                    ModelCapability::Tools,
                ]),
            ),
        ],
    };
    let request_debug = format!("{request:?}");
    assert!(request_debug.contains("ApplicationProviderCapabilityRequest"));
    assert!(!request_debug.contains("gpt-base"));
    assert!(!request_debug.contains("gpt-5.3-codex"));

    let plan = plan_application_provider_capability(request).unwrap();
    let plan_debug = format!("{plan:?}");
    assert!(plan_debug.contains("ApplicationProviderCapabilityPlan"));
    assert!(!plan_debug.contains("gpt-base"));
    assert!(!plan_debug.contains("gpt-5.3-codex"));

    assert_eq!(plan.provider.route.provider, ProviderId::Copilot);
    assert_eq!(plan.provider.route.model, "gpt-5.3-codex");
    assert!(
        plan.provider
            .capabilities
            .contains(ModelCapability::Streaming)
    );
}

#[test]
fn application_provider_capability_errors_are_stable_and_redacted() {
    let error = plan_application_provider_capability(ApplicationProviderCapabilityRequest {
        request: CapabilityRequest::new(CapabilitySet::new(vec![
            ModelCapability::ResponsesApi,
            ModelCapability::RemoteCompact,
        ])),
        candidates: vec![ProviderRouteCapabilityCandidate::new(
            ProviderRoute::new(
                ProviderId::OpenAi,
                ProviderEndpoint::Responses,
                ProviderWireFormat::OpenAiResponses,
                "internal-premium-model",
            )
            .unwrap(),
            CapabilitySet::new(vec![ModelCapability::ResponsesApi]),
        )],
    })
    .unwrap_err();
    assert!(matches!(
        error,
        ApplicationProviderCapabilityError::Incompatible(_)
    ));
    let error_debug = format!("{error:?}");
    assert!(!error_debug.contains("internal-premium-model"));
    assert!(!error_debug.contains("RemoteCompact"));
    let response = plan_application_provider_capability_error_response(&error);
    assert_eq!(response.status, CapabilityErrorStatus::UnprocessableRequest);
    assert_eq!(response.code, "model_capability_unsupported");
    assert_eq!(
        response.message,
        "requested model capabilities are not supported"
    );
    assert!(!response.message.contains("internal-premium-model"));
    assert!(!response.message.contains("OpenAi"));

    let unavailable = plan_application_provider_capability(ApplicationProviderCapabilityRequest {
        request: CapabilityRequest::new(CapabilitySet::new(vec![ModelCapability::ResponsesApi])),
        candidates: vec![],
    })
    .unwrap_err();
    let unavailable_response = plan_application_provider_capability_error_response(&unavailable);
    assert_eq!(
        unavailable_response.status,
        CapabilityErrorStatus::ServiceUnavailable
    );
    assert_eq!(unavailable_response.code, "model_route_unavailable");
}

#[test]
fn application_provider_retry_is_bounded_to_precommit_stages() {
    let allowed = plan_application_provider_retry(ApplicationProviderRetryRequest {
        policy: ProviderRetryPolicy::single_retry(),
        stage: ProviderRetryStage::BeforeFirstByte,
        cause: ProviderRetryCause::NextModel,
        error_class: ProviderErrorClass::Transient,
        attempted_precommit_retries: 0,
    });
    assert_eq!(allowed.retry.decision, ProviderRetryDecision::Allowed);
    assert_eq!(allowed.retry.remaining_precommit_retries, 1);

    let ineligible = plan_application_provider_retry(ApplicationProviderRetryRequest {
        policy: ProviderRetryPolicy::single_retry(),
        stage: ProviderRetryStage::BeforeFirstByte,
        cause: ProviderRetryCause::RotateCredential,
        error_class: ProviderErrorClass::NotFound,
        attempted_precommit_retries: 0,
    });
    assert_eq!(
        ineligible.retry.decision,
        ProviderRetryDecision::DeniedNotRetryable
    );

    let exhausted = plan_application_provider_retry(ApplicationProviderRetryRequest {
        policy: ProviderRetryPolicy::single_retry(),
        stage: ProviderRetryStage::BeforeDispatch,
        cause: ProviderRetryCause::RotateCredential,
        error_class: ProviderErrorClass::Auth,
        attempted_precommit_retries: 1,
    });
    assert_eq!(
        exhausted.retry.decision,
        ProviderRetryDecision::DeniedBudgetExhausted
    );
    assert_eq!(exhausted.retry.remaining_precommit_retries, 0);

    let committed = plan_application_provider_retry(ApplicationProviderRetryRequest {
        policy: ProviderRetryPolicy::single_retry(),
        stage: ProviderRetryStage::AfterFirstByte,
        cause: ProviderRetryCause::RotateCredential,
        error_class: ProviderErrorClass::Auth,
        attempted_precommit_retries: 0,
    });
    assert_eq!(
        committed.retry.decision,
        ProviderRetryDecision::DeniedCommitted
    );

    let cancelled = plan_application_provider_retry(ApplicationProviderRetryRequest {
        policy: ProviderRetryPolicy::single_retry(),
        stage: ProviderRetryStage::AfterCancellation,
        cause: ProviderRetryCause::NextModel,
        error_class: ProviderErrorClass::Transient,
        attempted_precommit_retries: 0,
    });
    assert_eq!(
        cancelled.retry.decision,
        ProviderRetryDecision::DeniedCommitted
    );
}

#[test]
fn application_provider_circuit_breaker_opens_and_recovers_through_spi_plan() {
    let policy = ProviderCircuitBreakerPolicy {
        failure_threshold: 2,
        cooldown_ms: 1_000,
    };
    let initial =
        plan_application_provider_circuit_breaker(ApplicationProviderCircuitBreakerRequest {
            policy,
            state: ProviderCircuitBreakerState::default(),
            now_unix_ms: 10_000,
        });
    assert_eq!(initial.decision, ProviderCircuitBreakerDecision::Closed);

    let first = plan_application_provider_circuit_breaker_event(
        ApplicationProviderCircuitBreakerEventRequest {
            policy,
            state: ProviderCircuitBreakerState::default(),
            event: ProviderCircuitBreakerEvent::Failure {
                now_unix_ms: 10_000,
            },
        },
    );
    assert_eq!(first.event.next_state.consecutive_failures, 1);
    assert_eq!(first.event.decision, ProviderCircuitBreakerDecision::Closed);

    let second = plan_application_provider_circuit_breaker_event(
        ApplicationProviderCircuitBreakerEventRequest {
            policy,
            state: first.event.next_state,
            event: ProviderCircuitBreakerEvent::Failure {
                now_unix_ms: 10_100,
            },
        },
    );
    assert_eq!(second.event.next_state.open_until_unix_ms, Some(11_100));
    let open =
        plan_application_provider_circuit_breaker(ApplicationProviderCircuitBreakerRequest {
            policy,
            state: second.event.next_state,
            now_unix_ms: 10_500,
        });
    assert_eq!(
        open.decision,
        ProviderCircuitBreakerDecision::Open {
            retry_after_ms: 600
        }
    );

    let half_open =
        plan_application_provider_circuit_breaker(ApplicationProviderCircuitBreakerRequest {
            policy,
            state: second.event.next_state,
            now_unix_ms: 11_100,
        });
    assert_eq!(
        half_open.decision,
        ProviderCircuitBreakerDecision::HalfOpenProbe
    );
}

#[test]
fn application_data_plane_composes_http_policy_and_gateway_admission() {
    let tenant_id = TenantId::new();
    let request = data_plane_request(tenant_id);
    let request_debug = format!("{request:?}");
    assert!(request_debug.contains("ApplicationDataPlaneRequest"));
    assert!(!request_debug.contains(&tenant_id.to_string()));
    assert!(!request_debug.contains("/v1/responses"));

    let plan =
        plan_application_data_plane(GatewayHttpPolicy::production_default(), request).unwrap();
    let plan_debug = format!("{plan:?}");
    assert!(plan_debug.contains("ApplicationDataPlanePlan"));
    assert!(!plan_debug.contains(&tenant_id.to_string()));
    assert!(!plan_debug.contains("/v1/responses"));

    assert_eq!(plan.http.route, GatewayHttpRouteKind::DataPlaneResponses);
    assert_eq!(
        plan.inspection.result.coverage(),
        prodex_domain::InspectionCoverage::Unsupported
    );
    assert_eq!(
        plan.inspection.result.classification(),
        prodex_domain::DataClassification::Internal
    );
    assert_eq!(plan.admission.tenant.tenant_id, tenant_id);
    assert_eq!(plan.admission.reservation.storage_key.tenant_id, tenant_id);
    assert_eq!(
        plan.http
            .preserved_upstream_headers
            .iter()
            .map(|header| header.name.as_str())
            .collect::<Vec<_>>(),
        vec!["traceparent", "session_id"]
    );
    assert_eq!(plan.http.stripped_headers, vec!["authorization"]);
}

#[test]
fn application_data_plane_rejects_control_plane_route_before_admission() {
    let tenant_id = TenantId::new();
    let mut request = data_plane_request(tenant_id);
    request.http.path = "/admin/keys".to_string();
    request.http.method = GatewayHttpMethod::Get;

    assert_eq!(
        plan_application_data_plane(GatewayHttpPolicy::production_default(), request),
        Err(ApplicationDataPlaneError::WrongRoute(
            GatewayHttpRouteKind::ControlPlane
        ))
    );
    assert_eq!(
        ApplicationDataPlaneError::WrongRoute(GatewayHttpRouteKind::ControlPlane).to_string(),
        "application route is not available"
    );
    assert!(
        !ApplicationDataPlaneError::WrongRoute(GatewayHttpRouteKind::ControlPlane)
            .to_string()
            .contains("ControlPlane")
    );
    let error_debug = format!(
        "{:?}",
        ApplicationDataPlaneError::WrongRoute(GatewayHttpRouteKind::ControlPlane)
    );
    assert!(!error_debug.contains("ControlPlane"));
    assert!(!error_debug.contains("/admin/keys"));
}

#[test]
fn application_data_plane_error_response_exposes_stable_http_and_admission_errors() {
    let tenant_id = TenantId::new();
    let mut request = data_plane_request(tenant_id);
    request.http.body_len = 1024;
    let error = plan_application_data_plane(
        GatewayHttpPolicy {
            max_body_bytes: 128,
            ..GatewayHttpPolicy::production_default()
        },
        request,
    )
    .unwrap_err();

    let response = plan_application_data_plane_error_response(&error);
    let http = response.http.expect("expected HTTP error response");
    assert_eq!(http.status, GatewayHttpErrorStatus::PayloadTooLarge);
    assert_eq!(http.code, "request_body_too_large");
    assert_eq!(http.message, "request body is too large");
    assert!(!http.message.contains("1024"));
    assert_eq!(response.admission, None);

    let mut admission = data_plane_request(tenant_id);
    admission.admission.principal =
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane);
    let error = plan_application_data_plane(GatewayHttpPolicy::production_default(), admission)
        .unwrap_err();
    let response = plan_application_data_plane_error_response(&error);
    assert_eq!(response.http, None);
    let admission = response
        .admission
        .expect("expected admission error response");
    assert_eq!(admission.status, GatewayAdmissionErrorStatus::Forbidden);
    assert_eq!(admission.code, "credential_scope_not_allowed");
    assert!(!admission.message.contains("ControlPlane"));
    assert!(!admission.message.contains(&tenant_id.to_string()));

    let mut wrong_route = data_plane_request(tenant_id);
    wrong_route.http.path = "/admin/keys".to_string();
    wrong_route.http.method = GatewayHttpMethod::Get;
    let error = plan_application_data_plane(GatewayHttpPolicy::production_default(), wrong_route)
        .unwrap_err();
    let response = plan_application_data_plane_error_response(&error);
    let http = response.http.expect("expected wrong-route response");
    assert_eq!(http.status, GatewayHttpErrorStatus::BadRequest);
    assert_eq!(http.code, "route_not_available");
    assert_eq!(http.message, "route is not available for this endpoint");
    assert!(!http.message.contains("ControlPlane"));
    assert!(!http.message.contains("/admin"));
    assert_eq!(response.admission, None);
}

#[test]
fn application_atomic_reservation_selects_postgres_storage_plan() {
    let tenant_id = TenantId::new();
    let plan = plan_application_atomic_reservation(atomic_reservation_request(
        DurableStoreKind::Postgres,
        tenant_id,
    ))
    .unwrap();

    let ApplicationAtomicReservationStoragePlan::Postgres(storage) = plan.storage else {
        panic!("expected Postgres atomic reservation storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.statements[0].name, "set_tenant_context");
    assert_eq!(storage.statements[1].name, "reserve_usage_atomically");
}

#[test]
fn application_atomic_reservation_selects_sqlite_storage_plan() {
    let tenant_id = TenantId::new();
    let plan = plan_application_atomic_reservation(atomic_reservation_request(
        DurableStoreKind::Sqlite,
        tenant_id,
    ))
    .unwrap();

    let ApplicationAtomicReservationStoragePlan::Sqlite(storage) = plan.storage else {
        panic!("expected SQLite atomic reservation storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.statements[0].name, "begin_immediate_reservation");
    assert_eq!(storage.statements[1].name, "reserve_usage_locally");
}

#[test]
fn application_atomic_reservation_rejects_invalid_storage_inputs() {
    let tenant_id = TenantId::new();
    let mut request = atomic_reservation_request(DurableStoreKind::Postgres, tenant_id);
    request.reservation.storage_key = TenantStorageKey::tenant(TenantId::new());

    assert!(matches!(
        plan_application_atomic_reservation(request),
        Err(ApplicationAtomicReservationError::Postgres(_))
    ));
}

#[test]
fn application_atomic_reservation_error_responses_are_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let other_tenant = TenantId::new();

    let error = ApplicationAtomicReservationError::Postgres(
        prodex_storage_postgres::PostgresStoragePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        },
    );
    let response = plan_application_atomic_reservation_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationAtomicReservationErrorStatus::ServiceUnavailable
    );
    assert_eq!(response.code, "atomic_reservation_storage_unavailable");
    assert_eq!(
        response.message,
        "atomic reservation storage is temporarily unavailable"
    );
    assert!(!response.message.contains("PostgreSQL"));
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains(&other_tenant.to_string()));
}

#[test]
fn application_quota_read_composes_http_route_and_gateway_authorization() {
    let tenant_id = TenantId::new();
    let request = quota_read_request(tenant_id);
    let request_debug = format!("{request:?}");
    assert!(request_debug.contains("ApplicationQuotaReadRequest"));
    assert!(!request_debug.contains(&tenant_id.to_string()));
    assert!(!request_debug.contains("/v1/quota"));

    let plan =
        plan_application_quota_read(GatewayHttpPolicy::production_default(), request).unwrap();
    let plan_debug = format!("{plan:?}");
    assert!(plan_debug.contains("ApplicationQuotaReadPlan"));
    assert!(!plan_debug.contains(&tenant_id.to_string()));
    assert!(!plan_debug.contains("/v1/quota"));

    assert_eq!(plan.http.route, GatewayHttpRouteKind::DataPlaneQuota);
    assert_eq!(plan.authorization.tenant.tenant_id, tenant_id);
    assert_eq!(
        format!("{:?}", plan.authorization.authorization_boundary),
        "DataPlaneQuota"
    );
    assert_eq!(
        plan.http
            .preserved_upstream_headers
            .iter()
            .map(|header| header.name.as_str())
            .collect::<Vec<_>>(),
        vec!["traceparent", "session_id"]
    );
    assert_eq!(plan.http.stripped_headers, vec!["authorization"]);
}

#[test]
fn application_quota_read_rejects_wrong_route_before_authorization() {
    let tenant_id = TenantId::new();
    let mut request = quota_read_request(tenant_id);
    request.http.path = "/v1/responses".to_string();
    request.http.method = GatewayHttpMethod::Post;

    assert_eq!(
        plan_application_quota_read(GatewayHttpPolicy::production_default(), request),
        Err(ApplicationQuotaReadError::WrongRoute(
            GatewayHttpRouteKind::DataPlaneResponses
        ))
    );
    assert_eq!(
        ApplicationQuotaReadError::WrongRoute(GatewayHttpRouteKind::DataPlaneResponses).to_string(),
        "application route is not available"
    );
    assert!(
        !ApplicationQuotaReadError::WrongRoute(GatewayHttpRouteKind::DataPlaneResponses)
            .to_string()
            .contains("DataPlaneResponses")
    );
    let error_debug = format!(
        "{:?}",
        ApplicationQuotaReadError::WrongRoute(GatewayHttpRouteKind::DataPlaneResponses)
    );
    assert!(!error_debug.contains("DataPlaneResponses"));
    assert!(!error_debug.contains("/v1/responses"));
}

#[test]
fn application_quota_read_error_response_exposes_stable_http_and_authorization_errors() {
    let tenant_id = TenantId::new();
    let mut request = quota_read_request(tenant_id);
    request.http.method = GatewayHttpMethod::Post;
    let error =
        plan_application_quota_read(GatewayHttpPolicy::production_default(), request).unwrap_err();
    let response = plan_application_quota_read_error_response(&error);
    let http = response.http.expect("expected HTTP error response");
    assert_eq!(http.status, GatewayHttpErrorStatus::MethodNotAllowed);
    assert_eq!(http.code, "method_not_allowed");
    assert_eq!(response.authorization, None);

    let mut authorization = quota_read_request(tenant_id);
    authorization.authorization.principal =
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane);
    let error = plan_application_quota_read(GatewayHttpPolicy::production_default(), authorization)
        .unwrap_err();
    let response = plan_application_quota_read_error_response(&error);
    assert_eq!(response.http, None);
    let authorization = response
        .authorization
        .expect("expected authorization error response");
    assert_eq!(
        authorization.status,
        GatewayQuotaReadAuthorizationErrorStatus::Forbidden
    );
    assert_eq!(authorization.code, "credential_scope_not_allowed");
    assert!(!authorization.message.contains("ControlPlane"));
    assert!(!authorization.message.contains(&tenant_id.to_string()));
}

#[test]
fn application_usage_reconciliation_composes_gateway_post_provider_accounting() {
    let tenant_id = TenantId::new();
    let plan = plan_application_usage_reconciliation(usage_reconciliation_request(
        DurableStoreKind::Postgres,
        tenant_id,
        UsageAmount::new(100, 1_000),
        UsageAmount::new(40, 400),
    ))
    .unwrap();

    assert_eq!(plan.gateway.tenant.tenant_id, tenant_id);
    assert_eq!(plan.gateway.reconciliation.ledger_events.len(), 2);
    assert_eq!(
        plan.gateway.reconciliation.updated_snapshot.committed,
        UsageAmount::new(41, 410)
    );
    assert_eq!(
        plan.gateway.spans[0].descriptor.name,
        "prodex.gateway.usage_reconciliation"
    );
    let ApplicationUsageReconciliationStoragePlan::Postgres(storage) = plan.storage else {
        panic!("expected Postgres reconciliation storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.ledger_event_count, 2);
    assert_eq!(storage.statements[0].name, "set_tenant_context");
}

#[test]
fn application_usage_reconciliation_selects_sqlite_storage_plan() {
    let tenant_id = TenantId::new();
    let plan = plan_application_usage_reconciliation(usage_reconciliation_request(
        DurableStoreKind::Sqlite,
        tenant_id,
        UsageAmount::new(100, 1_000),
        UsageAmount::new(100, 1_000),
    ))
    .unwrap();

    let ApplicationUsageReconciliationStoragePlan::Sqlite(storage) = plan.storage else {
        panic!("expected SQLite reconciliation storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.ledger_event_count, 1);
    assert_eq!(
        storage.statements[0].name,
        "begin_immediate_usage_reconciliation"
    );
}

#[test]
fn application_usage_reconciliation_rejects_cross_tenant_storage_context() {
    let tenant_id = TenantId::new();
    let mut request = usage_reconciliation_request(
        DurableStoreKind::Postgres,
        tenant_id,
        UsageAmount::new(10, 100),
        UsageAmount::new(1, 10),
    );
    request.gateway.reconciliation.storage_key = TenantStorageKey::tenant(TenantId::new());

    assert!(matches!(
        plan_application_usage_reconciliation(request),
        Err(ApplicationUsageReconciliationError::Postgres(_))
    ));
}

#[test]
fn application_usage_reconciliation_error_responses_are_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let other_tenant = TenantId::new();

    let mut gateway_mismatch = usage_reconciliation_request(
        DurableStoreKind::Postgres,
        tenant_id,
        UsageAmount::new(10, 100),
        UsageAmount::new(1, 10),
    );
    gateway_mismatch.gateway.tenant = TenantContext {
        tenant_id: other_tenant,
    };
    let gateway_error = plan_application_usage_reconciliation(gateway_mismatch).unwrap_err();
    let gateway_response = plan_application_usage_reconciliation_error_response(&gateway_error);
    assert_eq!(
        gateway_response.status,
        ApplicationUsageReconciliationErrorStatus::BadRequest
    );
    assert_eq!(gateway_response.code, "usage_reconciliation_rejected");
    assert_eq!(
        gateway_response.message,
        "usage reconciliation request is invalid"
    );
    assert!(!gateway_response.message.contains(&tenant_id.to_string()));
    assert!(!gateway_response.message.contains(&other_tenant.to_string()));
    assert!(!gateway_response.message.contains("tokens"));

    let storage_error = ApplicationUsageReconciliationError::Postgres(
        prodex_storage_postgres::PostgresStoragePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        },
    );
    let storage_response = plan_application_usage_reconciliation_error_response(&storage_error);
    assert_eq!(
        storage_response.status,
        ApplicationUsageReconciliationErrorStatus::ServiceUnavailable
    );
    assert_eq!(
        storage_response.code,
        "usage_reconciliation_storage_unavailable"
    );
    assert_eq!(
        storage_response.message,
        "usage reconciliation storage is temporarily unavailable"
    );
    assert!(!storage_response.message.contains("PostgreSQL"));
    assert!(!storage_response.message.contains(&tenant_id.to_string()));
    assert!(!storage_response.message.contains(&other_tenant.to_string()));
}

#[test]
fn application_expired_reservation_recovery_selects_postgres_storage_plan() {
    let tenant_id = TenantId::new();
    let plan = plan_application_expired_reservation_recovery(expired_recovery_request(
        DurableStoreKind::Postgres,
        tenant_id,
        UsageAmount::new(10, 100),
        1_500,
    ))
    .unwrap();

    assert_eq!(plan.gateway.tenant.tenant_id, tenant_id);
    assert_eq!(plan.gateway.recovery.ledger_event.tenant_id, tenant_id);
    assert_eq!(
        plan.gateway.spans[0].descriptor.name,
        "prodex.gateway.expired_reservation_recovery"
    );
    let ApplicationExpiredReservationRecoveryStoragePlan::Postgres(storage) = plan.storage else {
        panic!("expected Postgres expired recovery storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.statements[0].name, "set_tenant_context");
    assert_eq!(plan.coordination, None);
}

#[test]
fn application_expired_reservation_recovery_selects_sqlite_storage_plan() {
    let tenant_id = TenantId::new();
    let plan = plan_application_expired_reservation_recovery(expired_recovery_request(
        DurableStoreKind::Sqlite,
        tenant_id,
        UsageAmount::new(10, 100),
        1_500,
    ))
    .unwrap();

    let ApplicationExpiredReservationRecoveryStoragePlan::Sqlite(storage) = plan.storage else {
        panic!("expected SQLite expired recovery storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(
        storage.statements[0].name,
        "begin_immediate_expired_recovery"
    );
    assert_eq!(plan.coordination, None);
}

#[test]
fn application_expired_reservation_recovery_can_include_redis_recovery_lease() {
    let tenant_id = TenantId::new();
    let mut request = expired_recovery_request(
        DurableStoreKind::Postgres,
        tenant_id,
        UsageAmount::new(10, 100),
        1_500,
    );
    request.coordination = Some(ApplicationExpiredReservationRecoveryCoordinationRequest {
        shard: "expired-reservations".to_string(),
        owner_token: "replica-a".to_string(),
        ttl_seconds: 45,
    });

    let plan = plan_application_expired_reservation_recovery(request).unwrap();

    let Some(ApplicationExpiredReservationRecoveryCoordinationPlan::RedisLease(lease)) =
        plan.coordination
    else {
        panic!("expected Redis recovery lease coordination plan");
    };
    assert_eq!(lease.tenant_id, tenant_id);
    assert_eq!(lease.owner_token, "replica-a");
    assert_eq!(lease.ttl_seconds, 45);
    assert!(lease.key.as_str().contains("recovery_lease"));
    assert_eq!(lease.script.name, "prodex_recovery_lease_acquire_v1");
}

#[test]
fn application_expired_reservation_recovery_rejects_invalid_recovery_inputs() {
    let tenant_id = TenantId::new();
    assert!(matches!(
        plan_application_expired_reservation_recovery(expired_recovery_request(
            DurableStoreKind::Postgres,
            tenant_id,
            UsageAmount::new(10, 100),
            1_499,
        )),
        Err(ApplicationExpiredReservationRecoveryError::Postgres(_))
    ));

    let mut request = expired_recovery_request(
        DurableStoreKind::Sqlite,
        tenant_id,
        UsageAmount::new(10, 100),
        1_500,
    );
    request.gateway.recovery.storage_key = TenantStorageKey::tenant(TenantId::new());
    assert!(matches!(
        plan_application_expired_reservation_recovery(request),
        Err(ApplicationExpiredReservationRecoveryError::Sqlite(_))
    ));

    let mut invalid_coordination = expired_recovery_request(
        DurableStoreKind::Postgres,
        tenant_id,
        UsageAmount::new(10, 100),
        1_500,
    );
    invalid_coordination.coordination =
        Some(ApplicationExpiredReservationRecoveryCoordinationRequest {
            shard: "expired-reservations".to_string(),
            owner_token: "   ".to_string(),
            ttl_seconds: 45,
        });
    assert!(matches!(
        plan_application_expired_reservation_recovery(invalid_coordination),
        Err(ApplicationExpiredReservationRecoveryError::Redis(_))
    ));
}

#[test]
fn application_expired_reservation_recovery_error_responses_are_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let other_tenant = TenantId::new();

    let gateway_error = ApplicationExpiredReservationRecoveryError::Gateway(
        prodex_gateway_core::GatewayExpiredReservationRecoveryError::TenantMismatch,
    );
    let gateway_response =
        plan_application_expired_reservation_recovery_error_response(&gateway_error);
    assert_eq!(
        gateway_response.status,
        ApplicationExpiredReservationRecoveryErrorStatus::BadRequest
    );
    assert_eq!(
        gateway_response.code,
        "expired_reservation_recovery_rejected"
    );
    assert_eq!(
        gateway_response.message,
        "expired reservation recovery request is invalid"
    );
    assert!(!gateway_response.message.contains(&tenant_id.to_string()));

    let invalid_storage = plan_application_expired_reservation_recovery(expired_recovery_request(
        DurableStoreKind::Postgres,
        tenant_id,
        UsageAmount::new(10, 100),
        1_499,
    ))
    .unwrap_err();
    let invalid_storage_response =
        plan_application_expired_reservation_recovery_error_response(&invalid_storage);
    assert_eq!(
        invalid_storage_response.status,
        ApplicationExpiredReservationRecoveryErrorStatus::ServiceUnavailable
    );
    assert_eq!(
        invalid_storage_response.code,
        "expired_reservation_recovery_storage_unavailable"
    );
    assert!(!invalid_storage_response.message.contains("tokens"));
    assert!(
        !invalid_storage_response
            .message
            .contains(&tenant_id.to_string())
    );

    let storage_error = ApplicationExpiredReservationRecoveryError::Postgres(
        prodex_storage_postgres::PostgresStoragePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: other_tenant,
        },
    );
    let storage_response =
        plan_application_expired_reservation_recovery_error_response(&storage_error);
    assert_eq!(
        storage_response.status,
        ApplicationExpiredReservationRecoveryErrorStatus::ServiceUnavailable
    );
    assert_eq!(
        storage_response.code,
        "expired_reservation_recovery_storage_unavailable"
    );
    assert_eq!(
        storage_response.message,
        "expired reservation recovery storage is temporarily unavailable"
    );
    assert!(!storage_response.message.contains("PostgreSQL"));
    assert!(!storage_response.message.contains(&tenant_id.to_string()));
    assert!(!storage_response.message.contains(&other_tenant.to_string()));

    let redis_error = ApplicationExpiredReservationRecoveryError::Redis(
        prodex_storage_redis::RedisPlanError::EmptyLeaseOwner,
    );
    let redis_response = plan_application_expired_reservation_recovery_error_response(&redis_error);
    assert_eq!(
        redis_response.status,
        ApplicationExpiredReservationRecoveryErrorStatus::ServiceUnavailable
    );
    assert_eq!(redis_response.code, "recovery_coordination_unavailable");
    assert_eq!(
        redis_response.message,
        "expired reservation recovery coordination is temporarily unavailable"
    );
    assert!(!redis_response.message.contains("Redis"));
    assert!(!redis_response.message.contains("owner"));
}

#[test]
fn application_recovery_lease_release_is_tenant_scoped_and_owner_token_based() {
    let tenant_id = TenantId::new();
    let plan = plan_application_recovery_lease_release(ApplicationRecoveryLeaseReleaseRequest {
        tenant_id,
        storage_key: TenantStorageKey::tenant(tenant_id),
        shard: "expired-reservations".to_string(),
        owner_token: "replica-a".to_string(),
    })
    .unwrap();

    assert_eq!(plan.release.tenant_id, tenant_id);
    assert_eq!(plan.release.storage_key.tenant_id, tenant_id);
    assert_eq!(plan.release.owner_token, "replica-a");
    assert!(plan.release.key.as_str().contains("recovery_lease"));
    assert_eq!(plan.release.script.name, "prodex_recovery_lease_release_v1");
}

#[test]
fn application_recovery_lease_release_rejects_cross_tenant_or_empty_owner() {
    let tenant_id = TenantId::new();
    let other_tenant = TenantId::new();

    assert!(matches!(
        plan_application_recovery_lease_release(ApplicationRecoveryLeaseReleaseRequest {
            tenant_id,
            storage_key: TenantStorageKey::tenant(other_tenant),
            shard: "expired-reservations".to_string(),
            owner_token: "replica-a".to_string(),
        }),
        Err(ApplicationRecoveryLeaseReleaseError::Redis(_))
    ));
    assert!(matches!(
        plan_application_recovery_lease_release(ApplicationRecoveryLeaseReleaseRequest {
            tenant_id,
            storage_key: TenantStorageKey::tenant(tenant_id),
            shard: "expired-reservations".to_string(),
            owner_token: "   ".to_string(),
        }),
        Err(ApplicationRecoveryLeaseReleaseError::Redis(_))
    ));
}

#[test]
fn application_recovery_lease_release_error_responses_are_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let other_tenant = TenantId::new();
    let error = plan_application_recovery_lease_release(ApplicationRecoveryLeaseReleaseRequest {
        tenant_id,
        storage_key: TenantStorageKey::tenant(other_tenant),
        shard: "expired-reservations".to_string(),
        owner_token: "replica-a-secret-owner-token".to_string(),
    })
    .unwrap_err();

    let response = plan_application_recovery_lease_release_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationRecoveryLeaseReleaseErrorStatus::ServiceUnavailable
    );
    assert_eq!(response.code, "recovery_lease_release_unavailable");
    assert_eq!(
        response.message,
        "recovery lease release is temporarily unavailable"
    );
    assert!(!response.message.contains("Redis"));
    assert!(!response.message.contains("owner"));
    assert!(!response.message.contains("replica-a-secret-owner-token"));
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains(&other_tenant.to_string()));
}

#[test]
fn application_control_plane_authorized_action_selects_postgres_audit_storage_plan() {
    let tenant_id = TenantId::new();
    let plan = plan_application_control_plane_with_audit_storage(control_plane_audit_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::PolicyPublish,
        ResourceKind::Policy,
    ))
    .unwrap();

    let ControlPlaneDecision::Authorized(action) = plan.decision else {
        panic!("expected authorized control-plane action");
    };
    assert_eq!(action.tenant.tenant_id, tenant_id);
    assert_eq!(action.audit_write.tenant_partition_key, tenant_id);
    let ApplicationControlPlaneAuditStoragePlan::Postgres(storage) = plan.audit_storage else {
        panic!("expected Postgres audit storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.statements[0].name, "set_tenant_context");
}

#[test]
fn application_control_plane_denied_action_selects_sqlite_audit_storage_plan() {
    let tenant_id = TenantId::new();
    let plan = plan_application_control_plane_with_audit_storage(control_plane_audit_request(
        DurableStoreKind::Sqlite,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::DataPlane),
        ControlPlaneOperation::VirtualKeyCreate,
        ResourceKind::VirtualKey,
    ))
    .unwrap();

    let ControlPlaneDecision::Denied {
        audit_event,
        audit_write,
        ..
    } = plan.decision
    else {
        panic!("expected denied control-plane action");
    };
    assert_eq!(audit_event.tenant_id, tenant_id);
    assert_eq!(audit_write.tenant_partition_key, tenant_id);
    let ApplicationControlPlaneAuditStoragePlan::Sqlite(storage) = plan.audit_storage else {
        panic!("expected SQLite audit storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.statements[0].name, "begin_immediate_audit_append");
}

#[test]
fn application_control_plane_audit_from_http_validates_route_before_storage() {
    let tenant_id = TenantId::new();
    let request = control_plane_audit_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::BudgetUpdate,
        ResourceKind::Budget,
    );
    let mut http = control_plane_http_request("/admin/budgets/default");
    http.method = GatewayHttpMethod::Patch;

    let plan = plan_application_control_plane_with_audit_storage_from_http(request, &http).unwrap();
    let ControlPlaneDecision::Authorized(action) = plan.decision else {
        panic!("expected authorized budget update");
    };
    assert_eq!(action.operation, ControlPlaneOperation::BudgetUpdate);
    assert_eq!(action.audit_write.tenant_partition_key, tenant_id);
    let ApplicationControlPlaneAuditStoragePlan::Postgres(storage) = plan.audit_storage else {
        panic!("expected Postgres audit storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);

    let wrong_route = plan_application_control_plane_with_audit_storage_from_http(
        control_plane_audit_request(
            DurableStoreKind::Postgres,
            tenant_id,
            control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
            ControlPlaneOperation::BudgetUpdate,
            ResourceKind::Budget,
        ),
        &control_plane_http_request("/admin/keys"),
    )
    .unwrap_err();
    let wrong_route_response = plan_application_control_plane_audit_error_response(&wrong_route);
    assert_eq!(
        wrong_route_response.status,
        ApplicationControlPlaneAuditErrorStatus::BadRequest
    );
    assert_eq!(wrong_route_response.code, "control_plane_route_invalid");
    assert_eq!(
        wrong_route_response.message,
        "control-plane route is invalid"
    );
    assert!(!wrong_route_response.message.contains("BudgetUpdate"));
    assert!(!wrong_route_response.message.contains("VirtualKeyCreate"));
    assert!(
        !wrong_route_response
            .message
            .contains(&tenant_id.to_string())
    );

    let mut method_not_allowed = control_plane_http_request("/admin/budgets/default");
    method_not_allowed.method = GatewayHttpMethod::Get;
    let method_error = plan_application_control_plane_with_audit_storage_from_http(
        control_plane_audit_request(
            DurableStoreKind::Postgres,
            tenant_id,
            control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
            ControlPlaneOperation::BudgetUpdate,
            ResourceKind::Budget,
        ),
        &method_not_allowed,
    )
    .unwrap_err();
    let method_response = plan_application_control_plane_audit_error_response(&method_error);
    assert_eq!(
        method_response.status,
        ApplicationControlPlaneAuditErrorStatus::MethodNotAllowed
    );
    assert_eq!(method_response.code, "control_plane_method_not_allowed");
}

#[test]
fn application_control_plane_audit_correlation_from_http_uses_trace_and_audit_event() {
    let tenant_id = TenantId::new();
    let request_id = RequestId::new();
    let call_id = CallId::new();
    let mut http = control_plane_http_request("/admin/budgets/default");
    http.method = GatewayHttpMethod::Patch;
    let audit = plan_application_control_plane_with_audit_storage_from_http(
        control_plane_audit_request(
            DurableStoreKind::Postgres,
            tenant_id,
            control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
            ControlPlaneOperation::BudgetUpdate,
            ResourceKind::Budget,
        ),
        &http,
    )
    .unwrap();
    let expected_audit_event_id = match &audit.decision {
        ControlPlaneDecision::Authorized(action) => action.audit_event.id,
        ControlPlaneDecision::Denied { audit_event, .. } => audit_event.id,
    };

    let plan = plan_application_control_plane_audit_correlation_from_http(
        ApplicationControlPlaneAuditCorrelationRequest {
            request_id,
            call_id: Some(call_id),
            http_policy: GatewayHttpPolicy::production_default(),
            http: http.clone(),
            audit,
        },
    )
    .unwrap();

    assert_eq!(plan.correlation.request_id, request_id);
    assert_eq!(plan.correlation.call_id, Some(call_id));
    assert_eq!(plan.correlation.tenant_id, Some(tenant_id));
    assert_eq!(
        plan.correlation.audit_event_id,
        Some(expected_audit_event_id)
    );
    assert_eq!(
        plan.correlation.trace_id.as_ref().unwrap().as_str(),
        "4bf92f3577b34da6a3ce929d0e0e4736"
    );

    http.headers
        .retain(|header| header.normalized_name() != "traceparent");
    let audit_without_trace = plan_application_control_plane_with_audit_storage_from_http(
        control_plane_audit_request(
            DurableStoreKind::Postgres,
            tenant_id,
            control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
            ControlPlaneOperation::BudgetUpdate,
            ResourceKind::Budget,
        ),
        &http,
    )
    .unwrap();
    let error = plan_application_control_plane_audit_correlation_from_http(
        ApplicationControlPlaneAuditCorrelationRequest {
            request_id: RequestId::new(),
            call_id: None,
            http_policy: GatewayHttpPolicy::production_default(),
            http,
            audit: audit_without_trace,
        },
    )
    .unwrap_err();
    let response = plan_application_control_plane_audit_correlation_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationControlPlaneAuditCorrelationErrorStatus::BadRequest
    );
    assert_eq!(response.code, "invalid_trace_context");
    assert_eq!(
        response.message,
        "trace context is required and must be valid"
    );
    assert!(!response.message.contains(&tenant_id.to_string()));
}

#[test]
fn application_control_plane_audit_emission_span_uses_trace_only_correlation_attributes() {
    let tenant_id = TenantId::new();
    let mut http = control_plane_http_request("/admin/budgets/default");
    http.method = GatewayHttpMethod::Patch;
    let audit = plan_application_control_plane_with_audit_storage_from_http(
        control_plane_audit_request(
            DurableStoreKind::Postgres,
            tenant_id,
            control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
            ControlPlaneOperation::BudgetUpdate,
            ResourceKind::Budget,
        ),
        &http,
    )
    .unwrap();
    let correlation = plan_application_control_plane_audit_correlation_from_http(
        ApplicationControlPlaneAuditCorrelationRequest {
            request_id: RequestId::new(),
            call_id: Some(CallId::new()),
            http_policy: GatewayHttpPolicy::production_default(),
            http,
            audit,
        },
    )
    .unwrap()
    .correlation;

    let plan = plan_application_control_plane_audit_emission_span(
        ApplicationControlPlaneAuditEmissionSpanRequest {
            correlation: correlation.clone(),
        },
    )
    .unwrap();

    assert_eq!(
        plan.span.descriptor.kind,
        prodex_domain::GatewaySpanKind::AuditEmission
    );
    assert_eq!(plan.span.descriptor.name, "prodex.control_plane.audit.emit");
    assert_eq!(plan.span.correlation, correlation);
    assert!(plan.span.trace_context.is_none());
    assert!(plan.span.descriptor.metric_labels().unwrap().is_empty());
    assert!(plan.span.descriptor.attributes.iter().any(|attribute| {
        attribute.key == "tenant_id"
            && attribute.value == tenant_id.to_string()
            && attribute.scope == prodex_domain::TelemetryAttributeScope::TraceOnly
    }));
    assert!(plan.span.descriptor.attributes.iter().any(|attribute| {
        attribute.key == "audit_event_id"
            && attribute.scope == prodex_domain::TelemetryAttributeScope::TraceOnly
    }));

    let mut missing_audit_event = correlation;
    missing_audit_event.audit_event_id = None;
    let error = plan_application_control_plane_audit_emission_span(
        ApplicationControlPlaneAuditEmissionSpanRequest {
            correlation: missing_audit_event,
        },
    )
    .unwrap_err();
    let response = plan_application_control_plane_audit_emission_span_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationControlPlaneAuditEmissionSpanErrorStatus::ServiceUnavailable
    );
    assert_eq!(response.code, "telemetry_unavailable");
    assert_eq!(
        response.message,
        "telemetry planning is temporarily unavailable"
    );
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains("audit_event_id"));
}

#[test]
fn application_control_plane_audit_persistence_span_uses_low_cardinality_storage_label() {
    let tenant_id = TenantId::new();
    let mut http = control_plane_http_request("/admin/budgets/default");
    http.method = GatewayHttpMethod::Patch;
    let audit = plan_application_control_plane_with_audit_storage_from_http(
        control_plane_audit_request(
            DurableStoreKind::Postgres,
            tenant_id,
            control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
            ControlPlaneOperation::BudgetUpdate,
            ResourceKind::Budget,
        ),
        &http,
    )
    .unwrap();
    let correlation = plan_application_control_plane_audit_correlation_from_http(
        ApplicationControlPlaneAuditCorrelationRequest {
            request_id: RequestId::new(),
            call_id: Some(CallId::new()),
            http_policy: GatewayHttpPolicy::production_default(),
            http,
            audit: audit.clone(),
        },
    )
    .unwrap()
    .correlation;

    let plan = plan_application_control_plane_audit_persistence_span(
        ApplicationControlPlaneAuditPersistenceSpanRequest {
            correlation: correlation.clone(),
            audit_storage: audit.audit_storage,
        },
    )
    .unwrap();

    assert_eq!(
        plan.span.descriptor.kind,
        prodex_domain::GatewaySpanKind::Persistence
    );
    assert_eq!(
        plan.span.descriptor.name,
        "prodex.control_plane.audit.persist"
    );
    assert_eq!(
        plan.span.descriptor.metric_labels().unwrap(),
        vec![("storage_backend", "postgres")]
    );
    assert_eq!(plan.span.correlation, correlation);
    assert!(plan.span.descriptor.attributes.iter().any(|attribute| {
        attribute.key == "tenant_id"
            && attribute.value == tenant_id.to_string()
            && attribute.scope == prodex_domain::TelemetryAttributeScope::TraceOnly
    }));
    assert!(plan.span.descriptor.attributes.iter().any(|attribute| {
        attribute.key == "audit_event_id"
            && attribute.scope == prodex_domain::TelemetryAttributeScope::TraceOnly
    }));

    let missing_tenant = prodex_domain::CorrelationContext::new(RequestId::new());
    let error = plan_application_control_plane_audit_persistence_span(
        ApplicationControlPlaneAuditPersistenceSpanRequest {
            correlation: missing_tenant,
            audit_storage: plan_application_control_plane_with_audit_storage(
                control_plane_audit_request(
                    DurableStoreKind::Sqlite,
                    tenant_id,
                    control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
                    ControlPlaneOperation::BudgetUpdate,
                    ResourceKind::Budget,
                ),
            )
            .unwrap()
            .audit_storage,
        },
    )
    .unwrap_err();
    let response = plan_application_control_plane_audit_persistence_span_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationControlPlaneAuditPersistenceSpanErrorStatus::ServiceUnavailable
    );
    assert_eq!(response.code, "telemetry_unavailable");
    assert_eq!(
        response.message,
        "telemetry planning is temporarily unavailable"
    );
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains("sqlite"));
}

#[test]
fn application_break_glass_action_selects_append_only_audit_storage_plan() {
    let tenant_id = TenantId::new();
    let normal_control_plane = plan_application_control_plane(control_plane_action(
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::BreakGlass),
        ControlPlaneOperation::ProviderCredentialRotate,
        ResourceKind::ProviderCredential,
    ));
    assert!(matches!(
        normal_control_plane.decision,
        ControlPlaneDecision::Denied { .. }
    ));

    let plan = plan_application_break_glass_with_audit_storage(ApplicationBreakGlassAuditRequest {
        durable_store: DurableStoreKind::Postgres,
        action: control_plane_action(
            tenant_id,
            control_plane_principal(tenant_id, Role::Admin, CredentialScope::BreakGlass),
            ControlPlaneOperation::ProviderCredentialRotate,
            ResourceKind::ProviderCredential,
        ),
        authorization: BreakGlassAuthorization {
            reason: "incident response".to_string(),
            expires_at_unix_ms: 11_000,
        },
        previous_digest: Some(AuditDigest::new("sha256:previous").unwrap()),
        event_digest: AuditDigest::new("sha256:break-glass-current").unwrap(),
    })
    .unwrap();

    let ControlPlaneDecision::Authorized(action) = plan.decision else {
        panic!("expected authorized break-glass action");
    };
    assert_eq!(action.tenant.tenant_id, tenant_id);
    assert_eq!(action.audit_write.tenant_partition_key, tenant_id);
    let ApplicationControlPlaneAuditStoragePlan::Postgres(storage) = plan.audit_storage else {
        panic!("expected Postgres audit storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.statements[0].name, "set_tenant_context");
}

#[test]
fn application_break_glass_denial_is_audited_when_expired() {
    let tenant_id = TenantId::new();
    let plan = plan_application_break_glass_with_audit_storage(ApplicationBreakGlassAuditRequest {
        durable_store: DurableStoreKind::Sqlite,
        action: control_plane_action(
            tenant_id,
            control_plane_principal(tenant_id, Role::Admin, CredentialScope::BreakGlass),
            ControlPlaneOperation::ProviderCredentialRotate,
            ResourceKind::ProviderCredential,
        ),
        authorization: BreakGlassAuthorization {
            reason: "incident response".to_string(),
            expires_at_unix_ms: 10_000,
        },
        previous_digest: Some(AuditDigest::new("sha256:previous").unwrap()),
        event_digest: AuditDigest::new("sha256:break-glass-denied").unwrap(),
    })
    .unwrap();

    let ControlPlaneDecision::Denied {
        audit_event,
        audit_write,
        ..
    } = plan.decision
    else {
        panic!("expected denied break-glass action");
    };
    assert_eq!(audit_event.tenant_id, tenant_id);
    assert_eq!(audit_write.tenant_partition_key, tenant_id);
    let ApplicationControlPlaneAuditStoragePlan::Sqlite(storage) = plan.audit_storage else {
        panic!("expected SQLite audit storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.statements[0].name, "begin_immediate_audit_append");
}

#[test]
fn application_control_plane_audit_error_responses_are_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let wrong_tenant = TenantId::new();
    let error = ApplicationControlPlaneAuditError::Postgres(
        prodex_storage_postgres::PostgresStoragePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: wrong_tenant,
        },
    );

    let response = plan_application_control_plane_audit_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable
    );
    assert_eq!(response.code, "audit_storage_unavailable");
    assert_eq!(response.message, "audit storage is temporarily unavailable");
    assert!(!response.message.contains("PostgreSQL"));
    assert!(!response.message.contains("SQLite"));
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains(&wrong_tenant.to_string()));
}

#[test]
fn application_control_plane_idempotency_requires_key_for_retention_purge() {
    let tenant_id = TenantId::new();
    let action = control_plane_action(
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::AuditRetentionPurge,
        ResourceKind::AuditLog,
    );

    let error =
        plan_application_control_plane_idempotency(ApplicationControlPlaneIdempotencyRequest {
            action,
            idempotency_key: None,
            request_fingerprint: "sha256:retention-purge".to_string(),
        })
        .unwrap_err();

    assert_eq!(
        error,
        ApplicationControlPlaneIdempotencyError::IdempotencyKeyRequired
    );
    let response = plan_application_control_plane_idempotency_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationControlPlaneIdempotencyErrorStatus::BadRequest
    );
    assert_eq!(response.code, "control_plane_idempotency_key_required");
    assert_eq!(
        response.message,
        "control-plane idempotency key is required"
    );
    assert!(!response.message.contains(&tenant_id.to_string()));
}

#[test]
fn application_control_plane_idempotency_rejects_empty_request_fingerprint() {
    let tenant_id = TenantId::new();
    let action = control_plane_action(
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::AuditRetentionPurge,
        ResourceKind::AuditLog,
    );

    let error =
        plan_application_control_plane_idempotency(ApplicationControlPlaneIdempotencyRequest {
            action,
            idempotency_key: Some(IdempotencyKey::new("retention-purge-empty-fp").unwrap()),
            request_fingerprint: " ".to_string(),
        })
        .unwrap_err();
    let response = plan_application_control_plane_idempotency_error_response(&error);

    assert_eq!(
        response.status,
        ApplicationControlPlaneIdempotencyErrorStatus::BadRequest
    );
    assert_eq!(response.code, "request_fingerprint_invalid");
    assert_eq!(response.message, "request fingerprint is invalid");
    assert!(!response.message.contains(&tenant_id.to_string()));
}

#[test]
fn application_control_plane_idempotency_plans_tenant_scoped_operation() {
    let tenant_id = TenantId::new();
    let key = IdempotencyKey::new("retention-purge-application-1").unwrap();
    let principal = control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane);
    let principal_id = principal.id;
    let action = control_plane_action(
        tenant_id,
        principal,
        ControlPlaneOperation::AuditRetentionPurge,
        ResourceKind::AuditLog,
    );

    let plan =
        plan_application_control_plane_idempotency(ApplicationControlPlaneIdempotencyRequest {
            action,
            idempotency_key: Some(key.clone()),
            request_fingerprint: "sha256:retention-purge".to_string(),
        })
        .unwrap();
    let operation = plan.operation.unwrap();

    assert_eq!(
        plan.action.operation,
        ControlPlaneOperation::AuditRetentionPurge
    );
    assert_eq!(operation.tenant_id, tenant_id);
    assert_eq!(
        operation.key,
        application_control_plane_principal_scoped_idempotency_key(principal_id, &key)
    );
    assert!(!operation.key.as_str().contains(key.as_str()));
    assert_eq!(
        format!("{:?}", operation.key),
        "IdempotencyKey(\"<redacted>\")"
    );
    assert_eq!(operation.request_fingerprint, "sha256:retention-purge");
}

#[test]
fn application_control_plane_idempotency_key_is_principal_scoped_and_redacted() {
    let principal_id = "018f0000-0000-7000-8000-000000000001"
        .parse::<PrincipalId>()
        .unwrap();
    let other_principal_id = "018f0000-0000-7000-8000-000000000002"
        .parse::<PrincipalId>()
        .unwrap();
    let presented_key = IdempotencyKey::new("presented-key-example").unwrap();

    let key =
        application_control_plane_principal_scoped_idempotency_key(principal_id, &presented_key);

    assert_eq!(
        key.as_str(),
        "cp:v1:018f0000-0000-7000-8000-000000000001:c58196082c6ea6f04a2677fddf0321bf736e9e516935cf9df31a6663b4e927c9"
    );
    assert_ne!(
        key,
        application_control_plane_principal_scoped_idempotency_key(
            other_principal_id,
            &presented_key,
        )
    );
    assert!(!key.as_str().contains(presented_key.as_str()));
    assert_eq!(format!("{key:?}"), "IdempotencyKey(\"<redacted>\")");
}

#[test]
fn application_control_plane_idempotency_reads_key_from_http_header() {
    let tenant_id = TenantId::new();
    let principal = control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane);
    let principal_id = principal.id;
    let action = control_plane_action(
        tenant_id,
        principal,
        ControlPlaneOperation::AuditRetentionPurge,
        ResourceKind::AuditLog,
    );
    let mut http = control_plane_http_request("/admin/audit/retention/purge");
    http.method = GatewayHttpMethod::Delete;
    http.headers.push(GatewayHttpHeader::new(
        "Idempotency-Key",
        "retention-purge-http-1",
    ));

    let plan = plan_application_control_plane_idempotency_from_http(
        action,
        &http,
        "sha256:retention-purge-http",
    )
    .unwrap();
    let operation = plan.operation.unwrap();

    assert_eq!(operation.tenant_id, tenant_id);
    assert_eq!(
        operation.key,
        application_control_plane_principal_scoped_idempotency_key(
            principal_id,
            &IdempotencyKey::new("retention-purge-http-1").unwrap(),
        )
    );
    assert_eq!(operation.request_fingerprint, "sha256:retention-purge-http");
}

#[test]
fn application_virtual_key_rotate_accepts_legacy_patch_route_as_exact_action() {
    let tenant_id = TenantId::new();
    let action = control_plane_action(
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::VirtualKeyRotateSecret,
        ResourceKind::VirtualKey,
    );
    let http = GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Patch,
        path: "/admin/keys/resource-1".to_string(),
        body_len: 2,
        headers: vec![
            GatewayHttpHeader::new("Idempotency-Key", "rotate-key-1"),
            GatewayHttpHeader::new("If-Match", "\"revision-1\""),
        ],
    };

    let idempotency = plan_application_control_plane_idempotency_from_http_digest(
        action.clone(),
        &http,
        "sha256:rotate-body",
    )
    .unwrap();
    let audit = plan_application_control_plane_audit_from_http(action.clone(), &http).unwrap();
    let precondition =
        plan_application_control_plane_precondition_from_http(action, &http).unwrap();

    assert!(idempotency.operation.is_some());
    assert_eq!(
        audit.action.operation,
        ControlPlaneOperation::VirtualKeyRotateSecret
    );
    assert!(precondition.entity_tag.is_some());
}

#[test]
fn application_control_plane_http_route_maps_to_canonical_operation() {
    let key_route =
        plan_application_control_plane_http_route(&control_plane_http_request("/admin/keys"))
            .unwrap();
    assert_eq!(
        key_route,
        ApplicationControlPlaneHttpRoutePlan {
            http: prodex_gateway_http::GatewayControlPlaneRoutePlan {
                operation: GatewayControlPlaneOperation::VirtualKeyCreate,
                requires_idempotency: true,
                requires_audit: true,
            },
            operation: ControlPlaneOperation::VirtualKeyCreate,
        }
    );

    let scim_delete = plan_application_control_plane_http_route(&GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Delete,
        path: "/v1/scim/v2/Users/user-1".to_string(),
        body_len: 0,
        headers: vec![traceparent()],
    })
    .unwrap();
    assert_eq!(scim_delete.operation, ControlPlaneOperation::ScimUserDelete);
    assert_eq!(
        scim_delete.http.operation,
        GatewayControlPlaneOperation::ScimUserDelete
    );
    assert!(scim_delete.http.requires_idempotency);
    assert!(scim_delete.http.requires_audit);

    let scim_replace = plan_application_control_plane_http_route(&GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Put,
        path: "/v1/scim/v2/Users/user-1".to_string(),
        body_len: 128,
        headers: vec![traceparent()],
    })
    .unwrap();
    assert_eq!(
        scim_replace.operation,
        ControlPlaneOperation::ScimUserUpdate
    );
    assert_eq!(
        scim_replace.http.operation,
        GatewayControlPlaneOperation::ScimUserUpdate
    );
    assert!(scim_replace.http.requires_idempotency);
    assert!(scim_replace.http.requires_audit);

    let scim_post = plan_application_control_plane_http_route(&GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Post,
        path: "/v1/scim/v2/Users/user-1".to_string(),
        body_len: 0,
        headers: vec![traceparent()],
    })
    .unwrap_err();
    assert_eq!(
        scim_post,
        ApplicationControlPlaneHttpRouteError::Route(
            prodex_gateway_http::GatewayControlPlaneRouteError::MethodNotAllowed {
                operation: GatewayControlPlaneOperation::ScimUserUpdate,
                method: GatewayHttpMethod::Post,
            }
        )
    );
}

#[test]
fn application_control_plane_http_route_error_response_preserves_shared_method_rejection() {
    let mut http = control_plane_http_request("/admin/keys");
    http.method = GatewayHttpMethod::Delete;

    let error = plan_application_control_plane_http_route(&http).unwrap_err();
    let response = plan_application_control_plane_http_route_error_response(&error);

    assert_eq!(
        response.status,
        ApplicationControlPlaneHttpRouteErrorStatus::MethodNotAllowed
    );
    assert_eq!(response.code, "control_plane_method_not_allowed");
    assert_eq!(
        response.message,
        "HTTP method is not allowed for this control-plane route"
    );
}

#[test]
fn application_control_plane_page_request_from_http_query_validates_route_and_redacts_errors() {
    let tenant_id = TenantId::new();
    let action = control_plane_action(
        tenant_id,
        control_plane_principal(tenant_id, Role::Viewer, CredentialScope::ControlPlane),
        ControlPlaneOperation::BillingRead,
        ResourceKind::Billing,
    );
    let http = GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Get,
        path: "/admin/billing".to_string(),
        body_len: 0,
        headers: vec![traceparent()],
    };

    let plan = plan_application_control_plane_page_request_from_http_query(
        action.clone(),
        &http,
        "?limit=25&cursor=billing:v1:next",
    )
    .unwrap();
    assert_eq!(plan.action.operation, ControlPlaneOperation::BillingRead);
    assert_eq!(plan.page_request.limit, 25);
    assert_eq!(
        plan.page_request.cursor.as_ref().unwrap().as_str(),
        "billing:v1:next"
    );

    let invalid_cursor = plan_application_control_plane_page_request_from_http_query(
        action.clone(),
        &http,
        "cursor=",
    )
    .unwrap_err();
    let invalid_cursor_response =
        plan_application_control_plane_page_request_error_response(&invalid_cursor);
    assert_eq!(
        invalid_cursor_response.status,
        ApplicationControlPlanePageRequestErrorStatus::BadRequest
    );
    assert_eq!(invalid_cursor_response.code, "pagination_cursor_invalid");
    assert_eq!(
        invalid_cursor_response.message,
        "pagination cursor is invalid"
    );
    assert!(
        !invalid_cursor_response
            .message
            .contains(&tenant_id.to_string())
    );

    let wrong_route = plan_application_control_plane_page_request_from_http_query(
        action,
        &control_plane_http_request("/admin/keys"),
        "limit=10",
    )
    .unwrap_err();
    assert!(matches!(
        wrong_route,
        ApplicationControlPlanePageRequestError::OperationMismatch { .. }
    ));
    let wrong_route_response =
        plan_application_control_plane_page_request_error_response(&wrong_route);
    assert_eq!(wrong_route_response.code, "control_plane_route_invalid");
    assert_eq!(
        wrong_route_response.message,
        "control-plane route is invalid"
    );
    assert!(!wrong_route_response.message.contains("BillingRead"));
    assert!(!wrong_route_response.message.contains("VirtualKeyCreate"));
}

#[test]
fn application_control_plane_precondition_from_http_validates_route_and_redacts_errors() {
    let tenant_id = TenantId::new();
    let action = control_plane_action(
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::BudgetUpdate,
        ResourceKind::Budget,
    );
    let mut http = control_plane_http_request("/admin/budgets/default");
    http.method = GatewayHttpMethod::Patch;
    http.headers
        .push(GatewayHttpHeader::new("If-Match", "W/\"42\""));

    let plan =
        plan_application_control_plane_precondition_from_http(action.clone(), &http).unwrap();
    assert_eq!(plan.action.operation, ControlPlaneOperation::BudgetUpdate);
    assert_eq!(plan.entity_tag.as_ref().unwrap().as_str(), "W/\"42\"");

    let mut invalid = http.clone();
    invalid
        .headers
        .retain(|header| header.normalized_name() != "if-match");
    invalid
        .headers
        .push(GatewayHttpHeader::new("If-Match", "x".repeat(300)));
    let invalid_error =
        plan_application_control_plane_precondition_from_http(action.clone(), &invalid)
            .unwrap_err();
    let invalid_response =
        plan_application_control_plane_precondition_error_response(&invalid_error);
    assert_eq!(
        invalid_response.status,
        ApplicationControlPlanePreconditionErrorStatus::BadRequest
    );
    assert_eq!(invalid_response.code, "entity_tag_invalid");
    assert_eq!(invalid_response.message, "entity tag is invalid");
    assert!(!invalid_response.message.contains("300"));
    assert!(!invalid_response.message.contains(&tenant_id.to_string()));

    let wrong_route = plan_application_control_plane_precondition_from_http(
        action,
        &control_plane_http_request("/admin/keys"),
    )
    .unwrap_err();
    assert!(matches!(
        wrong_route,
        ApplicationControlPlanePreconditionError::OperationMismatch { .. }
    ));
    let wrong_route_response =
        plan_application_control_plane_precondition_error_response(&wrong_route);
    assert_eq!(wrong_route_response.code, "control_plane_route_invalid");
    assert_eq!(
        wrong_route_response.message,
        "control-plane route is invalid"
    );
    assert!(!wrong_route_response.message.contains("BudgetUpdate"));
    assert!(!wrong_route_response.message.contains("VirtualKeyCreate"));
}

#[test]
fn application_control_plane_idempotency_http_header_errors_are_redacted() {
    let tenant_id = TenantId::new();
    let action = control_plane_action(
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::AuditRetentionPurge,
        ResourceKind::AuditLog,
    );
    let mut missing = control_plane_http_request("/admin/audit/retention/purge");
    missing.method = GatewayHttpMethod::Delete;
    let missing_error = plan_application_control_plane_idempotency_from_http(
        action.clone(),
        &missing,
        "sha256:retention-purge-http",
    )
    .unwrap_err();
    assert_eq!(
        missing_error,
        ApplicationControlPlaneIdempotencyError::IdempotencyKeyRequired
    );

    let mut invalid = control_plane_http_request("/admin/audit/retention/purge");
    invalid.method = GatewayHttpMethod::Delete;
    invalid
        .headers
        .push(GatewayHttpHeader::new("Idempotency-Key", "bad key"));
    let invalid_error = plan_application_control_plane_idempotency_from_http(
        action,
        &invalid,
        "sha256:retention-purge-http",
    )
    .unwrap_err();
    let response = plan_application_control_plane_idempotency_error_response(&invalid_error);

    assert_eq!(
        response.status,
        ApplicationControlPlaneIdempotencyErrorStatus::BadRequest
    );
    assert_eq!(response.code, "idempotency_key_invalid");
    assert_eq!(response.message, "idempotency key is invalid");
    assert!(!response.message.contains("bad key"));
    assert!(!response.message.contains(&tenant_id.to_string()));
}

#[test]
fn application_control_plane_idempotency_from_http_digest_builds_canonical_fingerprint() {
    let tenant_id = TenantId::new();
    let principal = control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane);
    let principal_id = principal.id;
    let action = control_plane_action(
        tenant_id,
        principal,
        ControlPlaneOperation::PolicyPublish,
        ResourceKind::Policy,
    );
    let mut http = control_plane_http_request("/admin/policies/revision-1");
    http.headers.push(GatewayHttpHeader::new(
        "Idempotency-Key",
        "policy-publish-http-1",
    ));

    let plan = plan_application_control_plane_idempotency_from_http_digest(
        action,
        &http,
        "sha256:policy-body",
    )
    .unwrap();
    let operation = plan.operation.unwrap();

    assert_eq!(
        operation.key,
        application_control_plane_principal_scoped_idempotency_key(
            principal_id,
            &IdempotencyKey::new("policy-publish-http-1").unwrap(),
        )
    );
    assert_eq!(
        operation.request_fingerprint,
        "http:post:path:/admin/policies/revision-1:body:sha256:policy-body"
    );
}

#[test]
fn application_control_plane_idempotency_from_http_digest_rejects_route_action_mismatch() {
    let tenant_id = TenantId::new();
    let action = control_plane_action(
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::PolicyPublish,
        ResourceKind::Policy,
    );
    let mut http = control_plane_http_request("/admin/audit/exports");
    http.headers.push(GatewayHttpHeader::new(
        "Idempotency-Key",
        "policy-publish-http-1",
    ));

    let error = plan_application_control_plane_idempotency_from_http_digest(
        action,
        &http,
        "sha256:policy-body",
    )
    .unwrap_err();

    assert_eq!(
        error,
        ApplicationControlPlaneIdempotencyError::OperationMismatch {
            route_operation: ControlPlaneOperation::AuditExport,
            action_operation: ControlPlaneOperation::PolicyPublish,
        }
    );
    let response = plan_application_control_plane_idempotency_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationControlPlaneIdempotencyErrorStatus::BadRequest
    );
    assert_eq!(response.code, "control_plane_route_invalid");
    assert_eq!(response.message, "control-plane route is invalid");
    assert!(!response.message.contains("PolicyPublish"));
    assert!(!response.message.contains("AuditExport"));
    assert!(!response.message.contains(&tenant_id.to_string()));
}

#[test]
fn application_control_plane_idempotency_from_http_rejects_route_action_mismatch() {
    let tenant_id = TenantId::new();
    let action = control_plane_action(
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::VirtualKeyCreate,
        ResourceKind::VirtualKey,
    );
    let mut http = control_plane_http_request("/admin/billing");
    http.method = GatewayHttpMethod::Get;
    http.headers.push(GatewayHttpHeader::new(
        "Idempotency-Key",
        "virtual-key-create-http-1",
    ));

    let error = plan_application_control_plane_idempotency_from_http(
        action,
        &http,
        "manual-fingerprint-that-must-not-bypass-route-binding",
    )
    .unwrap_err();

    assert_eq!(
        error,
        ApplicationControlPlaneIdempotencyError::OperationMismatch {
            route_operation: ControlPlaneOperation::BillingRead,
            action_operation: ControlPlaneOperation::VirtualKeyCreate,
        }
    );
    let response = plan_application_control_plane_idempotency_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationControlPlaneIdempotencyErrorStatus::BadRequest
    );
    assert_eq!(response.code, "control_plane_route_invalid");
    assert_eq!(response.message, "control-plane route is invalid");
    assert!(!response.message.contains("manual-fingerprint"));
    assert!(!response.message.contains("VirtualKeyCreate"));
    assert!(!response.message.contains("BillingRead"));
}

#[test]
fn application_control_plane_idempotency_from_http_preserves_method_not_allowed_response() {
    let tenant_id = TenantId::new();
    let action = control_plane_action(
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::VirtualKeyCreate,
        ResourceKind::VirtualKey,
    );
    let mut http = control_plane_http_request("/admin/keys");
    http.method = GatewayHttpMethod::Get;
    http.headers.push(GatewayHttpHeader::new(
        "Idempotency-Key",
        "virtual-key-create-http-2",
    ));

    let error = plan_application_control_plane_idempotency_from_http(
        action,
        &http,
        "manual-fingerprint-method-denied",
    )
    .unwrap_err();
    let response = plan_application_control_plane_idempotency_error_response(&error);

    assert_eq!(
        response.status,
        ApplicationControlPlaneIdempotencyErrorStatus::MethodNotAllowed
    );
    assert_eq!(response.code, "control_plane_method_not_allowed");
    assert_eq!(
        response.message,
        "HTTP method is not allowed for this control-plane route"
    );
    assert!(!response.message.contains("VirtualKeyCreate"));
    assert!(!response.message.contains("Get"));
    assert!(!response.message.contains("manual-fingerprint"));
}

#[test]
fn application_control_plane_idempotency_from_http_digest_errors_are_redacted() {
    let tenant_id = TenantId::new();
    let action = control_plane_action(
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::PolicyPublish,
        ResourceKind::Policy,
    );
    let mut http = control_plane_http_request("/admin/policies/revision-1");
    http.headers.push(GatewayHttpHeader::new(
        "Idempotency-Key",
        "policy-publish-http-1",
    ));

    let error = plan_application_control_plane_idempotency_from_http_digest(
        action,
        &http,
        "sha256:bad body",
    )
    .unwrap_err();
    let response = plan_application_control_plane_idempotency_error_response(&error);

    assert_eq!(
        response.status,
        ApplicationControlPlaneIdempotencyErrorStatus::BadRequest
    );
    assert_eq!(response.code, "request_fingerprint_invalid");
    assert_eq!(response.message, "request fingerprint is invalid");
    assert!(!response.message.contains("sha256"));
    assert!(!response.message.contains(&tenant_id.to_string()));
}

#[test]
fn application_control_plane_idempotency_replay_executes_when_no_record_exists() {
    let tenant_id = TenantId::new();
    let operation = prodex_domain::IdempotentOperation::new(
        tenant_id,
        IdempotencyKey::new("retention-purge-replay-1").unwrap(),
        "sha256:retention-purge",
    )
    .unwrap();

    assert_eq!(
        plan_application_control_plane_idempotency_replay::<String>(&operation, None).unwrap(),
        IdempotencyReplayDecision::ExecuteAndRecordPending
    );
}

#[test]
fn application_control_plane_idempotency_replay_reports_pending_duplicate() {
    let tenant_id = TenantId::new();
    let operation = prodex_domain::IdempotentOperation::new(
        tenant_id,
        IdempotencyKey::new("retention-purge-replay-1").unwrap(),
        "sha256:retention-purge",
    )
    .unwrap();
    let existing = IdempotencyEntry::<String>::pending(operation.clone(), 1_800_000_000_000);

    assert_eq!(
        plan_application_control_plane_idempotency_replay(&operation, Some(&existing)).unwrap(),
        IdempotencyReplayDecision::AlreadyInProgress {
            started_at_unix_ms: 1_800_000_000_000
        }
    );
}

#[test]
fn application_control_plane_idempotency_replay_returns_completed_response() {
    let tenant_id = TenantId::new();
    let operation = prodex_domain::IdempotentOperation::new(
        tenant_id,
        IdempotencyKey::new("retention-purge-replay-1").unwrap(),
        "sha256:retention-purge",
    )
    .unwrap();
    let existing = IdempotencyEntry::completed(IdempotencyRecord {
        operation: operation.clone(),
        response: "purged".to_string(),
    });

    assert_eq!(
        plan_application_control_plane_idempotency_replay(&operation, Some(&existing)).unwrap(),
        IdempotencyReplayDecision::Replay("purged".to_string())
    );
}

#[test]
fn application_control_plane_idempotency_replay_conflict_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let key = IdempotencyKey::new("retention-purge-replay-1").unwrap();
    let operation =
        prodex_domain::IdempotentOperation::new(tenant_id, key.clone(), "sha256:retention-purge-a")
            .unwrap();
    let existing = IdempotencyEntry::<String>::pending(
        prodex_domain::IdempotentOperation::new(tenant_id, key, "sha256:retention-purge-b")
            .unwrap(),
        1_800_000_000_000,
    );

    let error =
        plan_application_control_plane_idempotency_replay(&operation, Some(&existing)).unwrap_err();
    assert_eq!(
        error,
        ApplicationControlPlaneIdempotencyError::ReplayConflict(
            IdempotencyConflict::RequestFingerprintMismatch
        )
    );
    let response = plan_application_control_plane_idempotency_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationControlPlaneIdempotencyErrorStatus::Conflict
    );
    assert_eq!(response.code, "idempotency_key_reused");
    assert_eq!(
        response.message,
        "idempotency key was reused for a different request"
    );
    assert!(!response.message.contains("sha256"));
    assert!(!response.message.contains(&tenant_id.to_string()));
}

fn idempotency_storage_operation(tenant_id: TenantId) -> prodex_domain::IdempotentOperation {
    prodex_domain::IdempotentOperation::new(
        tenant_id,
        IdempotencyKey::new("retention-purge-storage-1").unwrap(),
        "sha256:retention-purge-storage",
    )
    .unwrap()
}

fn idempotency_lookup_row(
    tenant_id: TenantId,
    status: IdempotencyRecordLookupRowStatus,
) -> IdempotencyRecordLookupRow {
    IdempotencyRecordLookupRow {
        tenant_id,
        idempotency_key: IdempotencyKey::new("retention-purge-storage-1").unwrap(),
        request_fingerprint: "sha256:retention-purge-storage".to_string(),
        status,
        started_at_unix_ms: 1_800_000_000_000,
        completed_at_unix_ms: Some(1_800_000_001_000),
        response_body: Some(br#"{"status":"purged"}"#.to_vec()),
    }
}

#[test]
fn application_control_plane_idempotency_replay_from_lookup_row_executes_when_missing() {
    let tenant_id = TenantId::new();
    let operation = idempotency_storage_operation(tenant_id);

    assert_eq!(
        plan_application_control_plane_idempotency_replay_from_lookup_row(&operation, None)
            .unwrap(),
        IdempotencyReplayDecision::ExecuteAndRecordPending
    );
}

#[test]
fn application_control_plane_idempotency_replay_from_lookup_row_reports_pending_duplicate() {
    let tenant_id = TenantId::new();
    let operation = idempotency_storage_operation(tenant_id);

    assert_eq!(
        plan_application_control_plane_idempotency_replay_from_lookup_row(
            &operation,
            Some(idempotency_lookup_row(
                tenant_id,
                IdempotencyRecordLookupRowStatus::Pending,
            )),
        )
        .unwrap(),
        IdempotencyReplayDecision::AlreadyInProgress {
            started_at_unix_ms: 1_800_000_000_000,
        }
    );
}

#[test]
fn application_control_plane_idempotency_replay_from_lookup_row_replays_completed_bytes() {
    let tenant_id = TenantId::new();
    let operation = idempotency_storage_operation(tenant_id);

    assert_eq!(
        plan_application_control_plane_idempotency_replay_from_lookup_row(
            &operation,
            Some(idempotency_lookup_row(
                tenant_id,
                IdempotencyRecordLookupRowStatus::Completed,
            )),
        )
        .unwrap(),
        IdempotencyReplayDecision::Replay(br#"{"status":"purged"}"#.to_vec())
    );
}

#[test]
fn application_control_plane_idempotency_replay_from_lookup_row_preserves_conflict_semantics() {
    let tenant_id = TenantId::new();
    let operation = idempotency_storage_operation(tenant_id);
    let mut row = idempotency_lookup_row(tenant_id, IdempotencyRecordLookupRowStatus::Pending);
    row.request_fingerprint = "sha256:other-retention-purge".to_string();

    let error =
        plan_application_control_plane_idempotency_replay_from_lookup_row(&operation, Some(row))
            .unwrap_err();
    assert_eq!(
        error,
        ApplicationControlPlaneIdempotencyError::ReplayConflict(
            IdempotencyConflict::RequestFingerprintMismatch,
        )
    );
}

#[test]
fn application_control_plane_idempotency_replay_from_lookup_row_rejects_invalid_row_redacted() {
    let tenant_id = TenantId::new();
    let operation = idempotency_storage_operation(tenant_id);
    let other_tenant = TenantId::new();
    let row = idempotency_lookup_row(other_tenant, IdempotencyRecordLookupRowStatus::Pending);

    let error =
        plan_application_control_plane_idempotency_replay_from_lookup_row(&operation, Some(row))
            .unwrap_err();
    let response = plan_application_control_plane_idempotency_error_response(&error);

    assert_eq!(
        response.status,
        ApplicationControlPlaneIdempotencyErrorStatus::ServiceUnavailable
    );
    assert_eq!(response.code, "control_plane_idempotency_lookup_invalid");
    assert_eq!(
        response.message,
        "control-plane idempotency lookup is invalid"
    );
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains(&other_tenant.to_string()));
}

#[test]
fn application_control_plane_idempotency_storage_prepare_selects_postgres_lookup_and_pending() {
    let tenant_id = TenantId::new();
    let plan = plan_application_control_plane_idempotency_storage_prepare(
        ApplicationControlPlaneIdempotencyStoragePrepareRequest {
            durable_store: DurableStoreKind::Postgres,
            storage_key: TenantStorageKey::tenant(tenant_id),
            operation: idempotency_storage_operation(tenant_id),
            started_at_unix_ms: 1_800_000_000_000,
        },
    )
    .unwrap();

    let ApplicationControlPlaneIdempotencyLookupStoragePlan::Postgres(lookup) = plan.lookup else {
        panic!("expected Postgres lookup plan");
    };
    assert_eq!(lookup.tenant_id, tenant_id);
    assert_eq!(lookup.statements[0].name, "set_tenant_context");

    let ApplicationControlPlaneIdempotencyPendingStoragePlan::Postgres(pending) = plan.pending
    else {
        panic!("expected Postgres pending plan");
    };
    assert_eq!(pending.tenant_id, tenant_id);
    assert_eq!(pending.statements[0].name, "set_tenant_context");
}

#[test]
fn application_control_plane_idempotency_storage_prepare_selects_sqlite_lookup_and_pending() {
    let tenant_id = TenantId::new();
    let plan = plan_application_control_plane_idempotency_storage_prepare(
        ApplicationControlPlaneIdempotencyStoragePrepareRequest {
            durable_store: DurableStoreKind::Sqlite,
            storage_key: TenantStorageKey::tenant(tenant_id),
            operation: idempotency_storage_operation(tenant_id),
            started_at_unix_ms: 1_800_000_000_000,
        },
    )
    .unwrap();

    let ApplicationControlPlaneIdempotencyLookupStoragePlan::Sqlite(lookup) = plan.lookup else {
        panic!("expected SQLite lookup plan");
    };
    assert_eq!(lookup.tenant_id, tenant_id);
    assert_eq!(
        lookup.statements[0].name,
        "lookup_idempotency_record_locally"
    );

    let ApplicationControlPlaneIdempotencyPendingStoragePlan::Sqlite(pending) = plan.pending else {
        panic!("expected SQLite pending plan");
    };
    assert_eq!(pending.tenant_id, tenant_id);
    assert_eq!(
        pending.statements[0].name,
        "begin_immediate_idempotency_pending"
    );
}

#[test]
fn application_control_plane_idempotency_storage_complete_selects_durable_store() {
    let tenant_id = TenantId::new();
    let postgres = plan_application_control_plane_idempotency_storage_complete(
        ApplicationControlPlaneIdempotencyStorageCompleteRequest {
            durable_store: DurableStoreKind::Postgres,
            storage_key: TenantStorageKey::tenant(tenant_id),
            operation: idempotency_storage_operation(tenant_id),
            completed_at_unix_ms: 1_800_000_001_000,
            response_body: br#"{"status":"purged"}"#.to_vec(),
        },
    )
    .unwrap();
    let ApplicationControlPlaneIdempotencyCompletedStoragePlan::Postgres(completed) =
        postgres.completed
    else {
        panic!("expected Postgres completed plan");
    };
    assert_eq!(completed.tenant_id, tenant_id);
    assert_eq!(
        completed.response_byte_count,
        br#"{"status":"purged"}"#.len()
    );

    let sqlite = plan_application_control_plane_idempotency_storage_complete(
        ApplicationControlPlaneIdempotencyStorageCompleteRequest {
            durable_store: DurableStoreKind::Sqlite,
            storage_key: TenantStorageKey::tenant(tenant_id),
            operation: idempotency_storage_operation(tenant_id),
            completed_at_unix_ms: 1_800_000_001_000,
            response_body: br#"{"status":"purged"}"#.to_vec(),
        },
    )
    .unwrap();
    let ApplicationControlPlaneIdempotencyCompletedStoragePlan::Sqlite(completed) =
        sqlite.completed
    else {
        panic!("expected SQLite completed plan");
    };
    assert_eq!(completed.tenant_id, tenant_id);
    assert_eq!(
        completed.statements[0].name,
        "begin_immediate_idempotency_completion"
    );
}

#[test]
fn application_control_plane_idempotency_storage_rejects_cross_tenant_storage_key() {
    let tenant_id = TenantId::new();
    let wrong_tenant = TenantId::new();

    let error = plan_application_control_plane_idempotency_storage_prepare(
        ApplicationControlPlaneIdempotencyStoragePrepareRequest {
            durable_store: DurableStoreKind::Postgres,
            storage_key: TenantStorageKey::tenant(wrong_tenant),
            operation: idempotency_storage_operation(tenant_id),
            started_at_unix_ms: 1_800_000_000_000,
        },
    )
    .unwrap_err();

    assert_eq!(
        error,
        ApplicationControlPlaneIdempotencyStorageError::Postgres(
            prodex_storage_postgres::PostgresStoragePlanError::TenantMismatch {
                key_tenant: wrong_tenant,
                request_tenant: tenant_id,
            },
        )
    );

    let response = plan_application_control_plane_idempotency_storage_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationControlPlaneIdempotencyStorageErrorStatus::ServiceUnavailable
    );
    assert_eq!(
        response.code,
        "control_plane_idempotency_storage_unavailable"
    );
    assert_eq!(
        response.message,
        "control-plane idempotency storage is temporarily unavailable"
    );
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains(&wrong_tenant.to_string()));
    assert!(!response.message.contains("PostgreSQL"));
}

#[test]
fn application_control_plane_idempotency_skips_read_only_operations() {
    let tenant_id = TenantId::new();
    let action = control_plane_action(
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::AuditExport,
        ResourceKind::AuditLog,
    );

    let plan =
        plan_application_control_plane_idempotency(ApplicationControlPlaneIdempotencyRequest {
            action,
            idempotency_key: None,
            request_fingerprint: "sha256:audit-export".to_string(),
        })
        .unwrap();

    assert_eq!(plan.action.operation, ControlPlaneOperation::AuditExport);
    assert_eq!(plan.operation, None);
}

#[test]
fn application_audit_retention_purge_operation_selects_audit_storage() {
    let tenant_id = TenantId::new();
    let plan = plan_application_control_plane_with_audit_storage(control_plane_audit_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::AuditRetentionPurge,
        ResourceKind::AuditLog,
    ))
    .unwrap();

    let ControlPlaneDecision::Authorized(action) = plan.decision else {
        panic!("expected authorized retention purge operation");
    };
    assert_eq!(action.tenant.tenant_id, tenant_id);
    assert_eq!(
        action.audit_event.action.as_str(),
        "control_plane.audit.retention_purge"
    );
    assert_eq!(action.audit_write.tenant_partition_key, tenant_id);
    let ApplicationControlPlaneAuditStoragePlan::Postgres(storage) = plan.audit_storage else {
        panic!("expected Postgres audit storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.statements[0].name, "set_tenant_context");
}

#[test]
fn application_denied_audit_retention_purge_still_selects_audit_storage() {
    let tenant_id = TenantId::new();
    let plan = plan_application_control_plane_with_audit_storage(control_plane_audit_request(
        DurableStoreKind::Sqlite,
        tenant_id,
        control_plane_principal(tenant_id, Role::Viewer, CredentialScope::ControlPlane),
        ControlPlaneOperation::AuditRetentionPurge,
        ResourceKind::AuditLog,
    ))
    .unwrap();

    let ControlPlaneDecision::Denied {
        audit_event,
        audit_write,
        ..
    } = plan.decision
    else {
        panic!("expected denied retention purge operation");
    };
    assert_eq!(audit_event.tenant_id, tenant_id);
    assert_eq!(audit_event.outcome, prodex_domain::AuditOutcome::Denied);
    assert_eq!(
        audit_event.action.as_str(),
        "control_plane.audit.retention_purge"
    );
    assert_eq!(audit_write.tenant_partition_key, tenant_id);
    let ApplicationControlPlaneAuditStoragePlan::Sqlite(storage) = plan.audit_storage else {
        panic!("expected SQLite audit storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.statements[0].name, "begin_immediate_audit_append");
}

#[test]
fn application_audit_retention_purge_selects_postgres_storage_plan() {
    let tenant_id = TenantId::new();
    let plan = plan_application_audit_retention_purge(audit_retention_purge_request(
        DurableStoreKind::Postgres,
        tenant_id,
    ))
    .unwrap();

    let ApplicationAuditRetentionPurgeStoragePlan::Postgres(storage) = plan.storage else {
        panic!("expected Postgres retention purge storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.storage_key.tenant_id, tenant_id);
    assert_eq!(storage.event_count, 2);
    assert_eq!(storage.statements[0].name, "set_tenant_context");
}

#[test]
fn application_audit_retention_purge_selects_sqlite_storage_plan() {
    let tenant_id = TenantId::new();
    let plan = plan_application_audit_retention_purge(audit_retention_purge_request(
        DurableStoreKind::Sqlite,
        tenant_id,
    ))
    .unwrap();

    let ApplicationAuditRetentionPurgeStoragePlan::Sqlite(storage) = plan.storage else {
        panic!("expected SQLite retention purge storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.storage_key.tenant_id, tenant_id);
    assert_eq!(storage.event_count, 2);
    assert_eq!(
        storage.statements[0].name,
        "begin_immediate_audit_retention_purge"
    );
}

#[test]
fn application_audit_retention_purge_error_responses_are_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let wrong_tenant = TenantId::new();
    let error = ApplicationAuditRetentionPurgeError::Postgres(
        prodex_storage_postgres::PostgresStoragePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: wrong_tenant,
        },
    );

    let response = plan_application_audit_retention_purge_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationAuditRetentionPurgeErrorStatus::ServiceUnavailable
    );
    assert_eq!(response.code, "audit_retention_purge_storage_unavailable");
    assert_eq!(
        response.message,
        "audit retention purge storage is temporarily unavailable"
    );
    assert!(!response.message.contains("PostgreSQL"));
    assert!(!response.message.contains("SQLite"));
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains(&wrong_tenant.to_string()));
}

#[test]
fn application_audit_export_authorized_selects_postgres_audit_and_query_storage() {
    let tenant_id = TenantId::new();
    let request = audit_export_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
    );
    let request_debug = format!("{request:?}");
    let plan = plan_application_audit_export(request).unwrap();
    let plan_debug = format!("{plan:?}");

    let ControlPlaneDecision::Authorized(action) = plan.decision else {
        panic!("expected authorized audit export");
    };
    assert_eq!(action.tenant.tenant_id, tenant_id);
    assert_eq!(
        action.audit_event.action.as_str(),
        "control_plane.audit.export"
    );
    let ApplicationControlPlaneAuditStoragePlan::Postgres(audit_storage) = plan.audit_storage
    else {
        panic!("expected Postgres audit storage plan");
    };
    assert_eq!(audit_storage.statements[0].name, "set_tenant_context");
    let Some(ApplicationAuditExportQueryStoragePlan::Postgres(query_storage)) = plan.query_storage
    else {
        panic!("expected Postgres audit export query storage plan");
    };
    assert_eq!(query_storage.tenant_id, tenant_id);
    assert_eq!(query_storage.sort_order, AuditSortOrder::OccurredAtDesc);
    assert_eq!(query_storage.page_limit, 250);
    assert_eq!(query_storage.statements[1].name, "query_audit_export_desc");
    assert!(!request_debug.contains(&tenant_id.to_string()));
    assert!(!request_debug.contains("control_plane.audit.export"));
    assert!(!plan_debug.contains(&tenant_id.to_string()));
    assert!(!plan_debug.contains("query_audit_export_desc"));
}

#[test]
fn application_denied_audit_export_still_audits_without_query_storage() {
    let tenant_id = TenantId::new();
    let plan = plan_application_audit_export(audit_export_request(
        DurableStoreKind::Sqlite,
        tenant_id,
        control_plane_principal(tenant_id, Role::Viewer, CredentialScope::ControlPlane),
    ))
    .unwrap();

    let ControlPlaneDecision::Denied {
        audit_event,
        audit_write,
        ..
    } = plan.decision
    else {
        panic!("expected denied audit export");
    };
    assert_eq!(audit_event.tenant_id, tenant_id);
    assert_eq!(audit_event.outcome, prodex_domain::AuditOutcome::Denied);
    assert_eq!(audit_event.action.as_str(), "control_plane.audit.export");
    assert_eq!(audit_write.tenant_partition_key, tenant_id);
    let ApplicationControlPlaneAuditStoragePlan::Sqlite(audit_storage) = plan.audit_storage else {
        panic!("expected SQLite audit storage plan");
    };
    assert_eq!(
        audit_storage.statements[0].name,
        "begin_immediate_audit_append"
    );
    assert_eq!(plan.query_storage, None);
}

#[test]
fn application_audit_export_rejects_cross_tenant_query_before_storage() {
    let tenant_id = TenantId::new();
    let wrong_tenant = TenantId::new();
    let mut request = audit_export_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
    );
    request.export = audit_export_request(
        DurableStoreKind::Postgres,
        wrong_tenant,
        control_plane_principal(wrong_tenant, Role::Admin, CredentialScope::ControlPlane),
    )
    .export;

    let error = plan_application_audit_export(request).unwrap_err();

    assert_eq!(
        error,
        ApplicationAuditExportError::TenantMismatch {
            action_tenant: tenant_id,
            query_tenant: wrong_tenant,
        }
    );
    assert_eq!(error.to_string(), "audit export request is invalid");
    assert!(!error.to_string().contains(&tenant_id.to_string()));
    assert!(!error.to_string().contains(&wrong_tenant.to_string()));
    assert!(
        !ApplicationAuditExportError::WrongOperation(ControlPlaneOperation::BillingRead)
            .to_string()
            .contains("BillingRead")
    );
    assert!(
        !ApplicationAuditExportError::WrongResourceKind(ResourceKind::Billing)
            .to_string()
            .contains("Billing")
    );
    let error_debug = format!(
        "{error:?} {:?} {:?}",
        ApplicationAuditExportError::WrongOperation(ControlPlaneOperation::BillingRead),
        ApplicationAuditExportError::WrongResourceKind(ResourceKind::Billing)
    );
    assert!(!error_debug.contains(&tenant_id.to_string()));
    assert!(!error_debug.contains(&wrong_tenant.to_string()));
    assert!(!error_debug.contains("BillingRead"));
    assert!(!error_debug.contains("Billing"));
    let response = plan_application_audit_export_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationAuditExportErrorStatus::BadRequest
    );
    assert_eq!(response.code, "audit_export_invalid");
    assert_eq!(response.message, "audit export request is invalid");
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains(&wrong_tenant.to_string()));
}

#[test]
fn application_billing_read_authorized_selects_postgres_audit_and_query_storage() {
    let tenant_id = TenantId::new();
    let request = billing_read_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Viewer, CredentialScope::ControlPlane),
    );
    let request_debug = format!("{request:?}");
    let plan = plan_application_billing_read(request).unwrap();
    let plan_debug = format!("{plan:?}");

    let ControlPlaneDecision::Authorized(action) = plan.decision else {
        panic!("expected authorized billing read");
    };
    assert_eq!(action.tenant.tenant_id, tenant_id);
    assert_eq!(
        action.audit_event.action.as_str(),
        "control_plane.billing.read"
    );
    let ApplicationControlPlaneAuditStoragePlan::Postgres(audit_storage) = plan.audit_storage
    else {
        panic!("expected Postgres audit storage plan");
    };
    assert_eq!(audit_storage.statements[0].name, "set_tenant_context");
    let Some(ApplicationBillingLedgerStoragePlan::Postgres(query_storage)) = plan.query_storage
    else {
        panic!("expected Postgres billing ledger query storage plan");
    };
    assert_eq!(query_storage.tenant_id, tenant_id);
    assert_eq!(
        query_storage.sort_order,
        BillingLedgerSortOrder::OccurredAtDesc
    );
    assert_eq!(query_storage.page_limit, 250);
    assert_eq!(
        query_storage.statements[1].name,
        "query_billing_ledger_desc"
    );
    assert!(!request_debug.contains(&tenant_id.to_string()));
    assert!(!request_debug.contains("control_plane.billing.read"));
    assert!(!plan_debug.contains(&tenant_id.to_string()));
    assert!(!plan_debug.contains("query_billing_ledger_desc"));
}

#[test]
fn application_denied_billing_read_still_audits_without_query_storage() {
    let tenant_id = TenantId::new();
    let plan = plan_application_billing_read(billing_read_request(
        DurableStoreKind::Sqlite,
        tenant_id,
        control_plane_principal(tenant_id, Role::Viewer, CredentialScope::DataPlane),
    ))
    .unwrap();

    let ControlPlaneDecision::Denied {
        audit_event,
        audit_write,
        ..
    } = plan.decision
    else {
        panic!("expected denied billing read");
    };
    assert_eq!(audit_event.tenant_id, tenant_id);
    assert_eq!(audit_event.outcome, prodex_domain::AuditOutcome::Denied);
    assert_eq!(audit_event.action.as_str(), "control_plane.billing.read");
    assert_eq!(audit_write.tenant_partition_key, tenant_id);
    let ApplicationControlPlaneAuditStoragePlan::Sqlite(audit_storage) = plan.audit_storage else {
        panic!("expected SQLite audit storage plan");
    };
    assert_eq!(
        audit_storage.statements[0].name,
        "begin_immediate_audit_append"
    );
    assert_eq!(plan.query_storage, None);
}

#[test]
fn application_billing_read_rejects_cross_tenant_query_before_storage() {
    let tenant_id = TenantId::new();
    let wrong_tenant = TenantId::new();
    let mut request = billing_read_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Viewer, CredentialScope::ControlPlane),
    );
    request.query = billing_read_request(
        DurableStoreKind::Postgres,
        wrong_tenant,
        control_plane_principal(wrong_tenant, Role::Viewer, CredentialScope::ControlPlane),
    )
    .query;

    let error = plan_application_billing_read(request).unwrap_err();

    assert_eq!(
        error,
        ApplicationBillingReadError::TenantMismatch {
            action_tenant: tenant_id,
            query_tenant: wrong_tenant,
        }
    );
    assert_eq!(error.to_string(), "billing read request is invalid");
    assert!(!error.to_string().contains(&tenant_id.to_string()));
    assert!(!error.to_string().contains(&wrong_tenant.to_string()));
    assert!(
        !ApplicationBillingReadError::WrongOperation(ControlPlaneOperation::AuditExport)
            .to_string()
            .contains("AuditExport")
    );
    assert!(
        !ApplicationBillingReadError::WrongResourceKind(ResourceKind::AuditLog)
            .to_string()
            .contains("AuditLog")
    );
    let error_debug = format!(
        "{error:?} {:?} {:?}",
        ApplicationBillingReadError::WrongOperation(ControlPlaneOperation::AuditExport),
        ApplicationBillingReadError::WrongResourceKind(ResourceKind::AuditLog)
    );
    assert!(!error_debug.contains(&tenant_id.to_string()));
    assert!(!error_debug.contains(&wrong_tenant.to_string()));
    assert!(!error_debug.contains("AuditExport"));
    assert!(!error_debug.contains("AuditLog"));
    let response = plan_application_billing_read_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationBillingReadErrorStatus::BadRequest
    );
    assert_eq!(response.code, "billing_read_invalid");
    assert_eq!(response.message, "billing read request is invalid");
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains(&wrong_tenant.to_string()));
}

#[test]
fn application_service_identity_create_selects_postgres_storage_plan() {
    let tenant_id = TenantId::new();
    let request = service_identity_create_request(DurableStoreKind::Postgres, tenant_id);
    let request_debug = format!("{request:?}");
    assert!(request_debug.contains("ApplicationServiceIdentityCreateRequest"));
    assert!(!request_debug.contains(&tenant_id.to_string()));
    assert!(!request_debug.contains("svc-ingest"));

    let plan = plan_application_service_identity_create(request).unwrap();
    let plan_debug = format!("{plan:?}");
    assert!(plan_debug.contains("ApplicationServiceIdentityCreatePlan"));
    assert!(!plan_debug.contains(&tenant_id.to_string()));
    assert!(!plan_debug.contains("svc-ingest"));

    let ApplicationServiceIdentityCreateStoragePlan::Postgres(storage) = plan.storage else {
        panic!("expected Postgres service identity storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.storage_key.tenant_id, tenant_id);
    assert_eq!(storage.display_name, "svc-ingest");
    assert_eq!(storage.statements[0].name, "set_tenant_context");
    assert_eq!(storage.statements[1].name, "upsert_service_identity");
}

#[test]
fn application_service_identity_create_selects_sqlite_storage_plan() {
    let tenant_id = TenantId::new();
    let plan = plan_application_service_identity_create(service_identity_create_request(
        DurableStoreKind::Sqlite,
        tenant_id,
    ))
    .unwrap();

    let ApplicationServiceIdentityCreateStoragePlan::Sqlite(storage) = plan.storage else {
        panic!("expected SQLite service identity storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.storage_key.tenant_id, tenant_id);
    assert_eq!(storage.display_name, "svc-ingest");
    assert_eq!(
        storage.statements[0].name,
        "begin_immediate_service_identity_create"
    );
    assert_eq!(
        storage.statements[1].name,
        "upsert_service_identity_locally"
    );
}

#[test]
fn application_service_identity_create_rejects_cross_tenant_storage_key() {
    let tenant_id = TenantId::new();
    let wrong_tenant = TenantId::new();
    let mut request = service_identity_create_request(DurableStoreKind::Postgres, tenant_id);
    request.identity.storage_key = TenantStorageKey::tenant(wrong_tenant);

    let error = plan_application_service_identity_create(request).unwrap_err();

    assert_eq!(
        error,
        ApplicationServiceIdentityCreateError::Postgres(
            prodex_storage_postgres::PostgresStoragePlanError::TenantMismatch {
                key_tenant: wrong_tenant,
                request_tenant: tenant_id,
            },
        )
    );
    let error_debug = format!("{error:?}");
    assert!(!error_debug.contains(&tenant_id.to_string()));
    assert!(!error_debug.contains(&wrong_tenant.to_string()));
    assert!(!error_debug.contains("svc-ingest"));
    assert!(!error_debug.contains("TenantMismatch"));
    let response = plan_application_service_identity_create_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationServiceIdentityCreateErrorStatus::ServiceUnavailable
    );
    assert_eq!(response.code, "service_identity_storage_unavailable");
    assert_eq!(
        response.message,
        "service identity storage is temporarily unavailable"
    );
    assert!(!response.message.contains("svc-ingest"));
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains(&wrong_tenant.to_string()));
}

#[test]
fn application_service_identity_lifecycle_authorized_create_selects_postgres_audit_and_storage() {
    let tenant_id = TenantId::new();
    let request = service_identity_lifecycle_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
    );
    let request_debug = format!("{request:?}");
    assert!(request_debug.contains("ApplicationServiceIdentityLifecycleRequest"));
    assert!(!request_debug.contains(&tenant_id.to_string()));
    assert!(!request_debug.contains("svc-ingest"));

    let plan = plan_application_service_identity_lifecycle(request).unwrap();
    let plan_debug = format!("{plan:?}");
    assert!(plan_debug.contains("ApplicationServiceIdentityLifecyclePlan"));
    assert!(!plan_debug.contains(&tenant_id.to_string()));
    assert!(!plan_debug.contains("svc-ingest"));
    assert!(!plan_debug.contains("control_plane.service_identity.create"));

    let ControlPlaneDecision::Authorized(action) = plan.decision else {
        panic!("expected authorized service identity create");
    };
    assert_eq!(action.tenant.tenant_id, tenant_id);
    assert_eq!(
        action.audit_event.action.as_str(),
        "control_plane.service_identity.create"
    );
    assert_eq!(action.audit_write.tenant_partition_key, tenant_id);
    let ApplicationControlPlaneAuditStoragePlan::Postgres(audit_storage) = plan.audit_storage
    else {
        panic!("expected Postgres audit storage plan");
    };
    assert_eq!(audit_storage.tenant_id, tenant_id);
    assert_eq!(audit_storage.statements[0].name, "set_tenant_context");
    let Some(ApplicationServiceIdentityCreateStoragePlan::Postgres(identity_storage)) =
        plan.identity_storage
    else {
        panic!("expected Postgres service identity storage plan");
    };
    assert_eq!(identity_storage.tenant_id, tenant_id);
    assert_eq!(
        identity_storage.statements[1].name,
        "upsert_service_identity"
    );
}

#[test]
fn application_denied_service_identity_lifecycle_still_audits_without_storage() {
    let tenant_id = TenantId::new();
    let plan = plan_application_service_identity_lifecycle(service_identity_lifecycle_request(
        DurableStoreKind::Sqlite,
        tenant_id,
        control_plane_principal(tenant_id, Role::Viewer, CredentialScope::ControlPlane),
    ))
    .unwrap();

    let ControlPlaneDecision::Denied {
        audit_event,
        audit_write,
        ..
    } = plan.decision
    else {
        panic!("expected denied service identity create");
    };
    assert_eq!(audit_event.tenant_id, tenant_id);
    assert_eq!(audit_event.outcome, prodex_domain::AuditOutcome::Denied);
    assert_eq!(
        audit_event.action.as_str(),
        "control_plane.service_identity.create"
    );
    assert_eq!(audit_write.tenant_partition_key, tenant_id);
    let ApplicationControlPlaneAuditStoragePlan::Sqlite(audit_storage) = plan.audit_storage else {
        panic!("expected SQLite audit storage plan");
    };
    assert_eq!(audit_storage.tenant_id, tenant_id);
    assert_eq!(
        audit_storage.statements[0].name,
        "begin_immediate_audit_append"
    );
    assert_eq!(plan.identity_storage, None);
}

#[test]
fn application_service_identity_lifecycle_rejects_cross_tenant_identity_before_storage() {
    let tenant_id = TenantId::new();
    let wrong_tenant = TenantId::new();
    let mut request = service_identity_lifecycle_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
    );
    request.identity.tenant_id = wrong_tenant;
    request.identity.storage_key = TenantStorageKey::tenant(wrong_tenant);

    let error = plan_application_service_identity_lifecycle(request).unwrap_err();

    assert_eq!(
        error,
        ApplicationServiceIdentityLifecycleError::TenantMismatch {
            action_tenant: tenant_id,
            identity_tenant: wrong_tenant,
        }
    );
    assert_eq!(
        error.to_string(),
        "service identity lifecycle request is invalid"
    );
    assert!(!error.to_string().contains(&tenant_id.to_string()));
    assert!(!error.to_string().contains(&wrong_tenant.to_string()));
    assert!(
        !ApplicationServiceIdentityLifecycleError::WrongResourceKind(ResourceKind::Billing)
            .to_string()
            .contains("Billing")
    );
    let error_debug = format!(
        "{error:?} {:?}",
        ApplicationServiceIdentityLifecycleError::WrongResourceKind(ResourceKind::Billing)
    );
    assert!(!error_debug.contains(&tenant_id.to_string()));
    assert!(!error_debug.contains(&wrong_tenant.to_string()));
    assert!(!error_debug.contains("svc-ingest"));
    assert!(!error_debug.contains("Billing"));
    let response = plan_application_service_identity_lifecycle_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationServiceIdentityLifecycleErrorStatus::BadRequest
    );
    assert_eq!(response.code, "service_identity_lifecycle_invalid");
    assert_eq!(
        response.message,
        "service identity lifecycle request is invalid"
    );
    assert!(!response.message.contains("svc-ingest"));
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains(&wrong_tenant.to_string()));
}

#[test]
fn application_service_identity_lifecycle_rejects_wrong_operation_with_redacted_response() {
    let tenant_id = TenantId::new();
    let mut request = service_identity_lifecycle_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
    );
    request.action.operation = ControlPlaneOperation::VirtualKeyCreate;

    let error = plan_application_service_identity_lifecycle(request).unwrap_err();

    assert_eq!(
        error,
        ApplicationServiceIdentityLifecycleError::WrongOperation(
            ControlPlaneOperation::VirtualKeyCreate
        )
    );
    assert!(!error.to_string().contains("VirtualKeyCreate"));
    let response = plan_application_service_identity_lifecycle_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationServiceIdentityLifecycleErrorStatus::BadRequest
    );
    assert_eq!(response.code, "service_identity_lifecycle_invalid");
    assert_eq!(
        response.message,
        "service identity lifecycle request is invalid"
    );
    assert!(!response.message.contains("VirtualKeyCreate"));
    assert!(!response.message.contains(&tenant_id.to_string()));
}

#[test]
fn application_budget_policy_update_selects_postgres_storage_plan() {
    let tenant_id = TenantId::new();
    let request = budget_policy_update_request(DurableStoreKind::Postgres, tenant_id);
    let request_debug = format!("{request:?}");
    assert!(request_debug.contains("ApplicationBudgetPolicyUpdateRequest"));
    assert!(!request_debug.contains(&tenant_id.to_string()));
    assert!(!request_debug.contains("tenant-default"));

    let plan = plan_application_budget_policy_update(request).unwrap();
    let plan_debug = format!("{plan:?}");
    assert!(plan_debug.contains("ApplicationBudgetPolicyUpdatePlan"));
    assert!(!plan_debug.contains(&tenant_id.to_string()));
    assert!(!plan_debug.contains("tenant-default"));

    let ApplicationBudgetPolicyUpdateStoragePlan::Postgres(storage) = plan.storage else {
        panic!("expected Postgres budget policy storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.storage_key.tenant_id, tenant_id);
    assert_eq!(storage.budget_scope, "tenant-default");
    assert_eq!(storage.limit.max, UsageAmount::new(10_000, 1_000_000));
    assert_eq!(storage.statements[0].name, "set_tenant_context");
    assert_eq!(storage.statements[1].name, "upsert_budget_policy");
}

#[test]
fn application_budget_policy_update_selects_sqlite_storage_plan() {
    let tenant_id = TenantId::new();
    let plan = plan_application_budget_policy_update(budget_policy_update_request(
        DurableStoreKind::Sqlite,
        tenant_id,
    ))
    .unwrap();

    let ApplicationBudgetPolicyUpdateStoragePlan::Sqlite(storage) = plan.storage else {
        panic!("expected SQLite budget policy storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.storage_key.tenant_id, tenant_id);
    assert_eq!(storage.budget_scope, "tenant-default");
    assert_eq!(
        storage.statements[0].name,
        "begin_immediate_budget_policy_update"
    );
    assert_eq!(storage.statements[1].name, "upsert_budget_policy_locally");
}

#[test]
fn application_budget_policy_update_rejects_empty_scope_with_redacted_response() {
    let tenant_id = TenantId::new();
    let mut request = budget_policy_update_request(DurableStoreKind::Postgres, tenant_id);
    request.policy.budget_scope = " ".to_string();

    let error = plan_application_budget_policy_update(request).unwrap_err();

    assert_eq!(
        error,
        ApplicationBudgetPolicyUpdateError::Postgres(
            prodex_storage_postgres::PostgresStoragePlanError::EmptyBudgetScope
        )
    );
    let error_debug = format!("{error:?}");
    assert!(!error_debug.contains(&tenant_id.to_string()));
    assert!(!error_debug.contains("tenant-default"));
    assert!(!error_debug.contains("EmptyBudgetScope"));
    let response = plan_application_budget_policy_update_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationBudgetPolicyUpdateErrorStatus::ServiceUnavailable
    );
    assert_eq!(response.code, "budget_policy_storage_unavailable");
    assert_eq!(
        response.message,
        "budget policy storage is temporarily unavailable"
    );
    assert!(!response.message.contains("tenant-default"));
    assert!(!response.message.contains(&tenant_id.to_string()));
}

#[test]
fn application_budget_policy_lifecycle_authorized_update_selects_postgres_audit_and_storage() {
    let tenant_id = TenantId::new();
    let request = budget_policy_lifecycle_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
    );
    let request_debug = format!("{request:?}");
    assert!(request_debug.contains("ApplicationBudgetPolicyLifecycleRequest"));
    assert!(!request_debug.contains(&tenant_id.to_string()));
    assert!(!request_debug.contains("tenant-default"));
    assert!(!request_debug.contains("BudgetUpdate"));

    let plan = plan_application_budget_policy_lifecycle(request).unwrap();
    let plan_debug = format!("{plan:?}");
    assert!(plan_debug.contains("ApplicationBudgetPolicyLifecyclePlan"));
    assert!(!plan_debug.contains(&tenant_id.to_string()));
    assert!(!plan_debug.contains("tenant-default"));
    assert!(!plan_debug.contains("control_plane.budget.update"));

    let ControlPlaneDecision::Authorized(action) = plan.decision else {
        panic!("expected authorized budget policy update");
    };
    assert_eq!(action.tenant.tenant_id, tenant_id);
    assert_eq!(
        action.audit_event.action.as_str(),
        "control_plane.budget.update"
    );
    let ApplicationControlPlaneAuditStoragePlan::Postgres(audit_storage) = plan.audit_storage
    else {
        panic!("expected Postgres audit storage plan");
    };
    assert_eq!(audit_storage.statements[0].name, "set_tenant_context");
    let Some(ApplicationBudgetPolicyUpdateStoragePlan::Postgres(policy_storage)) =
        plan.policy_storage
    else {
        panic!("expected Postgres budget policy storage plan");
    };
    assert_eq!(policy_storage.budget_scope, "tenant-default");
    assert_eq!(policy_storage.statements[1].name, "upsert_budget_policy");
}

#[test]
fn application_denied_budget_policy_lifecycle_still_audits_without_storage() {
    let tenant_id = TenantId::new();
    let plan = plan_application_budget_policy_lifecycle(budget_policy_lifecycle_request(
        DurableStoreKind::Sqlite,
        tenant_id,
        control_plane_principal(tenant_id, Role::Viewer, CredentialScope::ControlPlane),
    ))
    .unwrap();

    let ControlPlaneDecision::Denied {
        audit_event,
        audit_write,
        ..
    } = plan.decision
    else {
        panic!("expected denied budget policy update");
    };
    assert_eq!(audit_event.tenant_id, tenant_id);
    assert_eq!(audit_event.outcome, prodex_domain::AuditOutcome::Denied);
    assert_eq!(audit_event.action.as_str(), "control_plane.budget.update");
    assert_eq!(audit_write.tenant_partition_key, tenant_id);
    let ApplicationControlPlaneAuditStoragePlan::Sqlite(audit_storage) = plan.audit_storage else {
        panic!("expected SQLite audit storage plan");
    };
    assert_eq!(
        audit_storage.statements[0].name,
        "begin_immediate_audit_append"
    );
    assert_eq!(plan.policy_storage, None);
}

#[test]
fn application_budget_policy_lifecycle_rejects_cross_tenant_policy_before_storage() {
    let tenant_id = TenantId::new();
    let wrong_tenant = TenantId::new();
    let mut request = budget_policy_lifecycle_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
    );
    request.policy.tenant_id = wrong_tenant;
    request.policy.storage_key = TenantStorageKey::tenant(wrong_tenant);

    let error = plan_application_budget_policy_lifecycle(request).unwrap_err();

    assert_eq!(
        error,
        ApplicationBudgetPolicyLifecycleError::TenantMismatch {
            action_tenant: tenant_id,
            policy_tenant: wrong_tenant,
        }
    );
    assert_eq!(
        error.to_string(),
        "budget policy lifecycle request is invalid"
    );
    assert!(!error.to_string().contains(&tenant_id.to_string()));
    assert!(!error.to_string().contains(&wrong_tenant.to_string()));
    assert!(
        !ApplicationBudgetPolicyLifecycleError::WrongOperation(ControlPlaneOperation::BillingRead)
            .to_string()
            .contains("BillingRead")
    );
    assert!(
        !ApplicationBudgetPolicyLifecycleError::WrongResourceKind(ResourceKind::Billing)
            .to_string()
            .contains("Billing")
    );
    let error_debug = format!(
        "{error:?} {:?} {:?}",
        ApplicationBudgetPolicyLifecycleError::WrongOperation(ControlPlaneOperation::BillingRead),
        ApplicationBudgetPolicyLifecycleError::WrongResourceKind(ResourceKind::Billing)
    );
    assert!(!error_debug.contains(&tenant_id.to_string()));
    assert!(!error_debug.contains(&wrong_tenant.to_string()));
    assert!(!error_debug.contains("tenant-default"));
    assert!(!error_debug.contains("BillingRead"));
    assert!(!error_debug.contains("Billing"));
    let response = plan_application_budget_policy_lifecycle_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationBudgetPolicyLifecycleErrorStatus::BadRequest
    );
    assert_eq!(response.code, "budget_policy_lifecycle_invalid");
    assert_eq!(
        response.message,
        "budget policy lifecycle request is invalid"
    );
    assert!(!response.message.contains("tenant-default"));
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains(&wrong_tenant.to_string()));
}

#[test]
fn application_tenant_lifecycle_authorized_create_selects_postgres_audit_and_storage() {
    let tenant_id = TenantId::new();
    let request = tenant_lifecycle_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        TenantLifecycleKind::Create,
    );
    let request_debug = format!("{request:?}");
    let plan = plan_application_tenant_lifecycle(request).unwrap();
    let plan_debug = format!("{plan:?}");

    let ControlPlaneDecision::Authorized(action) = plan.decision else {
        panic!("expected authorized tenant create");
    };
    assert_eq!(action.tenant.tenant_id, tenant_id);
    assert_eq!(
        action.audit_event.action.as_str(),
        "control_plane.tenant.create"
    );
    assert_eq!(action.audit_write.tenant_partition_key, tenant_id);
    let ApplicationControlPlaneAuditStoragePlan::Postgres(audit_storage) = plan.audit_storage
    else {
        panic!("expected Postgres audit storage plan");
    };
    assert_eq!(audit_storage.tenant_id, tenant_id);
    assert_eq!(audit_storage.statements[0].name, "set_tenant_context");
    let Some(ApplicationTenantLifecycleStoragePlan::Postgres(tenant_storage)) = plan.tenant_storage
    else {
        panic!("expected Postgres tenant lifecycle storage plan");
    };
    assert_eq!(tenant_storage.tenant_id, tenant_id);
    assert_eq!(tenant_storage.display_name, "acme-prod");
    assert_eq!(tenant_storage.kind, TenantLifecycleKind::Create);
    assert_eq!(tenant_storage.statements[1].name, "upsert_tenant_lifecycle");
    assert!(!request_debug.contains(&tenant_id.to_string()));
    assert!(!request_debug.contains("control_plane.tenant.create"));
    assert!(!plan_debug.contains(&tenant_id.to_string()));
    assert!(!plan_debug.contains("upsert_tenant_lifecycle"));
}

#[test]
fn application_denied_tenant_lifecycle_still_audits_without_storage() {
    let tenant_id = TenantId::new();
    let plan = plan_application_tenant_lifecycle(tenant_lifecycle_request(
        DurableStoreKind::Sqlite,
        tenant_id,
        control_plane_principal(tenant_id, Role::Viewer, CredentialScope::ControlPlane),
        TenantLifecycleKind::Update,
    ))
    .unwrap();

    let ControlPlaneDecision::Denied {
        audit_event,
        audit_write,
        ..
    } = plan.decision
    else {
        panic!("expected denied tenant update");
    };
    assert_eq!(audit_event.tenant_id, tenant_id);
    assert_eq!(audit_event.outcome, prodex_domain::AuditOutcome::Denied);
    assert_eq!(audit_event.action.as_str(), "control_plane.tenant.update");
    assert_eq!(audit_write.tenant_partition_key, tenant_id);
    let ApplicationControlPlaneAuditStoragePlan::Sqlite(audit_storage) = plan.audit_storage else {
        panic!("expected SQLite audit storage plan");
    };
    assert_eq!(
        audit_storage.statements[0].name,
        "begin_immediate_audit_append"
    );
    assert_eq!(plan.tenant_storage, None);
}

#[test]
fn application_tenant_lifecycle_rejects_cross_tenant_command_before_storage() {
    let tenant_id = TenantId::new();
    let wrong_tenant = TenantId::new();
    let mut request = tenant_lifecycle_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        TenantLifecycleKind::Create,
    );
    request.tenant.tenant_id = wrong_tenant;
    request.tenant.storage_key = TenantStorageKey::tenant(wrong_tenant);

    let error = plan_application_tenant_lifecycle(request).unwrap_err();

    assert_eq!(
        error,
        ApplicationTenantLifecycleError::TenantMismatch {
            action_tenant: tenant_id,
            command_tenant: wrong_tenant,
        }
    );
    assert_eq!(error.to_string(), "tenant lifecycle request is invalid");
    assert!(!error.to_string().contains(&tenant_id.to_string()));
    assert!(!error.to_string().contains(&wrong_tenant.to_string()));
    assert!(
        !ApplicationTenantLifecycleError::WrongOperation(ControlPlaneOperation::BillingRead)
            .to_string()
            .contains("BillingRead")
    );
    assert!(
        !ApplicationTenantLifecycleError::WrongResourceKind(ResourceKind::Billing)
            .to_string()
            .contains("Billing")
    );
    let error_debug = format!(
        "{error:?} {:?} {:?}",
        ApplicationTenantLifecycleError::WrongOperation(ControlPlaneOperation::BillingRead),
        ApplicationTenantLifecycleError::WrongResourceKind(ResourceKind::Billing)
    );
    assert!(!error_debug.contains(&tenant_id.to_string()));
    assert!(!error_debug.contains(&wrong_tenant.to_string()));
    assert!(!error_debug.contains("BillingRead"));
    assert!(!error_debug.contains("Billing"));
    let response = plan_application_tenant_lifecycle_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationTenantLifecycleErrorStatus::BadRequest
    );
    assert_eq!(response.code, "tenant_lifecycle_invalid");
    assert_eq!(response.message, "tenant lifecycle request is invalid");
    assert!(!response.message.contains("acme-prod"));
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains(&wrong_tenant.to_string()));
}

#[test]
fn application_tenant_lifecycle_rejects_operation_kind_mismatch_with_redacted_response() {
    let tenant_id = TenantId::new();
    let mut request = tenant_lifecycle_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        TenantLifecycleKind::Create,
    );
    request.action.operation = ControlPlaneOperation::TenantUpdate;

    let error = plan_application_tenant_lifecycle(request).unwrap_err();

    assert_eq!(
        error,
        ApplicationTenantLifecycleError::KindMismatch {
            operation: ControlPlaneOperation::TenantUpdate,
            kind: TenantLifecycleKind::Create,
        }
    );
    assert!(!error.to_string().contains("TenantUpdate"));
    assert!(!error.to_string().contains("Create"));
    let response = plan_application_tenant_lifecycle_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationTenantLifecycleErrorStatus::BadRequest
    );
    assert_eq!(response.code, "tenant_lifecycle_invalid");
    assert_eq!(response.message, "tenant lifecycle request is invalid");
    assert!(!response.message.contains("TenantUpdate"));
    assert!(!response.message.contains("Create"));
    assert!(!response.message.contains(&tenant_id.to_string()));
}

#[test]
fn application_user_lifecycle_authorized_scim_create_selects_postgres_audit_and_storage() {
    let tenant_id = TenantId::new();
    let request = user_lifecycle_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::ScimUserCreate,
        UserLifecycleKind::Create,
    );
    let request_debug = format!("{request:?}");
    assert!(request_debug.contains("ApplicationUserLifecycleRequest"));
    assert!(!request_debug.contains(&tenant_id.to_string()));
    assert!(!request_debug.contains("scim-user-1"));
    assert!(!request_debug.contains("Ada Lovelace"));
    assert!(!request_debug.contains("ScimUserCreate"));

    let plan = plan_application_user_lifecycle(request).unwrap();
    let plan_debug = format!("{plan:?}");
    assert!(plan_debug.contains("ApplicationUserLifecyclePlan"));
    assert!(!plan_debug.contains(&tenant_id.to_string()));
    assert!(!plan_debug.contains("scim-user-1"));
    assert!(!plan_debug.contains("Ada Lovelace"));
    assert!(!plan_debug.contains("control_plane.scim_user.create"));

    let ControlPlaneDecision::Authorized(action) = plan.decision else {
        panic!("expected authorized SCIM user create");
    };
    assert_eq!(action.tenant.tenant_id, tenant_id);
    assert_eq!(
        action.audit_event.action.as_str(),
        "control_plane.scim_user.create"
    );
    assert_eq!(action.audit_write.tenant_partition_key, tenant_id);
    let ApplicationControlPlaneAuditStoragePlan::Postgres(audit_storage) = plan.audit_storage
    else {
        panic!("expected Postgres audit storage plan");
    };
    assert_eq!(audit_storage.statements[0].name, "set_tenant_context");
    let Some(ApplicationUserLifecycleStoragePlan::Postgres(user_storage)) = plan.user_storage
    else {
        panic!("expected Postgres user lifecycle storage plan");
    };
    assert_eq!(user_storage.tenant_id, tenant_id);
    assert_eq!(user_storage.external_id, "scim-user-1");
    assert_eq!(user_storage.display_name, "Ada Lovelace");
    assert_eq!(user_storage.kind, UserLifecycleKind::Create);
    assert_eq!(user_storage.statements[1].name, "upsert_user_lifecycle");
}

#[test]
fn application_denied_user_lifecycle_still_audits_without_storage() {
    let tenant_id = TenantId::new();
    let plan = plan_application_user_lifecycle(user_lifecycle_request(
        DurableStoreKind::Sqlite,
        tenant_id,
        control_plane_principal(tenant_id, Role::Viewer, CredentialScope::ControlPlane),
        ControlPlaneOperation::ScimUserDelete,
        UserLifecycleKind::Delete,
    ))
    .unwrap();

    let ControlPlaneDecision::Denied {
        audit_event,
        audit_write,
        ..
    } = plan.decision
    else {
        panic!("expected denied SCIM user delete");
    };
    assert_eq!(audit_event.tenant_id, tenant_id);
    assert_eq!(audit_event.outcome, prodex_domain::AuditOutcome::Denied);
    assert_eq!(
        audit_event.action.as_str(),
        "control_plane.scim_user.delete"
    );
    assert_eq!(audit_write.tenant_partition_key, tenant_id);
    let ApplicationControlPlaneAuditStoragePlan::Sqlite(audit_storage) = plan.audit_storage else {
        panic!("expected SQLite audit storage plan");
    };
    assert_eq!(
        audit_storage.statements[0].name,
        "begin_immediate_audit_append"
    );
    assert_eq!(plan.user_storage, None);
}

#[test]
fn application_user_lifecycle_rejects_cross_tenant_command_before_storage() {
    let tenant_id = TenantId::new();
    let wrong_tenant = TenantId::new();
    let mut request = user_lifecycle_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::ScimUserUpdate,
        UserLifecycleKind::Update,
    );
    request.user.tenant_id = wrong_tenant;
    request.user.storage_key = TenantStorageKey::tenant(wrong_tenant);

    let error = plan_application_user_lifecycle(request).unwrap_err();

    assert_eq!(
        error,
        ApplicationUserLifecycleError::TenantMismatch {
            action_tenant: tenant_id,
            command_tenant: wrong_tenant,
        }
    );
    assert_eq!(error.to_string(), "user lifecycle request is invalid");
    assert!(!error.to_string().contains(&tenant_id.to_string()));
    assert!(!error.to_string().contains(&wrong_tenant.to_string()));
    assert!(
        !ApplicationUserLifecycleError::WrongOperation(ControlPlaneOperation::BillingRead)
            .to_string()
            .contains("BillingRead")
    );
    assert!(
        !ApplicationUserLifecycleError::WrongResourceKind(ResourceKind::Billing)
            .to_string()
            .contains("Billing")
    );
    let error_debug = format!(
        "{error:?} {:?} {:?}",
        ApplicationUserLifecycleError::WrongOperation(ControlPlaneOperation::BillingRead),
        ApplicationUserLifecycleError::WrongResourceKind(ResourceKind::Billing)
    );
    assert!(!error_debug.contains(&tenant_id.to_string()));
    assert!(!error_debug.contains(&wrong_tenant.to_string()));
    assert!(!error_debug.contains("scim-user-1"));
    assert!(!error_debug.contains("Ada Lovelace"));
    assert!(!error_debug.contains("BillingRead"));
    assert!(!error_debug.contains("Billing"));
    let response = plan_application_user_lifecycle_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationUserLifecycleErrorStatus::BadRequest
    );
    assert_eq!(response.code, "user_lifecycle_invalid");
    assert_eq!(response.message, "user lifecycle request is invalid");
    assert!(!response.message.contains("scim-user-1"));
    assert!(!response.message.contains("Ada Lovelace"));
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains(&wrong_tenant.to_string()));
}

#[test]
fn application_user_lifecycle_rejects_operation_kind_mismatch_with_redacted_response() {
    let tenant_id = TenantId::new();
    let request = user_lifecycle_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::ScimUserDelete,
        UserLifecycleKind::Update,
    );

    let error = plan_application_user_lifecycle(request).unwrap_err();

    assert_eq!(
        error,
        ApplicationUserLifecycleError::KindMismatch {
            operation: ControlPlaneOperation::ScimUserDelete,
            kind: UserLifecycleKind::Update,
        }
    );
    assert!(!error.to_string().contains("ScimUserDelete"));
    assert!(!error.to_string().contains("Update"));
    let response = plan_application_user_lifecycle_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationUserLifecycleErrorStatus::BadRequest
    );
    assert_eq!(response.code, "user_lifecycle_invalid");
    assert_eq!(response.message, "user lifecycle request is invalid");
    assert!(!response.message.contains("ScimUserDelete"));
    assert!(!response.message.contains("Update"));
    assert!(!response.message.contains(&tenant_id.to_string()));
}

#[test]
fn application_role_binding_grant_selects_postgres_storage_plan() {
    let tenant_id = TenantId::new();
    let request = role_binding_mutation_request(
        DurableStoreKind::Postgres,
        tenant_id,
        RoleBindingMutationKind::Grant,
    );
    let request_debug = format!("{request:?}");
    assert!(request_debug.contains("ApplicationRoleBindingMutationRequest"));
    assert!(!request_debug.contains(&tenant_id.to_string()));
    assert!(!request_debug.contains("Operator"));

    let plan = plan_application_role_binding_mutation(request).unwrap();
    let plan_debug = format!("{plan:?}");
    assert!(plan_debug.contains("ApplicationRoleBindingMutationPlan"));
    assert!(!plan_debug.contains(&tenant_id.to_string()));
    assert!(!plan_debug.contains("Operator"));

    let ApplicationRoleBindingMutationStoragePlan::Postgres(storage) = plan.storage else {
        panic!("expected Postgres role-binding storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.storage_key.tenant_id, tenant_id);
    assert_eq!(storage.role, Role::Operator);
    assert_eq!(storage.kind, RoleBindingMutationKind::Grant);
    assert_eq!(storage.statements[0].name, "set_tenant_context");
    assert_eq!(storage.statements[1].name, "grant_role_binding");
}

#[test]
fn application_role_binding_revoke_selects_sqlite_storage_plan() {
    let tenant_id = TenantId::new();
    let plan = plan_application_role_binding_mutation(role_binding_mutation_request(
        DurableStoreKind::Sqlite,
        tenant_id,
        RoleBindingMutationKind::Revoke,
    ))
    .unwrap();

    let ApplicationRoleBindingMutationStoragePlan::Sqlite(storage) = plan.storage else {
        panic!("expected SQLite role-binding storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.storage_key.tenant_id, tenant_id);
    assert_eq!(storage.kind, RoleBindingMutationKind::Revoke);
    assert_eq!(
        storage.statements[0].name,
        "begin_immediate_role_binding_mutation"
    );
    assert_eq!(storage.statements[1].name, "revoke_role_binding_locally");
}

#[test]
fn application_role_binding_mutation_rejects_cross_tenant_storage_key() {
    let tenant_id = TenantId::new();
    let wrong_tenant = TenantId::new();
    let mut request = role_binding_mutation_request(
        DurableStoreKind::Postgres,
        tenant_id,
        RoleBindingMutationKind::Grant,
    );
    request.mutation.storage_key = TenantStorageKey::tenant(wrong_tenant);

    let error = plan_application_role_binding_mutation(request).unwrap_err();

    assert_eq!(
        error,
        ApplicationRoleBindingMutationError::Postgres(
            prodex_storage_postgres::PostgresStoragePlanError::TenantMismatch {
                key_tenant: wrong_tenant,
                request_tenant: tenant_id,
            },
        )
    );
    let error_debug = format!("{error:?}");
    assert!(!error_debug.contains(&tenant_id.to_string()));
    assert!(!error_debug.contains(&wrong_tenant.to_string()));
    assert!(!error_debug.contains("Operator"));
    assert!(!error_debug.contains("TenantMismatch"));
    let response = plan_application_role_binding_mutation_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationRoleBindingMutationErrorStatus::ServiceUnavailable
    );
    assert_eq!(response.code, "role_binding_storage_unavailable");
    assert_eq!(
        response.message,
        "role-binding storage is temporarily unavailable"
    );
    assert!(!response.message.contains("PostgreSQL"));
    assert!(!response.message.contains("SQLite"));
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains(&wrong_tenant.to_string()));
}

#[test]
fn application_role_binding_lifecycle_authorized_grant_selects_postgres_audit_and_storage() {
    let tenant_id = TenantId::new();
    let request = role_binding_lifecycle_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        RoleBindingMutationKind::Grant,
    );
    let request_debug = format!("{request:?}");
    assert!(request_debug.contains("ApplicationRoleBindingLifecycleRequest"));
    assert!(!request_debug.contains(&tenant_id.to_string()));
    assert!(!request_debug.contains("Operator"));

    let plan = plan_application_role_binding_lifecycle(request).unwrap();
    let plan_debug = format!("{plan:?}");
    assert!(plan_debug.contains("ApplicationRoleBindingLifecyclePlan"));
    assert!(!plan_debug.contains(&tenant_id.to_string()));
    assert!(!plan_debug.contains("Operator"));
    assert!(!plan_debug.contains("control_plane.role_binding.grant"));

    let ControlPlaneDecision::Authorized(action) = plan.decision else {
        panic!("expected authorized role-binding grant");
    };
    assert_eq!(action.tenant.tenant_id, tenant_id);
    assert_eq!(
        action.audit_event.action.as_str(),
        "control_plane.role_binding.grant"
    );
    assert_eq!(action.audit_write.tenant_partition_key, tenant_id);
    let ApplicationControlPlaneAuditStoragePlan::Postgres(audit_storage) = plan.audit_storage
    else {
        panic!("expected Postgres audit storage plan");
    };
    assert_eq!(audit_storage.tenant_id, tenant_id);
    assert_eq!(audit_storage.statements[0].name, "set_tenant_context");
    let Some(ApplicationRoleBindingMutationStoragePlan::Postgres(mutation_storage)) =
        plan.mutation_storage
    else {
        panic!("expected Postgres role-binding mutation storage plan");
    };
    assert_eq!(mutation_storage.tenant_id, tenant_id);
    assert_eq!(mutation_storage.storage_key.tenant_id, tenant_id);
    assert_eq!(mutation_storage.kind, RoleBindingMutationKind::Grant);
    assert_eq!(mutation_storage.statements[1].name, "grant_role_binding");
}

#[test]
fn application_denied_role_binding_lifecycle_still_audits_without_storage() {
    let tenant_id = TenantId::new();
    let plan = plan_application_role_binding_lifecycle(role_binding_lifecycle_request(
        DurableStoreKind::Sqlite,
        tenant_id,
        control_plane_principal(tenant_id, Role::Viewer, CredentialScope::ControlPlane),
        RoleBindingMutationKind::Revoke,
    ))
    .unwrap();

    let ControlPlaneDecision::Denied {
        audit_event,
        audit_write,
        ..
    } = plan.decision
    else {
        panic!("expected denied role-binding revoke");
    };
    assert_eq!(audit_event.tenant_id, tenant_id);
    assert_eq!(audit_event.outcome, prodex_domain::AuditOutcome::Denied);
    assert_eq!(
        audit_event.action.as_str(),
        "control_plane.role_binding.revoke"
    );
    assert_eq!(audit_write.tenant_partition_key, tenant_id);
    let ApplicationControlPlaneAuditStoragePlan::Sqlite(audit_storage) = plan.audit_storage else {
        panic!("expected SQLite audit storage plan");
    };
    assert_eq!(audit_storage.tenant_id, tenant_id);
    assert_eq!(
        audit_storage.statements[0].name,
        "begin_immediate_audit_append"
    );
    assert_eq!(plan.mutation_storage, None);
}

#[test]
fn application_role_binding_lifecycle_rejects_cross_tenant_mutation_before_storage() {
    let tenant_id = TenantId::new();
    let wrong_tenant = TenantId::new();
    let mut request = role_binding_lifecycle_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        RoleBindingMutationKind::Grant,
    );
    request.mutation.tenant_id = wrong_tenant;
    request.mutation.storage_key = TenantStorageKey::tenant(wrong_tenant);

    let error = plan_application_role_binding_lifecycle(request).unwrap_err();

    assert_eq!(
        error,
        ApplicationRoleBindingLifecycleError::TenantMismatch {
            action_tenant: tenant_id,
            mutation_tenant: wrong_tenant,
        }
    );
    assert_eq!(
        error.to_string(),
        "role-binding lifecycle request is invalid"
    );
    assert!(!error.to_string().contains(&tenant_id.to_string()));
    assert!(!error.to_string().contains(&wrong_tenant.to_string()));
    assert!(
        !ApplicationRoleBindingLifecycleError::WrongOperation(ControlPlaneOperation::BillingRead)
            .to_string()
            .contains("BillingRead")
    );
    assert!(
        !ApplicationRoleBindingLifecycleError::WrongResourceKind(ResourceKind::Billing)
            .to_string()
            .contains("Billing")
    );
    let error_debug = format!(
        "{error:?} {:?} {:?}",
        ApplicationRoleBindingLifecycleError::WrongOperation(ControlPlaneOperation::BillingRead),
        ApplicationRoleBindingLifecycleError::WrongResourceKind(ResourceKind::Billing)
    );
    assert!(!error_debug.contains(&tenant_id.to_string()));
    assert!(!error_debug.contains(&wrong_tenant.to_string()));
    assert!(!error_debug.contains("Operator"));
    assert!(!error_debug.contains("BillingRead"));
    assert!(!error_debug.contains("Billing"));
    let response = plan_application_role_binding_lifecycle_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationRoleBindingLifecycleErrorStatus::BadRequest
    );
    assert_eq!(response.code, "role_binding_lifecycle_invalid");
    assert_eq!(
        response.message,
        "role-binding lifecycle request is invalid"
    );
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains(&wrong_tenant.to_string()));
}

#[test]
fn application_role_binding_lifecycle_rejects_operation_kind_mismatch_with_redacted_response() {
    let tenant_id = TenantId::new();
    let mut request = role_binding_lifecycle_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        RoleBindingMutationKind::Grant,
    );
    request.action.operation = ControlPlaneOperation::RoleBindingRevoke;

    let error = plan_application_role_binding_lifecycle(request).unwrap_err();

    assert_eq!(
        error,
        ApplicationRoleBindingLifecycleError::MutationKindMismatch {
            operation: ControlPlaneOperation::RoleBindingRevoke,
            mutation_kind: RoleBindingMutationKind::Grant,
        }
    );
    assert!(!error.to_string().contains("RoleBindingRevoke"));
    assert!(!error.to_string().contains("Grant"));
    let response = plan_application_role_binding_lifecycle_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationRoleBindingLifecycleErrorStatus::BadRequest
    );
    assert_eq!(response.code, "role_binding_lifecycle_invalid");
    assert_eq!(
        response.message,
        "role-binding lifecycle request is invalid"
    );
    assert!(!response.message.contains("RoleBindingRevoke"));
    assert!(!response.message.contains("Grant"));
    assert!(!response.message.contains(&tenant_id.to_string()));
}

#[test]
fn application_virtual_key_secret_reference_selects_postgres_storage_plan() {
    let tenant_id = TenantId::new();
    let request = virtual_key_secret_reference_request(
        DurableStoreKind::Postgres,
        tenant_id,
        VirtualKeySecretReferenceKind::Create,
    );
    let request_debug = format!("{request:?}");
    assert!(request_debug.contains("ApplicationVirtualKeySecretReferenceRequest"));
    assert!(!request_debug.contains(&tenant_id.to_string()));
    assert!(!request_debug.contains("prod-api"));

    let plan = plan_application_virtual_key_secret_reference(request).unwrap();
    let plan_debug = format!("{plan:?}");
    assert!(plan_debug.contains("ApplicationVirtualKeySecretReferencePlan"));
    assert!(!plan_debug.contains(&tenant_id.to_string()));
    assert!(!plan_debug.contains("prod-api"));

    let ApplicationVirtualKeySecretReferenceStoragePlan::Postgres(storage) = plan.storage else {
        panic!("expected Postgres virtual-key secret storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.storage_key.tenant_id, tenant_id);
    assert_eq!(
        storage.storage_key.virtual_key_id,
        Some(storage.virtual_key_id)
    );
    assert_eq!(storage.display_name, "prod-api");
    assert_eq!(storage.kind, VirtualKeySecretReferenceKind::Create);
    assert_eq!(storage.statements[0].name, "set_tenant_context");
    assert_eq!(
        storage.statements[1].name,
        "upsert_virtual_key_secret_reference"
    );
}

#[test]
fn application_virtual_key_secret_reference_selects_sqlite_storage_plan() {
    let tenant_id = TenantId::new();
    let plan = plan_application_virtual_key_secret_reference(virtual_key_secret_reference_request(
        DurableStoreKind::Sqlite,
        tenant_id,
        VirtualKeySecretReferenceKind::Rotate,
    ))
    .unwrap();

    let ApplicationVirtualKeySecretReferenceStoragePlan::Sqlite(storage) = plan.storage else {
        panic!("expected SQLite virtual-key secret storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.storage_key.tenant_id, tenant_id);
    assert_eq!(
        storage.storage_key.virtual_key_id,
        Some(storage.virtual_key_id)
    );
    assert_eq!(storage.display_name, "prod-api");
    assert_eq!(storage.kind, VirtualKeySecretReferenceKind::Rotate);
    assert_eq!(
        storage.statements[0].name,
        "begin_immediate_virtual_key_secret_reference"
    );
    assert_eq!(
        storage.statements[1].name,
        "upsert_virtual_key_secret_reference_locally"
    );
}

#[test]
fn application_virtual_key_secret_reference_rejects_mismatched_storage_key_with_redacted_response()
{
    let tenant_id = TenantId::new();
    let mut request = virtual_key_secret_reference_request(
        DurableStoreKind::Postgres,
        tenant_id,
        VirtualKeySecretReferenceKind::Create,
    );
    request.reference.storage_key = TenantStorageKey::virtual_key(tenant_id, VirtualKeyId::new());

    let error = plan_application_virtual_key_secret_reference(request).unwrap_err();

    let ApplicationVirtualKeySecretReferenceError::Postgres(
        prodex_storage_postgres::PostgresStoragePlanError::VirtualKeyMismatch { .. },
    ) = error
    else {
        panic!("expected Postgres virtual-key mismatch");
    };
    let error_debug = format!("{error:?}");
    assert!(!error_debug.contains(&tenant_id.to_string()));
    assert!(!error_debug.contains("prod-api"));
    assert!(!error_debug.contains("VirtualKeyMismatch"));
    let response = plan_application_virtual_key_secret_reference_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationVirtualKeySecretReferenceErrorStatus::ServiceUnavailable
    );
    assert_eq!(response.code, "virtual_key_secret_storage_unavailable");
    assert_eq!(
        response.message,
        "virtual-key secret storage is temporarily unavailable"
    );
    assert!(!response.message.contains("virtual-keys/prod-api"));
    assert!(!response.message.contains(&tenant_id.to_string()));
}

#[test]
fn application_virtual_key_lifecycle_authorized_create_selects_postgres_audit_and_reference_storage()
 {
    let tenant_id = TenantId::new();
    let request = virtual_key_lifecycle_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        VirtualKeySecretReferenceKind::Create,
    );
    let request_debug = format!("{request:?}");
    assert!(request_debug.contains("ApplicationVirtualKeyLifecycleRequest"));
    assert!(!request_debug.contains(&tenant_id.to_string()));
    assert!(!request_debug.contains("virtual-keys/prod-api"));

    let plan = plan_application_virtual_key_lifecycle(request).unwrap();
    let plan_debug = format!("{plan:?}");
    assert!(plan_debug.contains("ApplicationVirtualKeyLifecyclePlan"));
    assert!(!plan_debug.contains(&tenant_id.to_string()));
    assert!(!plan_debug.contains("virtual-keys/prod-api"));
    assert!(!plan_debug.contains("control_plane.virtual_key.create"));

    let ControlPlaneDecision::Authorized(action) = plan.decision else {
        panic!("expected authorized virtual-key create");
    };
    assert_eq!(action.tenant.tenant_id, tenant_id);
    assert_eq!(
        action.audit_event.action.as_str(),
        "control_plane.virtual_key.create"
    );
    assert_eq!(action.audit_write.tenant_partition_key, tenant_id);
    let ApplicationControlPlaneAuditStoragePlan::Postgres(audit_storage) = plan.audit_storage
    else {
        panic!("expected Postgres audit storage plan");
    };
    assert_eq!(audit_storage.tenant_id, tenant_id);
    assert_eq!(audit_storage.statements[0].name, "set_tenant_context");
    let Some(ApplicationVirtualKeySecretReferenceStoragePlan::Postgres(reference_storage)) =
        plan.reference_storage
    else {
        panic!("expected Postgres virtual-key secret reference storage plan");
    };
    assert_eq!(reference_storage.tenant_id, tenant_id);
    assert_eq!(
        reference_storage.kind,
        VirtualKeySecretReferenceKind::Create
    );
    assert_eq!(
        reference_storage.statements[1].name,
        "upsert_virtual_key_secret_reference"
    );
}

#[test]
fn application_denied_virtual_key_lifecycle_still_audits_without_reference_storage() {
    let tenant_id = TenantId::new();
    let plan = plan_application_virtual_key_lifecycle(virtual_key_lifecycle_request(
        DurableStoreKind::Sqlite,
        tenant_id,
        control_plane_principal(tenant_id, Role::Viewer, CredentialScope::ControlPlane),
        VirtualKeySecretReferenceKind::Rotate,
    ))
    .unwrap();

    let ControlPlaneDecision::Denied {
        audit_event,
        audit_write,
        ..
    } = plan.decision
    else {
        panic!("expected denied virtual-key rotate");
    };
    assert_eq!(audit_event.tenant_id, tenant_id);
    assert_eq!(audit_event.outcome, prodex_domain::AuditOutcome::Denied);
    assert_eq!(
        audit_event.action.as_str(),
        "control_plane.virtual_key.rotate_secret"
    );
    assert_eq!(audit_write.tenant_partition_key, tenant_id);
    let ApplicationControlPlaneAuditStoragePlan::Sqlite(audit_storage) = plan.audit_storage else {
        panic!("expected SQLite audit storage plan");
    };
    assert_eq!(audit_storage.tenant_id, tenant_id);
    assert_eq!(
        audit_storage.statements[0].name,
        "begin_immediate_audit_append"
    );
    assert_eq!(plan.reference_storage, None);
}

#[test]
fn application_virtual_key_lifecycle_rejects_cross_tenant_reference_before_storage() {
    let tenant_id = TenantId::new();
    let wrong_tenant = TenantId::new();
    let mut request = virtual_key_lifecycle_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        VirtualKeySecretReferenceKind::Create,
    );
    request.reference.tenant_id = wrong_tenant;
    request.reference.storage_key =
        TenantStorageKey::virtual_key(wrong_tenant, request.reference.virtual_key_id);

    let error = plan_application_virtual_key_lifecycle(request).unwrap_err();

    assert_eq!(
        error,
        ApplicationVirtualKeyLifecycleError::TenantMismatch {
            action_tenant: tenant_id,
            reference_tenant: wrong_tenant,
        }
    );
    assert_eq!(
        error.to_string(),
        "virtual-key lifecycle request is invalid"
    );
    assert!(!error.to_string().contains(&tenant_id.to_string()));
    assert!(!error.to_string().contains(&wrong_tenant.to_string()));
    assert!(
        !ApplicationVirtualKeyLifecycleError::WrongOperation(ControlPlaneOperation::BillingRead)
            .to_string()
            .contains("BillingRead")
    );
    assert!(
        !ApplicationVirtualKeyLifecycleError::WrongResourceKind(ResourceKind::Billing)
            .to_string()
            .contains("Billing")
    );
    let error_debug = format!(
        "{error:?} {:?} {:?}",
        ApplicationVirtualKeyLifecycleError::WrongOperation(ControlPlaneOperation::BillingRead),
        ApplicationVirtualKeyLifecycleError::WrongResourceKind(ResourceKind::Billing)
    );
    assert!(!error_debug.contains(&tenant_id.to_string()));
    assert!(!error_debug.contains(&wrong_tenant.to_string()));
    assert!(!error_debug.contains("virtual-keys/prod-api"));
    assert!(!error_debug.contains("BillingRead"));
    assert!(!error_debug.contains("Billing"));
    let response = plan_application_virtual_key_lifecycle_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationVirtualKeyLifecycleErrorStatus::BadRequest
    );
    assert_eq!(response.code, "virtual_key_lifecycle_invalid");
    assert_eq!(response.message, "virtual-key lifecycle request is invalid");
    assert!(!response.message.contains("virtual-keys/prod-api"));
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains(&wrong_tenant.to_string()));
}

#[test]
fn application_virtual_key_lifecycle_rejects_operation_kind_mismatch_with_redacted_response() {
    let tenant_id = TenantId::new();
    let mut request = virtual_key_lifecycle_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        VirtualKeySecretReferenceKind::Create,
    );
    request.action.operation = ControlPlaneOperation::VirtualKeyRotateSecret;

    let error = plan_application_virtual_key_lifecycle(request).unwrap_err();

    assert_eq!(
        error,
        ApplicationVirtualKeyLifecycleError::ReferenceKindMismatch {
            operation: ControlPlaneOperation::VirtualKeyRotateSecret,
            reference_kind: VirtualKeySecretReferenceKind::Create,
        }
    );
    assert!(!error.to_string().contains("VirtualKeyRotateSecret"));
    assert!(!error.to_string().contains("Create"));
    let response = plan_application_virtual_key_lifecycle_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationVirtualKeyLifecycleErrorStatus::BadRequest
    );
    assert_eq!(response.code, "virtual_key_lifecycle_invalid");
    assert_eq!(response.message, "virtual-key lifecycle request is invalid");
    assert!(!response.message.contains("VirtualKeyRotateSecret"));
    assert!(!response.message.contains("Create"));
    assert!(!response.message.contains(&tenant_id.to_string()));
}

#[test]
fn application_provider_credential_reference_selects_postgres_storage_plan() {
    let tenant_id = TenantId::new();
    let request = provider_credential_reference_request(DurableStoreKind::Postgres, tenant_id);
    let request_debug = format!("{request:?}");
    assert!(request_debug.contains("ApplicationProviderCredentialReferenceRequest"));
    assert!(!request_debug.contains(&tenant_id.to_string()));
    assert!(!request_debug.contains("providers/openai"));
    assert!(!request_debug.contains("openai"));

    let plan = plan_application_provider_credential_reference(request).unwrap();
    let plan_debug = format!("{plan:?}");
    assert!(plan_debug.contains("ApplicationProviderCredentialReferencePlan"));
    assert!(!plan_debug.contains(&tenant_id.to_string()));
    assert!(!plan_debug.contains("providers/openai"));
    assert!(!plan_debug.contains("openai"));

    let ApplicationProviderCredentialReferenceStoragePlan::Postgres(storage) = plan.storage else {
        panic!("expected Postgres provider credential storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.storage_key.tenant_id, tenant_id);
    assert_eq!(storage.provider_name, "openai");
    assert_eq!(storage.statements[0].name, "set_tenant_context");
    assert_eq!(
        storage.statements[1].name,
        "upsert_provider_credential_reference"
    );
}

#[test]
fn application_provider_credential_reference_selects_sqlite_storage_plan() {
    let tenant_id = TenantId::new();
    let plan = plan_application_provider_credential_reference(
        provider_credential_reference_request(DurableStoreKind::Sqlite, tenant_id),
    )
    .unwrap();

    let ApplicationProviderCredentialReferenceStoragePlan::Sqlite(storage) = plan.storage else {
        panic!("expected SQLite provider credential storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.storage_key.tenant_id, tenant_id);
    assert_eq!(storage.provider_name, "openai");
    assert_eq!(
        storage.statements[0].name,
        "begin_immediate_provider_credential_reference"
    );
    assert_eq!(
        storage.statements[1].name,
        "upsert_provider_credential_reference_locally"
    );
}

#[test]
fn application_provider_credential_reference_rejects_cross_tenant_storage_key() {
    let tenant_id = TenantId::new();
    let wrong_tenant = TenantId::new();
    let mut request = provider_credential_reference_request(DurableStoreKind::Postgres, tenant_id);
    request.reference.storage_key = TenantStorageKey::tenant(wrong_tenant);

    let error = plan_application_provider_credential_reference(request).unwrap_err();

    assert_eq!(
        error,
        ApplicationProviderCredentialReferenceError::Postgres(
            prodex_storage_postgres::PostgresStoragePlanError::TenantMismatch {
                key_tenant: wrong_tenant,
                request_tenant: tenant_id,
            },
        )
    );
    let error_debug = format!("{error:?}");
    assert!(!error_debug.contains(&tenant_id.to_string()));
    assert!(!error_debug.contains(&wrong_tenant.to_string()));
    assert!(!error_debug.contains("providers/openai"));
    assert!(!error_debug.contains("TenantMismatch"));
    let response = plan_application_provider_credential_reference_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationProviderCredentialReferenceErrorStatus::ServiceUnavailable
    );
    assert_eq!(response.code, "provider_credential_storage_unavailable");
    assert_eq!(
        response.message,
        "provider credential storage is temporarily unavailable"
    );
    assert!(!response.message.contains("PostgreSQL"));
    assert!(!response.message.contains("SQLite"));
    assert!(!response.message.contains("providers/openai"));
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains(&wrong_tenant.to_string()));
}

#[test]
fn application_provider_credential_rotation_authorized_selects_postgres_audit_and_reference_storage()
 {
    let tenant_id = TenantId::new();
    let request = provider_credential_rotation_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
    );
    let request_debug = format!("{request:?}");
    assert!(request_debug.contains("ApplicationProviderCredentialRotationRequest"));
    assert!(!request_debug.contains(&tenant_id.to_string()));
    assert!(!request_debug.contains("providers/openai"));
    assert!(!request_debug.contains("openai"));

    let plan = plan_application_provider_credential_rotation(request).unwrap();
    let plan_debug = format!("{plan:?}");
    assert!(plan_debug.contains("ApplicationProviderCredentialRotationPlan"));
    assert!(!plan_debug.contains(&tenant_id.to_string()));
    assert!(!plan_debug.contains("providers/openai"));
    assert!(!plan_debug.contains("openai"));
    assert!(!plan_debug.contains("control_plane.provider_credential.rotate_secret"));

    let ControlPlaneDecision::Authorized(action) = plan.decision else {
        panic!("expected authorized provider credential rotation");
    };
    assert_eq!(action.tenant.tenant_id, tenant_id);
    assert_eq!(
        action.audit_event.action.as_str(),
        "control_plane.provider_credential.rotate_secret"
    );
    assert_eq!(action.audit_write.tenant_partition_key, tenant_id);
    let ApplicationControlPlaneAuditStoragePlan::Postgres(audit_storage) = plan.audit_storage
    else {
        panic!("expected Postgres audit storage plan");
    };
    assert_eq!(audit_storage.tenant_id, tenant_id);
    assert_eq!(audit_storage.statements[0].name, "set_tenant_context");
    let Some(ApplicationProviderCredentialReferenceStoragePlan::Postgres(reference_storage)) =
        plan.reference_storage
    else {
        panic!("expected Postgres provider credential reference storage plan");
    };
    assert_eq!(reference_storage.tenant_id, tenant_id);
    assert_eq!(reference_storage.storage_key.tenant_id, tenant_id);
    assert_eq!(reference_storage.provider_name, "openai");
    assert_eq!(
        reference_storage.statements[1].name,
        "upsert_provider_credential_reference"
    );
}

#[test]
fn application_denied_provider_credential_rotation_still_audits_without_reference_storage() {
    let tenant_id = TenantId::new();
    let plan = plan_application_provider_credential_rotation(provider_credential_rotation_request(
        DurableStoreKind::Sqlite,
        tenant_id,
        control_plane_principal(tenant_id, Role::Viewer, CredentialScope::ControlPlane),
    ))
    .unwrap();

    let ControlPlaneDecision::Denied {
        audit_event,
        audit_write,
        ..
    } = plan.decision
    else {
        panic!("expected denied provider credential rotation");
    };
    assert_eq!(audit_event.tenant_id, tenant_id);
    assert_eq!(audit_event.outcome, prodex_domain::AuditOutcome::Denied);
    assert_eq!(
        audit_event.action.as_str(),
        "control_plane.provider_credential.rotate_secret"
    );
    assert_eq!(audit_write.tenant_partition_key, tenant_id);
    let ApplicationControlPlaneAuditStoragePlan::Sqlite(audit_storage) = plan.audit_storage else {
        panic!("expected SQLite audit storage plan");
    };
    assert_eq!(audit_storage.tenant_id, tenant_id);
    assert_eq!(
        audit_storage.statements[0].name,
        "begin_immediate_audit_append"
    );
    assert_eq!(plan.reference_storage, None);
}

#[test]
fn application_provider_credential_rotation_rejects_cross_tenant_reference_before_storage() {
    let tenant_id = TenantId::new();
    let wrong_tenant = TenantId::new();
    let mut request = provider_credential_rotation_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
    );
    request.reference.tenant_id = wrong_tenant;
    request.reference.storage_key = TenantStorageKey::tenant(wrong_tenant);

    let error = plan_application_provider_credential_rotation(request).unwrap_err();

    assert_eq!(
        error,
        ApplicationProviderCredentialRotationError::TenantMismatch {
            action_tenant: tenant_id,
            reference_tenant: wrong_tenant,
        }
    );
    assert_eq!(
        error.to_string(),
        "provider credential rotation request is invalid"
    );
    assert!(!error.to_string().contains(&tenant_id.to_string()));
    assert!(!error.to_string().contains(&wrong_tenant.to_string()));
    assert!(
        !ApplicationProviderCredentialRotationError::WrongResourceKind(ResourceKind::Billing)
            .to_string()
            .contains("Billing")
    );
    let error_debug = format!(
        "{error:?} {:?}",
        ApplicationProviderCredentialRotationError::WrongResourceKind(ResourceKind::Billing)
    );
    assert!(!error_debug.contains(&tenant_id.to_string()));
    assert!(!error_debug.contains(&wrong_tenant.to_string()));
    assert!(!error_debug.contains("providers/openai"));
    assert!(!error_debug.contains("openai"));
    assert!(!error_debug.contains("Billing"));
    let response = plan_application_provider_credential_rotation_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationProviderCredentialRotationErrorStatus::BadRequest
    );
    assert_eq!(response.code, "provider_credential_rotation_invalid");
    assert_eq!(
        response.message,
        "provider credential rotation request is invalid"
    );
    assert!(!response.message.contains("openai"));
    assert!(!response.message.contains("providers/openai"));
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains(&wrong_tenant.to_string()));
}

#[test]
fn application_provider_credential_rotation_rejects_wrong_operation_with_redacted_response() {
    let tenant_id = TenantId::new();
    let mut request = provider_credential_rotation_request(
        DurableStoreKind::Postgres,
        tenant_id,
        control_plane_principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
    );
    request.action.operation = ControlPlaneOperation::PolicyPublish;

    let error = plan_application_provider_credential_rotation(request).unwrap_err();

    assert_eq!(
        error,
        ApplicationProviderCredentialRotationError::WrongOperation(
            ControlPlaneOperation::PolicyPublish
        )
    );
    assert!(!error.to_string().contains("PolicyPublish"));
    let response = plan_application_provider_credential_rotation_error_response(&error);
    assert_eq!(
        response.status,
        ApplicationProviderCredentialRotationErrorStatus::BadRequest
    );
    assert_eq!(response.code, "provider_credential_rotation_invalid");
    assert_eq!(
        response.message,
        "provider credential rotation request is invalid"
    );
    assert!(!response.message.contains("PolicyPublish"));
    assert!(!response.message.contains(&tenant_id.to_string()));
}

#[test]
fn application_configuration_publication_authorized_selects_postgres_audit_storage_plan() {
    let tenant_id = TenantId::new();
    let plan = plan_application_configuration_publication(configuration_publication_request(
        DurableStoreKind::Postgres,
        tenant_id,
        tenant_id,
    ))
    .unwrap();

    let ConfigurationPublicationDecision::Authorized(publication) = plan.decision else {
        panic!("expected authorized configuration publication");
    };
    assert_eq!(publication.action.tenant.tenant_id, tenant_id);
    let ApplicationConfigurationPublicationAuditStoragePlan::Postgres(storage) = plan.audit_storage
    else {
        panic!("expected Postgres audit storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.statements[0].name, "set_tenant_context");
}

#[test]
fn application_configuration_publication_rejection_selects_sqlite_audit_storage_plan() {
    let tenant_id = TenantId::new();
    let wrong_tenant = TenantId::new();
    let plan = plan_application_configuration_publication(configuration_publication_request(
        DurableStoreKind::Sqlite,
        tenant_id,
        wrong_tenant,
    ))
    .unwrap();

    let ConfigurationPublicationDecision::Denied {
        audit_event,
        audit_write,
        ..
    } = plan.decision
    else {
        panic!("expected denied configuration publication");
    };
    assert_eq!(audit_event.tenant_id, tenant_id);
    assert_eq!(audit_write.tenant_partition_key, tenant_id);
    let ApplicationConfigurationPublicationAuditStoragePlan::Sqlite(storage) = plan.audit_storage
    else {
        panic!("expected SQLite audit storage plan");
    };
    assert_eq!(storage.tenant_id, tenant_id);
    assert_eq!(storage.statements[0].name, "begin_immediate_audit_append");
}

#[test]
fn application_configuration_publication_error_responses_are_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let wrong_tenant = TenantId::new();
    let plan = plan_application_configuration_publication(configuration_publication_request(
        DurableStoreKind::Postgres,
        tenant_id,
        wrong_tenant,
    ))
    .unwrap();

    let response =
        plan_application_configuration_publication_decision_error_response(&plan.decision)
            .expect("expected denied configuration publication response");
    assert_eq!(
        response.status,
        ApplicationConfigurationPublicationErrorStatus::BadRequest
    );
    assert_eq!(response.code, "configuration_tenant_mismatch");
    assert_eq!(
        response.message,
        "configuration tenant does not match request tenant"
    );
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains(&wrong_tenant.to_string()));
    assert!(!response.message.contains("config-payload"));

    let authorized = plan_application_configuration_publication(configuration_publication_request(
        DurableStoreKind::Postgres,
        tenant_id,
        tenant_id,
    ))
    .unwrap();
    assert_eq!(
        plan_application_configuration_publication_decision_error_response(&authorized.decision),
        None
    );

    let storage_error = ApplicationConfigurationPublicationError::Postgres(
        prodex_storage_postgres::PostgresStoragePlanError::TenantMismatch {
            key_tenant: tenant_id,
            request_tenant: wrong_tenant,
        },
    );
    let storage_response =
        plan_application_configuration_publication_error_response(&storage_error);
    assert_eq!(
        storage_response.status,
        ApplicationConfigurationPublicationErrorStatus::ServiceUnavailable
    );
    assert_eq!(storage_response.code, "audit_storage_unavailable");
    assert_eq!(
        storage_response.message,
        "audit storage is temporarily unavailable"
    );
    assert!(!storage_response.message.contains("PostgreSQL"));
    assert!(!storage_response.message.contains(&tenant_id.to_string()));
    assert!(!storage_response.message.contains(&wrong_tenant.to_string()));
}

fn complete_config_event_delivery() -> ConfigPublicationEventDelivery {
    ConfigPublicationEventDelivery {
        gateway_cache_refresh: true,
        runtime_policy_reload: true,
    }
}

#[test]
fn application_configuration_activation_uses_authorized_publication_candidate() {
    let tenant_id = TenantId::new();
    let current = PolicyRevisionId::new();
    let publication = plan_application_configuration_publication(
        configuration_publication_request(DurableStoreKind::Postgres, tenant_id, tenant_id),
    )
    .unwrap();
    let state = ConfigCacheState {
        tenant_id,
        active_revision_id: Some(current),
        last_known_good_revision_id: None,
        invalidated_revision_id: Some(current),
        window: ConfigCacheWindow::new(1_000, 2_000, 3_000).unwrap(),
    };

    let activation_plan =
        plan_application_configuration_activation(ApplicationConfigurationActivationRequest {
            cache_state: &state,
            publication_decision: &publication.decision,
            redis_cache_ttl_seconds: Some(300),
            event_delivery: complete_config_event_delivery(),
        })
        .unwrap();
    let activation = activation_plan
        .activation
        .expect("expected activation for authorized publication");

    let ConfigurationPublicationDecision::Authorized(publication) = publication.decision else {
        panic!("expected authorized publication");
    };
    assert_eq!(activation.previous_active_revision_id, Some(current));
    assert_eq!(
        activation.activated_revision_id,
        publication.candidate.revision_id
    );
    assert_eq!(
        activation.next_state.active_revision_id,
        Some(publication.candidate.revision_id)
    );
    assert_eq!(activation.next_state.last_known_good_revision_id, None);
    assert_eq!(activation.next_state.invalidated_revision_id, None);
    let redis_cache = activation_plan
        .redis_cache
        .expect("expected Redis cache plan");
    assert_eq!(redis_cache.tenant_id, tenant_id);
    assert_eq!(
        redis_cache.revision_id,
        Some(publication.candidate.revision_id)
    );
    assert_eq!(redis_cache.ttl_seconds, 300);
    assert_eq!(redis_cache.purpose, RedisCachePurpose::PolicyRevision);
    assert!(
        redis_cache
            .key
            .as_str()
            .contains(&publication.candidate.revision_id.to_string())
    );
    let publication_event = activation_plan
        .publication_event
        .expect("expected publication event plan");
    assert_eq!(publication_event.tenant_id, tenant_id);
    assert_eq!(
        publication_event.activated_revision_id,
        publication.candidate.revision_id
    );
    assert_eq!(publication_event.previous_active_revision_id, Some(current));
    assert_eq!(
        publication_event.targets,
        [
            ConfigPublicationEventTarget::GatewayCacheRefresh,
            ConfigPublicationEventTarget::RuntimePolicyReload,
        ]
    );
}

#[test]
fn application_configuration_activation_skips_denied_publication_and_redacts_errors() {
    let tenant_id = TenantId::new();
    let wrong_tenant = TenantId::new();
    let denied = plan_application_configuration_publication(configuration_publication_request(
        DurableStoreKind::Postgres,
        tenant_id,
        wrong_tenant,
    ))
    .unwrap();
    let state = ConfigCacheState {
        tenant_id,
        active_revision_id: Some(PolicyRevisionId::new()),
        last_known_good_revision_id: None,
        invalidated_revision_id: None,
        window: ConfigCacheWindow::new(1_000, 2_000, 3_000).unwrap(),
    };

    let skipped =
        plan_application_configuration_activation(ApplicationConfigurationActivationRequest {
            cache_state: &state,
            publication_decision: &denied.decision,
            redis_cache_ttl_seconds: Some(300),
            event_delivery: complete_config_event_delivery(),
        })
        .unwrap();
    assert_eq!(skipped.activation, None);
    assert_eq!(skipped.redis_cache, None);
    assert_eq!(skipped.publication_event, None);

    let authorized = plan_application_configuration_publication(configuration_publication_request(
        DurableStoreKind::Postgres,
        tenant_id,
        tenant_id,
    ))
    .unwrap();
    let mut stale_state = state.clone();
    let ConfigurationPublicationDecision::Authorized(publication) = &authorized.decision else {
        panic!("expected authorized publication");
    };
    stale_state.active_revision_id = Some(publication.candidate.revision_id);
    let error =
        plan_application_configuration_activation(ApplicationConfigurationActivationRequest {
            cache_state: &stale_state,
            publication_decision: &authorized.decision,
            redis_cache_ttl_seconds: Some(300),
            event_delivery: complete_config_event_delivery(),
        })
        .unwrap_err();
    let response = plan_application_configuration_activation_error_response(&error);

    assert_eq!(
        response.status,
        ApplicationConfigurationActivationErrorStatus::Conflict
    );
    assert_eq!(response.code, "configuration_revision_not_newer");
    assert_eq!(
        response.message,
        "configuration revision is not newer than active revision"
    );
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(
        !response
            .message
            .contains(&publication.candidate.revision_id.to_string())
    );
    assert!(matches!(
        error,
        ApplicationConfigurationActivationError::Publication(_)
    ));

    let redis_error =
        plan_application_configuration_activation(ApplicationConfigurationActivationRequest {
            cache_state: &state,
            publication_decision: &authorized.decision,
            redis_cache_ttl_seconds: Some(0),
            event_delivery: complete_config_event_delivery(),
        })
        .unwrap_err();
    let redis_response = plan_application_configuration_activation_error_response(&redis_error);
    assert_eq!(
        redis_response.status,
        ApplicationConfigurationActivationErrorStatus::ServiceUnavailable
    );
    assert_eq!(redis_response.code, "configuration_cache_unavailable");
    assert_eq!(
        redis_response.message,
        "configuration cache is temporarily unavailable"
    );
    assert!(matches!(
        redis_error,
        ApplicationConfigurationActivationError::Redis(_)
    ));
}

#[test]
fn application_configuration_activation_requires_publication_event_delivery_targets() {
    let tenant_id = TenantId::new();
    let current = PolicyRevisionId::new();
    let authorized = plan_application_configuration_publication(configuration_publication_request(
        DurableStoreKind::Postgres,
        tenant_id,
        tenant_id,
    ))
    .unwrap();
    let state = ConfigCacheState {
        tenant_id,
        active_revision_id: Some(current),
        last_known_good_revision_id: None,
        invalidated_revision_id: None,
        window: ConfigCacheWindow::new(1_000, 2_000, 3_000).unwrap(),
    };

    let error =
        plan_application_configuration_activation(ApplicationConfigurationActivationRequest {
            cache_state: &state,
            publication_decision: &authorized.decision,
            redis_cache_ttl_seconds: Some(300),
            event_delivery: ConfigPublicationEventDelivery {
                gateway_cache_refresh: true,
                runtime_policy_reload: false,
            },
        })
        .unwrap_err();
    let response = plan_application_configuration_activation_error_response(&error);

    assert!(matches!(
        error,
        ApplicationConfigurationActivationError::PublicationEvent(_)
    ));
    assert_eq!(
        response.status,
        ApplicationConfigurationActivationErrorStatus::ServiceUnavailable
    );
    assert_eq!(response.code, "configuration_publication_event_incomplete");
    assert_eq!(
        response.message,
        "configuration publication event is incomplete"
    );
    assert!(!response.message.contains(&tenant_id.to_string()));
    if let ConfigurationPublicationDecision::Authorized(publication) = &authorized.decision {
        assert!(
            !response
                .message
                .contains(&publication.candidate.revision_id.to_string())
        );
    }
}

#[test]
fn application_configuration_readiness_snapshot_uses_active_revision_before_refresh() {
    let tenant_id = TenantId::new();
    let active_revision = PolicyRevisionId::new();
    let state = ConfigCacheState {
        tenant_id,
        active_revision_id: Some(active_revision),
        last_known_good_revision_id: None,
        invalidated_revision_id: None,
        window: ConfigCacheWindow::new(1_000, 2_000, 3_000).unwrap(),
    };

    let plan = plan_application_configuration_readiness_snapshot(
        ApplicationConfigurationReadinessRequest {
            live: true,
            startup_complete: true,
            draining: false,
            cache_state: state,
            now_unix_ms: 500,
            checks: vec![HealthCheck::new(
                "database",
                HealthState::Passing,
                None::<String>,
            )],
        },
    );

    assert_eq!(
        plan.refresh_decision,
        prodex_config::ConfigRefreshDecision::UseActive
    );
    assert_eq!(plan.snapshot.active_policy_revision, Some(active_revision));
    assert_eq!(plan.snapshot.readyz(), HealthState::Passing);
    assert!(
        plan.snapshot
            .checks
            .iter()
            .any(|check| { check.name == "configuration" && check.state == HealthState::Passing })
    );
}

#[test]
fn application_configuration_readiness_snapshot_fails_readyz_while_draining() {
    let tenant_id = TenantId::new();
    let active_revision = PolicyRevisionId::new();
    let state = ConfigCacheState {
        tenant_id,
        active_revision_id: Some(active_revision),
        last_known_good_revision_id: None,
        invalidated_revision_id: None,
        window: ConfigCacheWindow::new(1_000, 2_000, 3_000).unwrap(),
    };

    let plan = plan_application_configuration_readiness_snapshot(
        ApplicationConfigurationReadinessRequest {
            live: true,
            startup_complete: true,
            draining: true,
            cache_state: state,
            now_unix_ms: 500,
            checks: vec![HealthCheck::new(
                "database",
                HealthState::Passing,
                None::<String>,
            )],
        },
    );

    assert_eq!(
        plan.refresh_decision,
        prodex_config::ConfigRefreshDecision::UseActive
    );
    assert_eq!(plan.snapshot.active_policy_revision, Some(active_revision));
    assert_eq!(plan.snapshot.livez(), HealthState::Passing);
    assert_eq!(plan.snapshot.startupz(), HealthState::Passing);
    assert_eq!(plan.snapshot.readyz(), HealthState::Failing);
}

#[test]
fn application_configuration_readiness_snapshot_reports_async_refresh_as_degraded() {
    let tenant_id = TenantId::new();
    let active_revision = PolicyRevisionId::new();
    let state = ConfigCacheState {
        tenant_id,
        active_revision_id: Some(active_revision),
        last_known_good_revision_id: None,
        invalidated_revision_id: None,
        window: ConfigCacheWindow::new(1_000, 2_000, 3_000).unwrap(),
    };

    let plan = plan_application_configuration_readiness_snapshot(
        ApplicationConfigurationReadinessRequest {
            live: true,
            startup_complete: true,
            draining: false,
            cache_state: state,
            now_unix_ms: 1_500,
            checks: vec![],
        },
    );

    assert_eq!(
        plan.refresh_decision,
        prodex_config::ConfigRefreshDecision::RefreshAsync
    );
    assert_eq!(plan.snapshot.active_policy_revision, Some(active_revision));
    assert_eq!(plan.snapshot.readyz(), HealthState::Degraded);
    assert!(
        plan.snapshot
            .checks
            .iter()
            .any(|check| { check.name == "configuration" && check.state == HealthState::Degraded })
    );
}

#[test]
fn application_configuration_readiness_snapshot_uses_lkg_for_stale_or_invalidated_active() {
    let tenant_id = TenantId::new();
    let active_revision = PolicyRevisionId::new();
    let last_known_good = PolicyRevisionId::new();
    let stale_state = ConfigCacheState {
        tenant_id,
        active_revision_id: Some(active_revision),
        last_known_good_revision_id: Some(last_known_good),
        invalidated_revision_id: None,
        window: ConfigCacheWindow::new(1_000, 2_000, 3_000).unwrap(),
    };

    let stale_plan = plan_application_configuration_readiness_snapshot(
        ApplicationConfigurationReadinessRequest {
            live: true,
            startup_complete: true,
            draining: false,
            cache_state: stale_state,
            now_unix_ms: 2_500,
            checks: vec![],
        },
    );

    assert_eq!(
        stale_plan.refresh_decision,
        prodex_config::ConfigRefreshDecision::UseLastKnownGood
    );
    assert_eq!(
        stale_plan.snapshot.active_policy_revision,
        Some(last_known_good)
    );
    assert_eq!(stale_plan.snapshot.readyz(), HealthState::Degraded);
    assert!(
        stale_plan
            .snapshot
            .checks
            .iter()
            .any(|check| { check.name == "configuration" && check.state == HealthState::Degraded })
    );

    let invalidated_state = ConfigCacheState {
        tenant_id,
        active_revision_id: Some(active_revision),
        last_known_good_revision_id: Some(last_known_good),
        invalidated_revision_id: Some(active_revision),
        window: ConfigCacheWindow::new(1_000, 2_000, 3_000).unwrap(),
    };

    let invalidated_plan = plan_application_configuration_readiness_snapshot(
        ApplicationConfigurationReadinessRequest {
            live: true,
            startup_complete: true,
            draining: false,
            cache_state: invalidated_state,
            now_unix_ms: 500,
            checks: vec![],
        },
    );

    assert_eq!(
        invalidated_plan.refresh_decision,
        prodex_config::ConfigRefreshDecision::UseLastKnownGood
    );
    assert_eq!(
        invalidated_plan.snapshot.active_policy_revision,
        Some(last_known_good)
    );
    assert_eq!(invalidated_plan.snapshot.readyz(), HealthState::Degraded);
}

#[test]
fn application_configuration_readiness_snapshot_fails_without_usable_revision() {
    let tenant_id = TenantId::new();
    let active_revision = PolicyRevisionId::new();
    let state = ConfigCacheState {
        tenant_id,
        active_revision_id: Some(active_revision),
        last_known_good_revision_id: None,
        invalidated_revision_id: Some(active_revision),
        window: ConfigCacheWindow::new(1_000, 2_000, 3_000).unwrap(),
    };

    let plan = plan_application_configuration_readiness_snapshot(
        ApplicationConfigurationReadinessRequest {
            live: true,
            startup_complete: true,
            draining: false,
            cache_state: state,
            now_unix_ms: 500,
            checks: vec![],
        },
    );

    assert_eq!(
        plan.refresh_decision,
        prodex_config::ConfigRefreshDecision::RejectedInvalidated
    );
    assert_eq!(plan.snapshot.active_policy_revision, None);
    assert_eq!(plan.snapshot.readyz(), HealthState::Failing);
    assert!(
        plan.snapshot
            .checks
            .iter()
            .any(|check| { check.name == "configuration" && check.state == HealthState::Failing })
    );
}

#[test]
fn application_runtime_requires_external_migrations_and_valid_gateway_storage_topology() {
    let topology = ApplicationRuntimeTopology {
        storage: StorageTopology {
            durable_store: DurableStoreKind::Postgres,
            cache_store: Some(CacheStoreKind::Redis),
            migration_policy: MigrationRuntimePolicy::RequestPathForbidden,
        },
        migrations: ApplicationMigrationMode::ExternalOnly,
        gateway_replica_count: 1,
        require_multi_replica_accounting_checks: false,
        redis_rate_limit: None,
        redis_cache: vec![],
        redis_coordination: vec![],
    };

    let plan = plan_application_runtime(topology).unwrap();

    assert_eq!(plan.durable_store, DurableStoreKind::Postgres);
    assert!(plan.migrations_are_external);
    assert!(!plan.redis_is_rebuildable);
    assert_eq!(plan.accounting_concurrency, None);
}

#[test]
fn application_runtime_can_require_multi_replica_accounting_concurrency_spec() {
    let topology = ApplicationRuntimeTopology {
        storage: StorageTopology {
            durable_store: DurableStoreKind::Postgres,
            cache_store: Some(CacheStoreKind::Redis),
            migration_policy: MigrationRuntimePolicy::RequestPathForbidden,
        },
        migrations: ApplicationMigrationMode::ExternalOnly,
        gateway_replica_count: 2,
        require_multi_replica_accounting_checks: true,
        redis_rate_limit: None,
        redis_cache: vec![],
        redis_coordination: vec![],
    };

    let plan = plan_application_runtime(topology).unwrap();

    let concurrency = plan
        .accounting_concurrency
        .expect("expected accounting concurrency spec");
    assert_eq!(concurrency.gateway_replica_count, 2);
    assert_eq!(
        concurrency.topology.durable_store,
        DurableStoreKind::Postgres
    );
    assert_eq!(
        concurrency.topology.cache_store,
        Some(CacheStoreKind::Redis)
    );
    assert_eq!(concurrency.checks.len(), 5);
}

fn complete_multi_replica_accounting_evidence(
    topology: StorageTopology,
    gateway_replica_count: u16,
) -> MultiReplicaAccountingEvidence {
    MultiReplicaAccountingEvidence {
        topology,
        gateway_replica_count,
        passed_checks: vec![
            MultiReplicaAccountingCheck::NoLostUpdate,
            MultiReplicaAccountingCheck::NoDuplicateCharge,
            MultiReplicaAccountingCheck::NoDroppedLedgerEvent,
            MultiReplicaAccountingCheck::NoRequestIdCollision,
            MultiReplicaAccountingCheck::NoLimitOvershoot,
        ],
        documented_limit_overshoot_tolerance: true,
    }
}

#[test]
fn application_runtime_accounting_verification_accepts_complete_evidence() {
    let topology = ApplicationRuntimeTopology {
        storage: StorageTopology {
            durable_store: DurableStoreKind::Postgres,
            cache_store: Some(CacheStoreKind::Redis),
            migration_policy: MigrationRuntimePolicy::RequestPathForbidden,
        },
        migrations: ApplicationMigrationMode::ExternalOnly,
        gateway_replica_count: 2,
        require_multi_replica_accounting_checks: true,
        redis_rate_limit: None,
        redis_cache: vec![],
        redis_coordination: vec![],
    };
    let evidence_topology = topology.storage;
    let runtime_plan = plan_application_runtime(topology).unwrap();

    let verification = plan_application_runtime_accounting_verification(
        ApplicationRuntimeAccountingVerificationRequest {
            runtime_plan: &runtime_plan,
            evidence: complete_multi_replica_accounting_evidence(evidence_topology, 2),
        },
    )
    .unwrap();

    assert_eq!(verification.verification.topology, evidence_topology);
    assert_eq!(verification.verification.gateway_replica_count, 2);
    assert_eq!(
        verification.verification.verified_checks,
        runtime_plan.accounting_concurrency.unwrap().checks
    );
}

#[test]
fn application_runtime_accounting_verification_rejects_missing_or_weak_evidence() {
    let optional_topology = ApplicationRuntimeTopology {
        storage: StorageTopology {
            durable_store: DurableStoreKind::Postgres,
            cache_store: Some(CacheStoreKind::Redis),
            migration_policy: MigrationRuntimePolicy::RequestPathForbidden,
        },
        migrations: ApplicationMigrationMode::ExternalOnly,
        gateway_replica_count: 1,
        require_multi_replica_accounting_checks: false,
        redis_rate_limit: None,
        redis_cache: vec![],
        redis_coordination: vec![],
    };
    let optional_plan = plan_application_runtime(optional_topology.clone()).unwrap();
    let not_required = plan_application_runtime_accounting_verification(
        ApplicationRuntimeAccountingVerificationRequest {
            runtime_plan: &optional_plan,
            evidence: complete_multi_replica_accounting_evidence(optional_topology.storage, 1),
        },
    )
    .unwrap_err();
    let not_required_response =
        plan_application_runtime_accounting_verification_error_response(&not_required);

    assert_eq!(
        not_required,
        ApplicationRuntimeAccountingVerificationError::AccountingNotRequired
    );
    assert_eq!(
        not_required_response.status,
        ApplicationRuntimePlanErrorStatus::InvalidConfiguration
    );
    assert_eq!(
        not_required_response.code,
        "runtime_accounting_verification_invalid"
    );
    assert_eq!(
        not_required_response.message,
        "runtime accounting verification is invalid"
    );

    let required_topology = ApplicationRuntimeTopology {
        require_multi_replica_accounting_checks: true,
        gateway_replica_count: 2,
        ..optional_topology
    };
    let required_storage = required_topology.storage;
    let required_plan = plan_application_runtime(required_topology).unwrap();
    let weak_evidence = plan_application_runtime_accounting_verification(
        ApplicationRuntimeAccountingVerificationRequest {
            runtime_plan: &required_plan,
            evidence: complete_multi_replica_accounting_evidence(required_storage, 1),
        },
    )
    .unwrap_err();
    let weak_response =
        plan_application_runtime_accounting_verification_error_response(&weak_evidence);

    assert!(matches!(
        weak_evidence,
        ApplicationRuntimeAccountingVerificationError::AccountingConcurrency(_)
    ));
    assert_eq!(
        weak_response.status,
        ApplicationRuntimePlanErrorStatus::InvalidConfiguration
    );
    assert_eq!(
        weak_response.code,
        "runtime_accounting_verification_invalid"
    );
    assert_eq!(
        weak_response.message,
        "runtime accounting verification is invalid"
    );
    assert!(!weak_response.message.contains("replica"));
    assert!(!weak_response.message.contains("PostgreSQL"));
    assert!(!weak_response.message.contains("Redis"));
}

#[test]
fn application_runtime_accounting_verification_required_response_is_stable() {
    let response = plan_application_runtime_accounting_verification_required_response();
    assert_eq!(
        response.status,
        ApplicationRuntimePlanErrorStatus::InvalidConfiguration
    );
    assert_eq!(response.code, "runtime_accounting_verification_invalid");
    assert_eq!(
        response.message,
        "runtime accounting verification is invalid"
    );
}

#[test]
fn application_runtime_rejects_request_path_migration_mode() {
    let topology = ApplicationRuntimeTopology {
        storage: StorageTopology {
            durable_store: DurableStoreKind::Sqlite,
            cache_store: None,
            migration_policy: MigrationRuntimePolicy::RequestPathForbidden,
        },
        migrations: ApplicationMigrationMode::RequestPath,
        gateway_replica_count: 1,
        require_multi_replica_accounting_checks: false,
        redis_rate_limit: None,
        redis_cache: vec![],
        redis_coordination: vec![],
    };

    assert!(plan_application_runtime(topology).is_err());
}

#[test]
fn application_runtime_rejects_invalid_multi_replica_accounting_topology() {
    let topology = ApplicationRuntimeTopology {
        storage: StorageTopology {
            durable_store: DurableStoreKind::Postgres,
            cache_store: Some(CacheStoreKind::Redis),
            migration_policy: MigrationRuntimePolicy::RequestPathForbidden,
        },
        migrations: ApplicationMigrationMode::ExternalOnly,
        gateway_replica_count: 1,
        require_multi_replica_accounting_checks: true,
        redis_rate_limit: None,
        redis_cache: vec![],
        redis_coordination: vec![],
    };

    assert!(matches!(
        plan_application_runtime(topology),
        Err(ApplicationRuntimePlanError::AccountingConcurrency(_))
    ));
}

#[test]
fn application_runtime_error_responses_are_stable_and_redacted() {
    let request_path_error = ApplicationRuntimePlanError::MigrationOnRequestPath;
    let request_path_response = plan_application_runtime_error_response(&request_path_error);
    assert_eq!(
        request_path_response.status,
        ApplicationRuntimePlanErrorStatus::InvalidConfiguration
    );
    assert_eq!(request_path_response.code, "runtime_topology_invalid");
    assert_eq!(
        request_path_response.message,
        "runtime topology configuration is invalid"
    );
    assert!(!request_path_response.message.contains("request"));
    assert!(!request_path_response.message.contains("migration"));

    let accounting_error = ApplicationRuntimePlanError::AccountingConcurrency(
        prodex_storage::MultiReplicaAccountingConcurrencySpecError::RequiresAtLeastTwoGatewayReplicas {
            actual: 1,
        },
    );
    let accounting_response = plan_application_runtime_error_response(&accounting_error);
    assert_eq!(
        accounting_response.status,
        ApplicationRuntimePlanErrorStatus::InvalidConfiguration
    );
    assert_eq!(accounting_response.code, "runtime_topology_invalid");
    assert!(!accounting_response.message.contains("1"));
    assert!(!accounting_response.message.contains("replica"));
    assert!(!accounting_response.message.contains("PostgreSQL"));
    assert!(!accounting_response.message.contains("Redis"));

    let storage_error = ApplicationRuntimePlanError::Postgres(
        prodex_storage_postgres::PostgresStoragePlanError::DdlForbiddenOnRequestPath,
    );
    let storage_response = plan_application_runtime_error_response(&storage_error);
    assert_eq!(
        storage_response.status,
        ApplicationRuntimePlanErrorStatus::ServiceUnavailable
    );
    assert_eq!(storage_response.code, "runtime_storage_plan_unavailable");
    assert_eq!(
        storage_response.message,
        "runtime storage planning is temporarily unavailable"
    );
    assert!(!storage_response.message.contains("PostgreSQL"));
    assert!(!storage_response.message.contains("DDL"));
}
