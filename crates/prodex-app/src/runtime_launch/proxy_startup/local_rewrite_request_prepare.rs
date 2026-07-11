use super::super::local_rewrite_gateway_admin_router::runtime_gateway_admin_response;
use super::super::local_rewrite_gateway_route_load::RuntimeGatewayRouteLoadGuard;
use super::*;

pub(super) struct RuntimeLocalRewritePreparedRequest {
    pub(super) captured: RuntimeProxyRequest,
    pub(super) route_load_guard: Option<RuntimeGatewayRouteLoadGuard>,
}

pub(super) enum RuntimeLocalRewritePrepareOutcome {
    Prepared(RuntimeLocalRewritePreparedRequest),
    Respond(tiny_http::ResponseBox),
}

pub(super) fn prepare_runtime_local_rewrite_request(
    request_id: u64,
    request: &mut tiny_http::Request,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePrepareOutcome {
    let runtime_shared = &shared.runtime_shared;
    let mut captured = match capture_runtime_proxy_request(request) {
        Ok(captured) => captured,
        Err(err) => {
            runtime_proxy_log(
                runtime_shared,
                runtime_proxy_structured_log_message(
                    "local_rewrite_capture_error",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field("error", format!("{err:#}")),
                    ],
                ),
            );
            return RuntimeLocalRewritePrepareOutcome::Respond(build_runtime_proxy_text_response(
                502,
                &err.to_string(),
            ));
        }
    };
    if let Err(err) =
        apply_runtime_presidio_redaction_to_request(request_id, &mut captured, runtime_shared)
    {
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "local_rewrite_presidio_redaction_failed",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("error", format!("{err:#}")),
                ],
            ),
        );
        return RuntimeLocalRewritePrepareOutcome::Respond(build_runtime_proxy_text_response(
            502,
            &err.to_string(),
        ));
    }
    if let Some(response) = runtime_gateway_admin_response(request_id, &captured, shared) {
        return RuntimeLocalRewritePrepareOutcome::Respond(response);
    }
    if let Some(redacted_body) = runtime_proxy_crate::runtime_gateway_redact_request_body(
        &captured.body,
        &shared.gateway_guardrails,
    ) {
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_guardrail_pii_redacted",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("path", captured.path_and_query.as_str()),
                ],
            ),
        );
        captured.body = redacted_body;
    }
    if let Some(rejection) = runtime_gateway_virtual_key_rejection(request_id, &captured, shared) {
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_virtual_key_rejected",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("reason", rejection.code()),
                    runtime_proxy_log_field("path", captured.path_and_query.as_str()),
                ],
            ),
        );
        return RuntimeLocalRewritePrepareOutcome::Respond(
            build_runtime_proxy_json_error_response(
                rejection.status(),
                rejection.code(),
                "gateway virtual key policy rejected this request",
            ),
        );
    }
    if let Some(block) =
        runtime_gateway_guardrail_webhook_block("pre", request_id, &captured.body, shared)
    {
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_guardrail_webhook_blocked",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("phase", "pre"),
                    runtime_proxy_log_field("reason", block.reason.as_str()),
                    runtime_proxy_log_field("value", block.value.as_str()),
                    runtime_proxy_log_field("path", captured.path_and_query.as_str()),
                ],
            ),
        );
        return RuntimeLocalRewritePrepareOutcome::Respond(
            build_runtime_proxy_json_error_response(
                403,
                "policy_violation",
                "gateway guardrail webhook blocked this request",
            ),
        );
    }
    if let Some(block) = runtime_proxy_crate::runtime_gateway_guardrail_block(
        &captured.body,
        &shared.gateway_guardrails,
    ) {
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_guardrail_blocked",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("reason", block.kind.as_str()),
                    runtime_proxy_log_field("value", block.value.as_str()),
                    runtime_proxy_log_field("path", captured.path_and_query.as_str()),
                ],
            ),
        );
        return RuntimeLocalRewritePrepareOutcome::Respond(
            build_runtime_proxy_json_error_response(
                403,
                "policy_violation",
                "gateway guardrail blocked this request",
            ),
        );
    }
    let route_load = shared
        .gateway_route_load
        .lock()
        .map(|load| load.clone())
        .unwrap_or_default();
    let mut route_load_guard = None;
    if let Some(rewrite) = runtime_proxy_crate::runtime_gateway_rewrite_route_alias_with_state(
        &captured.body,
        &shared.gateway_route_aliases,
        request_id,
        &route_load,
    ) {
        route_load_guard = Some(RuntimeGatewayRouteLoadGuard::enter(
            Arc::clone(&shared.gateway_route_load),
            rewrite.model.as_str(),
            &captured.body,
        ));
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_route_alias_rewrite",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("alias", rewrite.alias.as_str()),
                    runtime_proxy_log_field("strategy", rewrite.strategy.as_str()),
                    runtime_proxy_log_field("model", rewrite.model.as_str()),
                    runtime_proxy_log_field("path", captured.path_and_query.as_str()),
                ],
            ),
        );
        captured.body = rewrite.body;
    }
    RuntimeLocalRewritePrepareOutcome::Prepared(RuntimeLocalRewritePreparedRequest {
        captured,
        route_load_guard,
    })
}
