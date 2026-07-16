#![forbid(unsafe_code)]
//! HTTP-neutral gateway data-plane admission and routing core.
//!
//! This crate composes authz, storage, provider SPI, and observability boundary
//! contracts into a gateway request plan without depending on HTTP frameworks,
//! async runtimes, storage drivers, filesystem, or provider SDKs.

mod virtual_key;

pub use virtual_key::*;

use std::error::Error;
use std::fmt;

use prodex_authz::{
    AuthorizationErrorStatus, BoundaryAuthorizationError, BoundaryKind,
    authorize_boundary_resource, data_plane_boundary_for_requirement,
    plan_authorization_error_response,
};
use prodex_domain::{
    AuthorizationRequirement, CorrelationContext, CredentialScope, GatewaySpanKind, Principal,
    RequestId, ResourceAction, ResourceKind, Role, TelemetryAttribute, TenantContext,
    TenantScopedResource,
};
use prodex_observability::{
    ObservabilityErrorResponsePlan, ObservabilityErrorStatus, SpanPlan, SpanPlanError,
    TraceContext, plan_gateway_span, plan_span_error_response,
};
use prodex_provider_spi::{
    ProviderInvocation, ProviderInvocationError, ProviderInvocationErrorStatus,
    plan_provider_invocation_error_response, validate_provider_invocation,
};
use prodex_storage::{
    AtomicReservationCommand, AtomicReservationPlan, AtomicReservationPlanError,
    ExpiredReservationRecoveryCommand, ExpiredReservationRecoveryPlan,
    ExpiredReservationRecoveryPlanError, StoragePlanErrorResponsePlan, StoragePlanErrorStatus,
    UsageReconciliationCommand, UsageReconciliationPlan, UsageReconciliationPlanError,
    plan_atomic_reservation, plan_atomic_reservation_error_response,
    plan_expired_reservation_recovery, plan_expired_reservation_recovery_error_response,
    plan_usage_reconciliation, plan_usage_reconciliation_error_response,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayAdmissionRequest<R> {
    pub tenant: TenantContext,
    pub principal: Principal,
    pub resource: R,
    pub reservation: AtomicReservationCommand,
    pub provider_invocation: ProviderInvocation,
    pub trace_context: TraceContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayAdmissionPlan {
    pub tenant: TenantContext,
    pub authorization_boundary: BoundaryKind,
    pub reservation: AtomicReservationPlan,
    pub provider_invocation: ProviderInvocation,
    pub spans: Vec<SpanPlan>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GatewayAdmissionError {
    AuthorizationBoundaryUnavailable,
    Authorization(BoundaryAuthorizationError),
    Reservation(AtomicReservationPlanError),
    ProviderInvocation(ProviderInvocationError),
    Span(SpanPlanError),
    TenantMismatch,
}

impl fmt::Display for GatewayAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthorizationBoundaryUnavailable => {
                write!(f, "gateway admission is temporarily unavailable")
            }
            Self::Authorization(err) => err.fmt(f),
            Self::Reservation(err) => err.fmt(f),
            Self::ProviderInvocation(err) => err.fmt(f),
            Self::Span(err) => err.fmt(f),
            Self::TenantMismatch => write!(f, "gateway admission request is invalid"),
        }
    }
}

impl Error for GatewayAdmissionError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayAdmissionErrorStatus {
    BadRequest,
    Forbidden,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayAdmissionErrorResponsePlan {
    pub status: GatewayAdmissionErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_gateway_admission_error_response(
    error: &GatewayAdmissionError,
) -> GatewayAdmissionErrorResponsePlan {
    match error {
        GatewayAdmissionError::Authorization(error) => {
            let response = plan_authorization_error_response(error);
            GatewayAdmissionErrorResponsePlan {
                status: match response.status {
                    AuthorizationErrorStatus::Forbidden => GatewayAdmissionErrorStatus::Forbidden,
                },
                code: response.code,
                message: response.message,
            }
        }
        GatewayAdmissionError::AuthorizationBoundaryUnavailable => {
            GatewayAdmissionErrorResponsePlan {
                status: GatewayAdmissionErrorStatus::ServiceUnavailable,
                code: "gateway_authorization_unavailable",
                message: "gateway authorization is temporarily unavailable",
            }
        }
        GatewayAdmissionError::ProviderInvocation(error) => {
            let response = plan_provider_invocation_error_response(error);
            GatewayAdmissionErrorResponsePlan {
                status: match response.status {
                    ProviderInvocationErrorStatus::Forbidden => {
                        GatewayAdmissionErrorStatus::Forbidden
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        GatewayAdmissionError::TenantMismatch => GatewayAdmissionErrorResponsePlan {
            status: GatewayAdmissionErrorStatus::BadRequest,
            code: "gateway_admission_rejected",
            message: "gateway admission request is invalid",
        },
        GatewayAdmissionError::Reservation(error) => gateway_admission_response_from_storage(
            plan_atomic_reservation_error_response(error),
            "gateway_admission_rejected",
            "gateway admission request is invalid",
        ),
        GatewayAdmissionError::Span(error) => {
            gateway_admission_response_from_observability(plan_span_error_response(error))
        }
    }
}

fn gateway_admission_response_from_observability(
    response: ObservabilityErrorResponsePlan,
) -> GatewayAdmissionErrorResponsePlan {
    GatewayAdmissionErrorResponsePlan {
        status: match response.status {
            ObservabilityErrorStatus::ServiceUnavailable => {
                GatewayAdmissionErrorStatus::ServiceUnavailable
            }
        },
        code: response.code,
        message: response.message,
    }
}

fn gateway_admission_response_from_storage(
    response: StoragePlanErrorResponsePlan,
    code: &'static str,
    message: &'static str,
) -> GatewayAdmissionErrorResponsePlan {
    GatewayAdmissionErrorResponsePlan {
        status: match response.status {
            StoragePlanErrorStatus::BadRequest | StoragePlanErrorStatus::InvalidConfiguration => {
                GatewayAdmissionErrorStatus::BadRequest
            }
        },
        code,
        message,
    }
}

fn resolve_data_plane_boundary(
    requirement: AuthorizationRequirement,
) -> Result<BoundaryKind, GatewayAuthorizationBoundaryError> {
    data_plane_boundary_for_requirement(requirement)
        .ok_or(GatewayAuthorizationBoundaryError::Unavailable)
}

fn gateway_stage_span(
    kind: GatewaySpanKind,
    name: &'static str,
    correlation: CorrelationContext,
    trace_context: TraceContext,
    stage: &'static str,
) -> Result<SpanPlan, SpanPlanError> {
    plan_gateway_span(
        kind,
        name,
        correlation,
        Some(trace_context),
        vec![TelemetryAttribute::metric_label("gateway_stage", stage)],
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayAuthorizationBoundaryError {
    Unavailable,
}

impl fmt::Display for GatewayAuthorizationBoundaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => write!(f, "gateway authorization is temporarily unavailable"),
        }
    }
}

impl Error for GatewayAuthorizationBoundaryError {}

pub fn plan_data_plane_admission<R>(
    request: GatewayAdmissionRequest<R>,
) -> Result<GatewayAdmissionPlan, GatewayAdmissionError>
where
    R: TenantScopedResource,
{
    let authorization_requirement = AuthorizationRequirement::new(
        ResourceKind::VirtualKey,
        ResourceAction::Read,
        CredentialScope::DataPlane,
        Role::Operator,
    );
    let authorization_boundary = resolve_data_plane_boundary(authorization_requirement)
        .map_err(|_| GatewayAdmissionError::AuthorizationBoundaryUnavailable)?;
    let tenant = authorize_boundary_resource(
        authorization_boundary,
        &request.principal,
        &request.resource,
    )
    .map_err(GatewayAdmissionError::Authorization)?;
    if tenant != request.tenant
        || request.reservation.storage_key.tenant_id != tenant.tenant_id
        || request.provider_invocation.tenant != tenant
    {
        return Err(GatewayAdmissionError::TenantMismatch);
    }

    let reservation =
        plan_atomic_reservation(request.reservation).map_err(GatewayAdmissionError::Reservation)?;
    validate_provider_invocation(&request.provider_invocation)
        .map_err(GatewayAdmissionError::ProviderInvocation)?;

    let correlation =
        prodex_domain::CorrelationContext::new(request.provider_invocation.request_id)
            .with_call_id(request.provider_invocation.call_id)
            .with_trace_id(request.trace_context.trace_id.clone())
            .with_tenant_id(tenant.tenant_id);
    let spans = vec![
        gateway_stage_span(
            GatewaySpanKind::Authorization,
            "prodex.gateway.authorization",
            correlation.clone(),
            request.trace_context.clone(),
            "authorization",
        )
        .map_err(GatewayAdmissionError::Span)?,
        gateway_stage_span(
            GatewaySpanKind::BudgetReservation,
            "prodex.gateway.budget_reservation",
            correlation.clone(),
            request.trace_context.clone(),
            "reservation",
        )
        .map_err(GatewayAdmissionError::Span)?,
        gateway_stage_span(
            GatewaySpanKind::ProviderRequest,
            "prodex.gateway.provider_request",
            correlation,
            request.trace_context,
            "provider",
        )
        .map_err(GatewayAdmissionError::Span)?,
    ];

    Ok(GatewayAdmissionPlan {
        tenant,
        authorization_boundary,
        reservation,
        provider_invocation: request.provider_invocation,
        spans,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayQuotaReadAuthorizationRequest<R> {
    pub tenant: TenantContext,
    pub principal: Principal,
    pub resource: R,
    pub request_id: RequestId,
    pub trace_context: TraceContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayQuotaReadAuthorizationPlan {
    pub tenant: TenantContext,
    pub authorization_boundary: BoundaryKind,
    pub spans: Vec<SpanPlan>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GatewayQuotaReadAuthorizationError {
    AuthorizationBoundaryUnavailable,
    Authorization(BoundaryAuthorizationError),
    TenantMismatch,
    Span(SpanPlanError),
}

impl fmt::Display for GatewayQuotaReadAuthorizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthorizationBoundaryUnavailable => {
                write!(f, "gateway quota authorization is temporarily unavailable")
            }
            Self::Authorization(err) => err.fmt(f),
            Self::TenantMismatch => write!(f, "gateway quota authorization request is invalid"),
            Self::Span(err) => err.fmt(f),
        }
    }
}

impl Error for GatewayQuotaReadAuthorizationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayQuotaReadAuthorizationErrorStatus {
    BadRequest,
    Forbidden,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayQuotaReadAuthorizationErrorResponsePlan {
    pub status: GatewayQuotaReadAuthorizationErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_gateway_quota_read_authorization_error_response(
    error: &GatewayQuotaReadAuthorizationError,
) -> GatewayQuotaReadAuthorizationErrorResponsePlan {
    match error {
        GatewayQuotaReadAuthorizationError::AuthorizationBoundaryUnavailable => {
            GatewayQuotaReadAuthorizationErrorResponsePlan {
                status: GatewayQuotaReadAuthorizationErrorStatus::ServiceUnavailable,
                code: "gateway_quota_authorization_unavailable",
                message: "gateway quota authorization is temporarily unavailable",
            }
        }
        GatewayQuotaReadAuthorizationError::Authorization(error) => {
            let response = plan_authorization_error_response(error);
            GatewayQuotaReadAuthorizationErrorResponsePlan {
                status: match response.status {
                    AuthorizationErrorStatus::Forbidden => {
                        GatewayQuotaReadAuthorizationErrorStatus::Forbidden
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        GatewayQuotaReadAuthorizationError::TenantMismatch => {
            GatewayQuotaReadAuthorizationErrorResponsePlan {
                status: GatewayQuotaReadAuthorizationErrorStatus::BadRequest,
                code: "gateway_quota_authorization_rejected",
                message: "gateway quota authorization request is invalid",
            }
        }
        GatewayQuotaReadAuthorizationError::Span(error) => {
            gateway_quota_authorization_response_from_observability(plan_span_error_response(error))
        }
    }
}

fn gateway_quota_authorization_response_from_observability(
    response: ObservabilityErrorResponsePlan,
) -> GatewayQuotaReadAuthorizationErrorResponsePlan {
    GatewayQuotaReadAuthorizationErrorResponsePlan {
        status: match response.status {
            ObservabilityErrorStatus::ServiceUnavailable => {
                GatewayQuotaReadAuthorizationErrorStatus::ServiceUnavailable
            }
        },
        code: response.code,
        message: response.message,
    }
}

pub fn plan_gateway_quota_read_authorization<R>(
    request: GatewayQuotaReadAuthorizationRequest<R>,
) -> Result<GatewayQuotaReadAuthorizationPlan, GatewayQuotaReadAuthorizationError>
where
    R: TenantScopedResource,
{
    let authorization_requirement = AuthorizationRequirement::new(
        ResourceKind::Budget,
        ResourceAction::Read,
        CredentialScope::DataPlane,
        Role::Operator,
    );
    let authorization_boundary = resolve_data_plane_boundary(authorization_requirement)
        .map_err(|_| GatewayQuotaReadAuthorizationError::AuthorizationBoundaryUnavailable)?;
    let tenant = authorize_boundary_resource(
        authorization_boundary,
        &request.principal,
        &request.resource,
    )
    .map_err(GatewayQuotaReadAuthorizationError::Authorization)?;
    if tenant != request.tenant {
        return Err(GatewayQuotaReadAuthorizationError::TenantMismatch);
    }

    let correlation = CorrelationContext::new(request.request_id)
        .with_trace_id(request.trace_context.trace_id.clone())
        .with_tenant_id(tenant.tenant_id);
    let spans = vec![
        gateway_stage_span(
            GatewaySpanKind::Authorization,
            "prodex.gateway.quota_authorization",
            correlation,
            request.trace_context,
            "quota_authorization",
        )
        .map_err(GatewayQuotaReadAuthorizationError::Span)?,
    ];

    Ok(GatewayQuotaReadAuthorizationPlan {
        tenant,
        authorization_boundary,
        spans,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayUsageReconciliationRequest {
    pub tenant: TenantContext,
    pub request_id: RequestId,
    pub reconciliation: UsageReconciliationCommand,
    pub trace_context: TraceContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayUsageReconciliationPlan {
    pub tenant: TenantContext,
    pub reconciliation: UsageReconciliationPlan,
    pub spans: Vec<SpanPlan>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GatewayUsageReconciliationError {
    TenantMismatch,
    Reconciliation(UsageReconciliationPlanError),
    Span(SpanPlanError),
}

impl fmt::Display for GatewayUsageReconciliationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch => write!(f, "usage reconciliation request is invalid"),
            Self::Reconciliation(err) => err.fmt(f),
            Self::Span(err) => err.fmt(f),
        }
    }
}

impl Error for GatewayUsageReconciliationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayUsageReconciliationErrorStatus {
    BadRequest,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayUsageReconciliationErrorResponsePlan {
    pub status: GatewayUsageReconciliationErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_gateway_usage_reconciliation_error_response(
    error: &GatewayUsageReconciliationError,
) -> GatewayUsageReconciliationErrorResponsePlan {
    match error {
        GatewayUsageReconciliationError::TenantMismatch => {
            GatewayUsageReconciliationErrorResponsePlan {
                status: GatewayUsageReconciliationErrorStatus::BadRequest,
                code: "usage_reconciliation_rejected",
                message: "usage reconciliation request is invalid",
            }
        }
        GatewayUsageReconciliationError::Reconciliation(error) => {
            gateway_usage_reconciliation_response_from_storage(
                plan_usage_reconciliation_error_response(error),
            )
        }
        GatewayUsageReconciliationError::Span(error) => {
            gateway_usage_reconciliation_response_from_observability(plan_span_error_response(
                error,
            ))
        }
    }
}

fn gateway_usage_reconciliation_response_from_observability(
    response: ObservabilityErrorResponsePlan,
) -> GatewayUsageReconciliationErrorResponsePlan {
    GatewayUsageReconciliationErrorResponsePlan {
        status: match response.status {
            ObservabilityErrorStatus::ServiceUnavailable => {
                GatewayUsageReconciliationErrorStatus::ServiceUnavailable
            }
        },
        code: response.code,
        message: response.message,
    }
}

fn gateway_usage_reconciliation_response_from_storage(
    response: StoragePlanErrorResponsePlan,
) -> GatewayUsageReconciliationErrorResponsePlan {
    GatewayUsageReconciliationErrorResponsePlan {
        status: match response.status {
            StoragePlanErrorStatus::BadRequest | StoragePlanErrorStatus::InvalidConfiguration => {
                GatewayUsageReconciliationErrorStatus::BadRequest
            }
        },
        code: response.code,
        message: response.message,
    }
}

pub fn plan_gateway_usage_reconciliation(
    request: GatewayUsageReconciliationRequest,
) -> Result<GatewayUsageReconciliationPlan, GatewayUsageReconciliationError> {
    if request.reconciliation.storage_key.tenant_id != request.tenant.tenant_id
        || request.reconciliation.record.tenant_id != request.tenant.tenant_id
    {
        return Err(GatewayUsageReconciliationError::TenantMismatch);
    }
    let correlation = CorrelationContext::new(request.request_id)
        .with_call_id(request.reconciliation.record.call_id)
        .with_trace_id(request.trace_context.trace_id.clone())
        .with_tenant_id(request.tenant.tenant_id);
    let reconciliation = plan_usage_reconciliation(request.reconciliation)
        .map_err(GatewayUsageReconciliationError::Reconciliation)?;
    let spans = vec![
        gateway_stage_span(
            GatewaySpanKind::Reconciliation,
            "prodex.gateway.usage_reconciliation",
            correlation,
            request.trace_context,
            "reconciliation",
        )
        .map_err(GatewayUsageReconciliationError::Span)?,
    ];
    Ok(GatewayUsageReconciliationPlan {
        tenant: request.tenant,
        reconciliation,
        spans,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayExpiredReservationRecoveryRequest {
    pub tenant: TenantContext,
    pub request_id: RequestId,
    pub recovery: ExpiredReservationRecoveryCommand,
    pub trace_context: TraceContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayExpiredReservationRecoveryPlan {
    pub tenant: TenantContext,
    pub recovery: ExpiredReservationRecoveryPlan,
    pub spans: Vec<SpanPlan>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GatewayExpiredReservationRecoveryError {
    TenantMismatch,
    Recovery(ExpiredReservationRecoveryPlanError),
    Span(SpanPlanError),
}

impl fmt::Display for GatewayExpiredReservationRecoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch => {
                write!(f, "expired reservation recovery request is invalid")
            }
            Self::Recovery(err) => err.fmt(f),
            Self::Span(err) => err.fmt(f),
        }
    }
}

impl Error for GatewayExpiredReservationRecoveryError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayExpiredReservationRecoveryErrorStatus {
    BadRequest,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayExpiredReservationRecoveryErrorResponsePlan {
    pub status: GatewayExpiredReservationRecoveryErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_gateway_expired_reservation_recovery_error_response(
    error: &GatewayExpiredReservationRecoveryError,
) -> GatewayExpiredReservationRecoveryErrorResponsePlan {
    match error {
        GatewayExpiredReservationRecoveryError::TenantMismatch => {
            GatewayExpiredReservationRecoveryErrorResponsePlan {
                status: GatewayExpiredReservationRecoveryErrorStatus::BadRequest,
                code: "expired_reservation_recovery_rejected",
                message: "expired reservation recovery request is invalid",
            }
        }
        GatewayExpiredReservationRecoveryError::Recovery(error) => {
            gateway_expired_reservation_recovery_response_from_storage(
                plan_expired_reservation_recovery_error_response(error),
            )
        }
        GatewayExpiredReservationRecoveryError::Span(error) => {
            gateway_expired_reservation_recovery_response_from_observability(
                plan_span_error_response(error),
            )
        }
    }
}

fn gateway_expired_reservation_recovery_response_from_observability(
    response: ObservabilityErrorResponsePlan,
) -> GatewayExpiredReservationRecoveryErrorResponsePlan {
    GatewayExpiredReservationRecoveryErrorResponsePlan {
        status: match response.status {
            ObservabilityErrorStatus::ServiceUnavailable => {
                GatewayExpiredReservationRecoveryErrorStatus::ServiceUnavailable
            }
        },
        code: response.code,
        message: response.message,
    }
}

fn gateway_expired_reservation_recovery_response_from_storage(
    response: StoragePlanErrorResponsePlan,
) -> GatewayExpiredReservationRecoveryErrorResponsePlan {
    GatewayExpiredReservationRecoveryErrorResponsePlan {
        status: match response.status {
            StoragePlanErrorStatus::BadRequest | StoragePlanErrorStatus::InvalidConfiguration => {
                GatewayExpiredReservationRecoveryErrorStatus::BadRequest
            }
        },
        code: response.code,
        message: response.message,
    }
}

pub fn plan_gateway_expired_reservation_recovery(
    request: GatewayExpiredReservationRecoveryRequest,
) -> Result<GatewayExpiredReservationRecoveryPlan, GatewayExpiredReservationRecoveryError> {
    if request.recovery.storage_key.tenant_id != request.tenant.tenant_id
        || request.recovery.record.tenant_id != request.tenant.tenant_id
    {
        return Err(GatewayExpiredReservationRecoveryError::TenantMismatch);
    }
    let correlation = CorrelationContext::new(request.request_id)
        .with_call_id(request.recovery.record.call_id)
        .with_trace_id(request.trace_context.trace_id.clone())
        .with_tenant_id(request.tenant.tenant_id);
    let recovery = plan_expired_reservation_recovery(request.recovery)
        .map_err(GatewayExpiredReservationRecoveryError::Recovery)?;
    let spans = vec![
        gateway_stage_span(
            GatewaySpanKind::Reconciliation,
            "prodex.gateway.expired_reservation_recovery",
            correlation,
            request.trace_context,
            "expired_reservation_recovery",
        )
        .map_err(GatewayExpiredReservationRecoveryError::Span)?,
    ];
    Ok(GatewayExpiredReservationRecoveryPlan {
        tenant: request.tenant,
        recovery,
        spans,
    })
}
