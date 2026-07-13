pub(super) fn runtime_local_rewrite_prepare_constraints<'target, 'shared>(
    mut request: RuntimeLocalRewriteCapturedRequest<'target>,
    shared: &'shared RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewritePreparedRequest<'target, 'shared>> {
    let mut constraints = match runtime_gateway_prepare_constraint_plan(
        request.state.request_id,
        &request.captured,
        shared,
    ) {
        Ok(plan) => plan,
        Err(response) => return Err(request.state.reject(response)),
    };
    let inspection = match apply_runtime_presidio_redaction_to_request(
        request.state.request_id,
        &mut request.captured,
        &shared.runtime_shared,
    ) {
        Ok(inspection) => inspection,
        Err(_) => {
            if let Some(plan) = constraints.as_mut() {
                plan.reject_governance("presidio_redaction_failed");
            }
            runtime_gateway_audit_data_plane_presidio_redaction_failed(
                shared,
                &request.captured.path_and_query,
            );
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "local_rewrite_presidio_redaction_failed",
                    [
                        runtime_proxy_log_field("request", request.state.request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field("reason", "presidio_redaction_failed"),
                        runtime_proxy_log_field("path", &request.captured.path_and_query),
                    ],
                ),
            );
            return Err(request.state.reject(build_runtime_proxy_text_response(
                502,
                "gateway PII redaction failed",
            )));
        }
    };
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "gateway_request_inspection",
            [
                runtime_proxy_log_field("request", request.state.request_id.to_string()),
                runtime_proxy_log_field("coverage", inspection.result.coverage().as_str()),
                runtime_proxy_log_field(
                    "classification",
                    inspection.result.classification().as_str(),
                ),
                runtime_proxy_log_field(
                    "finding_count",
                    inspection.result.findings().len().to_string(),
                ),
            ],
        ),
    );
    Ok(RuntimeLocalRewritePreparedRequest {
        state: request.state,
        captured: request.captured,
        constraints,
        inspection,
    })
}

pub(super) fn runtime_local_rewrite_dispatch_control_plane<'target, 'shared>(
    mut request: RuntimeLocalRewritePreparedRequest<'target, 'shared>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewritePreparedRequest<'target, 'shared>> {
    if let Some(response) = runtime_gateway_admin_response(
        request.state.request_id,
        &request.captured,
        shared,
        &request.state.context,
        request.state.admin.take(),
    ) {
        return Err(request.respond(response));
    }
    if request.state.context.plane() == prodex_gateway_http::GatewayHttpRoutePlane::ControlPlane {
        let response = build_runtime_proxy_json_error_response(
            404,
            "route_not_available",
            "route is not available",
        );
        return Err(request.reject(response));
    }
    Ok(request)
}

pub(super) fn runtime_local_rewrite_pre_reservation_governance<'target, 'shared>(
    mut request: RuntimeLocalRewritePreparedRequest<'target, 'shared>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewriteGovernedRequest<'target, 'shared>> {
    if let Some(block) = runtime_proxy_crate::runtime_gateway_guardrail_block(
        &request.captured.body,
        &shared.gateway_guardrails,
    ) {
        let reason = block.kind.as_str();
        if let Some(plan) = request.constraints.as_mut() {
            plan.reject_governance(reason);
        }
        runtime_gateway_audit_data_plane_guardrail_blocked(
            shared,
            &request.captured.path_and_query,
            reason,
        );
        runtime_local_rewrite_log_guardrail(
            request.state.request_id,
            &request.captured.path_and_query,
            reason,
            shared,
        );
        let response = build_runtime_proxy_json_error_response(
            403,
            reason,
            "gateway guardrail blocked this request",
        );
        return Err(request.reject(response));
    }
    if request.inspection.result.coverage() == prodex_domain::InspectionCoverage::Unsupported
        && let Some(body) = runtime_proxy_crate::runtime_gateway_redact_request_body(
            &request.captured.body,
            &shared.gateway_guardrails,
        )
    {
        runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_guardrail_pii_redacted",
                [
                    runtime_proxy_log_field("request", request.state.request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field(
                        "path",
                        path_without_query(&request.captured.path_and_query),
                    ),
                ],
            ),
        );
        request.captured.body = body;
    }
    Ok(RuntimeLocalRewriteGovernedRequest(request))
}

fn runtime_local_rewrite_log_guardrail(
    request_id: u64,
    path: &str,
    reason: &str,
    shared: &RuntimeLocalRewriteProxyShared,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "gateway_guardrail_blocked",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("reason", reason),
                runtime_proxy_log_field("matched_value_redacted", "true"),
                runtime_proxy_log_field("path", path_without_query(path)),
            ],
        ),
    );
}

pub(super) fn runtime_local_rewrite_reserve_virtual_key<'target, 'shared>(
    governed: RuntimeLocalRewriteGovernedRequest<'target, 'shared>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewriteReservedRequest<'target, 'shared>> {
    let mut request = governed.0;
    let application = match request.state.application.as_ref() {
        Some(application) => application,
        None => {
            return Err(request.reject(build_runtime_proxy_json_error_response(
                503,
                "gateway_admission_unavailable",
                "gateway admission is temporarily unavailable",
            )));
        }
    };
    let admission = match runtime_gateway_virtual_key_admission(
        request.state.request_id,
        &request.captured,
        shared,
        application,
        &request.inspection,
    ) {
        Ok(admission) => admission,
        Err(rejection) => {
            if let Some(plan) = request.constraints.as_mut() {
                plan.reject_virtual_key(rejection);
            }
            runtime_gateway_audit_data_plane_virtual_key_rejected(
                shared,
                &request.captured.path_and_query,
                rejection.code(),
            );
            runtime_local_rewrite_log_virtual_key_rejection(
                request.state.request_id,
                &request.captured.path_and_query,
                rejection.code(),
                shared,
            );
            let response = build_runtime_proxy_json_error_response(
                rejection.status(),
                rejection.code(),
                "gateway virtual key policy rejected this request",
            );
            return Err(request.reject(response));
        }
    };
    let namespace = admission.namespace;
    if !shared.allow_local_file_access {
        request.captured.headers.retain(|(name, _)| {
            !name.eq_ignore_ascii_case(RUNTIME_GATEWAY_CONVERSATION_NAMESPACE_HEADER)
        });
        request.captured.headers.push((
            RUNTIME_GATEWAY_CONVERSATION_NAMESPACE_HEADER.to_string(),
            namespace.unwrap_or_else(|| "gateway".to_string()),
        ));
    }
    request.state.guards.usage = Some(RuntimeGatewayUsageRequestGuard::new(
        shared,
        request.state.request_id,
    ));
    Ok(RuntimeLocalRewriteReservedRequest {
        request,
        application_admission: admission.application,
    })
}

fn runtime_local_rewrite_log_virtual_key_rejection(
    request_id: u64,
    path: &str,
    reason: &str,
    shared: &RuntimeLocalRewriteProxyShared,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "gateway_virtual_key_rejected",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("reason", reason),
                runtime_proxy_log_field("path", path_without_query(path)),
            ],
        ),
    );
}

pub(super) fn runtime_local_rewrite_post_reservation_governance<'target, 'shared>(
    reserved: RuntimeLocalRewriteReservedRequest<'target, 'shared>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewriteReservedRequest<'target, 'shared>> {
    let RuntimeLocalRewriteReservedRequest {
        mut request,
        application_admission,
    } = reserved;
    if let Some(block) = runtime_gateway_guardrail_webhook_block(
        "pre",
        request.state.request_id,
        &request.captured.body,
        shared,
    ) {
        if let Some(plan) = request.constraints.as_mut() {
            plan.reject_governance(&block.reason);
        }
        runtime_gateway_audit_data_plane_guardrail_webhook_blocked(
            shared,
            &request.captured.path_and_query,
            "pre",
            &block.reason,
        );
        runtime_local_rewrite_log_webhook_rejection(&request, &block.reason, shared);
        let response = build_runtime_proxy_json_error_response(
            403,
            "policy_violation",
            "gateway guardrail webhook blocked this request",
        );
        return Err(request.reject(response));
    }
    if let Some(block) = runtime_proxy_crate::runtime_gateway_guardrail_block(
        &request.captured.body,
        &shared.gateway_guardrails,
    ) {
        let reason = block.kind.as_str();
        if let Some(plan) = request.constraints.as_mut() {
            plan.reject_governance(reason);
        }
        runtime_local_rewrite_log_guardrail(
            request.state.request_id,
            &request.captured.path_and_query,
            reason,
            shared,
        );
        let response = build_runtime_proxy_json_error_response(
            403,
            "policy_violation",
            "gateway guardrail blocked this request",
        );
        return Err(request.reject(response));
    }
    Ok(RuntimeLocalRewriteReservedRequest {
        request,
        application_admission,
    })
}

fn runtime_local_rewrite_log_webhook_rejection(
    request: &RuntimeLocalRewritePreparedRequest<'_, '_>,
    reason: &str,
    shared: &RuntimeLocalRewriteProxyShared,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "gateway_guardrail_webhook_blocked",
            [
                runtime_proxy_log_field("request", request.state.request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("phase", "pre"),
                runtime_proxy_log_field("reason", reason),
                runtime_proxy_log_field("matched_value_redacted", "true"),
                runtime_proxy_log_field(
                    "path",
                    path_without_query(&request.captured.path_and_query),
                ),
            ],
        ),
    );
}

pub(super) fn runtime_local_rewrite_apply_constraints<'target>(
    reserved: RuntimeLocalRewriteReservedRequest<'target, '_>,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewriteDispatchReadyRequest<'target>> {
    let RuntimeLocalRewriteReservedRequest {
        mut request,
        application_admission,
    } = reserved;
    request.state.guards.route_load = match request.constraints.as_mut() {
        Some(plan) => match plan.apply(&mut request.captured) {
            Ok(guard) => guard,
            Err(response) => return Err(request.reject(response)),
        },
        None => None,
    };
    Ok(RuntimeLocalRewriteDispatchReadyRequest {
        state: request.state,
        captured: request.captured,
        application_admission,
    })
}
use super::{
    RUNTIME_GATEWAY_CONVERSATION_NAMESPACE_HEADER, RuntimeGatewayUsageRequestGuard,
    RuntimeLocalRewriteCapturedRequest, RuntimeLocalRewriteDispatchReadyRequest,
    RuntimeLocalRewriteGovernedRequest, RuntimeLocalRewritePipelineResult,
    RuntimeLocalRewritePreparedRequest, RuntimeLocalRewriteProxyShared,
    RuntimeLocalRewriteReservedRequest, build_runtime_proxy_json_error_response,
    build_runtime_proxy_text_response, path_without_query, runtime_gateway_admin_response,
    runtime_gateway_audit_data_plane_guardrail_blocked,
    runtime_gateway_audit_data_plane_guardrail_webhook_blocked,
    runtime_gateway_audit_data_plane_presidio_redaction_failed,
    runtime_gateway_audit_data_plane_virtual_key_rejected, runtime_gateway_guardrail_webhook_block,
    runtime_gateway_prepare_constraint_plan, runtime_gateway_virtual_key_admission,
    runtime_proxy_log, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};
use crate::runtime_proxy::apply_runtime_presidio_redaction_to_request;
