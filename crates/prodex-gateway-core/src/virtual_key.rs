use std::fmt;

use prodex_domain::{
    BudgetLimit, BudgetSnapshot, CallId, IdempotencyKey, RateLimitBucketKey, ReservationId,
    ReservationRequest, TenantId, UsageAmount, VirtualKeyId,
};
use prodex_storage::{
    AtomicReservationCommand, BudgetStorageScope, DurableStoreKind, TenantStorageKey,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayVirtualKeyPolicy {
    pub name: String,
    pub tenant_id: Option<String>,
    pub team_id: Option<String>,
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    pub budget_id: Option<String>,
    pub allowed_models: Vec<String>,
    pub budget_microusd: Option<u64>,
    pub request_budget: Option<u64>,
    pub rpm_limit: Option<u64>,
    pub tpm_limit: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayVirtualKeyUsage {
    pub minute_epoch: u64,
    pub requests_this_minute: u64,
    pub tokens_this_minute: u64,
    pub requests_total: u64,
    pub spend_microusd: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayVirtualKeyUsageEntry {
    pub policy: GatewayVirtualKeyPolicy,
    pub usage: GatewayVirtualKeyUsage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GatewayVirtualKeyReservationContext {
    pub tenant_id: TenantId,
    pub virtual_key_id: Option<VirtualKeyId>,
    pub call_id: CallId,
    pub reservation_id: ReservationId,
    pub durable_store: Option<DurableStoreKind>,
    pub created_at_unix_ms: u64,
    pub ttl_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayVirtualKeyAdmissionRequest {
    pub policy: GatewayVirtualKeyPolicy,
    pub usage: GatewayVirtualKeyUsage,
    /// Empty for the runtime-proxy compatibility API; populated by production admission.
    pub grouped_usage: Vec<GatewayVirtualKeyUsageEntry>,
    pub model: Option<String>,
    pub input_tokens: u64,
    pub reserved_tokens: u64,
    pub estimated_cost_microusd: Option<u64>,
    pub minute_epoch: u64,
    pub reservation: Option<GatewayVirtualKeyReservationContext>,
    pub distributed_rate_limit: bool,
    pub now_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayVirtualKeyAdmission {
    pub key_name: String,
    pub model: Option<String>,
    pub input_tokens: u64,
    pub reserved_tokens: u64,
    pub estimated_cost_microusd: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GatewayVirtualKeyUsageUpdate {
    pub minute_epoch: u64,
    pub reserved_tokens: u64,
    pub estimated_cost_microusd: Option<u64>,
}

impl GatewayVirtualKeyUsageUpdate {
    pub fn from_admission(admission: &GatewayVirtualKeyAdmission, minute_epoch: u64) -> Self {
        Self {
            minute_epoch,
            reserved_tokens: admission.reserved_tokens,
            estimated_cost_microusd: admission.estimated_cost_microusd,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayVirtualKeyDistributedRateLimitRequest {
    pub bucket: RateLimitBucketKey,
    pub window_seconds: u64,
    pub max_requests: Option<u64>,
    pub max_tokens: Option<u64>,
    pub increment_tokens: u64,
    pub now_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayVirtualKeyAdmissionPlan {
    pub admission: GatewayVirtualKeyAdmission,
    pub usage_update: GatewayVirtualKeyUsageUpdate,
    pub reservation: Option<AtomicReservationCommand>,
    pub durable_reservation: bool,
    pub distributed_rate_limit: Option<GatewayVirtualKeyDistributedRateLimitRequest>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayVirtualKeyAdmissionError {
    ModelNotAllowed,
    RequestBudgetExceeded,
    BudgetExceeded,
    RpmLimitExceeded,
    TpmLimitExceeded,
    PolicyStateUnavailable,
}

impl fmt::Display for GatewayVirtualKeyAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("gateway virtual-key admission rejected")
    }
}

impl std::error::Error for GatewayVirtualKeyAdmissionError {}

pub fn plan_gateway_virtual_key_admission(
    request: GatewayVirtualKeyAdmissionRequest,
) -> Result<GatewayVirtualKeyAdmissionPlan, GatewayVirtualKeyAdmissionError> {
    if let (Some(reservation), Some(policy_tenant_id)) = (
        request.reservation.as_ref(),
        request
            .policy
            .tenant_id
            .as_deref()
            .and_then(|tenant_id| tenant_id.parse::<TenantId>().ok()),
    ) && reservation.tenant_id != policy_tenant_id
    {
        return Err(GatewayVirtualKeyAdmissionError::PolicyStateUnavailable);
    }
    let durable_budget = gateway_virtual_key_durable_budget_enforced(&request);
    if !durable_budget {
        gateway_virtual_key_check_grouped_budget(&request)?;
    }

    let GatewayVirtualKeyAdmissionRequest {
        policy,
        mut usage,
        model,
        input_tokens,
        reserved_tokens,
        estimated_cost_microusd,
        minute_epoch,
        reservation,
        distributed_rate_limit,
        now_unix_ms,
        ..
    } = request;

    if !policy.allowed_models.is_empty()
        && model.as_ref().is_some_and(|model| {
            !policy
                .allowed_models
                .iter()
                .any(|allowed| allowed.trim().eq_ignore_ascii_case(model))
        })
    {
        return Err(GatewayVirtualKeyAdmissionError::ModelNotAllowed);
    }

    if durable_budget {
        if policy.budget_microusd.is_some() {
            usage.spend_microusd = 0;
        }
        if policy.request_budget.is_some() {
            usage.requests_total = 0;
        }
    }
    let same_minute = usage.minute_epoch == minute_epoch;
    let requests_this_minute = if same_minute {
        usage.requests_this_minute
    } else {
        0
    };
    let tokens_this_minute = if same_minute {
        usage.tokens_this_minute
    } else {
        0
    };

    if policy
        .request_budget
        .is_some_and(|limit| usage.requests_total >= limit)
    {
        return Err(GatewayVirtualKeyAdmissionError::RequestBudgetExceeded);
    }
    if let (Some(limit), Some(cost)) = (policy.budget_microusd, estimated_cost_microusd)
        && usage.spend_microusd.saturating_add(cost) > limit
    {
        return Err(GatewayVirtualKeyAdmissionError::BudgetExceeded);
    }
    if policy
        .rpm_limit
        .is_some_and(|limit| requests_this_minute.saturating_add(1) > limit)
    {
        return Err(GatewayVirtualKeyAdmissionError::RpmLimitExceeded);
    }
    if policy
        .tpm_limit
        .is_some_and(|limit| tokens_this_minute.saturating_add(reserved_tokens) > limit)
    {
        return Err(GatewayVirtualKeyAdmissionError::TpmLimitExceeded);
    }

    let admission = GatewayVirtualKeyAdmission {
        key_name: policy.name.clone(),
        model,
        input_tokens,
        reserved_tokens,
        estimated_cost_microusd,
    };
    let usage_update = GatewayVirtualKeyUsageUpdate::from_admission(&admission, minute_epoch);
    let durable_reservation =
        gateway_virtual_key_durable_reservation(&policy, reservation.as_ref());
    let reservation = reservation.map(|context| {
        gateway_virtual_key_reservation_command(&policy, &admission, context, durable_reservation)
    });
    let distributed_rate_limit = gateway_virtual_key_distributed_rate_limit(
        &policy,
        &admission,
        reservation
            .as_ref()
            .map(|command| command.request.tenant_id),
        reservation
            .as_ref()
            .and_then(|command| command.storage_key.virtual_key_id),
        minute_epoch,
        distributed_rate_limit,
        now_unix_ms,
    )?;

    Ok(GatewayVirtualKeyAdmissionPlan {
        admission,
        usage_update,
        reservation,
        durable_reservation,
        distributed_rate_limit,
    })
}

pub fn apply_gateway_virtual_key_usage_update(
    usage: &mut GatewayVirtualKeyUsage,
    update: GatewayVirtualKeyUsageUpdate,
) {
    if usage.minute_epoch != update.minute_epoch {
        usage.minute_epoch = update.minute_epoch;
        usage.requests_this_minute = 0;
        usage.tokens_this_minute = 0;
    }
    usage.requests_this_minute = usage.requests_this_minute.saturating_add(1);
    usage.tokens_this_minute = usage
        .tokens_this_minute
        .saturating_add(update.reserved_tokens);
    usage.requests_total = usage.requests_total.saturating_add(1);
    if let Some(cost) = update.estimated_cost_microusd {
        usage.spend_microusd = usage.spend_microusd.saturating_add(cost);
    }
}

fn gateway_virtual_key_check_grouped_budget(
    request: &GatewayVirtualKeyAdmissionRequest,
) -> Result<(), GatewayVirtualKeyAdmissionError> {
    let Some(budget_id) = request
        .policy
        .budget_id
        .as_deref()
        .filter(|budget_id| !budget_id.is_empty())
    else {
        return Ok(());
    };
    let mut requests_total = 0_u64;
    let mut spend_microusd = 0_u64;
    for entry in request.grouped_usage.iter().filter(|entry| {
        entry.policy.budget_id.as_deref() == Some(budget_id)
            && gateway_virtual_key_budget_scope_matches(&request.policy, &entry.policy)
    }) {
        requests_total = requests_total.saturating_add(entry.usage.requests_total);
        spend_microusd = spend_microusd.saturating_add(entry.usage.spend_microusd);
    }
    if request
        .policy
        .request_budget
        .is_some_and(|limit| requests_total >= limit)
    {
        return Err(GatewayVirtualKeyAdmissionError::RequestBudgetExceeded);
    }
    if let (Some(limit), Some(cost)) = (
        request.policy.budget_microusd,
        request.estimated_cost_microusd,
    ) && spend_microusd.saturating_add(cost) > limit
    {
        return Err(GatewayVirtualKeyAdmissionError::BudgetExceeded);
    }
    Ok(())
}

fn gateway_virtual_key_budget_scope_matches(
    selected: &GatewayVirtualKeyPolicy,
    candidate: &GatewayVirtualKeyPolicy,
) -> bool {
    selected.tenant_id == candidate.tenant_id
        && selected.team_id == candidate.team_id
        && selected.project_id == candidate.project_id
        && selected.user_id == candidate.user_id
}

fn gateway_virtual_key_durable_reservation(
    policy: &GatewayVirtualKeyPolicy,
    reservation: Option<&GatewayVirtualKeyReservationContext>,
) -> bool {
    reservation.is_some_and(|reservation| {
        reservation.durable_store.is_some()
            && reservation.virtual_key_id.is_some()
            && policy
                .tenant_id
                .as_deref()
                .and_then(|tenant_id| tenant_id.parse::<TenantId>().ok())
                .is_some()
    })
}

fn gateway_virtual_key_durable_budget_enforced(
    request: &GatewayVirtualKeyAdmissionRequest,
) -> bool {
    let Some(reservation) = request.reservation.as_ref() else {
        return false;
    };
    let supported = match reservation.durable_store {
        Some(DurableStoreKind::Postgres) => {
            request.policy.budget_microusd.is_some() || request.policy.request_budget.is_some()
        }
        Some(DurableStoreKind::Sqlite) => {
            request.policy.budget_microusd.is_some()
                && !request
                    .policy
                    .budget_id
                    .as_deref()
                    .is_some_and(|budget_id| !budget_id.is_empty())
        }
        None => false,
    };
    supported
        && reservation.virtual_key_id.is_some()
        && request
            .policy
            .tenant_id
            .as_deref()
            .and_then(|tenant_id| tenant_id.parse::<TenantId>().ok())
            .is_some()
}

fn gateway_virtual_key_reservation_command(
    policy: &GatewayVirtualKeyPolicy,
    admission: &GatewayVirtualKeyAdmission,
    context: GatewayVirtualKeyReservationContext,
    durable: bool,
) -> AtomicReservationCommand {
    let storage_key = context
        .virtual_key_id
        .map(|virtual_key_id| {
            gateway_virtual_key_budget_storage_key(context.tenant_id, virtual_key_id, policy)
        })
        .unwrap_or_else(|| TenantStorageKey::tenant(context.tenant_id));
    AtomicReservationCommand {
        storage_key,
        idempotency_key: IdempotencyKey::from_call_reservation(
            context.call_id,
            context.reservation_id,
        ),
        snapshot: BudgetSnapshot::default(),
        limit: BudgetLimit::new(
            i64::MAX as u64,
            policy.budget_microusd.unwrap_or(i64::MAX as u64),
        )
        .with_max_requests(policy.request_budget.unwrap_or(i64::MAX as u64)),
        request: ReservationRequest {
            tenant_id: context.tenant_id,
            call_id: context.call_id,
            reservation_id: context.reservation_id,
            estimate: UsageAmount::new(
                if durable {
                    admission.reserved_tokens
                } else {
                    admission.reserved_tokens.max(1)
                },
                admission.estimated_cost_microusd.unwrap_or_default(),
            ),
        },
        created_at_unix_ms: context.created_at_unix_ms,
        ttl_ms: context.ttl_ms,
    }
}

fn gateway_virtual_key_budget_storage_key(
    tenant_id: TenantId,
    virtual_key_id: VirtualKeyId,
    policy: &GatewayVirtualKeyPolicy,
) -> TenantStorageKey {
    let Some(budget_id) = policy
        .budget_id
        .as_deref()
        .filter(|budget_id| !budget_id.is_empty())
    else {
        return TenantStorageKey::virtual_key(tenant_id, virtual_key_id);
    };
    let mut digest = Sha256::new();
    gateway_virtual_key_hash_budget_scope_part(&mut digest, Some(&tenant_id.to_string()));
    gateway_virtual_key_hash_budget_scope_part(&mut digest, Some(budget_id));
    gateway_virtual_key_hash_budget_scope_part(&mut digest, policy.team_id.as_deref());
    gateway_virtual_key_hash_budget_scope_part(&mut digest, policy.project_id.as_deref());
    gateway_virtual_key_hash_budget_scope_part(&mut digest, policy.user_id.as_deref());
    TenantStorageKey::budget_group(
        tenant_id,
        virtual_key_id,
        BudgetStorageScope::from_digest(digest.finalize().into()),
    )
}

fn gateway_virtual_key_hash_budget_scope_part(digest: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            digest.update([1]);
            digest.update((value.len() as u64).to_le_bytes());
            digest.update(value.as_bytes());
        }
        None => digest.update([0]),
    }
}

fn gateway_virtual_key_distributed_rate_limit(
    policy: &GatewayVirtualKeyPolicy,
    admission: &GatewayVirtualKeyAdmission,
    reservation_tenant_id: Option<TenantId>,
    virtual_key_id: Option<VirtualKeyId>,
    minute_epoch: u64,
    enabled: bool,
    now_unix_ms: u64,
) -> Result<Option<GatewayVirtualKeyDistributedRateLimitRequest>, GatewayVirtualKeyAdmissionError> {
    if !enabled || (policy.rpm_limit.is_none() && policy.tpm_limit.is_none()) {
        return Ok(None);
    }
    let tenant_id = policy
        .tenant_id
        .as_deref()
        .and_then(|tenant_id| tenant_id.parse::<TenantId>().ok())
        .filter(|tenant_id| Some(*tenant_id) == reservation_tenant_id)
        .ok_or(GatewayVirtualKeyAdmissionError::PolicyStateUnavailable)?;
    let virtual_key_id =
        virtual_key_id.ok_or(GatewayVirtualKeyAdmissionError::PolicyStateUnavailable)?;
    let window_start_unix_ms = minute_epoch
        .checked_mul(60_000)
        .ok_or(GatewayVirtualKeyAdmissionError::PolicyStateUnavailable)?;
    Ok(Some(GatewayVirtualKeyDistributedRateLimitRequest {
        bucket: RateLimitBucketKey::new(tenant_id, Some(virtual_key_id), window_start_unix_ms),
        window_seconds: 60,
        max_requests: policy.rpm_limit,
        max_tokens: policy.tpm_limit,
        increment_tokens: admission.reserved_tokens,
        now_unix_ms,
    }))
}

#[cfg(test)]
#[path = "virtual_key_tests.rs"]
mod tests;
