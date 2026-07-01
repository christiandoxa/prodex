#![forbid(unsafe_code)]
//! HTTP data-plane policy boundary for gateway adapters.
//!
//! This crate models route classification, body limits, timeout budgets, trace
//! propagation, and upstream header handling without depending on Axum, Hyper,
//! Tower, Tokio, filesystem, storage drivers, or provider SDKs. Concrete async
//! server crates can translate framework requests into these types.

use std::borrow::Cow;
use std::error::Error;
use std::fmt;

use prodex_domain::{
    ApiVersion, ApiVersionDecision, ApiVersionError, ApiVersionErrorStatus, Cursor, CursorError,
    CursorErrorStatus, EntityTag, EntityTagError, IdempotencyKey, IdempotencyKeyError, PageRequest,
    evaluate_api_version, plan_api_version_error_response, plan_cursor_error_response,
    plan_entity_tag_error_response, plan_idempotency_key_error_response,
};
use prodex_gateway_core::GatewayAdmissionPlan;
use prodex_observability::{
    ObservabilityErrorResponsePlan, ObservabilityErrorStatus, TraceContext, TraceContextError,
    TracePropagationCarrier, TracePropagationMetricPlan, TracePropagationResult,
    plan_trace_context_error_response, plan_trace_propagation_metric,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayHttpMethod {
    Get,
    Post,
    Patch,
    Delete,
    Options,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayHttpHeader {
    pub name: String,
    pub value: String,
}

impl GatewayHttpHeader {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }

    pub fn normalized_name(&self) -> String {
        self.name.trim().to_ascii_lowercase()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayHttpRequestMeta {
    pub method: GatewayHttpMethod,
    pub path: String,
    pub body_len: usize,
    pub headers: Vec<GatewayHttpHeader>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GatewayHttpPolicy {
    pub max_body_bytes: usize,
    pub request_timeout_ms: u64,
    pub stream_idle_timeout_ms: u64,
    pub max_concurrent_streams: u32,
    pub connection_drain_timeout_ms: u64,
    pub require_trace_context: bool,
}

impl GatewayHttpPolicy {
    pub const fn production_default() -> Self {
        Self {
            max_body_bytes: 16 * 1024 * 1024,
            request_timeout_ms: 120_000,
            stream_idle_timeout_ms: 30_000,
            max_concurrent_streams: 1_024,
            connection_drain_timeout_ms: 30_000,
            require_trace_context: true,
        }
    }

    pub fn validate(self) -> Result<(), GatewayHttpPolicyError> {
        if self.max_body_bytes == 0 {
            return Err(GatewayHttpPolicyError::ZeroBodyLimit);
        }
        if self.request_timeout_ms == 0 || self.stream_idle_timeout_ms == 0 {
            return Err(GatewayHttpPolicyError::ZeroTimeout);
        }
        if self.stream_idle_timeout_ms > self.request_timeout_ms {
            return Err(GatewayHttpPolicyError::StreamTimeoutExceedsRequestTimeout);
        }
        if self.max_concurrent_streams == 0 {
            return Err(GatewayHttpPolicyError::ZeroConcurrencyLimit);
        }
        if self.connection_drain_timeout_ms == 0 {
            return Err(GatewayHttpPolicyError::ZeroDrainTimeout);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayHttpPolicyError {
    ZeroBodyLimit,
    ZeroTimeout,
    StreamTimeoutExceedsRequestTimeout,
    ZeroConcurrencyLimit,
    ZeroDrainTimeout,
}

impl fmt::Display for GatewayHttpPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroBodyLimit
            | Self::ZeroTimeout
            | Self::StreamTimeoutExceedsRequestTimeout
            | Self::ZeroConcurrencyLimit
            | Self::ZeroDrainTimeout => write!(f, "gateway HTTP policy is invalid"),
        }
    }
}

impl Error for GatewayHttpPolicyError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayHttpRouteKind {
    DataPlaneResponses,
    DataPlaneCompact,
    DataPlaneWebSocket,
    DataPlaneQuota,
    ControlPlane,
    HealthLive,
    HealthReady,
    HealthStartup,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayControlPlaneOperation {
    GatewayAdminRead,
    TenantCreate,
    TenantUpdate,
    UserInvite,
    ScimUserRead,
    ScimUserCreate,
    ScimUserUpdate,
    ScimUserDelete,
    RoleBindingGrant,
    RoleBindingRevoke,
    ServiceIdentityCreate,
    VirtualKeyRead,
    VirtualKeyCreate,
    VirtualKeyUpdate,
    VirtualKeyDelete,
    VirtualKeyRotateSecret,
    PolicyPublish,
    ProviderCredentialRotate,
    BudgetUpdate,
    BillingRead,
    AuditExport,
    AuditRetentionPurge,
    ConfigurationPublish,
}

impl GatewayControlPlaneOperation {
    pub const fn requires_idempotency(self) -> bool {
        !matches!(
            self,
            Self::GatewayAdminRead
                | Self::ScimUserRead
                | Self::VirtualKeyRead
                | Self::BillingRead
                | Self::AuditExport
        )
    }

    pub const fn requires_audit(self) -> bool {
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayControlPlaneRoutePlan {
    pub operation: GatewayControlPlaneOperation,
    pub requires_idempotency: bool,
    pub requires_audit: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GatewayControlPlaneRouteError {
    NotControlPlaneRoute,
    UnknownControlPlaneRoute,
    MethodNotAllowed {
        operation: GatewayControlPlaneOperation,
        method: GatewayHttpMethod,
    },
}

impl fmt::Display for GatewayControlPlaneRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotControlPlaneRoute | Self::UnknownControlPlaneRoute => {
                write!(f, "HTTP route is invalid")
            }
            Self::MethodNotAllowed { .. } => write!(f, "HTTP method is not allowed"),
        }
    }
}

impl Error for GatewayControlPlaneRouteError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayControlPlaneRouteErrorStatus {
    BadRequest,
    MethodNotAllowed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayControlPlaneRouteErrorResponsePlan {
    pub status: GatewayControlPlaneRouteErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_gateway_control_plane_route_error_response(
    error: &GatewayControlPlaneRouteError,
) -> GatewayControlPlaneRouteErrorResponsePlan {
    match error {
        GatewayControlPlaneRouteError::NotControlPlaneRoute
        | GatewayControlPlaneRouteError::UnknownControlPlaneRoute => {
            GatewayControlPlaneRouteErrorResponsePlan {
                status: GatewayControlPlaneRouteErrorStatus::BadRequest,
                code: "control_plane_route_invalid",
                message: "control-plane route is invalid",
            }
        }
        GatewayControlPlaneRouteError::MethodNotAllowed { .. } => {
            GatewayControlPlaneRouteErrorResponsePlan {
                status: GatewayControlPlaneRouteErrorStatus::MethodNotAllowed,
                code: "control_plane_method_not_allowed",
                message: "HTTP method is not allowed for this control-plane route",
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayHttpPlan {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GatewayHttpIdempotencyKeyError {
    Invalid(IdempotencyKeyError),
    Duplicate,
}

impl fmt::Display for GatewayHttpIdempotencyKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(error) => error.fmt(f),
            Self::Duplicate => write!(f, "request metadata is duplicated"),
        }
    }
}

impl Error for GatewayHttpIdempotencyKeyError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayHttpIdempotencyKeyErrorStatus {
    BadRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayHttpIdempotencyKeyErrorResponsePlan {
    pub status: GatewayHttpIdempotencyKeyErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GatewayHttpEntityTagError {
    Invalid(EntityTagError),
    Duplicate,
}

impl fmt::Display for GatewayHttpEntityTagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(error) => error.fmt(f),
            Self::Duplicate => write!(f, "request metadata is duplicated"),
        }
    }
}

impl Error for GatewayHttpEntityTagError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayHttpEntityTagErrorStatus {
    BadRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayHttpEntityTagErrorResponsePlan {
    pub status: GatewayHttpEntityTagErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GatewayHttpPaginationQueryError {
    Cursor(CursorError),
    InvalidLimit,
    DuplicateLimit,
    DuplicateCursor,
}

impl fmt::Display for GatewayHttpPaginationQueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cursor(error) => error.fmt(f),
            Self::InvalidLimit => write!(f, "pagination metadata is invalid"),
            Self::DuplicateLimit | Self::DuplicateCursor => {
                write!(f, "pagination metadata is duplicated")
            }
        }
    }
}

impl Error for GatewayHttpPaginationQueryError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayHttpPaginationQueryErrorStatus {
    BadRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayHttpPaginationQueryErrorResponsePlan {
    pub status: GatewayHttpPaginationQueryErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_gateway_http_pagination_query_error_response(
    error: &GatewayHttpPaginationQueryError,
) -> GatewayHttpPaginationQueryErrorResponsePlan {
    match error {
        GatewayHttpPaginationQueryError::Cursor(error) => {
            let response = plan_cursor_error_response(error);
            GatewayHttpPaginationQueryErrorResponsePlan {
                status: match response.status {
                    CursorErrorStatus::BadRequest => {
                        GatewayHttpPaginationQueryErrorStatus::BadRequest
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        GatewayHttpPaginationQueryError::InvalidLimit => {
            GatewayHttpPaginationQueryErrorResponsePlan {
                status: GatewayHttpPaginationQueryErrorStatus::BadRequest,
                code: "pagination_limit_invalid",
                message: "pagination limit is invalid",
            }
        }
        GatewayHttpPaginationQueryError::DuplicateLimit => {
            GatewayHttpPaginationQueryErrorResponsePlan {
                status: GatewayHttpPaginationQueryErrorStatus::BadRequest,
                code: "pagination_limit_invalid",
                message: "pagination limit is invalid",
            }
        }
        GatewayHttpPaginationQueryError::DuplicateCursor => {
            GatewayHttpPaginationQueryErrorResponsePlan {
                status: GatewayHttpPaginationQueryErrorStatus::BadRequest,
                code: "pagination_cursor_invalid",
                message: "pagination cursor is invalid",
            }
        }
    }
}

pub fn plan_gateway_http_entity_tag_error_response(
    error: &GatewayHttpEntityTagError,
) -> GatewayHttpEntityTagErrorResponsePlan {
    match error {
        GatewayHttpEntityTagError::Invalid(error) => {
            let response = plan_entity_tag_error_response(error);
            GatewayHttpEntityTagErrorResponsePlan {
                status: GatewayHttpEntityTagErrorStatus::BadRequest,
                code: response.code,
                message: response.message,
            }
        }
        GatewayHttpEntityTagError::Duplicate => GatewayHttpEntityTagErrorResponsePlan {
            status: GatewayHttpEntityTagErrorStatus::BadRequest,
            code: "entity_tag_invalid",
            message: "entity tag is invalid",
        },
    }
}

pub fn plan_gateway_http_idempotency_key_error_response(
    error: &GatewayHttpIdempotencyKeyError,
) -> GatewayHttpIdempotencyKeyErrorResponsePlan {
    match error {
        GatewayHttpIdempotencyKeyError::Invalid(error) => {
            let response = plan_idempotency_key_error_response(error);
            GatewayHttpIdempotencyKeyErrorResponsePlan {
                status: GatewayHttpIdempotencyKeyErrorStatus::BadRequest,
                code: response.code,
                message: response.message,
            }
        }
        GatewayHttpIdempotencyKeyError::Duplicate => GatewayHttpIdempotencyKeyErrorResponsePlan {
            status: GatewayHttpIdempotencyKeyErrorStatus::BadRequest,
            code: "idempotency_key_invalid",
            message: "idempotency key is invalid",
        },
    }
}

pub fn idempotency_key_from_headers(
    headers: &[GatewayHttpHeader],
) -> Result<Option<IdempotencyKey>, GatewayHttpIdempotencyKeyError> {
    let mut values = headers
        .iter()
        .filter(|header| header.normalized_name() == "idempotency-key");
    let Some(header) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(GatewayHttpIdempotencyKeyError::Duplicate);
    }
    IdempotencyKey::new(&header.value)
        .map(Some)
        .map_err(GatewayHttpIdempotencyKeyError::Invalid)
}

pub fn entity_tag_from_if_match_headers(
    headers: &[GatewayHttpHeader],
) -> Result<Option<EntityTag>, GatewayHttpEntityTagError> {
    let mut values = headers
        .iter()
        .filter(|header| header.normalized_name() == "if-match");
    let Some(header) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(GatewayHttpEntityTagError::Duplicate);
    }
    EntityTag::new(&header.value)
        .map(Some)
        .map_err(GatewayHttpEntityTagError::Invalid)
}

pub fn page_request_from_query(
    query: &str,
) -> Result<PageRequest, GatewayHttpPaginationQueryError> {
    let mut limit = None;
    let mut cursor = None;
    for pair in query
        .trim_start_matches('?')
        .split('&')
        .filter(|pair| !pair.is_empty())
    {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        match percent_decode_query_name(name).as_ref() {
            "limit" => {
                if limit.is_some() {
                    return Err(GatewayHttpPaginationQueryError::DuplicateLimit);
                }
                limit = Some(
                    value
                        .parse::<u16>()
                        .map_err(|_| GatewayHttpPaginationQueryError::InvalidLimit)?,
                );
            }
            "cursor" => {
                if cursor.is_some() {
                    return Err(GatewayHttpPaginationQueryError::DuplicateCursor);
                }
                cursor = Some(
                    Cursor::new(value.to_string())
                        .map_err(GatewayHttpPaginationQueryError::Cursor)?,
                );
            }
            _ => {}
        }
    }
    Ok(PageRequest::new(limit, cursor))
}

fn percent_decode_query_name(name: &str) -> Cow<'_, str> {
    if !name.as_bytes().contains(&b'%') {
        return Cow::Borrowed(name);
    }
    let bytes = name.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
            {
                decoded.push((high << 4) | low);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    Cow::Owned(String::from_utf8_lossy(&decoded).into_owned())
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GatewayHttpRequestFingerprintError {
    EmptyPath,
    EmptyBodyDigest,
    InvalidBodyDigestCharacter,
}

impl fmt::Display for GatewayHttpRequestFingerprintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPath | Self::EmptyBodyDigest | Self::InvalidBodyDigestCharacter => {
                write!(f, "request fingerprint is invalid")
            }
        }
    }
}

impl Error for GatewayHttpRequestFingerprintError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayHttpRequestFingerprintErrorStatus {
    BadRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayHttpRequestFingerprintErrorResponsePlan {
    pub status: GatewayHttpRequestFingerprintErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_gateway_http_request_fingerprint_error_response(
    _error: &GatewayHttpRequestFingerprintError,
) -> GatewayHttpRequestFingerprintErrorResponsePlan {
    GatewayHttpRequestFingerprintErrorResponsePlan {
        status: GatewayHttpRequestFingerprintErrorStatus::BadRequest,
        code: "request_fingerprint_invalid",
        message: "request fingerprint is invalid",
    }
}

pub fn control_plane_request_fingerprint(
    request: &GatewayHttpRequestMeta,
    body_digest: impl AsRef<str>,
) -> Result<String, GatewayHttpRequestFingerprintError> {
    let path = request.path.trim();
    if path.is_empty() {
        return Err(GatewayHttpRequestFingerprintError::EmptyPath);
    }
    let body_digest = body_digest.as_ref().trim();
    if body_digest.is_empty() {
        return Err(GatewayHttpRequestFingerprintError::EmptyBodyDigest);
    }
    if body_digest
        .chars()
        .any(|character| !character.is_ascii_graphic())
    {
        return Err(GatewayHttpRequestFingerprintError::InvalidBodyDigestCharacter);
    }
    Ok(format!(
        "http:{method}:path:{path}:body:{body_digest}",
        method = method_fingerprint_token(request.method),
    ))
}

fn method_fingerprint_token(method: GatewayHttpMethod) -> &'static str {
    match method {
        GatewayHttpMethod::Get => "get",
        GatewayHttpMethod::Post => "post",
        GatewayHttpMethod::Patch => "patch",
        GatewayHttpMethod::Delete => "delete",
        GatewayHttpMethod::Options => "options",
        GatewayHttpMethod::Other => "other",
    }
}

pub fn plan_gateway_http_error_response(
    error: &GatewayHttpPlanError,
) -> GatewayHttpErrorResponsePlan {
    match error {
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
    if request.body_len > policy.max_body_bytes {
        return Err(GatewayHttpPlanError::BodyTooLarge {
            max: policy.max_body_bytes,
            actual: request.body_len,
        });
    }

    let route = classify_route(&request.path);
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GatewayHttpApiVersionPlan {
    pub requested: ApiVersion,
    pub decision: ApiVersionDecision,
    pub explicit_path_version: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayHttpApiVersionErrorStatus {
    NotFound,
    Gone,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayHttpApiVersionErrorResponsePlan {
    pub status: GatewayHttpApiVersionErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_gateway_http_api_version(
    path: &str,
    default_version: ApiVersion,
    policies: &[prodex_domain::ApiVersionPolicy],
    now_unix_ms: u64,
) -> Result<GatewayHttpApiVersionPlan, ApiVersionError> {
    let (requested, explicit_path_version) = requested_api_version_for_path(path, default_version);
    let decision = evaluate_api_version(requested, policies, now_unix_ms)?;
    Ok(GatewayHttpApiVersionPlan {
        requested,
        decision,
        explicit_path_version,
    })
}

pub fn plan_gateway_http_api_version_error_response(
    error: &ApiVersionError,
) -> GatewayHttpApiVersionErrorResponsePlan {
    let response = plan_api_version_error_response(error);
    GatewayHttpApiVersionErrorResponsePlan {
        status: match response.status {
            ApiVersionErrorStatus::NotFound => GatewayHttpApiVersionErrorStatus::NotFound,
            ApiVersionErrorStatus::Gone => GatewayHttpApiVersionErrorStatus::Gone,
        },
        code: response.code,
        message: response.message,
    }
}

fn requested_api_version_for_path(path: &str, default_version: ApiVersion) -> (ApiVersion, bool) {
    let path = path_without_query_or_fragment(path);
    let first_segment = path
        .strip_prefix('/')
        .unwrap_or(path)
        .split('/')
        .next()
        .unwrap_or_default();
    let Some(version_digits) = first_segment.strip_prefix('v') else {
        return (default_version, false);
    };
    if version_digits.is_empty()
        || !version_digits
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        return (default_version, false);
    }
    match version_digits.parse::<u16>() {
        Ok(major) => (ApiVersion::new(major, 0), true),
        Err(_) => (ApiVersion::new(u16::MAX, 0), true),
    }
}

pub fn classify_route(path: &str) -> GatewayHttpRouteKind {
    let path = canonical_control_plane_path(path);
    match path.as_str() {
        "/livez" => GatewayHttpRouteKind::HealthLive,
        "/readyz" => GatewayHttpRouteKind::HealthReady,
        "/startupz" => GatewayHttpRouteKind::HealthStartup,
        "/v1/responses" | "/responses" => GatewayHttpRouteKind::DataPlaneResponses,
        "/v1/responses/compact" | "/responses/compact" => GatewayHttpRouteKind::DataPlaneCompact,
        "/v1/realtime" | "/realtime" => GatewayHttpRouteKind::DataPlaneWebSocket,
        "/v1/quota" | "/quota" => GatewayHttpRouteKind::DataPlaneQuota,
        _ if is_control_plane_route(&path) => GatewayHttpRouteKind::ControlPlane,
        _ => GatewayHttpRouteKind::Unknown,
    }
}

pub fn plan_control_plane_route(
    request: &GatewayHttpRequestMeta,
) -> Result<GatewayControlPlaneRoutePlan, GatewayControlPlaneRouteError> {
    if classify_route(&request.path) != GatewayHttpRouteKind::ControlPlane {
        return Err(GatewayControlPlaneRouteError::NotControlPlaneRoute);
    }
    let operation = control_plane_operation_for_path(&request.path, request.method)
        .ok_or(GatewayControlPlaneRouteError::UnknownControlPlaneRoute)?;
    if !control_plane_operation_allows_method(operation, request.method) {
        return Err(GatewayControlPlaneRouteError::MethodNotAllowed {
            operation,
            method: request.method,
        });
    }
    Ok(GatewayControlPlaneRoutePlan {
        operation,
        requires_idempotency: operation.requires_idempotency(),
        requires_audit: operation.requires_audit(),
    })
}

fn is_control_plane_route(path: &str) -> bool {
    let path = canonical_control_plane_path(path);
    let path = path.as_str();
    matches_control_plane_mount(path, "/admin")
        || matches_control_plane_mount(path, "/v1/admin")
        || matches_control_plane_mount(path, "/scim")
        || matches_control_plane_mount(path, "/v1/scim")
}

fn matches_control_plane_mount(path: &str, mount: &str) -> bool {
    path == mount
        || path
            .strip_prefix(mount)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn control_plane_operation_for_path(
    path: &str,
    method: GatewayHttpMethod,
) -> Option<GatewayControlPlaneOperation> {
    let path = canonical_control_plane_path(path);
    let path = path.as_str();
    if let Some(scim_path) = strip_control_plane_mount(path, "/scim")
        .or_else(|| strip_control_plane_mount(path, "/v1/scim"))
    {
        return match normalized_segments(scim_path).as_slice() {
            ["v2", "Users"] if method == GatewayHttpMethod::Get => {
                Some(GatewayControlPlaneOperation::ScimUserRead)
            }
            ["v2", "Users"] => Some(GatewayControlPlaneOperation::ScimUserCreate),
            ["v2", "Users", ..] if method == GatewayHttpMethod::Get => {
                Some(GatewayControlPlaneOperation::ScimUserRead)
            }
            ["v2", "Users", ..] if method == GatewayHttpMethod::Delete => {
                Some(GatewayControlPlaneOperation::ScimUserDelete)
            }
            ["v2", "Users", ..] => Some(GatewayControlPlaneOperation::ScimUserUpdate),
            _ => None,
        };
    }

    let admin_path = strip_control_plane_mount(path, "/admin")
        .or_else(|| strip_control_plane_mount(path, "/v1/admin"))?;
    match normalized_segments(admin_path).as_slice() {
        []
        | ["openapi.json"]
        | ["metrics"]
        | ["providers"]
        | ["observability"]
        | ["guardrails"]
        | ["usage"] => Some(GatewayControlPlaneOperation::GatewayAdminRead),
        ["tenants"] => Some(GatewayControlPlaneOperation::TenantCreate),
        ["tenants", ..] => Some(GatewayControlPlaneOperation::TenantUpdate),
        ["users"] | ["users", "invites"] => Some(GatewayControlPlaneOperation::UserInvite),
        ["role-bindings"] => Some(GatewayControlPlaneOperation::RoleBindingGrant),
        ["role-bindings", ..] => Some(GatewayControlPlaneOperation::RoleBindingRevoke),
        ["service-identities"] => Some(GatewayControlPlaneOperation::ServiceIdentityCreate),
        ["keys", _, "secret"] | ["keys", _, "secrets"] => {
            Some(GatewayControlPlaneOperation::VirtualKeyRotateSecret)
        }
        ["keys"] if method == GatewayHttpMethod::Get => {
            Some(GatewayControlPlaneOperation::VirtualKeyRead)
        }
        ["keys"] => Some(GatewayControlPlaneOperation::VirtualKeyCreate),
        ["keys", ..] if method == GatewayHttpMethod::Get => {
            Some(GatewayControlPlaneOperation::VirtualKeyRead)
        }
        ["keys", ..] if method == GatewayHttpMethod::Patch => {
            Some(GatewayControlPlaneOperation::VirtualKeyUpdate)
        }
        ["keys", ..] if method == GatewayHttpMethod::Delete => {
            Some(GatewayControlPlaneOperation::VirtualKeyDelete)
        }
        ["provider-credentials", _, "secret"] | ["provider-credentials", _, "secrets"] => {
            Some(GatewayControlPlaneOperation::ProviderCredentialRotate)
        }
        ["budgets", ..] => Some(GatewayControlPlaneOperation::BudgetUpdate),
        ["policies", ..] => Some(GatewayControlPlaneOperation::PolicyPublish),
        ["configuration", ..] | ["config", ..] => {
            Some(GatewayControlPlaneOperation::ConfigurationPublish)
        }
        ["billing", ..] => Some(GatewayControlPlaneOperation::BillingRead),
        ["ledger"] | ["ledger.csv"] | ["ledger", ..] => {
            Some(GatewayControlPlaneOperation::BillingRead)
        }
        ["audit", "exports"] | ["audit", "exports", ..] => {
            Some(GatewayControlPlaneOperation::AuditExport)
        }
        ["audit", "retention"] | ["audit", "retention", ..] => {
            Some(GatewayControlPlaneOperation::AuditRetentionPurge)
        }
        _ => None,
    }
}

fn canonical_control_plane_path(path: &str) -> String {
    let path = path_without_query_or_fragment(path);
    for mount in ["/prodex/gateway", "/v1/prodex/gateway"] {
        if path == mount {
            return "/admin".to_string();
        }
        let Some(suffix) = path
            .strip_prefix(mount)
            .and_then(|suffix| suffix.strip_prefix('/'))
        else {
            continue;
        };
        if suffix.starts_with("scim/") {
            return format!("/{suffix}");
        }
        if suffix == "admin" {
            return "/admin".to_string();
        }
        return format!("/admin/{suffix}");
    }
    path.to_string()
}

fn path_without_query_or_fragment(path: &str) -> &str {
    path.trim()
        .split_once(['?', '#'])
        .map_or_else(|| path.trim(), |(path, _)| path)
}

fn strip_control_plane_mount<'a>(path: &'a str, mount: &str) -> Option<&'a str> {
    if path == mount {
        Some("")
    } else {
        path.strip_prefix(mount)
            .and_then(|suffix| suffix.strip_prefix('/'))
    }
}

fn normalized_segments(path: &str) -> Vec<&str> {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn control_plane_operation_allows_method(
    operation: GatewayControlPlaneOperation,
    method: GatewayHttpMethod,
) -> bool {
    match operation {
        GatewayControlPlaneOperation::GatewayAdminRead
        | GatewayControlPlaneOperation::ScimUserRead
        | GatewayControlPlaneOperation::VirtualKeyRead
        | GatewayControlPlaneOperation::BillingRead => method == GatewayHttpMethod::Get,
        GatewayControlPlaneOperation::TenantCreate
        | GatewayControlPlaneOperation::UserInvite
        | GatewayControlPlaneOperation::ScimUserCreate
        | GatewayControlPlaneOperation::RoleBindingGrant
        | GatewayControlPlaneOperation::ServiceIdentityCreate
        | GatewayControlPlaneOperation::VirtualKeyCreate
        | GatewayControlPlaneOperation::VirtualKeyRotateSecret
        | GatewayControlPlaneOperation::ProviderCredentialRotate
        | GatewayControlPlaneOperation::PolicyPublish
        | GatewayControlPlaneOperation::ConfigurationPublish
        | GatewayControlPlaneOperation::AuditExport => method == GatewayHttpMethod::Post,
        GatewayControlPlaneOperation::TenantUpdate
        | GatewayControlPlaneOperation::ScimUserUpdate
        | GatewayControlPlaneOperation::VirtualKeyUpdate
        | GatewayControlPlaneOperation::BudgetUpdate => method == GatewayHttpMethod::Patch,
        GatewayControlPlaneOperation::ScimUserDelete
        | GatewayControlPlaneOperation::RoleBindingRevoke
        | GatewayControlPlaneOperation::VirtualKeyDelete
        | GatewayControlPlaneOperation::AuditRetentionPurge => method == GatewayHttpMethod::Delete,
    }
}

fn validate_method(
    route: GatewayHttpRouteKind,
    method: GatewayHttpMethod,
) -> Result<(), GatewayHttpPlanError> {
    let allowed = match route {
        GatewayHttpRouteKind::HealthLive
        | GatewayHttpRouteKind::HealthReady
        | GatewayHttpRouteKind::HealthStartup => method == GatewayHttpMethod::Get,
        GatewayHttpRouteKind::DataPlaneResponses | GatewayHttpRouteKind::DataPlaneCompact => {
            method == GatewayHttpMethod::Post
        }
        GatewayHttpRouteKind::DataPlaneWebSocket | GatewayHttpRouteKind::DataPlaneQuota => {
            method == GatewayHttpMethod::Get
        }
        GatewayHttpRouteKind::ControlPlane => matches!(
            method,
            GatewayHttpMethod::Get
                | GatewayHttpMethod::Post
                | GatewayHttpMethod::Patch
                | GatewayHttpMethod::Delete
                | GatewayHttpMethod::Options
        ),
        GatewayHttpRouteKind::Unknown => true,
    };
    if allowed {
        Ok(())
    } else {
        Err(GatewayHttpPlanError::MethodNotAllowed { route, method })
    }
}

fn requires_trace(route: GatewayHttpRouteKind) -> bool {
    matches!(
        route,
        GatewayHttpRouteKind::DataPlaneResponses
            | GatewayHttpRouteKind::DataPlaneCompact
            | GatewayHttpRouteKind::DataPlaneWebSocket
            | GatewayHttpRouteKind::DataPlaneQuota
            | GatewayHttpRouteKind::ControlPlane
    )
}

fn requires_cancellation_propagation(route: GatewayHttpRouteKind) -> bool {
    matches!(
        route,
        GatewayHttpRouteKind::DataPlaneResponses
            | GatewayHttpRouteKind::DataPlaneCompact
            | GatewayHttpRouteKind::DataPlaneWebSocket
    )
}

fn requires_streaming_backpressure(route: GatewayHttpRouteKind) -> bool {
    matches!(
        route,
        GatewayHttpRouteKind::DataPlaneResponses | GatewayHttpRouteKind::DataPlaneWebSocket
    )
}

fn trace_context_from_headers(
    headers: &[GatewayHttpHeader],
) -> Result<Option<TraceContext>, GatewayHttpPlanError> {
    let mut values = headers
        .iter()
        .filter(|header| header.normalized_name() == "traceparent");
    let Some(header) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(GatewayHttpPlanError::DuplicateTraceContext);
    }
    TraceContext::parse_traceparent(&header.value)
        .map(Some)
        .map_err(GatewayHttpPlanError::InvalidTraceContext)
}

fn trace_propagation_metrics_from_headers(
    headers: &[GatewayHttpHeader],
) -> Vec<TracePropagationMetricPlan> {
    [
        (TracePropagationCarrier::Traceparent, "traceparent"),
        (TracePropagationCarrier::Tracestate, "tracestate"),
        (TracePropagationCarrier::Baggage, "baggage"),
    ]
    .into_iter()
    .filter_map(|(carrier, name)| {
        let result = if headers
            .iter()
            .any(|header| header.normalized_name() == name)
        {
            TracePropagationResult::Propagated
        } else {
            TracePropagationResult::Missing
        };
        plan_trace_propagation_metric(carrier, result).ok()
    })
    .collect()
}

fn validate_singleton_credential_headers(
    headers: &[GatewayHttpHeader],
) -> Result<(), GatewayHttpPlanError> {
    let authorization_count = headers
        .iter()
        .filter(|header| header.normalized_name() == "authorization")
        .take(2)
        .count();
    if authorization_count > 1 {
        return Err(GatewayHttpPlanError::DuplicateAuthorization);
    }

    let account_count = headers
        .iter()
        .filter(|header| header.normalized_name() == "chatgpt-account-id")
        .take(2)
        .count();
    if account_count > 1 {
        return Err(GatewayHttpPlanError::DuplicateChatGptAccountId);
    }

    Ok(())
}

fn validate_singleton_affinity_headers(
    headers: &[GatewayHttpHeader],
) -> Result<(), GatewayHttpPlanError> {
    let session_count = headers
        .iter()
        .filter(|header| header.normalized_name() == "session_id")
        .take(2)
        .count();
    if session_count > 1 {
        return Err(GatewayHttpPlanError::DuplicateSessionId);
    }

    let turn_state_count = headers
        .iter()
        .filter(|header| header.normalized_name() == "x-codex-turn-state")
        .take(2)
        .count();
    if turn_state_count > 1 {
        return Err(GatewayHttpPlanError::DuplicateCodexTurnState);
    }

    Ok(())
}

fn validate_singleton_codex_metadata_headers(
    headers: &[GatewayHttpHeader],
) -> Result<(), GatewayHttpPlanError> {
    for name in [
        "x-openai-subagent",
        "x-codex-turn-metadata",
        "x-codex-beta-features",
    ] {
        let count = headers
            .iter()
            .filter(|header| header.normalized_name() == name)
            .take(2)
            .count();
        if count > 1 {
            return Err(GatewayHttpPlanError::DuplicateCodexMetadata);
        }
    }
    Ok(())
}

pub fn classify_upstream_headers(
    headers: &[GatewayHttpHeader],
) -> (Vec<GatewayHttpHeader>, Vec<String>) {
    let mut preserved = Vec::new();
    let mut stripped = Vec::new();
    for header in headers {
        let normalized = header.normalized_name();
        if preserve_upstream_header(&normalized) {
            preserved.push(GatewayHttpHeader::new(normalized, header.value.clone()));
        } else if strip_upstream_header(&normalized) {
            stripped.push(normalized);
        }
    }
    (preserved, stripped)
}

fn preserve_upstream_header(name: &str) -> bool {
    matches!(
        name,
        "session_id"
            | "x-openai-subagent"
            | "x-codex-turn-state"
            | "x-codex-turn-metadata"
            | "x-codex-beta-features"
            | "user-agent"
            | "traceparent"
            | "tracestate"
            | "baggage"
    )
}

fn strip_upstream_header(name: &str) -> bool {
    name == "host"
        || name == "connection"
        || name == "content-length"
        || name == "transfer-encoding"
        || name == "upgrade"
        || name.starts_with("sec-websocket-")
        || name == "authorization"
        || name == "chatgpt-account-id"
}
