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

mod request_target;

pub use request_target::{
    CanonicalRequestTarget, CanonicalRequestTargetError, MAX_REQUEST_TARGET_BYTES,
};
mod api_version;
mod edge_security;
mod gateway_admin_route;
mod planning;
mod preconditions;
mod request;
mod route;
mod upstream_headers;

pub use api_version::*;
pub use edge_security::*;
pub use gateway_admin_route::*;
pub use planning::*;
pub use preconditions::*;
pub use request::*;
pub use route::*;
pub use upstream_headers::*;
