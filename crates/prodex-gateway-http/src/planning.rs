use super::*;
use crate::route::{
    requires_cancellation_propagation, requires_streaming_backpressure, requires_trace,
    trace_context_from_headers, trace_propagation_metrics_from_headers, validate_method,
    validate_singleton_affinity_headers, validate_singleton_codex_metadata_headers,
    validate_singleton_credential_headers,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayHttpPlan {
    pub target: CanonicalRequestTarget,
    pub route: GatewayHttpRouteKind,
    pub trace_context: Option<TraceContext>,
    pub trace_propagation_metrics: Vec<TracePropagationMetricPlan>,
    pub timeout_budget: GatewayTimeoutBudget,
    pub execution: GatewayHttpExecutionPlan,
    pub preserved_upstream_headers: Vec<GatewayHttpHeader>,
    pub stripped_headers: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GatewayTimeoutBudget {
    pub request_timeout_ms: u64,
    pub stream_idle_timeout_ms: u64,
    pub connection_drain_timeout_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayHttpExecutionPlan {
    pub route: GatewayHttpRouteKind,
    pub max_body_bytes: usize,
    pub max_concurrent_streams: u32,
    pub timeout_budget: GatewayTimeoutBudget,
    pub cancellation_propagation_required: bool,
    pub streaming_backpressure_required: bool,
    pub graceful_drain_required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayHttpDrainPlan {
    pub connection_drain_timeout_ms: u64,
    pub prestop_delay_ms: u64,
    pub termination_grace_ms: u64,
    pub readiness_fails_before_drain: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayHttpDrainPlanError {
    Policy(GatewayHttpPolicyError),
    PreStopDelayRequired,
    TerminationGraceTooShort { required_ms: u64, actual_ms: u64 },
}

impl fmt::Display for GatewayHttpDrainPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Policy(err) => err.fmt(f),
            Self::PreStopDelayRequired => {
                write!(f, "HTTP drain delay is invalid")
            }
            Self::TerminationGraceTooShort { .. } => {
                write!(
                    f,
                    "HTTP termination grace is shorter than required drain budget"
                )
            }
        }
    }
}

impl Error for GatewayHttpDrainPlanError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GatewayHttpPlanError {
    Policy(GatewayHttpPolicyError),
    InvalidRequestTarget(CanonicalRequestTargetError),
    BodyTooLarge {
        max: usize,
        actual: usize,
    },
    MethodNotAllowed {
        route: GatewayHttpRouteKind,
        method: GatewayHttpMethod,
    },
    MissingTraceContext,
    DuplicateTraceContext,
    DuplicateAuthorization,
    DuplicateChatGptAccountId,
    DuplicateSessionId,
    DuplicateCodexTurnState,
    DuplicateCodexMetadata,
    InvalidTraceContext(TraceContextError),
}

impl fmt::Display for GatewayHttpPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Policy(err) => err.fmt(f),
            Self::InvalidRequestTarget(err) => err.fmt(f),
            Self::BodyTooLarge { .. } => write!(f, "HTTP body is too large"),
            Self::MethodNotAllowed { .. } => write!(f, "HTTP method is not allowed"),
            Self::MissingTraceContext => write!(f, "required request metadata is missing"),
            Self::DuplicateTraceContext
            | Self::DuplicateAuthorization
            | Self::DuplicateChatGptAccountId
            | Self::DuplicateSessionId
            | Self::DuplicateCodexTurnState
            | Self::DuplicateCodexMetadata => write!(f, "request metadata is duplicated"),
            Self::InvalidTraceContext(err) => err.fmt(f),
        }
    }
}

impl Error for GatewayHttpPlanError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayHttpErrorStatus {
    BadRequest,
    MethodNotAllowed,
    PayloadTooLarge,
    InternalServerError,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayHttpErrorResponsePlan {
    pub status: GatewayHttpErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_gateway_http_error_response(
    error: &GatewayHttpPlanError,
) -> GatewayHttpErrorResponsePlan {
    match error {
        GatewayHttpPlanError::InvalidRequestTarget(_) => GatewayHttpErrorResponsePlan {
            status: GatewayHttpErrorStatus::BadRequest,
            code: "invalid_request_target",
            message: "request target is invalid",
        },
        GatewayHttpPlanError::BodyTooLarge { .. } => GatewayHttpErrorResponsePlan {
            status: GatewayHttpErrorStatus::PayloadTooLarge,
            code: "request_body_too_large",
            message: "request body is too large",
        },
        GatewayHttpPlanError::MethodNotAllowed { .. } => GatewayHttpErrorResponsePlan {
            status: GatewayHttpErrorStatus::MethodNotAllowed,
            code: "method_not_allowed",
            message: "HTTP method is not allowed for this route",
        },
        GatewayHttpPlanError::MissingTraceContext => GatewayHttpErrorResponsePlan {
            status: GatewayHttpErrorStatus::BadRequest,
            code: "invalid_trace_context",
            message: "trace context is required and must be valid",
        },
        GatewayHttpPlanError::DuplicateTraceContext => GatewayHttpErrorResponsePlan {
            status: GatewayHttpErrorStatus::BadRequest,
            code: "invalid_trace_context",
            message: "trace context is required and must be valid",
        },
        GatewayHttpPlanError::DuplicateAuthorization
        | GatewayHttpPlanError::DuplicateChatGptAccountId => GatewayHttpErrorResponsePlan {
            status: GatewayHttpErrorStatus::BadRequest,
            code: "credential_header_invalid",
            message: "credential header is invalid",
        },
        GatewayHttpPlanError::DuplicateSessionId
        | GatewayHttpPlanError::DuplicateCodexTurnState => GatewayHttpErrorResponsePlan {
            status: GatewayHttpErrorStatus::BadRequest,
            code: "affinity_header_invalid",
            message: "affinity header is invalid",
        },
        GatewayHttpPlanError::DuplicateCodexMetadata => GatewayHttpErrorResponsePlan {
            status: GatewayHttpErrorStatus::BadRequest,
            code: "codex_metadata_header_invalid",
            message: "Codex metadata header is invalid",
        },
        GatewayHttpPlanError::InvalidTraceContext(error) => {
            gateway_http_response_from_observability(plan_trace_context_error_response(error))
        }
        GatewayHttpPlanError::Policy(_) => GatewayHttpErrorResponsePlan {
            status: GatewayHttpErrorStatus::InternalServerError,
            code: "gateway_http_policy_invalid",
            message: "gateway HTTP policy is invalid",
        },
    }
}

fn gateway_http_response_from_observability(
    response: ObservabilityErrorResponsePlan,
) -> GatewayHttpErrorResponsePlan {
    GatewayHttpErrorResponsePlan {
        status: match response.status {
            ObservabilityErrorStatus::ServiceUnavailable => GatewayHttpErrorStatus::BadRequest,
        },
        code: response.code,
        message: response.message,
    }
}

pub fn plan_gateway_http_request(
    policy: GatewayHttpPolicy,
    request: GatewayHttpRequestMeta,
) -> Result<GatewayHttpPlan, GatewayHttpPlanError> {
    policy.validate().map_err(GatewayHttpPlanError::Policy)?;
    let target = CanonicalRequestTarget::parse(request.path)
        .map_err(GatewayHttpPlanError::InvalidRequestTarget)?;
    if request.body_len > policy.max_body_bytes {
        return Err(GatewayHttpPlanError::BodyTooLarge {
            max: policy.max_body_bytes,
            actual: request.body_len,
        });
    }
    let route = classify_canonical_route(&target);
    validate_method(route, request.method)?;
    validate_singleton_credential_headers(&request.headers)?;
    validate_singleton_affinity_headers(&request.headers)?;
    validate_singleton_codex_metadata_headers(&request.headers)?;
    let trace_context = trace_context_from_headers(&request.headers)?;
    if policy.require_trace_context && requires_trace(route) && trace_context.is_none() {
        return Err(GatewayHttpPlanError::MissingTraceContext);
    }
    let (preserved_upstream_headers, stripped_headers) =
        classify_upstream_headers(&request.headers);
    let trace_propagation_metrics = trace_propagation_metrics_from_headers(&request.headers);
    Ok(GatewayHttpPlan {
        target,
        route,
        trace_context,
        trace_propagation_metrics,
        timeout_budget: GatewayTimeoutBudget {
            request_timeout_ms: policy.request_timeout_ms,
            stream_idle_timeout_ms: policy.stream_idle_timeout_ms,
            connection_drain_timeout_ms: policy.connection_drain_timeout_ms,
        },
        execution: plan_gateway_http_execution(policy, route)?,
        preserved_upstream_headers,
        stripped_headers,
    })
}

pub fn plan_gateway_http_execution(
    policy: GatewayHttpPolicy,
    route: GatewayHttpRouteKind,
) -> Result<GatewayHttpExecutionPlan, GatewayHttpPlanError> {
    policy.validate().map_err(GatewayHttpPlanError::Policy)?;
    let timeout_budget = GatewayTimeoutBudget {
        request_timeout_ms: policy.request_timeout_ms,
        stream_idle_timeout_ms: policy.stream_idle_timeout_ms,
        connection_drain_timeout_ms: policy.connection_drain_timeout_ms,
    };
    Ok(GatewayHttpExecutionPlan {
        route,
        max_body_bytes: policy.max_body_bytes,
        max_concurrent_streams: policy.max_concurrent_streams,
        timeout_budget,
        cancellation_propagation_required: requires_cancellation_propagation(route),
        streaming_backpressure_required: requires_streaming_backpressure(route),
        graceful_drain_required: true,
    })
}

pub fn plan_gateway_http_drain(
    policy: GatewayHttpPolicy,
    prestop_delay_ms: u64,
    termination_grace_ms: u64,
) -> Result<GatewayHttpDrainPlan, GatewayHttpDrainPlanError> {
    policy
        .validate()
        .map_err(GatewayHttpDrainPlanError::Policy)?;
    if prestop_delay_ms == 0 {
        return Err(GatewayHttpDrainPlanError::PreStopDelayRequired);
    }
    let required_ms = policy
        .connection_drain_timeout_ms
        .saturating_add(prestop_delay_ms);
    if termination_grace_ms < required_ms {
        return Err(GatewayHttpDrainPlanError::TerminationGraceTooShort {
            required_ms,
            actual_ms: termination_grace_ms,
        });
    }
    Ok(GatewayHttpDrainPlan {
        connection_drain_timeout_ms: policy.connection_drain_timeout_ms,
        prestop_delay_ms,
        termination_grace_ms,
        readiness_fails_before_drain: true,
    })
}

pub fn plan_gateway_http_response(
    admission: GatewayAdmissionPlan,
    http: GatewayHttpPlan,
) -> GatewayHttpResponsePlan {
    GatewayHttpResponsePlan {
        tenant: admission.tenant,
        route: http.route,
        upstream_headers: http.preserved_upstream_headers,
        trace_context: http.trace_context,
        trace_propagation_metrics: http.trace_propagation_metrics,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayHttpResponsePlan {
    pub tenant: prodex_domain::TenantContext,
    pub route: GatewayHttpRouteKind,
    pub upstream_headers: Vec<GatewayHttpHeader>,
    pub trace_context: Option<TraceContext>,
    pub trace_propagation_metrics: Vec<TracePropagationMetricPlan>,
}
