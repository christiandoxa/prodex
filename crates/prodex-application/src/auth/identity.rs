//! Tenant and user identity lifecycle planning.

use super::super::*;
use crate::control_plane::control_plane_audit_write;

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationTenantLifecycleRequest {
    pub durable_store: DurableStoreKind,
    pub action: ControlPlaneActionRequest,
    pub tenant: TenantLifecycleCommand,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

impl fmt::Debug for ApplicationTenantLifecycleRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationTenantLifecycleRequest")
            .field("durable_store", &self.durable_store)
            .field("action", &"<redacted>")
            .field("tenant", &"<redacted>")
            .field(
                "previous_digest",
                &self.previous_digest.as_ref().map(|_| "<redacted>"),
            )
            .field("event_digest", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationTenantLifecycleStoragePlan {
    Postgres(PostgresTenantLifecycleSqlPlan),
    Sqlite(SqliteTenantLifecycleSqlPlan),
}
#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationTenantLifecyclePlan {
    pub decision: ControlPlaneDecision,
    pub audit_storage: ApplicationControlPlaneAuditStoragePlan,
    pub tenant_storage: Option<ApplicationTenantLifecycleStoragePlan>,
}

impl fmt::Debug for ApplicationTenantLifecyclePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationTenantLifecyclePlan")
            .field("decision", &"<redacted>")
            .field("audit_storage", &"<redacted>")
            .field(
                "tenant_storage",
                &self.tenant_storage.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationTenantLifecycleError {
    WrongOperation(ControlPlaneOperation),
    WrongResourceKind(ResourceKind),
    TenantMismatch {
        action_tenant: TenantId,
        command_tenant: TenantId,
    },
    KindMismatch {
        operation: ControlPlaneOperation,
        kind: TenantLifecycleKind,
    },
    Audit(ApplicationControlPlaneAuditError),
    Storage(prodex_storage_postgres::PostgresStoragePlanError),
    SqliteStorage(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Debug for ApplicationTenantLifecycleError {
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
                .field("command_tenant", &"<redacted>")
                .finish(),
            Self::KindMismatch { .. } => f
                .debug_struct("KindMismatch")
                .field("operation", &"<redacted>")
                .field("kind", &"<redacted>")
                .finish(),
            Self::Audit(_) => f.debug_tuple("Audit").field(&"<redacted>").finish(),
            Self::Storage(_) => f.debug_tuple("Storage").field(&"<redacted>").finish(),
            Self::SqliteStorage(_) => f.debug_tuple("SqliteStorage").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationTenantLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => {
                write!(f, "control-plane operation is not tenant lifecycle")
            }
            Self::WrongResourceKind(_) => {
                write!(f, "control-plane resource kind is not tenant")
            }
            Self::TenantMismatch { .. } => write!(f, "tenant lifecycle request is invalid"),
            Self::KindMismatch { .. } => {
                write!(f, "tenant lifecycle operation does not match command kind")
            }
            Self::Audit(error) => error.fmt(f),
            Self::Storage(error) => error.fmt(f),
            Self::SqliteStorage(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationTenantLifecycleError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationTenantLifecycleErrorStatus {
    BadRequest,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationTenantLifecycleErrorResponsePlan {
    pub status: ApplicationTenantLifecycleErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_tenant_lifecycle_error_response(
    error: &ApplicationTenantLifecycleError,
) -> ApplicationTenantLifecycleErrorResponsePlan {
    match error {
        ApplicationTenantLifecycleError::WrongOperation(_)
        | ApplicationTenantLifecycleError::WrongResourceKind(_)
        | ApplicationTenantLifecycleError::TenantMismatch { .. }
        | ApplicationTenantLifecycleError::KindMismatch { .. } => {
            ApplicationTenantLifecycleErrorResponsePlan {
                status: ApplicationTenantLifecycleErrorStatus::BadRequest,
                code: "tenant_lifecycle_invalid",
                message: "tenant lifecycle request is invalid",
            }
        }
        ApplicationTenantLifecycleError::Audit(error) => {
            let response = plan_application_control_plane_audit_error_response(error);
            ApplicationTenantLifecycleErrorResponsePlan {
                status: match response.status {
                    ApplicationControlPlaneAuditErrorStatus::BadRequest
                    | ApplicationControlPlaneAuditErrorStatus::MethodNotAllowed => {
                        ApplicationTenantLifecycleErrorStatus::BadRequest
                    }
                    ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable => {
                        ApplicationTenantLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "tenant_lifecycle_audit_unavailable",
                message: "tenant lifecycle audit is temporarily unavailable",
            }
        }
        ApplicationTenantLifecycleError::Storage(error) => {
            let response = plan_postgres_storage_error_response(error);
            ApplicationTenantLifecycleErrorResponsePlan {
                status: match response.status {
                    PostgresStorageErrorStatus::ServiceUnavailable => {
                        ApplicationTenantLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "tenant_lifecycle_storage_unavailable",
                message: "tenant lifecycle storage is temporarily unavailable",
            }
        }
        ApplicationTenantLifecycleError::SqliteStorage(error) => {
            let response = plan_sqlite_storage_error_response(error);
            ApplicationTenantLifecycleErrorResponsePlan {
                status: match response.status {
                    SqliteStorageErrorStatus::ServiceUnavailable => {
                        ApplicationTenantLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "tenant_lifecycle_storage_unavailable",
                message: "tenant lifecycle storage is temporarily unavailable",
            }
        }
    }
}

pub fn plan_application_tenant_lifecycle(
    request: ApplicationTenantLifecycleRequest,
) -> Result<ApplicationTenantLifecyclePlan, ApplicationTenantLifecycleError> {
    if !matches!(
        request.action.operation,
        ControlPlaneOperation::TenantCreate | ControlPlaneOperation::TenantUpdate
    ) {
        return Err(ApplicationTenantLifecycleError::WrongOperation(
            request.action.operation,
        ));
    }
    if request.action.resource.kind != ResourceKind::Tenant {
        return Err(ApplicationTenantLifecycleError::WrongResourceKind(
            request.action.resource.kind,
        ));
    }
    if request.action.resource.tenant_id != request.tenant.tenant_id {
        return Err(ApplicationTenantLifecycleError::TenantMismatch {
            action_tenant: request.action.resource.tenant_id,
            command_tenant: request.tenant.tenant_id,
        });
    }
    let expected_kind = match request.action.operation {
        ControlPlaneOperation::TenantCreate => TenantLifecycleKind::Create,
        ControlPlaneOperation::TenantUpdate => TenantLifecycleKind::Update,
        _ => unreachable!("tenant lifecycle operation was already validated"),
    };
    if request.tenant.kind != expected_kind {
        return Err(ApplicationTenantLifecycleError::KindMismatch {
            operation: request.action.operation,
            kind: request.tenant.kind,
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
                .map_err(ApplicationTenantLifecycleError::Audit)?,
        ),
        DurableStoreKind::Sqlite => ApplicationControlPlaneAuditStoragePlan::Sqlite(
            plan_sqlite_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Sqlite)
                .map_err(ApplicationTenantLifecycleError::Audit)?,
        ),
    };
    let tenant_storage = if matches!(decision, ControlPlaneDecision::Authorized(_)) {
        Some(match request.durable_store {
            DurableStoreKind::Postgres => ApplicationTenantLifecycleStoragePlan::Postgres(
                plan_postgres_tenant_lifecycle(request.tenant)
                    .map_err(ApplicationTenantLifecycleError::Storage)?,
            ),
            DurableStoreKind::Sqlite => ApplicationTenantLifecycleStoragePlan::Sqlite(
                plan_sqlite_tenant_lifecycle(request.tenant)
                    .map_err(ApplicationTenantLifecycleError::SqliteStorage)?,
            ),
        })
    } else {
        None
    };

    Ok(ApplicationTenantLifecyclePlan {
        decision,
        audit_storage,
        tenant_storage,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationUserLifecycleRequest {
    pub durable_store: DurableStoreKind,
    pub action: ControlPlaneActionRequest,
    pub user: UserLifecycleCommand,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

impl fmt::Debug for ApplicationUserLifecycleRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationUserLifecycleRequest")
            .field("durable_store", &self.durable_store)
            .field("action", &"<redacted>")
            .field("user", &"<redacted>")
            .field(
                "previous_digest",
                &self.previous_digest.as_ref().map(|_| "<redacted>"),
            )
            .field("event_digest", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationUserLifecycleStoragePlan {
    Postgres(PostgresUserLifecycleSqlPlan),
    Sqlite(SqliteUserLifecycleSqlPlan),
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationUserLifecyclePlan {
    pub decision: ControlPlaneDecision,
    pub audit_storage: ApplicationControlPlaneAuditStoragePlan,
    pub user_storage: Option<ApplicationUserLifecycleStoragePlan>,
}

impl fmt::Debug for ApplicationUserLifecyclePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationUserLifecyclePlan")
            .field("decision", &"<redacted>")
            .field("audit_storage", &"<redacted>")
            .field(
                "user_storage",
                &self.user_storage.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationUserLifecycleError {
    WrongOperation(ControlPlaneOperation),
    WrongResourceKind(ResourceKind),
    TenantMismatch {
        action_tenant: TenantId,
        command_tenant: TenantId,
    },
    KindMismatch {
        operation: ControlPlaneOperation,
        kind: UserLifecycleKind,
    },
    Audit(ApplicationControlPlaneAuditError),
    Storage(prodex_storage_postgres::PostgresStoragePlanError),
    SqliteStorage(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Debug for ApplicationUserLifecycleError {
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
                .field("command_tenant", &"<redacted>")
                .finish(),
            Self::KindMismatch { .. } => f
                .debug_struct("KindMismatch")
                .field("operation", &"<redacted>")
                .field("kind", &"<redacted>")
                .finish(),
            Self::Audit(_) => f.debug_tuple("Audit").field(&"<redacted>").finish(),
            Self::Storage(_) => f.debug_tuple("Storage").field(&"<redacted>").finish(),
            Self::SqliteStorage(_) => f.debug_tuple("SqliteStorage").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationUserLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => {
                write!(f, "control-plane operation is not user lifecycle")
            }
            Self::WrongResourceKind(_) => {
                write!(f, "control-plane resource kind is not user")
            }
            Self::TenantMismatch { .. } => write!(f, "user lifecycle request is invalid"),
            Self::KindMismatch { .. } => {
                write!(f, "user lifecycle operation does not match command kind")
            }
            Self::Audit(error) => error.fmt(f),
            Self::Storage(error) => error.fmt(f),
            Self::SqliteStorage(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationUserLifecycleError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationUserLifecycleErrorStatus {
    BadRequest,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationUserLifecycleErrorResponsePlan {
    pub status: ApplicationUserLifecycleErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_user_lifecycle_error_response(
    error: &ApplicationUserLifecycleError,
) -> ApplicationUserLifecycleErrorResponsePlan {
    match error {
        ApplicationUserLifecycleError::WrongOperation(_)
        | ApplicationUserLifecycleError::WrongResourceKind(_)
        | ApplicationUserLifecycleError::TenantMismatch { .. }
        | ApplicationUserLifecycleError::KindMismatch { .. } => {
            ApplicationUserLifecycleErrorResponsePlan {
                status: ApplicationUserLifecycleErrorStatus::BadRequest,
                code: "user_lifecycle_invalid",
                message: "user lifecycle request is invalid",
            }
        }
        ApplicationUserLifecycleError::Audit(error) => {
            let response = plan_application_control_plane_audit_error_response(error);
            ApplicationUserLifecycleErrorResponsePlan {
                status: match response.status {
                    ApplicationControlPlaneAuditErrorStatus::BadRequest
                    | ApplicationControlPlaneAuditErrorStatus::MethodNotAllowed => {
                        ApplicationUserLifecycleErrorStatus::BadRequest
                    }
                    ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable => {
                        ApplicationUserLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "user_lifecycle_audit_unavailable",
                message: "user lifecycle audit is temporarily unavailable",
            }
        }
        ApplicationUserLifecycleError::Storage(error) => {
            let response = plan_postgres_storage_error_response(error);
            ApplicationUserLifecycleErrorResponsePlan {
                status: match response.status {
                    PostgresStorageErrorStatus::ServiceUnavailable => {
                        ApplicationUserLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "user_lifecycle_storage_unavailable",
                message: "user lifecycle storage is temporarily unavailable",
            }
        }
        ApplicationUserLifecycleError::SqliteStorage(error) => {
            let response = plan_sqlite_storage_error_response(error);
            ApplicationUserLifecycleErrorResponsePlan {
                status: match response.status {
                    SqliteStorageErrorStatus::ServiceUnavailable => {
                        ApplicationUserLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "user_lifecycle_storage_unavailable",
                message: "user lifecycle storage is temporarily unavailable",
            }
        }
    }
}

pub fn plan_application_user_lifecycle(
    request: ApplicationUserLifecycleRequest,
) -> Result<ApplicationUserLifecyclePlan, ApplicationUserLifecycleError> {
    if !matches!(
        request.action.operation,
        ControlPlaneOperation::UserInvite
            | ControlPlaneOperation::ScimUserCreate
            | ControlPlaneOperation::ScimUserUpdate
            | ControlPlaneOperation::ScimUserDelete
    ) {
        return Err(ApplicationUserLifecycleError::WrongOperation(
            request.action.operation,
        ));
    }
    if request.action.resource.kind != ResourceKind::User {
        return Err(ApplicationUserLifecycleError::WrongResourceKind(
            request.action.resource.kind,
        ));
    }
    if request.action.resource.tenant_id != request.user.tenant_id {
        return Err(ApplicationUserLifecycleError::TenantMismatch {
            action_tenant: request.action.resource.tenant_id,
            command_tenant: request.user.tenant_id,
        });
    }
    let expected_kind = match request.action.operation {
        ControlPlaneOperation::UserInvite | ControlPlaneOperation::ScimUserCreate => {
            UserLifecycleKind::Create
        }
        ControlPlaneOperation::ScimUserUpdate => UserLifecycleKind::Update,
        ControlPlaneOperation::ScimUserDelete => UserLifecycleKind::Delete,
        _ => unreachable!("user lifecycle operation was already validated"),
    };
    if request.user.kind != expected_kind {
        return Err(ApplicationUserLifecycleError::KindMismatch {
            operation: request.action.operation,
            kind: request.user.kind,
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
                .map_err(ApplicationUserLifecycleError::Audit)?,
        ),
        DurableStoreKind::Sqlite => ApplicationControlPlaneAuditStoragePlan::Sqlite(
            plan_sqlite_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Sqlite)
                .map_err(ApplicationUserLifecycleError::Audit)?,
        ),
    };
    let user_storage = if matches!(decision, ControlPlaneDecision::Authorized(_)) {
        Some(match request.durable_store {
            DurableStoreKind::Postgres => ApplicationUserLifecycleStoragePlan::Postgres(
                plan_postgres_user_lifecycle(request.user)
                    .map_err(ApplicationUserLifecycleError::Storage)?,
            ),
            DurableStoreKind::Sqlite => ApplicationUserLifecycleStoragePlan::Sqlite(
                plan_sqlite_user_lifecycle(request.user)
                    .map_err(ApplicationUserLifecycleError::SqliteStorage)?,
            ),
        })
    } else {
        None
    };

    Ok(ApplicationUserLifecyclePlan {
        decision,
        audit_storage,
        user_storage,
    })
}
