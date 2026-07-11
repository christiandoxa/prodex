//! Budget policy governance planning.

use super::super::*;
use super::*;

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationBudgetPolicyUpdateRequest {
    pub durable_store: DurableStoreKind,
    pub policy: BudgetPolicyUpdateCommand,
}

impl fmt::Debug for ApplicationBudgetPolicyUpdateRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationBudgetPolicyUpdateRequest")
            .field("durable_store", &self.durable_store)
            .field("policy", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationBudgetPolicyUpdateStoragePlan {
    Postgres(PostgresBudgetPolicyUpdateSqlPlan),
    Sqlite(SqliteBudgetPolicyUpdateSqlPlan),
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationBudgetPolicyUpdatePlan {
    pub storage: ApplicationBudgetPolicyUpdateStoragePlan,
}

impl fmt::Debug for ApplicationBudgetPolicyUpdatePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationBudgetPolicyUpdatePlan")
            .field("storage", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationBudgetPolicyUpdateError {
    Postgres(prodex_storage_postgres::PostgresStoragePlanError),
    Sqlite(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Debug for ApplicationBudgetPolicyUpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(_) => f.debug_tuple("Postgres").field(&"<redacted>").finish(),
            Self::Sqlite(_) => f.debug_tuple("Sqlite").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationBudgetPolicyUpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(error) => error.fmt(f),
            Self::Sqlite(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationBudgetPolicyUpdateError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationBudgetPolicyUpdateErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationBudgetPolicyUpdateErrorResponsePlan {
    pub status: ApplicationBudgetPolicyUpdateErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_budget_policy_update_error_response(
    error: &ApplicationBudgetPolicyUpdateError,
) -> ApplicationBudgetPolicyUpdateErrorResponsePlan {
    match error {
        ApplicationBudgetPolicyUpdateError::Postgres(error) => {
            let response = plan_postgres_storage_error_response(error);
            ApplicationBudgetPolicyUpdateErrorResponsePlan {
                status: match response.status {
                    PostgresStorageErrorStatus::ServiceUnavailable => {
                        ApplicationBudgetPolicyUpdateErrorStatus::ServiceUnavailable
                    }
                },
                code: "budget_policy_storage_unavailable",
                message: "budget policy storage is temporarily unavailable",
            }
        }
        ApplicationBudgetPolicyUpdateError::Sqlite(error) => {
            let response = plan_sqlite_storage_error_response(error);
            ApplicationBudgetPolicyUpdateErrorResponsePlan {
                status: match response.status {
                    SqliteStorageErrorStatus::ServiceUnavailable => {
                        ApplicationBudgetPolicyUpdateErrorStatus::ServiceUnavailable
                    }
                },
                code: "budget_policy_storage_unavailable",
                message: "budget policy storage is temporarily unavailable",
            }
        }
    }
}

pub fn plan_application_budget_policy_update(
    request: ApplicationBudgetPolicyUpdateRequest,
) -> Result<ApplicationBudgetPolicyUpdatePlan, ApplicationBudgetPolicyUpdateError> {
    let storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationBudgetPolicyUpdateStoragePlan::Postgres(
            plan_postgres_budget_policy_update(request.policy)
                .map_err(ApplicationBudgetPolicyUpdateError::Postgres)?,
        ),
        DurableStoreKind::Sqlite => ApplicationBudgetPolicyUpdateStoragePlan::Sqlite(
            plan_sqlite_budget_policy_update(request.policy)
                .map_err(ApplicationBudgetPolicyUpdateError::Sqlite)?,
        ),
    };
    Ok(ApplicationBudgetPolicyUpdatePlan { storage })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationBudgetPolicyLifecycleRequest {
    pub durable_store: DurableStoreKind,
    pub action: ControlPlaneActionRequest,
    pub policy: BudgetPolicyUpdateCommand,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

impl fmt::Debug for ApplicationBudgetPolicyLifecycleRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationBudgetPolicyLifecycleRequest")
            .field("durable_store", &self.durable_store)
            .field("action", &"<redacted>")
            .field("policy", &"<redacted>")
            .field(
                "previous_digest",
                &self.previous_digest.as_ref().map(|_| "<redacted>"),
            )
            .field("event_digest", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationBudgetPolicyLifecyclePlan {
    pub decision: ControlPlaneDecision,
    pub audit_storage: ApplicationControlPlaneAuditStoragePlan,
    pub policy_storage: Option<ApplicationBudgetPolicyUpdateStoragePlan>,
}

impl fmt::Debug for ApplicationBudgetPolicyLifecyclePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationBudgetPolicyLifecyclePlan")
            .field("decision", &"<redacted>")
            .field("audit_storage", &"<redacted>")
            .field(
                "policy_storage",
                &self.policy_storage.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationBudgetPolicyLifecycleError {
    WrongOperation(ControlPlaneOperation),
    WrongResourceKind(ResourceKind),
    TenantMismatch {
        action_tenant: TenantId,
        policy_tenant: TenantId,
    },
    Audit(ApplicationControlPlaneAuditError),
    BudgetPolicyUpdate(ApplicationBudgetPolicyUpdateError),
}

impl fmt::Debug for ApplicationBudgetPolicyLifecycleError {
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
                .field("policy_tenant", &"<redacted>")
                .finish(),
            Self::Audit(_) => f.debug_tuple("Audit").field(&"<redacted>").finish(),
            Self::BudgetPolicyUpdate(_) => f
                .debug_tuple("BudgetPolicyUpdate")
                .field(&"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for ApplicationBudgetPolicyLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOperation(_) => {
                write!(f, "control-plane operation is not budget policy update")
            }
            Self::WrongResourceKind(_) => {
                write!(f, "control-plane resource kind is not budget")
            }
            Self::TenantMismatch { .. } => {
                write!(f, "budget policy lifecycle request is invalid")
            }
            Self::Audit(error) => error.fmt(f),
            Self::BudgetPolicyUpdate(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationBudgetPolicyLifecycleError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationBudgetPolicyLifecycleErrorStatus {
    BadRequest,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationBudgetPolicyLifecycleErrorResponsePlan {
    pub status: ApplicationBudgetPolicyLifecycleErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_budget_policy_lifecycle_error_response(
    error: &ApplicationBudgetPolicyLifecycleError,
) -> ApplicationBudgetPolicyLifecycleErrorResponsePlan {
    match error {
        ApplicationBudgetPolicyLifecycleError::WrongOperation(_)
        | ApplicationBudgetPolicyLifecycleError::WrongResourceKind(_)
        | ApplicationBudgetPolicyLifecycleError::TenantMismatch { .. } => {
            ApplicationBudgetPolicyLifecycleErrorResponsePlan {
                status: ApplicationBudgetPolicyLifecycleErrorStatus::BadRequest,
                code: "budget_policy_lifecycle_invalid",
                message: "budget policy lifecycle request is invalid",
            }
        }
        ApplicationBudgetPolicyLifecycleError::Audit(error) => {
            let response = plan_application_control_plane_audit_error_response(error);
            ApplicationBudgetPolicyLifecycleErrorResponsePlan {
                status: match response.status {
                    ApplicationControlPlaneAuditErrorStatus::BadRequest
                    | ApplicationControlPlaneAuditErrorStatus::MethodNotAllowed => {
                        ApplicationBudgetPolicyLifecycleErrorStatus::BadRequest
                    }
                    ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable => {
                        ApplicationBudgetPolicyLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "budget_policy_lifecycle_audit_unavailable",
                message: "budget policy lifecycle audit is temporarily unavailable",
            }
        }
        ApplicationBudgetPolicyLifecycleError::BudgetPolicyUpdate(error) => {
            let response = plan_application_budget_policy_update_error_response(error);
            ApplicationBudgetPolicyLifecycleErrorResponsePlan {
                status: match response.status {
                    ApplicationBudgetPolicyUpdateErrorStatus::ServiceUnavailable => {
                        ApplicationBudgetPolicyLifecycleErrorStatus::ServiceUnavailable
                    }
                },
                code: "budget_policy_lifecycle_storage_unavailable",
                message: "budget policy lifecycle storage is temporarily unavailable",
            }
        }
    }
}

pub fn plan_application_budget_policy_lifecycle(
    request: ApplicationBudgetPolicyLifecycleRequest,
) -> Result<ApplicationBudgetPolicyLifecyclePlan, ApplicationBudgetPolicyLifecycleError> {
    if request.action.operation != ControlPlaneOperation::BudgetUpdate {
        return Err(ApplicationBudgetPolicyLifecycleError::WrongOperation(
            request.action.operation,
        ));
    }
    if request.action.resource.kind != ResourceKind::Budget {
        return Err(ApplicationBudgetPolicyLifecycleError::WrongResourceKind(
            request.action.resource.kind,
        ));
    }
    if request.action.resource.tenant_id != request.policy.tenant_id {
        return Err(ApplicationBudgetPolicyLifecycleError::TenantMismatch {
            action_tenant: request.action.resource.tenant_id,
            policy_tenant: request.policy.tenant_id,
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
                .map_err(ApplicationBudgetPolicyLifecycleError::Audit)?,
        ),
        DurableStoreKind::Sqlite => ApplicationControlPlaneAuditStoragePlan::Sqlite(
            plan_sqlite_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Sqlite)
                .map_err(ApplicationBudgetPolicyLifecycleError::Audit)?,
        ),
    };
    let policy_storage = if matches!(decision, ControlPlaneDecision::Authorized(_)) {
        let policy_plan =
            plan_application_budget_policy_update(ApplicationBudgetPolicyUpdateRequest {
                durable_store: request.durable_store,
                policy: request.policy,
            })
            .map_err(ApplicationBudgetPolicyLifecycleError::BudgetPolicyUpdate)?;
        Some(policy_plan.storage)
    } else {
        None
    };

    Ok(ApplicationBudgetPolicyLifecyclePlan {
        decision,
        audit_storage,
        policy_storage,
    })
}
