//! Role-binding and virtual-key lifecycle planning.

use super::super::*;
use crate::control_plane::control_plane_audit_write;

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationRoleBindingMutationRequest {
    pub durable_store: DurableStoreKind,
    pub mutation: RoleBindingMutationCommand,
}

impl fmt::Debug for ApplicationRoleBindingMutationRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationRoleBindingMutationRequest")
            .field("durable_store", &self.durable_store)
            .field("mutation", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationRoleBindingMutationStoragePlan {
    Postgres(PostgresRoleBindingMutationSqlPlan),
    Sqlite(SqliteRoleBindingMutationSqlPlan),
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationRoleBindingMutationPlan {
    pub storage: ApplicationRoleBindingMutationStoragePlan,
}

impl fmt::Debug for ApplicationRoleBindingMutationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationRoleBindingMutationPlan")
            .field("storage", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationRoleBindingMutationError {
    Postgres(prodex_storage_postgres::PostgresStoragePlanError),
    Sqlite(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Debug for ApplicationRoleBindingMutationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(_) => f.debug_tuple("Postgres").field(&"<redacted>").finish(),
            Self::Sqlite(_) => f.debug_tuple("Sqlite").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationRoleBindingMutationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(error) => error.fmt(f),
            Self::Sqlite(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationRoleBindingMutationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationRoleBindingMutationErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationRoleBindingMutationErrorResponsePlan {
    pub status: ApplicationRoleBindingMutationErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_role_binding_mutation_error_response(
    error: &ApplicationRoleBindingMutationError,
) -> ApplicationRoleBindingMutationErrorResponsePlan {
    match error {
        ApplicationRoleBindingMutationError::Postgres(error) => {
            application_role_binding_mutation_response_from_postgres(
                plan_postgres_storage_error_response(error),
            )
        }
        ApplicationRoleBindingMutationError::Sqlite(error) => {
            application_role_binding_mutation_response_from_sqlite(
                plan_sqlite_storage_error_response(error),
            )
        }
    }
}

fn application_role_binding_mutation_response_from_postgres(
    response: PostgresStorageErrorResponsePlan,
) -> ApplicationRoleBindingMutationErrorResponsePlan {
    ApplicationRoleBindingMutationErrorResponsePlan {
        status: match response.status {
            PostgresStorageErrorStatus::ServiceUnavailable => {
                ApplicationRoleBindingMutationErrorStatus::ServiceUnavailable
            }
        },
        code: "role_binding_storage_unavailable",
        message: "role-binding storage is temporarily unavailable",
    }
}

fn application_role_binding_mutation_response_from_sqlite(
    response: SqliteStorageErrorResponsePlan,
) -> ApplicationRoleBindingMutationErrorResponsePlan {
    ApplicationRoleBindingMutationErrorResponsePlan {
        status: match response.status {
            SqliteStorageErrorStatus::ServiceUnavailable => {
                ApplicationRoleBindingMutationErrorStatus::ServiceUnavailable
            }
        },
        code: "role_binding_storage_unavailable",
        message: "role-binding storage is temporarily unavailable",
    }
}

pub fn plan_application_role_binding_mutation(
    request: ApplicationRoleBindingMutationRequest,
) -> Result<ApplicationRoleBindingMutationPlan, ApplicationRoleBindingMutationError> {
    let storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationRoleBindingMutationStoragePlan::Postgres(
            plan_postgres_role_binding_mutation(request.mutation)
                .map_err(ApplicationRoleBindingMutationError::Postgres)?,
        ),
        DurableStoreKind::Sqlite => ApplicationRoleBindingMutationStoragePlan::Sqlite(
            plan_sqlite_role_binding_mutation(request.mutation)
                .map_err(ApplicationRoleBindingMutationError::Sqlite)?,
        ),
    };
    Ok(ApplicationRoleBindingMutationPlan { storage })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationRoleBindingLifecycleRequest {
    pub durable_store: DurableStoreKind,
    pub action: ControlPlaneActionRequest,
    pub mutation: RoleBindingMutationCommand,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

impl fmt::Debug for ApplicationRoleBindingLifecycleRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationRoleBindingLifecycleRequest")
            .field("durable_store", &self.durable_store)
            .field("action", &"<redacted>")
            .field("mutation", &"<redacted>")
            .field(
                "previous_digest",
                &self.previous_digest.as_ref().map(|_| "<redacted>"),
            )
            .field("event_digest", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationRoleBindingLifecyclePlan {
    pub decision: ControlPlaneDecision,
    pub audit_storage: ApplicationControlPlaneAuditStoragePlan,
    pub mutation_storage: Option<ApplicationRoleBindingMutationStoragePlan>,
}

impl fmt::Debug for ApplicationRoleBindingLifecyclePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationRoleBindingLifecyclePlan")
            .field("decision", &"<redacted>")
            .field("audit_storage", &"<redacted>")
            .field(
                "mutation_storage",
                &self.mutation_storage.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationRoleBindingLifecycleError {
    WrongOperation(ControlPlaneOperation),
    WrongResourceKind(ResourceKind),
    TenantMismatch {
        action_tenant: TenantId,
        mutation_tenant: TenantId,
    },
    MutationKindMismatch {
        operation: ControlPlaneOperation,
        mutation_kind: RoleBindingMutationKind,
    },
    Audit(ApplicationControlPlaneAuditError),
    RoleBindingMutation(ApplicationRoleBindingMutationError),
}

impl fmt::Debug for ApplicationRoleBindingLifecycleError {
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
                .field("mutation_tenant", &"<redacted>")
                .finish(),
            Self::MutationKindMismatch { .. } => f
                .debug_struct("MutationKindMismatch")
                .field("operation", &"<redacted>")
                .field("mutation_kind", &"<redacted>")
                .finish(),
            Self::Audit(_) => f.debug_tuple("Audit").field(&"<redacted>").finish(),
            Self::RoleBindingMutation(_) => f
                .debug_tuple("RoleBindingMutation")
                .field(&"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for ApplicationRoleBindingLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => {
                write!(
                    f,
                    "control-plane operation is not a role-binding lifecycle operation"
                )
            }
            Self::WrongResourceKind(_) => {
                write!(f, "control-plane resource kind is not role binding")
            }
            Self::TenantMismatch { .. } => {
                write!(f, "role-binding lifecycle request is invalid")
            }
            Self::MutationKindMismatch { .. } => {
                write!(
                    f,
                    "role-binding lifecycle operation does not match mutation kind"
                )
            }
            Self::Audit(error) => error.fmt(f),
            Self::RoleBindingMutation(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationRoleBindingLifecycleError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationRoleBindingLifecycleErrorStatus {
    BadRequest,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationRoleBindingLifecycleErrorResponsePlan {
    pub status: ApplicationRoleBindingLifecycleErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_role_binding_lifecycle_error_response(
    error: &ApplicationRoleBindingLifecycleError,
) -> ApplicationRoleBindingLifecycleErrorResponsePlan {
    match error {
        ApplicationRoleBindingLifecycleError::WrongOperation(_)
        | ApplicationRoleBindingLifecycleError::WrongResourceKind(_)
        | ApplicationRoleBindingLifecycleError::TenantMismatch { .. }
        | ApplicationRoleBindingLifecycleError::MutationKindMismatch { .. } => {
            ApplicationRoleBindingLifecycleErrorResponsePlan {
                status: ApplicationRoleBindingLifecycleErrorStatus::BadRequest,
                code: "role_binding_lifecycle_invalid",
                message: "role-binding lifecycle request is invalid",
            }
        }
        ApplicationRoleBindingLifecycleError::Audit(error) => {
            let response = plan_application_control_plane_audit_error_response(error);
            ApplicationRoleBindingLifecycleErrorResponsePlan {
                status: match response.status {
                    ApplicationControlPlaneAuditErrorStatus::BadRequest
                    | ApplicationControlPlaneAuditErrorStatus::MethodNotAllowed => {
                        ApplicationRoleBindingLifecycleErrorStatus::BadRequest
                    }
                    ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable => {
                        ApplicationRoleBindingLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "role_binding_lifecycle_audit_unavailable",
                message: "role-binding lifecycle audit is temporarily unavailable",
            }
        }
        ApplicationRoleBindingLifecycleError::RoleBindingMutation(error) => {
            let response = plan_application_role_binding_mutation_error_response(error);
            ApplicationRoleBindingLifecycleErrorResponsePlan {
                status: match response.status {
                    ApplicationRoleBindingMutationErrorStatus::ServiceUnavailable => {
                        ApplicationRoleBindingLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "role_binding_lifecycle_storage_unavailable",
                message: "role-binding lifecycle storage is temporarily unavailable",
            }
        }
    }
}

pub fn plan_application_role_binding_lifecycle(
    request: ApplicationRoleBindingLifecycleRequest,
) -> Result<ApplicationRoleBindingLifecyclePlan, ApplicationRoleBindingLifecycleError> {
    if !matches!(
        request.action.operation,
        ControlPlaneOperation::RoleBindingGrant | ControlPlaneOperation::RoleBindingRevoke
    ) {
        return Err(ApplicationRoleBindingLifecycleError::WrongOperation(
            request.action.operation,
        ));
    }
    if request.action.resource.kind != ResourceKind::RoleBinding {
        return Err(ApplicationRoleBindingLifecycleError::WrongResourceKind(
            request.action.resource.kind,
        ));
    }
    if request.action.resource.tenant_id != request.mutation.tenant_id {
        return Err(ApplicationRoleBindingLifecycleError::TenantMismatch {
            action_tenant: request.action.resource.tenant_id,
            mutation_tenant: request.mutation.tenant_id,
        });
    }
    let expected_kind = match request.action.operation {
        ControlPlaneOperation::RoleBindingGrant => RoleBindingMutationKind::Grant,
        ControlPlaneOperation::RoleBindingRevoke => RoleBindingMutationKind::Revoke,
        _ => unreachable!("role-binding lifecycle operation was already validated"),
    };
    if request.mutation.kind != expected_kind {
        return Err(ApplicationRoleBindingLifecycleError::MutationKindMismatch {
            operation: request.action.operation,
            mutation_kind: request.mutation.kind,
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
                .map_err(ApplicationRoleBindingLifecycleError::Audit)?,
        ),
        DurableStoreKind::Sqlite => ApplicationControlPlaneAuditStoragePlan::Sqlite(
            plan_sqlite_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Sqlite)
                .map_err(ApplicationRoleBindingLifecycleError::Audit)?,
        ),
    };
    let mutation_storage = if matches!(decision, ControlPlaneDecision::Authorized(_)) {
        let mutation_plan =
            plan_application_role_binding_mutation(ApplicationRoleBindingMutationRequest {
                durable_store: request.durable_store,
                mutation: request.mutation,
            })
            .map_err(ApplicationRoleBindingLifecycleError::RoleBindingMutation)?;
        Some(mutation_plan.storage)
    } else {
        None
    };

    Ok(ApplicationRoleBindingLifecyclePlan {
        decision,
        audit_storage,
        mutation_storage,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationVirtualKeySecretReferenceRequest {
    pub durable_store: DurableStoreKind,
    pub reference: VirtualKeySecretReferenceCommand,
}

impl fmt::Debug for ApplicationVirtualKeySecretReferenceRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationVirtualKeySecretReferenceRequest")
            .field("durable_store", &self.durable_store)
            .field("reference", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationVirtualKeySecretReferenceStoragePlan {
    Postgres(PostgresVirtualKeySecretReferenceSqlPlan),
    Sqlite(SqliteVirtualKeySecretReferenceSqlPlan),
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationVirtualKeySecretReferencePlan {
    pub storage: ApplicationVirtualKeySecretReferenceStoragePlan,
}

impl fmt::Debug for ApplicationVirtualKeySecretReferencePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationVirtualKeySecretReferencePlan")
            .field("storage", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationVirtualKeySecretReferenceError {
    Postgres(prodex_storage_postgres::PostgresStoragePlanError),
    Sqlite(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Debug for ApplicationVirtualKeySecretReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(_) => f.debug_tuple("Postgres").field(&"<redacted>").finish(),
            Self::Sqlite(_) => f.debug_tuple("Sqlite").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationVirtualKeySecretReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(error) => error.fmt(f),
            Self::Sqlite(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationVirtualKeySecretReferenceError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationVirtualKeySecretReferenceErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationVirtualKeySecretReferenceErrorResponsePlan {
    pub status: ApplicationVirtualKeySecretReferenceErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_virtual_key_secret_reference_error_response(
    error: &ApplicationVirtualKeySecretReferenceError,
) -> ApplicationVirtualKeySecretReferenceErrorResponsePlan {
    match error {
        ApplicationVirtualKeySecretReferenceError::Postgres(error) => {
            application_virtual_key_secret_reference_response_from_postgres(
                plan_postgres_storage_error_response(error),
            )
        }
        ApplicationVirtualKeySecretReferenceError::Sqlite(error) => {
            application_virtual_key_secret_reference_response_from_sqlite(
                plan_sqlite_storage_error_response(error),
            )
        }
    }
}

fn application_virtual_key_secret_reference_response_from_postgres(
    response: PostgresStorageErrorResponsePlan,
) -> ApplicationVirtualKeySecretReferenceErrorResponsePlan {
    ApplicationVirtualKeySecretReferenceErrorResponsePlan {
        status: match response.status {
            PostgresStorageErrorStatus::ServiceUnavailable => {
                ApplicationVirtualKeySecretReferenceErrorStatus::ServiceUnavailable
            }
        },
        code: "virtual_key_secret_storage_unavailable",
        message: "virtual-key secret storage is temporarily unavailable",
    }
}

fn application_virtual_key_secret_reference_response_from_sqlite(
    response: SqliteStorageErrorResponsePlan,
) -> ApplicationVirtualKeySecretReferenceErrorResponsePlan {
    ApplicationVirtualKeySecretReferenceErrorResponsePlan {
        status: match response.status {
            SqliteStorageErrorStatus::ServiceUnavailable => {
                ApplicationVirtualKeySecretReferenceErrorStatus::ServiceUnavailable
            }
        },
        code: "virtual_key_secret_storage_unavailable",
        message: "virtual-key secret storage is temporarily unavailable",
    }
}

pub fn plan_application_virtual_key_secret_reference(
    request: ApplicationVirtualKeySecretReferenceRequest,
) -> Result<ApplicationVirtualKeySecretReferencePlan, ApplicationVirtualKeySecretReferenceError> {
    let storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationVirtualKeySecretReferenceStoragePlan::Postgres(
            plan_postgres_virtual_key_secret_reference(request.reference)
                .map_err(ApplicationVirtualKeySecretReferenceError::Postgres)?,
        ),
        DurableStoreKind::Sqlite => ApplicationVirtualKeySecretReferenceStoragePlan::Sqlite(
            plan_sqlite_virtual_key_secret_reference(request.reference)
                .map_err(ApplicationVirtualKeySecretReferenceError::Sqlite)?,
        ),
    };
    Ok(ApplicationVirtualKeySecretReferencePlan { storage })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationVirtualKeyLifecycleRequest {
    pub durable_store: DurableStoreKind,
    pub action: ControlPlaneActionRequest,
    pub reference: VirtualKeySecretReferenceCommand,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

impl fmt::Debug for ApplicationVirtualKeyLifecycleRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationVirtualKeyLifecycleRequest")
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
pub struct ApplicationVirtualKeyLifecyclePlan {
    pub decision: ControlPlaneDecision,
    pub audit_storage: ApplicationControlPlaneAuditStoragePlan,
    pub reference_storage: Option<ApplicationVirtualKeySecretReferenceStoragePlan>,
}

impl fmt::Debug for ApplicationVirtualKeyLifecyclePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationVirtualKeyLifecyclePlan")
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
pub enum ApplicationVirtualKeyLifecycleError {
    WrongOperation(ControlPlaneOperation),
    WrongResourceKind(ResourceKind),
    TenantMismatch {
        action_tenant: TenantId,
        reference_tenant: TenantId,
    },
    ReferenceKindMismatch {
        operation: ControlPlaneOperation,
        reference_kind: VirtualKeySecretReferenceKind,
    },
    Audit(ApplicationControlPlaneAuditError),
    VirtualKeySecretReference(ApplicationVirtualKeySecretReferenceError),
}

impl fmt::Debug for ApplicationVirtualKeyLifecycleError {
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
            Self::ReferenceKindMismatch { .. } => f
                .debug_struct("ReferenceKindMismatch")
                .field("operation", &"<redacted>")
                .field("reference_kind", &"<redacted>")
                .finish(),
            Self::Audit(_) => f.debug_tuple("Audit").field(&"<redacted>").finish(),
            Self::VirtualKeySecretReference(_) => f
                .debug_tuple("VirtualKeySecretReference")
                .field(&"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for ApplicationVirtualKeyLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => {
                write!(
                    f,
                    "control-plane operation is not a virtual-key lifecycle operation"
                )
            }
            Self::WrongResourceKind(_) => {
                write!(f, "control-plane resource kind is not virtual key")
            }
            Self::TenantMismatch { .. } => {
                write!(f, "virtual-key lifecycle request is invalid")
            }
            Self::ReferenceKindMismatch { .. } => {
                write!(
                    f,
                    "virtual-key lifecycle operation does not match reference kind"
                )
            }
            Self::Audit(error) => error.fmt(f),
            Self::VirtualKeySecretReference(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationVirtualKeyLifecycleError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationVirtualKeyLifecycleErrorStatus {
    BadRequest,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationVirtualKeyLifecycleErrorResponsePlan {
    pub status: ApplicationVirtualKeyLifecycleErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_virtual_key_lifecycle_error_response(
    error: &ApplicationVirtualKeyLifecycleError,
) -> ApplicationVirtualKeyLifecycleErrorResponsePlan {
    match error {
        ApplicationVirtualKeyLifecycleError::WrongOperation(_)
        | ApplicationVirtualKeyLifecycleError::WrongResourceKind(_)
        | ApplicationVirtualKeyLifecycleError::TenantMismatch { .. }
        | ApplicationVirtualKeyLifecycleError::ReferenceKindMismatch { .. } => {
            ApplicationVirtualKeyLifecycleErrorResponsePlan {
                status: ApplicationVirtualKeyLifecycleErrorStatus::BadRequest,
                code: "virtual_key_lifecycle_invalid",
                message: "virtual-key lifecycle request is invalid",
            }
        }
        ApplicationVirtualKeyLifecycleError::Audit(error) => {
            let response = plan_application_control_plane_audit_error_response(error);
            ApplicationVirtualKeyLifecycleErrorResponsePlan {
                status: match response.status {
                    ApplicationControlPlaneAuditErrorStatus::BadRequest
                    | ApplicationControlPlaneAuditErrorStatus::MethodNotAllowed => {
                        ApplicationVirtualKeyLifecycleErrorStatus::BadRequest
                    }
                    ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable => {
                        ApplicationVirtualKeyLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "virtual_key_lifecycle_audit_unavailable",
                message: "virtual-key lifecycle audit is temporarily unavailable",
            }
        }
        ApplicationVirtualKeyLifecycleError::VirtualKeySecretReference(error) => {
            let response = plan_application_virtual_key_secret_reference_error_response(error);
            ApplicationVirtualKeyLifecycleErrorResponsePlan {
                status: match response.status {
                    ApplicationVirtualKeySecretReferenceErrorStatus::ServiceUnavailable => {
                        ApplicationVirtualKeyLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "virtual_key_lifecycle_storage_unavailable",
                message: "virtual-key lifecycle storage is temporarily unavailable",
            }
        }
    }
}

pub fn plan_application_virtual_key_lifecycle(
    request: ApplicationVirtualKeyLifecycleRequest,
) -> Result<ApplicationVirtualKeyLifecyclePlan, ApplicationVirtualKeyLifecycleError> {
    if !matches!(
        request.action.operation,
        ControlPlaneOperation::VirtualKeyCreate | ControlPlaneOperation::VirtualKeyRotateSecret
    ) {
        return Err(ApplicationVirtualKeyLifecycleError::WrongOperation(
            request.action.operation,
        ));
    }
    if request.action.resource.kind != ResourceKind::VirtualKey {
        return Err(ApplicationVirtualKeyLifecycleError::WrongResourceKind(
            request.action.resource.kind,
        ));
    }
    if request.action.resource.tenant_id != request.reference.tenant_id {
        return Err(ApplicationVirtualKeyLifecycleError::TenantMismatch {
            action_tenant: request.action.resource.tenant_id,
            reference_tenant: request.reference.tenant_id,
        });
    }
    let expected_kind = match request.action.operation {
        ControlPlaneOperation::VirtualKeyCreate => VirtualKeySecretReferenceKind::Create,
        ControlPlaneOperation::VirtualKeyRotateSecret => VirtualKeySecretReferenceKind::Rotate,
        _ => unreachable!("virtual-key lifecycle operation was already validated"),
    };
    if request.reference.kind != expected_kind {
        return Err(ApplicationVirtualKeyLifecycleError::ReferenceKindMismatch {
            operation: request.action.operation,
            reference_kind: request.reference.kind,
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
                .map_err(ApplicationVirtualKeyLifecycleError::Audit)?,
        ),
        DurableStoreKind::Sqlite => ApplicationControlPlaneAuditStoragePlan::Sqlite(
            plan_sqlite_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Sqlite)
                .map_err(ApplicationVirtualKeyLifecycleError::Audit)?,
        ),
    };
    let reference_storage = if matches!(decision, ControlPlaneDecision::Authorized(_)) {
        let reference_plan = plan_application_virtual_key_secret_reference(
            ApplicationVirtualKeySecretReferenceRequest {
                durable_store: request.durable_store,
                reference: request.reference,
            },
        )
        .map_err(ApplicationVirtualKeyLifecycleError::VirtualKeySecretReference)?;
        Some(reference_plan.storage)
    } else {
        None
    };

    Ok(ApplicationVirtualKeyLifecyclePlan {
        decision,
        audit_storage,
        reference_storage,
    })
}
