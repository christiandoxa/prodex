#[path = "local_rewrite_pipeline_dispatch.rs"]
mod dispatch;
#[path = "local_rewrite_pipeline/errors.rs"]
mod errors;
#[path = "local_rewrite_pipeline_websocket.rs"]
mod websocket;

use dispatch::{
    runtime_local_rewrite_dispatch_builtin_models, runtime_local_rewrite_dispatch_compact,
    runtime_local_rewrite_dispatch_provider,
};
pub(super) use errors::runtime_local_rewrite_request_timeout_response;
use websocket::runtime_local_rewrite_dispatch_websocket;

use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_application_data_plane::RuntimeGatewayApplicationAdmission;
use super::local_rewrite_request::RuntimeLocalRewriteRequest;
use super::local_rewrite_request::runtime_api_route_kind;
use crate::runtime_proxy::{
    RuntimeProxyAdmissionRejection, acquire_runtime_proxy_active_request_slot_with_wait,
    build_runtime_proxy_json_error_response, build_runtime_proxy_text_response,
    mark_runtime_proxy_local_overload, runtime_proxy_error_is_body_too_large,
    runtime_proxy_overloaded_response, runtime_route_kind_label,
};
use crate::runtime_proxy_shared::RuntimeProxyActiveRequestGuard;
use crate::{runtime_proxy_log, runtime_proxy_next_request_id};
use runtime_proxy_crate::{
    RuntimeProxyRequest, path_without_query, runtime_proxy_log_field,
    runtime_proxy_structured_log_message,
};
use std::time::{Duration, Instant};

const RUNTIME_LOCAL_REWRITE_UPSTREAM_REQUEST_FAILED_MESSAGE: &str = "upstream request failed";

#[derive(Default)]
pub(super) struct RuntimeLocalRewritePipelineGuards {
    pub(super) active: Option<RuntimeProxyActiveRequestGuard>,
}

pub(super) struct RuntimeLocalRewriteRequestState {
    pub(super) request: RuntimeLocalRewriteRequest,
    pub(super) path: String,
    pub(super) request_id: u64,
    deadline: Instant,
    pub(super) guards: RuntimeLocalRewritePipelineGuards,
}

pub(super) struct RuntimeLocalRewriteCapturedRequest {
    pub(super) state: RuntimeLocalRewriteRequestState,
    pub(super) captured: RuntimeProxyRequest,
}

pub(super) struct RuntimeLocalRewriteDispatchReadyRequest {
    pub(super) state: RuntimeLocalRewriteRequestState,
    pub(super) captured: RuntimeProxyRequest,
    pub(super) admission: RuntimeGatewayApplicationAdmission,
}

struct RuntimeLocalRewritePipelineReply {
    request: RuntimeLocalRewriteRequest,
    response: tiny_http::ResponseBox,
    _guards: RuntimeLocalRewritePipelineGuards,
}

enum RuntimeLocalRewritePipelineExit {
    Rejected(Box<RuntimeLocalRewritePipelineReply>),
    Responded(Box<RuntimeLocalRewritePipelineReply>),
}

type RuntimeLocalRewritePipelineResult<T> = Result<T, RuntimeLocalRewritePipelineExit>;

impl RuntimeLocalRewriteRequestState {
    pub(super) fn deadline_expired(&self) -> bool {
        Instant::now() >= self.deadline
    }

    fn reply(self, response: tiny_http::ResponseBox) -> RuntimeLocalRewritePipelineReply {
        RuntimeLocalRewritePipelineReply {
            request: self.request,
            response,
            _guards: self.guards,
        }
    }

    fn reject(self, response: tiny_http::ResponseBox) -> RuntimeLocalRewritePipelineExit {
        RuntimeLocalRewritePipelineExit::Rejected(Box::new(self.reply(response)))
    }

    fn respond(self, response: tiny_http::ResponseBox) -> RuntimeLocalRewritePipelineExit {
        RuntimeLocalRewritePipelineExit::Responded(Box::new(self.reply(response)))
    }
}

impl RuntimeLocalRewritePipelineExit {
    pub(super) fn finish(self) {
        let reply = match self {
            Self::Rejected(reply) | Self::Responded(reply) => reply,
        };
        let RuntimeLocalRewritePipelineReply {
            request,
            response,
            _guards,
        } = *reply;
        let _ = request.respond(response);
    }
}

pub(super) fn run_runtime_local_rewrite_pipeline(
    request: RuntimeLocalRewriteRequest,
    target: String,
    shared: &RuntimeLocalRewriteProxyShared,
) {
    if let Err(exit) = try_run_runtime_local_rewrite_pipeline(request, &target, shared) {
        exit.finish();
    }
}

fn try_run_runtime_local_rewrite_pipeline(
    request: RuntimeLocalRewriteRequest,
    target: &str,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<()> {
    let state = runtime_local_rewrite_request_state(request, target, shared);
    let state = runtime_local_rewrite_bounded_admission(state, shared)?;
    let captured = runtime_local_rewrite_capture_body(state, shared)?;
    let admission =
        match RuntimeGatewayApplicationAdmission::from_request(&captured.captured, shared) {
            Ok(admission) => admission,
            Err(_) => {
                return Err(captured
                    .state
                    .reject(build_runtime_proxy_json_error_response(
                        404,
                        "unsupported_provider_route",
                        "provider route is not supported",
                    )));
            }
        };
    let ready = RuntimeLocalRewriteDispatchReadyRequest {
        state: captured.state,
        captured: captured.captured,
        admission,
    };
    let ready = runtime_local_rewrite_dispatch_websocket(ready, shared)?;
    let ready = runtime_local_rewrite_dispatch_compact(ready, shared)?;
    let ready = runtime_local_rewrite_dispatch_builtin_models(ready, shared)?;
    runtime_local_rewrite_dispatch_provider(ready, shared)
}

fn runtime_local_rewrite_request_state(
    request: RuntimeLocalRewriteRequest,
    target: &str,
    _shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewriteRequestState {
    let request_id = runtime_proxy_next_request_id(&_shared.runtime_shared);
    let timeout = Duration::from_millis(120_000);
    RuntimeLocalRewriteRequestState {
        request,
        path: target.to_string(),
        request_id,
        deadline: Instant::now()
            .checked_add(timeout)
            .unwrap_or_else(Instant::now),
        guards: RuntimeLocalRewritePipelineGuards::default(),
    }
}

fn runtime_local_rewrite_bounded_admission(
    mut state: RuntimeLocalRewriteRequestState,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewriteRequestState> {
    if state.deadline_expired() {
        return Err(state.reject(runtime_local_rewrite_request_timeout_response()));
    }
    let websocket = state.request.is_websocket_upgrade();
    let metric_route = runtime_api_route_kind(&state.path, websocket);
    let transport = if websocket { "websocket" } else { "http" };
    state.guards.active = match acquire_runtime_proxy_active_request_slot_with_wait(
        &shared.runtime_shared,
        transport,
        &state.path,
    ) {
        Ok(guard) => Some(guard),
        Err(RuntimeProxyAdmissionRejection::GlobalLimit) => {
            crate::runtime_operational_metrics::record_runtime_api_admission_metric(
                metric_route,
                prodex_observability::ApiAdmissionResult::GlobalLimitReached,
            );
            mark_runtime_proxy_local_overload(&shared.runtime_shared, "active_request_limit");
            let response = runtime_proxy_overloaded_response(
                &shared.runtime_shared,
                &state.path,
                websocket,
                "active_request_limit",
            );
            return Err(state.reject(response));
        }
        Err(RuntimeProxyAdmissionRejection::LaneLimit(lane)) => {
            crate::runtime_operational_metrics::record_runtime_api_admission_metric(
                metric_route,
                prodex_observability::ApiAdmissionResult::RouteLimitReached,
            );
            let reason = format!("lane_limit:{}", runtime_route_kind_label(lane));
            let response = runtime_proxy_overloaded_response(
                &shared.runtime_shared,
                &state.path,
                websocket,
                &reason,
            );
            return Err(state.reject(response));
        }
    };
    crate::runtime_operational_metrics::record_runtime_api_admission_metric(
        metric_route,
        prodex_observability::ApiAdmissionResult::Accepted,
    );
    Ok(state)
}

fn runtime_local_rewrite_capture_body(
    mut state: RuntimeLocalRewriteRequestState,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewriteCapturedRequest> {
    if state.deadline_expired() {
        return Err(state.reject(runtime_local_rewrite_request_timeout_response()));
    }
    let mut captured = if state.request.is_websocket_upgrade() {
        state.request.header_request()
    } else {
        match state
            .request
            .capture(shared.runtime_shared.runtime_config.max_request_body_bytes)
        {
            Ok(captured) => captured,
            Err(err) => {
                let response = runtime_local_rewrite_capture_rejection(&state, shared, &err);
                return Err(state.reject(response));
            }
        }
    };
    captured.path_and_query = state.path.clone();
    Ok(RuntimeLocalRewriteCapturedRequest { state, captured })
}

fn runtime_local_rewrite_capture_rejection(
    state: &RuntimeLocalRewriteRequestState,
    shared: &RuntimeLocalRewriteProxyShared,
    err: &anyhow::Error,
) -> tiny_http::ResponseBox {
    let body_too_large = runtime_proxy_error_is_body_too_large(err);
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            if body_too_large {
                "local_rewrite_request_body_too_large"
            } else {
                "local_rewrite_capture_error"
            },
            [
                runtime_proxy_log_field("request", state.request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("path", path_without_query(&state.path)),
            ],
        ),
    );
    if body_too_large {
        build_runtime_proxy_text_response(413, "proxied request body is too large")
    } else {
        build_runtime_proxy_text_response(502, "gateway request capture failed")
    }
}
