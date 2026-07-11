#![forbid(unsafe_code)]
//! Transport-neutral provider SPI primitives.
//!
//! This crate intentionally models the boundary between the gateway data plane
//! and provider adapters without depending on HTTP frameworks, provider SDKs,
//! filesystem access, database drivers, or async runtimes.

use std::error::Error;
use std::fmt;

use prodex_domain::{
    CallId, CapabilityDecision, CapabilityErrorResponsePlan, CapabilityRequest, CapabilitySet,
    CredentialScope, ModelRouteCandidate, Principal, RequestId, SecretRef, TenantContext,
    UsageAmount, negotiate_capability, plan_capability_decision_error_response,
};
use prodex_provider_core::{ProviderEndpoint, ProviderId, ProviderWireFormat};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderRoute {
    pub provider: ProviderId,
    pub endpoint: ProviderEndpoint,
    pub wire_format: ProviderWireFormat,
    pub model: String,
}

impl ProviderRoute {
    pub fn new(
        provider: ProviderId,
        endpoint: ProviderEndpoint,
        wire_format: ProviderWireFormat,
        model: impl Into<String>,
    ) -> Result<Self, ProviderRouteError> {
        let model = model.into();
        let trimmed = model.trim();
        if trimmed.is_empty() {
            return Err(ProviderRouteError::MissingModel);
        }
        if trimmed.len() > 256 {
            return Err(ProviderRouteError::ModelTooLong {
                length: trimmed.len(),
            });
        }
        Ok(Self {
            provider,
            endpoint,
            wire_format,
            model: trimmed.to_string(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderRouteError {
    MissingModel,
    ModelTooLong { length: usize },
}

impl fmt::Display for ProviderRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingModel | Self::ModelTooLong { .. } => {
                write!(f, "provider route is invalid")
            }
        }
    }
}

impl Error for ProviderRouteError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderRouteErrorStatus {
    BadRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderRouteErrorResponsePlan {
    pub status: ProviderRouteErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_provider_route_error_response(
    _error: &ProviderRouteError,
) -> ProviderRouteErrorResponsePlan {
    ProviderRouteErrorResponsePlan {
        status: ProviderRouteErrorStatus::BadRequest,
        code: "provider_route_invalid",
        message: "provider route is invalid",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderStreamMode {
    Unary,
    Streaming,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderInvocation {
    pub tenant: TenantContext,
    pub principal: Principal,
    pub request_id: RequestId,
    pub call_id: CallId,
    pub route: ProviderRoute,
    pub credential_ref: SecretRef,
    pub stream_mode: ProviderStreamMode,
    pub estimated_usage: UsageAmount,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderInvocationError {
    PrincipalMissingTenant,
    CrossTenantPrincipal {
        tenant: TenantContext,
        principal_tenant: TenantContext,
    },
    CredentialScopeMismatch {
        actual: CredentialScope,
    },
}

impl fmt::Display for ProviderInvocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PrincipalMissingTenant
            | Self::CrossTenantPrincipal { .. }
            | Self::CredentialScopeMismatch { .. } => {
                write!(f, "provider invocation is not authorized")
            }
        }
    }
}

impl Error for ProviderInvocationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderInvocationErrorStatus {
    Forbidden,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderInvocationErrorResponsePlan {
    pub status: ProviderInvocationErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_provider_invocation_error_response(
    _error: &ProviderInvocationError,
) -> ProviderInvocationErrorResponsePlan {
    ProviderInvocationErrorResponsePlan {
        status: ProviderInvocationErrorStatus::Forbidden,
        code: "provider_invocation_not_authorized",
        message: "provider invocation is not authorized",
    }
}

pub fn validate_provider_invocation(
    invocation: &ProviderInvocation,
) -> Result<(), ProviderInvocationError> {
    let Some(principal_tenant_id) = invocation.principal.tenant_id else {
        return Err(ProviderInvocationError::PrincipalMissingTenant);
    };
    if principal_tenant_id != invocation.tenant.tenant_id {
        return Err(ProviderInvocationError::CrossTenantPrincipal {
            tenant: invocation.tenant,
            principal_tenant: TenantContext {
                tenant_id: principal_tenant_id,
            },
        });
    }
    if invocation.principal.credential_scope != CredentialScope::DataPlane {
        return Err(ProviderInvocationError::CredentialScopeMismatch {
            actual: invocation.principal.credential_scope,
        });
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderRouteCapabilityCandidate {
    pub route: ProviderRoute,
    pub capabilities: CapabilitySet,
}

impl ProviderRouteCapabilityCandidate {
    pub fn new(route: ProviderRoute, capabilities: CapabilitySet) -> Self {
        Self {
            route,
            capabilities,
        }
    }

    fn model_candidate(&self) -> ModelRouteCandidate {
        ModelRouteCandidate::new(
            self.route.provider.label(),
            self.route.model.clone(),
            self.capabilities.clone(),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderCapabilityNegotiationPlan {
    pub route: ProviderRoute,
    pub capabilities: CapabilitySet,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderCapabilityNegotiationDecision {
    Compatible(ProviderCapabilityNegotiationPlan),
    Incompatible(CapabilityDecision),
}

pub fn negotiate_provider_route_capability(
    request: &CapabilityRequest,
    candidates: &[ProviderRouteCapabilityCandidate],
) -> ProviderCapabilityNegotiationDecision {
    let model_candidates = candidates
        .iter()
        .map(ProviderRouteCapabilityCandidate::model_candidate)
        .collect::<Vec<_>>();
    match negotiate_capability(request, &model_candidates) {
        CapabilityDecision::Compatible(model) => {
            let Some(candidate) = candidates.iter().find(|candidate| {
                candidate.route.provider.label() == model.provider
                    && candidate.route.model.eq_ignore_ascii_case(&model.model)
            }) else {
                return ProviderCapabilityNegotiationDecision::Incompatible(
                    CapabilityDecision::NoCandidate,
                );
            };
            ProviderCapabilityNegotiationDecision::Compatible(ProviderCapabilityNegotiationPlan {
                route: candidate.route.clone(),
                capabilities: candidate.capabilities.clone(),
            })
        }
        decision => ProviderCapabilityNegotiationDecision::Incompatible(decision),
    }
}

pub fn plan_provider_capability_negotiation_error_response(
    decision: &ProviderCapabilityNegotiationDecision,
) -> Option<CapabilityErrorResponsePlan> {
    match decision {
        ProviderCapabilityNegotiationDecision::Compatible(_) => None,
        ProviderCapabilityNegotiationDecision::Incompatible(decision) => {
            plan_capability_decision_error_response(decision)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderRetryStage {
    BeforeDispatch,
    BeforeFirstByte,
    AfterFirstByte,
    AfterCancellation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderRetryDecision {
    Allowed,
    DeniedCommitted,
    DeniedBudgetExhausted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderRetryDecisionStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderRetryDecisionResponsePlan {
    pub status: ProviderRetryDecisionStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_provider_retry_decision_response(
    decision: ProviderRetryDecision,
) -> Option<ProviderRetryDecisionResponsePlan> {
    match decision {
        ProviderRetryDecision::Allowed => None,
        ProviderRetryDecision::DeniedCommitted => Some(ProviderRetryDecisionResponsePlan {
            status: ProviderRetryDecisionStatus::ServiceUnavailable,
            code: "provider_retry_not_safe",
            message: "provider retry is not safe",
        }),
        ProviderRetryDecision::DeniedBudgetExhausted => Some(ProviderRetryDecisionResponsePlan {
            status: ProviderRetryDecisionStatus::ServiceUnavailable,
            code: "provider_retry_budget_exhausted",
            message: "provider retry budget is exhausted",
        }),
    }
}

pub fn evaluate_provider_retry(stage: ProviderRetryStage) -> ProviderRetryDecision {
    match stage {
        ProviderRetryStage::BeforeDispatch | ProviderRetryStage::BeforeFirstByte => {
            ProviderRetryDecision::Allowed
        }
        ProviderRetryStage::AfterFirstByte | ProviderRetryStage::AfterCancellation => {
            ProviderRetryDecision::DeniedCommitted
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderRetryPolicy {
    pub max_precommit_attempts: u8,
}

impl ProviderRetryPolicy {
    pub const fn single_retry() -> Self {
        Self {
            max_precommit_attempts: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderRetryPlan {
    pub stage: ProviderRetryStage,
    pub decision: ProviderRetryDecision,
    pub attempted_precommit_retries: u8,
    pub remaining_precommit_retries: u8,
}

pub fn plan_provider_retry(
    policy: ProviderRetryPolicy,
    stage: ProviderRetryStage,
    attempted_precommit_retries: u8,
) -> ProviderRetryPlan {
    let base_decision = evaluate_provider_retry(stage);
    let remaining_precommit_retries = policy
        .max_precommit_attempts
        .saturating_sub(attempted_precommit_retries);
    let decision = match base_decision {
        ProviderRetryDecision::Allowed if remaining_precommit_retries == 0 => {
            ProviderRetryDecision::DeniedBudgetExhausted
        }
        decision => decision,
    };

    ProviderRetryPlan {
        stage,
        decision,
        attempted_precommit_retries,
        remaining_precommit_retries,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderCircuitBreakerPolicy {
    pub failure_threshold: u8,
    pub cooldown_ms: u64,
}

impl ProviderCircuitBreakerPolicy {
    pub const fn conservative() -> Self {
        Self {
            failure_threshold: 3,
            cooldown_ms: 30_000,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ProviderCircuitBreakerState {
    pub consecutive_failures: u8,
    pub open_until_unix_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderCircuitBreakerDecision {
    Closed,
    Open { retry_after_ms: u64 },
    HalfOpenProbe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderCircuitBreakerEvent {
    Success,
    Failure { now_unix_ms: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderCircuitBreakerPlan {
    pub decision: ProviderCircuitBreakerDecision,
    pub next_state: ProviderCircuitBreakerState,
}

pub fn plan_provider_circuit_breaker(
    policy: ProviderCircuitBreakerPolicy,
    state: ProviderCircuitBreakerState,
    now_unix_ms: u64,
) -> ProviderCircuitBreakerDecision {
    if let Some(open_until_unix_ms) = state.open_until_unix_ms {
        if now_unix_ms < open_until_unix_ms {
            return ProviderCircuitBreakerDecision::Open {
                retry_after_ms: open_until_unix_ms - now_unix_ms,
            };
        }
        return ProviderCircuitBreakerDecision::HalfOpenProbe;
    }
    let threshold = policy.failure_threshold.max(1);
    if state.consecutive_failures >= threshold {
        ProviderCircuitBreakerDecision::Open { retry_after_ms: 0 }
    } else {
        ProviderCircuitBreakerDecision::Closed
    }
}

pub fn plan_provider_circuit_breaker_event(
    policy: ProviderCircuitBreakerPolicy,
    state: ProviderCircuitBreakerState,
    event: ProviderCircuitBreakerEvent,
) -> ProviderCircuitBreakerPlan {
    let threshold = policy.failure_threshold.max(1);
    let next_state = match event {
        ProviderCircuitBreakerEvent::Success => ProviderCircuitBreakerState::default(),
        ProviderCircuitBreakerEvent::Failure { now_unix_ms } => {
            let failures = state.consecutive_failures.saturating_add(1);
            ProviderCircuitBreakerState {
                consecutive_failures: failures,
                open_until_unix_ms: (failures >= threshold)
                    .then_some(now_unix_ms.saturating_add(policy.cooldown_ms)),
            }
        }
    };
    let now_unix_ms = match event {
        ProviderCircuitBreakerEvent::Success => 0,
        ProviderCircuitBreakerEvent::Failure { now_unix_ms } => now_unix_ms,
    };

    ProviderCircuitBreakerPlan {
        decision: plan_provider_circuit_breaker(policy, next_state, now_unix_ms),
        next_state,
    }
}
