//! Provider capability, resilience, and credential lifecycle plans.

use super::*;
use crate::control_plane::control_plane_audit_write;

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationProviderCapabilityRequest {
    pub request: CapabilityRequest,
    pub candidates: Vec<ProviderRouteCapabilityCandidate>,
}

impl fmt::Debug for ApplicationProviderCapabilityRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationProviderCapabilityRequest")
            .field("request", &"<redacted>")
            .field("candidates", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationProviderCapabilityPlan {
    pub provider: ProviderCapabilityNegotiationPlan,
}

impl fmt::Debug for ApplicationProviderCapabilityPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationProviderCapabilityPlan")
            .field("provider", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationProviderCapabilityError {
    Incompatible(ProviderCapabilityNegotiationDecision),
}

impl fmt::Debug for ApplicationProviderCapabilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Incompatible(_) => f.debug_tuple("Incompatible").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationProviderCapabilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Incompatible(_) => write!(f, "provider capability negotiation failed"),
        }
    }
}

impl Error for ApplicationProviderCapabilityError {}

pub fn plan_application_provider_capability_error_response(
    error: &ApplicationProviderCapabilityError,
) -> CapabilityErrorResponsePlan {
    match error {
        ApplicationProviderCapabilityError::Incompatible(decision) => {
            plan_provider_capability_negotiation_error_response(decision).unwrap_or(
                CapabilityErrorResponsePlan {
                    status: prodex_domain::CapabilityErrorStatus::ServiceUnavailable,
                    code: "model_route_unavailable",
                    message: "model route is temporarily unavailable",
                },
            )
        }
    }
}

pub fn plan_application_provider_capability(
    request: ApplicationProviderCapabilityRequest,
) -> Result<ApplicationProviderCapabilityPlan, ApplicationProviderCapabilityError> {
    match negotiate_provider_route_capability(&request.request, &request.candidates) {
        ProviderCapabilityNegotiationDecision::Compatible(provider) => {
            Ok(ApplicationProviderCapabilityPlan { provider })
        }
        decision => Err(ApplicationProviderCapabilityError::Incompatible(decision)),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationProviderRetryRequest {
    pub policy: ProviderRetryPolicy,
    pub stage: ProviderRetryStage,
    pub attempted_precommit_retries: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationProviderRetryPlan {
    pub retry: ProviderRetryPlan,
}

pub fn plan_application_provider_retry(
    request: ApplicationProviderRetryRequest,
) -> ApplicationProviderRetryPlan {
    ApplicationProviderRetryPlan {
        retry: plan_provider_retry(
            request.policy,
            request.stage,
            request.attempted_precommit_retries,
        ),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationProviderCircuitBreakerRequest {
    pub policy: ProviderCircuitBreakerPolicy,
    pub state: ProviderCircuitBreakerState,
    pub now_unix_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationProviderCircuitBreakerPlan {
    pub decision: ProviderCircuitBreakerDecision,
}

pub fn plan_application_provider_circuit_breaker(
    request: ApplicationProviderCircuitBreakerRequest,
) -> ApplicationProviderCircuitBreakerPlan {
    ApplicationProviderCircuitBreakerPlan {
        decision: plan_provider_circuit_breaker(request.policy, request.state, request.now_unix_ms),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationProviderCircuitBreakerEventRequest {
    pub policy: ProviderCircuitBreakerPolicy,
    pub state: ProviderCircuitBreakerState,
    pub event: ProviderCircuitBreakerEvent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationProviderCircuitBreakerEventPlan {
    pub event: ProviderCircuitBreakerPlan,
}

pub fn plan_application_provider_circuit_breaker_event(
    request: ApplicationProviderCircuitBreakerEventRequest,
) -> ApplicationProviderCircuitBreakerEventPlan {
    ApplicationProviderCircuitBreakerEventPlan {
        event: plan_provider_circuit_breaker_event(request.policy, request.state, request.event),
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationProviderCredentialReferenceRequest {
    pub durable_store: DurableStoreKind,
    pub reference: ProviderCredentialReferenceCommand,
}

impl fmt::Debug for ApplicationProviderCredentialReferenceRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationProviderCredentialReferenceRequest")
            .field("durable_store", &self.durable_store)
            .field("reference", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationProviderCredentialReferenceStoragePlan {
    Postgres(PostgresProviderCredentialReferenceSqlPlan),
    Sqlite(SqliteProviderCredentialReferenceSqlPlan),
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationProviderCredentialReferencePlan {
    pub storage: ApplicationProviderCredentialReferenceStoragePlan,
}

impl fmt::Debug for ApplicationProviderCredentialReferencePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationProviderCredentialReferencePlan")
            .field("storage", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationProviderCredentialReferenceError {
    Postgres(prodex_storage_postgres::PostgresStoragePlanError),
    Sqlite(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Debug for ApplicationProviderCredentialReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(_) => f.debug_tuple("Postgres").field(&"<redacted>").finish(),
            Self::Sqlite(_) => f.debug_tuple("Sqlite").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationProviderCredentialReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(error) => error.fmt(f),
            Self::Sqlite(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationProviderCredentialReferenceError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationProviderCredentialReferenceErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationProviderCredentialReferenceErrorResponsePlan {
    pub status: ApplicationProviderCredentialReferenceErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_provider_credential_reference_error_response(
    error: &ApplicationProviderCredentialReferenceError,
) -> ApplicationProviderCredentialReferenceErrorResponsePlan {
    match error {
        ApplicationProviderCredentialReferenceError::Postgres(error) => {
            application_provider_credential_reference_response_from_postgres(
                plan_postgres_storage_error_response(error),
            )
        }
        ApplicationProviderCredentialReferenceError::Sqlite(error) => {
            application_provider_credential_reference_response_from_sqlite(
                plan_sqlite_storage_error_response(error),
            )
        }
    }
}

fn application_provider_credential_reference_response_from_postgres(
    response: PostgresStorageErrorResponsePlan,
) -> ApplicationProviderCredentialReferenceErrorResponsePlan {
    ApplicationProviderCredentialReferenceErrorResponsePlan {
        status: match response.status {
            PostgresStorageErrorStatus::ServiceUnavailable => {
                ApplicationProviderCredentialReferenceErrorStatus::ServiceUnavailable
            }
        },
        code: "provider_credential_storage_unavailable",
        message: "provider credential storage is temporarily unavailable",
    }
}

fn application_provider_credential_reference_response_from_sqlite(
    response: SqliteStorageErrorResponsePlan,
) -> ApplicationProviderCredentialReferenceErrorResponsePlan {
    ApplicationProviderCredentialReferenceErrorResponsePlan {
        status: match response.status {
            SqliteStorageErrorStatus::ServiceUnavailable => {
                ApplicationProviderCredentialReferenceErrorStatus::ServiceUnavailable
            }
        },
        code: "provider_credential_storage_unavailable",
        message: "provider credential storage is temporarily unavailable",
    }
}

pub fn plan_application_provider_credential_reference(
    request: ApplicationProviderCredentialReferenceRequest,
) -> Result<ApplicationProviderCredentialReferencePlan, ApplicationProviderCredentialReferenceError>
{
    let storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationProviderCredentialReferenceStoragePlan::Postgres(
            plan_postgres_provider_credential_reference(request.reference)
                .map_err(ApplicationProviderCredentialReferenceError::Postgres)?,
        ),
        DurableStoreKind::Sqlite => ApplicationProviderCredentialReferenceStoragePlan::Sqlite(
            plan_sqlite_provider_credential_reference(request.reference)
                .map_err(ApplicationProviderCredentialReferenceError::Sqlite)?,
        ),
    };
    Ok(ApplicationProviderCredentialReferencePlan { storage })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationProviderCredentialRotationRequest {
    pub durable_store: DurableStoreKind,
    pub action: ControlPlaneActionRequest,
    pub reference: ProviderCredentialReferenceCommand,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

impl fmt::Debug for ApplicationProviderCredentialRotationRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationProviderCredentialRotationRequest")
            .field("durable_store", &self.durable_store)
            .field("action", &"<redacted>")
            .field("reference", &"<redacted>")
            .field(
                "previous_digest",
                &self.previous_digest.as_ref().map(|_| "<redacted>"),
            )
            .field("event_digest", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationProviderCredentialRotationPlan {
    pub decision: ControlPlaneDecision,
    pub audit_storage: ApplicationControlPlaneAuditStoragePlan,
    pub reference_storage: Option<ApplicationProviderCredentialReferenceStoragePlan>,
}

impl fmt::Debug for ApplicationProviderCredentialRotationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationProviderCredentialRotationPlan")
            .field("decision", &"<redacted>")
            .field("audit_storage", &"<redacted>")
            .field(
                "reference_storage",
                &self.reference_storage.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationProviderCredentialRotationError {
    WrongOperation(ControlPlaneOperation),
    WrongResourceKind(ResourceKind),
    TenantMismatch {
        action_tenant: TenantId,
        reference_tenant: TenantId,
    },
    Audit(ApplicationControlPlaneAuditError),
    ProviderCredentialReference(ApplicationProviderCredentialReferenceError),
}

impl fmt::Debug for ApplicationProviderCredentialRotationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => f
                .debug_tuple("WrongOperation")
                .field(&"<redacted>")
                .finish(),
            Self::WrongResourceKind(_) => f
                .debug_tuple("WrongResourceKind")
                .field(&"<redacted>")
                .finish(),
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("action_tenant", &"<redacted>")
                .field("reference_tenant", &"<redacted>")
                .finish(),
            Self::Audit(_) => f.debug_tuple("Audit").field(&"<redacted>").finish(),
            Self::ProviderCredentialReference(_) => f
                .debug_tuple("ProviderCredentialReference")
                .field(&"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for ApplicationProviderCredentialRotationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => {
                write!(
                    f,
                    "control-plane operation is not provider credential rotation"
                )
            }
            Self::WrongResourceKind(_) => {
                write!(f, "control-plane resource kind is not provider credential")
            }
            Self::TenantMismatch { .. } => {
                write!(f, "provider credential rotation request is invalid")
            }
            Self::Audit(error) => error.fmt(f),
            Self::ProviderCredentialReference(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationProviderCredentialRotationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationProviderCredentialRotationErrorStatus {
    BadRequest,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationProviderCredentialRotationErrorResponsePlan {
    pub status: ApplicationProviderCredentialRotationErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_provider_credential_rotation_error_response(
    error: &ApplicationProviderCredentialRotationError,
) -> ApplicationProviderCredentialRotationErrorResponsePlan {
    match error {
        ApplicationProviderCredentialRotationError::WrongOperation(_)
        | ApplicationProviderCredentialRotationError::WrongResourceKind(_)
        | ApplicationProviderCredentialRotationError::TenantMismatch { .. } => {
            ApplicationProviderCredentialRotationErrorResponsePlan {
                status: ApplicationProviderCredentialRotationErrorStatus::BadRequest,
                code: "provider_credential_rotation_invalid",
                message: "provider credential rotation request is invalid",
            }
        }
        ApplicationProviderCredentialRotationError::Audit(error) => {
            let response = plan_application_control_plane_audit_error_response(error);
            ApplicationProviderCredentialRotationErrorResponsePlan {
                status: match response.status {
                    ApplicationControlPlaneAuditErrorStatus::BadRequest
                    | ApplicationControlPlaneAuditErrorStatus::MethodNotAllowed => {
                        ApplicationProviderCredentialRotationErrorStatus::BadRequest
                    }
                    ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable => {
                        ApplicationProviderCredentialRotationErrorStatus::ServiceUnavailable
                    }
                },
                code: "provider_credential_rotation_audit_unavailable",
                message: "provider credential rotation audit is temporarily unavailable",
            }
        }
        ApplicationProviderCredentialRotationError::ProviderCredentialReference(error) => {
            let response = plan_application_provider_credential_reference_error_response(error);
            ApplicationProviderCredentialRotationErrorResponsePlan {
                status: match response.status {
                    ApplicationProviderCredentialReferenceErrorStatus::ServiceUnavailable => {
                        ApplicationProviderCredentialRotationErrorStatus::ServiceUnavailable
                    }
                },
                code: "provider_credential_rotation_storage_unavailable",
                message: "provider credential rotation storage is temporarily unavailable",
            }
        }
    }
}

pub fn plan_application_provider_credential_rotation(
    request: ApplicationProviderCredentialRotationRequest,
) -> Result<ApplicationProviderCredentialRotationPlan, ApplicationProviderCredentialRotationError> {
    if request.action.operation != ControlPlaneOperation::ProviderCredentialRotate {
        return Err(ApplicationProviderCredentialRotationError::WrongOperation(
            request.action.operation,
        ));
    }
    if request.action.resource.kind != ResourceKind::ProviderCredential {
        return Err(
            ApplicationProviderCredentialRotationError::WrongResourceKind(
                request.action.resource.kind,
            ),
        );
    }
    if request.action.resource.tenant_id != request.reference.tenant_id {
        return Err(ApplicationProviderCredentialRotationError::TenantMismatch {
            action_tenant: request.action.resource.tenant_id,
            reference_tenant: request.reference.tenant_id,
        });
    }

    let decision = decide_control_plane_action(request.action);
    let audit_write = control_plane_audit_write(&decision);
    let audit_command = AppendOnlyAuditCommand {
        storage_key: TenantStorageKey::tenant(audit_write.tenant_partition_key),
        event: audit_write.event.clone(),
        previous_digest: request.previous_digest,
        event_digest: request.event_digest,
    };
    let audit_storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationControlPlaneAuditStoragePlan::Postgres(
            plan_postgres_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Postgres)
                .map_err(ApplicationProviderCredentialRotationError::Audit)?,
        ),
        DurableStoreKind::Sqlite => ApplicationControlPlaneAuditStoragePlan::Sqlite(
            plan_sqlite_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Sqlite)
                .map_err(ApplicationProviderCredentialRotationError::Audit)?,
        ),
    };
    let reference_storage = if matches!(decision, ControlPlaneDecision::Authorized(_)) {
        let reference_plan = plan_application_provider_credential_reference(
            ApplicationProviderCredentialReferenceRequest {
                durable_store: request.durable_store,
                reference: request.reference,
            },
        )
        .map_err(ApplicationProviderCredentialRotationError::ProviderCredentialReference)?;
        Some(reference_plan.storage)
    } else {
        None
    };

    Ok(ApplicationProviderCredentialRotationPlan {
        decision,
        audit_storage,
        reference_storage,
    })
}
