#[path = "local_rewrite_pipeline_dispatch.rs"]
mod dispatch;
#[path = "local_rewrite_pipeline_governance.rs"]
mod governance;

#[cfg(test)]
pub(super) use dispatch::runtime_local_rewrite_remote_compact_unsupported_message;
use dispatch::{
    runtime_gateway_operational_probe_response, runtime_local_rewrite_dispatch_builtin_models,
    runtime_local_rewrite_dispatch_compact, runtime_local_rewrite_dispatch_provider,
};
use governance::{
    runtime_local_rewrite_apply_constraints, runtime_local_rewrite_dispatch_control_plane,
    runtime_local_rewrite_post_reservation_governance,
    runtime_local_rewrite_pre_reservation_governance, runtime_local_rewrite_prepare_constraints,
    runtime_local_rewrite_reserve_virtual_key,
};

use super::local_rewrite::{
    RUNTIME_GATEWAY_CONVERSATION_NAMESPACE_HEADER, RuntimeLocalRewriteProxyShared,
};
use super::local_rewrite_application_boundary::{
    RuntimeGatewayAdminPreauthorization, RuntimeGatewayApplicationBoundaryError,
    runtime_gateway_admin_preauthorization, runtime_gateway_application_data_plane_authorization,
    runtime_gateway_application_request_context, runtime_gateway_data_plane_credential,
};
use super::local_rewrite_application_data_plane::{
    RuntimeGatewayApplicationAdmission, runtime_gateway_application_http_policy,
    runtime_gateway_application_local_admission, runtime_gateway_application_provider_dispatch,
};
use super::local_rewrite_constraints::{
    RuntimeGatewayPendingConstraintPlan, runtime_gateway_prepare_constraint_plan,
};
use super::local_rewrite_copilot::runtime_copilot_model_catalog_from_provider;
use super::local_rewrite_gateway_admin_audit::runtime_gateway_audit_admin_auth_event;
use super::local_rewrite_gateway_admin_auth::runtime_gateway_admin_auth;
use super::local_rewrite_gateway_admin_dispatch::runtime_gateway_respond_route_explain;
use super::local_rewrite_gateway_admin_router::{
    runtime_gateway_admin_authorization_rejection_response, runtime_gateway_admin_response,
    runtime_gateway_http_request_meta, runtime_gateway_request_path_is_admin,
    runtime_gateway_request_path_is_route_explain,
    runtime_gateway_request_path_requires_admin_auth,
};
use super::local_rewrite_gateway_data_plane_audit::{
    runtime_gateway_audit_data_plane_auth_failed,
    runtime_gateway_audit_data_plane_guardrail_blocked,
    runtime_gateway_audit_data_plane_guardrail_webhook_blocked,
    runtime_gateway_audit_data_plane_presidio_redaction_failed,
    runtime_gateway_audit_data_plane_request_body_too_large,
    runtime_gateway_audit_data_plane_request_capture_failed,
    runtime_gateway_audit_data_plane_virtual_key_rejected,
};
use super::local_rewrite_gateway_guardrail_webhook::runtime_gateway_guardrail_webhook_block;
use super::local_rewrite_gateway_keys::{
    runtime_gateway_request_header_virtual_key, runtime_gateway_virtual_key_admission,
    runtime_gateway_virtual_key_entries_is_empty, runtime_local_rewrite_request_is_authorized,
};
use super::local_rewrite_gateway_route_load::RuntimeGatewayRouteLoadGuard;
use super::local_rewrite_gateway_usage::RuntimeGatewayUsageRequestGuard;
use super::local_rewrite_gemini_compact::respond_runtime_gemini_compact_request;
use super::local_rewrite_gemini_live::handle_runtime_gemini_live_websocket_request;
use super::local_rewrite_kiro::{
    runtime_kiro_compact_response_parts, runtime_kiro_model_catalog_from_provider,
    runtime_kiro_models_buffered_response,
};
use super::local_rewrite_options::RuntimeLocalRewriteProviderOptions;
use super::local_rewrite_request::RuntimeLocalRewriteRequest;
use super::local_rewrite_response::{
    respond_runtime_local_rewrite_proxy_request, runtime_local_rewrite_response_with_call_id,
};
use super::local_rewrite_upstream::send_runtime_local_rewrite_upstream_request;
use super::provider_bridge::{
    runtime_provider_models_buffered_response, runtime_provider_request_ledger_message,
};
use crate::runtime_proxy::{
    RuntimeProxyAdmissionRejection, acquire_runtime_proxy_active_request_slot_with_wait,
    build_runtime_proxy_json_error_response, build_runtime_proxy_text_response,
    mark_runtime_proxy_local_overload, runtime_proxy_error_is_body_too_large,
    runtime_proxy_overloaded_response, runtime_route_kind_label,
};
use crate::runtime_proxy_shared::RuntimeProxyActiveRequestGuard;
use crate::{runtime_proxy_log, runtime_proxy_next_request_id};
use prodex_application::{
    ApplicationInspectionPlan, ApplicationRequestContextError, ApplicationRequestDeadline,
};
use prodex_domain::RequestId;
use redaction::redaction_redact_secret_like_text;
use runtime_proxy_crate::{
    RuntimeProxyRequest, is_runtime_realtime_websocket_path, path_without_query,
    runtime_proxy_log_field, runtime_proxy_structured_log_message,
};
use std::time::{Duration, Instant};

const RUNTIME_LOCAL_REWRITE_UPSTREAM_REQUEST_FAILED_MESSAGE: &str = "upstream request failed";

#[derive(Default)]
struct RuntimeLocalRewritePipelineGuards {
    active: Option<RuntimeProxyActiveRequestGuard>,
    usage: Option<RuntimeGatewayUsageRequestGuard>,
    route_load: Option<RuntimeGatewayRouteLoadGuard>,
}

struct RuntimeLocalRewriteRequestState<'target> {
    request: RuntimeLocalRewriteRequest,
    context: prodex_application::ApplicationRequestContext<'target>,
    path: String,
    request_id: u64,
    admin: Option<RuntimeGatewayAdminPreauthorization<'target>>,
    application: Option<prodex_application::ApplicationAuthorizedRequestContext<'target>>,
    guards: RuntimeLocalRewritePipelineGuards,
}

struct RuntimeLocalRewriteCanonicalRequest<'target>(RuntimeLocalRewriteRequestState<'target>);
struct RuntimeLocalRewriteAuthenticatedRequest<'target>(RuntimeLocalRewriteRequestState<'target>);
struct RuntimeLocalRewriteAdmittedRequest<'target>(RuntimeLocalRewriteRequestState<'target>);

struct RuntimeLocalRewriteCapturedRequest<'target> {
    state: RuntimeLocalRewriteRequestState<'target>,
    captured: RuntimeProxyRequest,
}

struct RuntimeLocalRewritePreparedRequest<'target, 'shared> {
    state: RuntimeLocalRewriteRequestState<'target>,
    captured: RuntimeProxyRequest,
    constraints: Option<RuntimeGatewayPendingConstraintPlan<'shared>>,
    inspection: ApplicationInspectionPlan,
}

impl RuntimeLocalRewritePreparedRequest<'_, '_> {
    fn reject(self, response: tiny_http::ResponseBox) -> RuntimeLocalRewritePipelineExit {
        self.state.reject(response).finish();
        RuntimeLocalRewritePipelineExit::Handled
    }

    fn respond(self, response: tiny_http::ResponseBox) -> RuntimeLocalRewritePipelineExit {
        self.state.respond(response).finish();
        RuntimeLocalRewritePipelineExit::Handled
    }
}

struct RuntimeLocalRewriteGovernedRequest<'target, 'shared>(
    RuntimeLocalRewritePreparedRequest<'target, 'shared>,
);
struct RuntimeLocalRewriteReservedRequest<'target, 'shared> {
    request: RuntimeLocalRewritePreparedRequest<'target, 'shared>,
    application_admission: RuntimeGatewayApplicationAdmission,
}

struct RuntimeLocalRewriteDispatchReadyRequest<'target> {
    state: RuntimeLocalRewriteRequestState<'target>,
    captured: RuntimeProxyRequest,
    application_admission: RuntimeGatewayApplicationAdmission,
}

struct RuntimeLocalRewritePipelineReply {
    request: RuntimeLocalRewriteRequest,
    response: tiny_http::ResponseBox,
    _guards: RuntimeLocalRewritePipelineGuards,
}

enum RuntimeLocalRewritePipelineExit {
    Rejected(Box<RuntimeLocalRewritePipelineReply>),
    Responded(Box<RuntimeLocalRewritePipelineReply>),
    Handled,
}

type RuntimeLocalRewritePipelineResult<T> = Result<T, RuntimeLocalRewritePipelineExit>;

impl RuntimeLocalRewriteRequestState<'_> {
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
    fn finish(self) {
        let reply = match self {
            Self::Rejected(reply) | Self::Responded(reply) => reply,
            Self::Handled => return,
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
    target: prodex_gateway_http::CanonicalRequestTarget,
    shared: &RuntimeLocalRewriteProxyShared,
) {
    if let Err(exit) = try_run_runtime_local_rewrite_pipeline(request, &target, shared) {
        exit.finish();
    }
}

fn try_run_runtime_local_rewrite_pipeline(
    request: RuntimeLocalRewriteRequest,
    target: &prodex_gateway_http::CanonicalRequestTarget,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<()> {
    let canonical = runtime_local_rewrite_canonical_context(request, target, shared)?;
    let authenticated = runtime_local_rewrite_authenticate(canonical, shared)?;
    let admitted = runtime_local_rewrite_bounded_admission(authenticated, shared)?;
    let admitted = runtime_local_rewrite_dispatch_websocket(admitted, shared)?;
    let captured = runtime_local_rewrite_capture_body(admitted, shared)?;
    let prepared = runtime_local_rewrite_prepare_constraints(captured, shared)?;
    let prepared = runtime_local_rewrite_dispatch_control_plane(prepared, shared)?;
    let governed = runtime_local_rewrite_pre_reservation_governance(prepared, shared)?;
    let reserved = runtime_local_rewrite_reserve_virtual_key(governed, shared)?;
    let reserved = runtime_local_rewrite_post_reservation_governance(reserved, shared)?;
    let ready = runtime_local_rewrite_apply_constraints(reserved)?;
    let ready = runtime_local_rewrite_dispatch_compact(ready, shared)?;
    let ready = runtime_local_rewrite_dispatch_builtin_models(ready, shared)?;
    runtime_local_rewrite_dispatch_provider(ready, shared)
}

fn runtime_local_rewrite_canonical_context<'target>(
    request: RuntimeLocalRewriteRequest,
    target: &'target prodex_gateway_http::CanonicalRequestTarget,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewriteCanonicalRequest<'target>> {
    let path = target.path_and_query().to_string();
    let request_id = if runtime_gateway_request_path_is_route_explain(&path, shared) {
        0
    } else {
        runtime_proxy_next_request_id(&shared.runtime_shared)
    };
    let typed_request_id = RequestId::new();
    let started_at = Instant::now();
    let request_timeout =
        Duration::from_millis(runtime_gateway_application_http_policy(shared).request_timeout_ms);
    let deadline = ApplicationRequestDeadline::at(
        started_at
            .checked_add(request_timeout)
            .unwrap_or(started_at),
    );
    let header_request = request.header_request();
    let context = match runtime_gateway_application_request_context(
        target,
        typed_request_id,
        deadline,
        &header_request.headers,
    ) {
        Ok(context) => context,
        Err(error) => {
            return Err(RuntimeLocalRewritePipelineExit::Rejected(Box::new(
                RuntimeLocalRewritePipelineReply {
                    request,
                    response: runtime_local_rewrite_application_context_rejection(error),
                    _guards: RuntimeLocalRewritePipelineGuards::default(),
                },
            )));
        }
    };
    let state = RuntimeLocalRewriteRequestState {
        request,
        context,
        path,
        request_id,
        admin: None,
        application: None,
        guards: RuntimeLocalRewritePipelineGuards::default(),
    };
    if let Some(response) =
        runtime_gateway_operational_probe_response(state.request.method(), &state.path, shared)
    {
        return Err(state.respond(response));
    }
    Ok(RuntimeLocalRewriteCanonicalRequest(state))
}

fn runtime_local_rewrite_application_context_rejection(
    error: ApplicationRequestContextError,
) -> tiny_http::ResponseBox {
    let ApplicationRequestContextError::Trace(error) = error else {
        return build_runtime_proxy_json_error_response(
            404,
            "route_not_available",
            "route is not available",
        );
    };
    let response = prodex_gateway_http::plan_gateway_http_error_response(&error);
    let status = match response.status {
        prodex_gateway_http::GatewayHttpErrorStatus::BadRequest => 400,
        prodex_gateway_http::GatewayHttpErrorStatus::MethodNotAllowed => 405,
        prodex_gateway_http::GatewayHttpErrorStatus::PayloadTooLarge => 413,
        prodex_gateway_http::GatewayHttpErrorStatus::InternalServerError => 500,
    };
    build_runtime_proxy_json_error_response(status, response.code, response.message)
}

fn runtime_local_rewrite_authenticate<'target>(
    canonical: RuntimeLocalRewriteCanonicalRequest<'target>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewriteAuthenticatedRequest<'target>> {
    let mut state = canonical.0;
    state.admin = match runtime_local_rewrite_preauthorize_admin(&state, shared) {
        Ok(admin) => admin,
        Err(response) => return Err(state.reject(response)),
    };
    if runtime_gateway_request_path_is_route_explain(&state.path, shared) {
        runtime_gateway_respond_route_explain(
            state.request,
            &state.path,
            shared,
            state.context,
            state.admin,
        );
        return Err(RuntimeLocalRewritePipelineExit::Handled);
    }
    state.application = match runtime_local_rewrite_authorize_data_plane(&state, shared) {
        Ok(application) => application,
        Err(response) => return Err(state.reject(response)),
    };
    Ok(RuntimeLocalRewriteAuthenticatedRequest(state))
}

fn runtime_local_rewrite_preauthorize_admin<'target>(
    state: &RuntimeLocalRewriteRequestState<'target>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<Option<RuntimeGatewayAdminPreauthorization<'target>>, tiny_http::ResponseBox> {
    if !runtime_gateway_request_path_requires_admin_auth(&state.path, shared) {
        return Ok(None);
    }
    let header_request = state.request.header_request();
    let Some(authentication) = runtime_gateway_admin_auth(&header_request, shared) else {
        return Err(runtime_local_rewrite_admin_auth_rejection(state, shared));
    };
    let admin_auth = &authentication.auth;
    let http = runtime_gateway_http_request_meta(
        &header_request,
        path_without_query(state.context.target().path_and_query()),
    );
    let application =
        match runtime_gateway_admin_preauthorization(&state.context, &http, &authentication) {
            Ok(application) => application,
            Err(RuntimeGatewayApplicationBoundaryError::Authorization(_)) => {
                return Err(runtime_gateway_admin_authorization_rejection_response(
                    state.request_id,
                    state.request.method(),
                    path_without_query(&state.path),
                    shared,
                    admin_auth,
                ));
            }
            Err(RuntimeGatewayApplicationBoundaryError::Authentication(_))
            | Err(RuntimeGatewayApplicationBoundaryError::Route) => {
                return Err(build_runtime_proxy_json_error_response(
                    401,
                    "invalid_admin_token",
                    "missing or invalid gateway admin bearer token",
                ));
            }
        };
    Ok(Some(RuntimeGatewayAdminPreauthorization {
        auth: authentication.auth,
        application,
    }))
}

fn runtime_local_rewrite_admin_auth_rejection(
    state: &RuntimeLocalRewriteRequestState<'_>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> tiny_http::ResponseBox {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "gateway_admin_auth_rejected",
            [
                runtime_proxy_log_field("request", state.request_id.to_string()),
                runtime_proxy_log_field("path", path_without_query(&state.path)),
                runtime_proxy_log_field("stage", "headers"),
            ],
        ),
    );
    let configured = !shared.gateway_admin_tokens.is_empty()
        || shared.gateway_sso.proxy_token_hash.is_some()
        || shared.gateway_sso.oidc.is_some();
    let (status, code, message, reason) = if configured {
        (
            401,
            "invalid_admin_token",
            "missing or invalid gateway admin bearer token",
            "admin_authentication_required",
        )
    } else {
        (
            403,
            "admin_auth_not_configured",
            "configure a gateway admin bearer token to use gateway admin endpoints",
            "admin_auth_not_configured",
        )
    };
    runtime_gateway_audit_admin_auth_event(
        shared,
        "auth_failed",
        "failure",
        serde_json::json!({
            "reason": reason,
            "method": state.request.method(),
            "path": path_without_query(&state.path),
        }),
    );
    build_runtime_proxy_json_error_response(status, code, message)
}

fn runtime_local_rewrite_authorize_data_plane<'target>(
    state: &RuntimeLocalRewriteRequestState<'target>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<
    Option<prodex_application::ApplicationAuthorizedRequestContext<'target>>,
    tiny_http::ResponseBox,
> {
    let virtual_keys_empty = runtime_gateway_virtual_key_entries_is_empty(shared);
    let legacy_authorized = shared
        .gateway_auth_token_hash
        .as_ref()
        .is_some_and(|hash| runtime_local_rewrite_request_is_authorized(&state.request, hash));
    let admin_configured = !shared.gateway_admin_tokens.is_empty()
        || shared.gateway_sso.proxy_token_hash.is_some()
        || shared.gateway_sso.oidc.is_some();
    let virtual_key =
        runtime_local_rewrite_verified_virtual_key(state, shared, virtual_keys_empty)?;
    if state.context.plane() != prodex_gateway_http::GatewayHttpRoutePlane::DataPlane {
        return Ok(None);
    }
    let mut credential = runtime_gateway_data_plane_credential(
        virtual_key.as_ref(),
        legacy_authorized,
        shared.gateway_auth_token_hash.is_some() || admin_configured,
    );
    credential.anonymous_allowed &= shared
        .runtime_shared
        .runtime_config
        .governance
        .anonymous_data_plane;
    runtime_gateway_application_data_plane_authorization(&state.context, credential)
        .map(Some)
        .map_err(|_| runtime_local_rewrite_data_auth_rejection(state, shared))
}

fn runtime_local_rewrite_verified_virtual_key(
    state: &RuntimeLocalRewriteRequestState<'_>,
    shared: &RuntimeLocalRewriteProxyShared,
    virtual_keys_empty: bool,
) -> Result<Option<runtime_proxy_crate::RuntimeGatewayVirtualKey>, tiny_http::ResponseBox> {
    if virtual_keys_empty || runtime_gateway_request_path_is_admin(&state.path, shared) {
        return Ok(None);
    }
    runtime_gateway_request_header_virtual_key(state.request_id, &state.request, shared).map_err(
        |rejection| {
            runtime_gateway_audit_data_plane_virtual_key_rejected(
                shared,
                &state.path,
                rejection.code(),
            );
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_virtual_key_rejected",
                    [
                        runtime_proxy_log_field("request", state.request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field("reason", rejection.code()),
                        runtime_proxy_log_field("path", path_without_query(&state.path)),
                        runtime_proxy_log_field("stage", "headers"),
                    ],
                ),
            );
            build_runtime_proxy_json_error_response(
                rejection.status(),
                rejection.code(),
                "gateway virtual key policy rejected this request",
            )
        },
    )
}

fn runtime_local_rewrite_data_auth_rejection(
    state: &RuntimeLocalRewriteRequestState<'_>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> tiny_http::ResponseBox {
    runtime_gateway_audit_data_plane_auth_failed(shared, &state.path);
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "gateway_auth_rejected",
            [
                runtime_proxy_log_field("request", state.request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("path", path_without_query(&state.path)),
            ],
        ),
    );
    build_runtime_proxy_text_response(401, "missing or invalid gateway bearer token")
}

fn runtime_local_rewrite_bounded_admission<'target>(
    authenticated: RuntimeLocalRewriteAuthenticatedRequest<'target>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewriteAdmittedRequest<'target>> {
    let mut state = authenticated.0;
    if state.context.plane() == prodex_gateway_http::GatewayHttpRoutePlane::DataPlane {
        let Some(application) = state.application.as_ref() else {
            return Err(state.reject(build_runtime_proxy_json_error_response(
                503,
                "gateway_admission_unavailable",
                "gateway admission is temporarily unavailable",
            )));
        };
        let execution = match runtime_gateway_application_local_admission(application, shared) {
            Ok(execution) => execution,
            Err(_) => {
                return Err(state.reject(build_runtime_proxy_json_error_response(
                    503,
                    "gateway_admission_unavailable",
                    "gateway admission is temporarily unavailable",
                )));
            }
        };
        if execution.max_concurrent_streams as usize != shared.runtime_shared.active_request_limit {
            return Err(state.reject(build_runtime_proxy_json_error_response(
                503,
                "gateway_admission_unavailable",
                "gateway admission is temporarily unavailable",
            )));
        }
    }
    let websocket = state.request.is_websocket_upgrade();
    let transport = if websocket { "websocket" } else { "http" };
    state.guards.active = match acquire_runtime_proxy_active_request_slot_with_wait(
        &shared.runtime_shared,
        transport,
        &state.path,
    ) {
        Ok(guard) => Some(guard),
        Err(RuntimeProxyAdmissionRejection::GlobalLimit) => {
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
    Ok(RuntimeLocalRewriteAdmittedRequest(state))
}

fn runtime_local_rewrite_dispatch_websocket<'target>(
    admitted: RuntimeLocalRewriteAdmittedRequest<'target>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewriteAdmittedRequest<'target>> {
    if !admitted.0.request.is_websocket_upgrade() {
        return Ok(admitted);
    }
    let state = admitted.0;
    if matches!(
        &shared.provider,
        RuntimeLocalRewriteProviderOptions::Gemini { .. }
    ) && is_runtime_realtime_websocket_path(&state.path)
    {
        handle_runtime_gemini_live_websocket_request(
            state.request_id,
            state.request,
            shared,
            state.application.as_ref(),
        );
        return Err(RuntimeLocalRewritePipelineExit::Handled);
    }
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_websocket_rejected",
            [
                runtime_proxy_log_field("request", state.request_id.to_string()),
                runtime_proxy_log_field("transport", "websocket"),
                runtime_proxy_log_field("path", path_without_query(&state.path)),
            ],
        ),
    );
    Err(state.reject(build_runtime_proxy_text_response(
        501,
        "runtime local rewrite proxy does not support websocket upstreams",
    )))
}

fn runtime_local_rewrite_capture_body<'target>(
    admitted: RuntimeLocalRewriteAdmittedRequest<'target>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewriteCapturedRequest<'target>> {
    let mut state = admitted.0;
    let mut captured = match state
        .request
        .capture(shared.runtime_shared.runtime_config.max_request_body_bytes)
    {
        Ok(captured) => captured,
        Err(err) => {
            let response = runtime_local_rewrite_capture_rejection(&state, shared, &err);
            return Err(state.reject(response));
        }
    };
    captured.path_and_query = state.context.target().path_and_query().to_string();
    Ok(RuntimeLocalRewriteCapturedRequest { state, captured })
}

fn runtime_local_rewrite_capture_rejection(
    state: &RuntimeLocalRewriteRequestState<'_>,
    shared: &RuntimeLocalRewriteProxyShared,
    err: &anyhow::Error,
) -> tiny_http::ResponseBox {
    let body_too_large = runtime_proxy_error_is_body_too_large(err);
    let reason = if body_too_large {
        runtime_gateway_audit_data_plane_request_body_too_large(shared, &state.path);
        "request_body_too_large"
    } else {
        runtime_gateway_audit_data_plane_request_capture_failed(shared, &state.path);
        "request_capture_failed"
    };
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
                runtime_proxy_log_field("reason", reason),
                runtime_proxy_log_field("path", state.path.as_str()),
            ],
        ),
    );
    if body_too_large {
        build_runtime_proxy_text_response(413, "proxied request body is too large")
    } else {
        build_runtime_proxy_text_response(502, "gateway request capture failed")
    }
}
