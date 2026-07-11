//! Service identity lifecycle planning.

use super::super::*;
use crate::control_plane::control_plane_audit_write;

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationServiceIdentityCreateRequest {
    pub durable_store: DurableStoreKind,
    pub identity: ServiceIdentityCreateCommand,
}

impl fmt::Debug for ApplicationServiceIdentityCreateRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationServiceIdentityCreateRequest")
            .field("durable_store", &self.durable_store)
            .field("identity", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationServiceIdentityCreateStoragePlan {
    Postgres(PostgresServiceIdentityCreateSqlPlan),
    Sqlite(SqliteServiceIdentityCreateSqlPlan),
}
#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationServiceIdentityCreatePlan {
    pub storage: ApplicationServiceIdentityCreateStoragePlan,
}

impl fmt::Debug for ApplicationServiceIdentityCreatePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationServiceIdentityCreatePlan")
            .field("storage", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationServiceIdentityCreateError {
    Postgres(prodex_storage_postgres::PostgresStoragePlanError),
    Sqlite(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Debug for ApplicationServiceIdentityCreateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(_) => f.debug_tuple("Postgres").field(&"<redacted>").finish(),
            Self::Sqlite(_) => f.debug_tuple("Sqlite").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationServiceIdentityCreateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(error) => error.fmt(f),
            Self::Sqlite(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationServiceIdentityCreateError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationServiceIdentityCreateErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationServiceIdentityCreateErrorResponsePlan {
    pub status: ApplicationServiceIdentityCreateErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_service_identity_create_error_response(
    error: &ApplicationServiceIdentityCreateError,
) -> ApplicationServiceIdentityCreateErrorResponsePlan {
    match error {
        ApplicationServiceIdentityCreateError::Postgres(error) => {
            application_service_identity_create_response_from_postgres(
                plan_postgres_storage_error_response(error),
            )
        }
        ApplicationServiceIdentityCreateError::Sqlite(error) => {
            application_service_identity_create_response_from_sqlite(
                plan_sqlite_storage_error_response(error),
            )
        }
    }
}

fn application_service_identity_create_response_from_postgres(
    response: PostgresStorageErrorResponsePlan,
) -> ApplicationServiceIdentityCreateErrorResponsePlan {
    ApplicationServiceIdentityCreateErrorResponsePlan {
        status: match response.status {
            PostgresStorageErrorStatus::ServiceUnavailable => {
                ApplicationServiceIdentityCreateErrorStatus::ServiceUnavailable
            }
        },
        code: "service_identity_storage_unavailable",
        message: "service identity storage is temporarily unavailable",
    }
}

fn application_service_identity_create_response_from_sqlite(
    response: SqliteStorageErrorResponsePlan,
) -> ApplicationServiceIdentityCreateErrorResponsePlan {
    ApplicationServiceIdentityCreateErrorResponsePlan {
        status: match response.status {
            SqliteStorageErrorStatus::ServiceUnavailable => {
                ApplicationServiceIdentityCreateErrorStatus::ServiceUnavailable
            }
        },
        code: "service_identity_storage_unavailable",
        message: "service identity storage is temporarily unavailable",
    }
}

pub fn plan_application_service_identity_create(
    request: ApplicationServiceIdentityCreateRequest,
) -> Result<ApplicationServiceIdentityCreatePlan, ApplicationServiceIdentityCreateError> {
    let storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationServiceIdentityCreateStoragePlan::Postgres(
            plan_postgres_service_identity_create(request.identity)
                .map_err(ApplicationServiceIdentityCreateError::Postgres)?,
        ),
        DurableStoreKind::Sqlite => ApplicationServiceIdentityCreateStoragePlan::Sqlite(
            plan_sqlite_service_identity_create(request.identity)
                .map_err(ApplicationServiceIdentityCreateError::Sqlite)?,
        ),
    };
    Ok(ApplicationServiceIdentityCreatePlan { storage })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationServiceIdentityLifecycleRequest {
    pub durable_store: DurableStoreKind,
    pub action: ControlPlaneActionRequest,
    pub identity: ServiceIdentityCreateCommand,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

impl fmt::Debug for ApplicationServiceIdentityLifecycleRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationServiceIdentityLifecycleRequest")
            .field("durable_store", &self.durable_store)
            .field("action", &"<redacted>")
            .field("identity", &"<redacted>")
            .field(
                "previous_digest",
                &self.previous_digest.as_ref().map(|_| "<redacted>"),
            )
            .field("event_digest", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationServiceIdentityLifecyclePlan {
    pub decision: ControlPlaneDecision,
    pub audit_storage: ApplicationControlPlaneAuditStoragePlan,
    pub identity_storage: Option<ApplicationServiceIdentityCreateStoragePlan>,
}

impl fmt::Debug for ApplicationServiceIdentityLifecyclePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationServiceIdentityLifecyclePlan")
            .field("decision", &"<redacted>")
            .field("audit_storage", &"<redacted>")
            .field(
                "identity_storage",
                &self.identity_storage.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationServiceIdentityLifecycleError {
    WrongOperation(ControlPlaneOperation),
    WrongResourceKind(ResourceKind),
    TenantMismatch {
        action_tenant: TenantId,
        identity_tenant: TenantId,
    },
    Audit(ApplicationControlPlaneAuditError),
    ServiceIdentityCreate(ApplicationServiceIdentityCreateError),
}

impl fmt::Debug for ApplicationServiceIdentityLifecycleError {
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
                .field("identity_tenant", &"<redacted>")
                .finish(),
            Self::Audit(_) => f.debug_tuple("Audit").field(&"<redacted>").finish(),
            Self::ServiceIdentityCreate(_) => f
                .debug_tuple("ServiceIdentityCreate")
                .field(&"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for ApplicationServiceIdentityLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => {
                write!(
                    f,
                    "control-plane operation is not service identity creation"
                )
            }
            Self::WrongResourceKind(_) => {
                write!(f, "control-plane resource kind is not service identity")
            }
            Self::TenantMismatch { .. } => {
                write!(f, "service identity lifecycle request is invalid")
            }
            Self::Audit(error) => error.fmt(f),
            Self::ServiceIdentityCreate(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationServiceIdentityLifecycleError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationServiceIdentityLifecycleErrorStatus {
    BadRequest,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationServiceIdentityLifecycleErrorResponsePlan {
    pub status: ApplicationServiceIdentityLifecycleErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_service_identity_lifecycle_error_response(
    error: &ApplicationServiceIdentityLifecycleError,
) -> ApplicationServiceIdentityLifecycleErrorResponsePlan {
    match error {
        ApplicationServiceIdentityLifecycleError::WrongOperation(_)
        | ApplicationServiceIdentityLifecycleError::WrongResourceKind(_)
        | ApplicationServiceIdentityLifecycleError::TenantMismatch { .. } => {
            ApplicationServiceIdentityLifecycleErrorResponsePlan {
                status: ApplicationServiceIdentityLifecycleErrorStatus::BadRequest,
                code: "service_identity_lifecycle_invalid",
                message: "service identity lifecycle request is invalid",
            }
        }
        ApplicationServiceIdentityLifecycleError::Audit(error) => {
            let response = plan_application_control_plane_audit_error_response(error);
            ApplicationServiceIdentityLifecycleErrorResponsePlan {
                status: match response.status {
                    ApplicationControlPlaneAuditErrorStatus::BadRequest
                    | ApplicationControlPlaneAuditErrorStatus::MethodNotAllowed => {
                        ApplicationServiceIdentityLifecycleErrorStatus::BadRequest
                    }
                    ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable => {
                        ApplicationServiceIdentityLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "service_identity_lifecycle_audit_unavailable",
                message: "service identity lifecycle audit is temporarily unavailable",
            }
        }
        ApplicationServiceIdentityLifecycleError::ServiceIdentityCreate(error) => {
            let response = plan_application_service_identity_create_error_response(error);
            ApplicationServiceIdentityLifecycleErrorResponsePlan {
                status: match response.status {
                    ApplicationServiceIdentityCreateErrorStatus::ServiceUnavailable => {
                        ApplicationServiceIdentityLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "service_identity_lifecycle_storage_unavailable",
                message: "service identity lifecycle storage is temporarily unavailable",
            }
        }
    }
}

pub fn plan_application_service_identity_lifecycle(
    request: ApplicationServiceIdentityLifecycleRequest,
) -> Result<ApplicationServiceIdentityLifecyclePlan, ApplicationServiceIdentityLifecycleError> {
    if request.action.operation != ControlPlaneOperation::ServiceIdentityCreate {
        return Err(ApplicationServiceIdentityLifecycleError::WrongOperation(
            request.action.operation,
        ));
    }
    if request.action.resource.kind != ResourceKind::ServiceIdentity {
        return Err(ApplicationServiceIdentityLifecycleError::WrongResourceKind(
            request.action.resource.kind,
        ));
    }
    if request.action.resource.tenant_id != request.identity.tenant_id {
        return Err(ApplicationServiceIdentityLifecycleError::TenantMismatch {
            action_tenant: request.action.resource.tenant_id,
            identity_tenant: request.identity.tenant_id,
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
                .map_err(ApplicationServiceIdentityLifecycleError::Audit)?,
        ),
        DurableStoreKind::Sqlite => ApplicationControlPlaneAuditStoragePlan::Sqlite(
            plan_sqlite_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Sqlite)
                .map_err(ApplicationServiceIdentityLifecycleError::Audit)?,
        ),
    };
    let identity_storage = if matches!(decision, ControlPlaneDecision::Authorized(_)) {
        let identity_plan =
            plan_application_service_identity_create(ApplicationServiceIdentityCreateRequest {
                durable_store: request.durable_store,
                identity: request.identity,
            })
            .map_err(ApplicationServiceIdentityLifecycleError::ServiceIdentityCreate)?;
        Some(identity_plan.storage)
    } else {
        None
    };

    Ok(ApplicationServiceIdentityLifecyclePlan {
        decision,
        audit_storage,
        identity_storage,
    })
}
