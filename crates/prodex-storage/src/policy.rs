use super::*;

#[derive(Clone, PartialEq, Eq)]
pub struct BudgetPolicyUpdateCommand {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub budget_scope: String,
    pub limit: BudgetLimit,
    pub updated_at_unix_ms: u64,
}

impl fmt::Debug for BudgetPolicyUpdateCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BudgetPolicyUpdateCommand")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("budget_scope", &"<redacted>")
            .field("limit", &"<redacted>")
            .field("updated_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct BudgetPolicyUpdatePlan {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub budget_scope: String,
    pub limit: BudgetLimit,
    pub updated_at_unix_ms: u64,
}

impl fmt::Debug for BudgetPolicyUpdatePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BudgetPolicyUpdatePlan")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("budget_scope", &"<redacted>")
            .field("limit", &"<redacted>")
            .field("updated_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum BudgetPolicyUpdatePlanError {
    TenantMismatch {
        key_tenant: TenantId,
        request_tenant: TenantId,
    },
    EmptyBudgetScope,
}

impl fmt::Debug for BudgetPolicyUpdatePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("request_tenant", &"<redacted>")
                .finish(),
            Self::EmptyBudgetScope => f.write_str("EmptyBudgetScope"),
        }
    }
}

impl fmt::Display for BudgetPolicyUpdatePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } | Self::EmptyBudgetScope => {
                write!(f, "budget policy update request is invalid")
            }
        }
    }
}

impl Error for BudgetPolicyUpdatePlanError {}

pub fn plan_budget_policy_update_error_response(
    _error: &BudgetPolicyUpdatePlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "budget_policy_update_rejected",
        message: "budget policy update request is invalid",
    }
}

pub fn plan_budget_policy_update(
    command: BudgetPolicyUpdateCommand,
) -> Result<BudgetPolicyUpdatePlan, BudgetPolicyUpdatePlanError> {
    if command.storage_key.tenant_id != command.tenant_id {
        return Err(BudgetPolicyUpdatePlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            request_tenant: command.tenant_id,
        });
    }
    if command.budget_scope.trim().is_empty() {
        return Err(BudgetPolicyUpdatePlanError::EmptyBudgetScope);
    }
    Ok(BudgetPolicyUpdatePlan {
        storage_key: command.storage_key,
        tenant_id: command.tenant_id,
        budget_scope: command.budget_scope,
        limit: command.limit,
        updated_at_unix_ms: command.updated_at_unix_ms,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TenantLifecycleKind {
    Create,
    Update,
}

#[derive(Clone, PartialEq, Eq)]
pub struct TenantLifecycleCommand {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub display_name: String,
    pub kind: TenantLifecycleKind,
    pub occurred_at_unix_ms: u64,
}

impl fmt::Debug for TenantLifecycleCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TenantLifecycleCommand")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("display_name", &"<redacted>")
            .field("kind", &self.kind)
            .field("occurred_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct TenantLifecyclePlan {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub display_name: String,
    pub kind: TenantLifecycleKind,
    pub occurred_at_unix_ms: u64,
}

impl fmt::Debug for TenantLifecyclePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TenantLifecyclePlan")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("display_name", &"<redacted>")
            .field("kind", &self.kind)
            .field("occurred_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum TenantLifecyclePlanError {
    TenantMismatch {
        key_tenant: TenantId,
        request_tenant: TenantId,
    },
    EmptyDisplayName,
}

impl fmt::Debug for TenantLifecyclePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("request_tenant", &"<redacted>")
                .finish(),
            Self::EmptyDisplayName => f.write_str("EmptyDisplayName"),
        }
    }
}

impl fmt::Display for TenantLifecyclePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } | Self::EmptyDisplayName => {
                write!(f, "tenant lifecycle request is invalid")
            }
        }
    }
}

impl Error for TenantLifecyclePlanError {}

pub fn plan_tenant_lifecycle_error_response(
    _error: &TenantLifecyclePlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "tenant_lifecycle_rejected",
        message: "tenant lifecycle request is invalid",
    }
}

pub fn plan_tenant_lifecycle(
    command: TenantLifecycleCommand,
) -> Result<TenantLifecyclePlan, TenantLifecyclePlanError> {
    if command.storage_key.tenant_id != command.tenant_id {
        return Err(TenantLifecyclePlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            request_tenant: command.tenant_id,
        });
    }
    if command.display_name.trim().is_empty() {
        return Err(TenantLifecyclePlanError::EmptyDisplayName);
    }
    Ok(TenantLifecyclePlan {
        storage_key: command.storage_key,
        tenant_id: command.tenant_id,
        display_name: command.display_name,
        kind: command.kind,
        occurred_at_unix_ms: command.occurred_at_unix_ms,
    })
}
