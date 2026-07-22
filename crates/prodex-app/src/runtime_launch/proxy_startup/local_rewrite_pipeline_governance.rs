use prodex_domain::AuditOutcome;

fn runtime_local_rewrite_material_audit(
    application: Option<&prodex_application::ApplicationAuthorizedRequestContext<'_>>,
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
    action: &str,
    outcome: AuditOutcome,
    reason: &str,
) -> Option<Result<(), prodex_storage::GovernanceRepositoryError>> {
    if !super::super::local_rewrite_governance_audit::runtime_governance_audit_is_durable(shared) {
        return None;
    }
    let context = application.and_then(
        super::super::local_rewrite_governance_audit::RuntimeGovernanceAuditContext::from_authorized,
    )?;
    Some(
        super::super::local_rewrite_governance_audit::persist_runtime_material_governance_audit(
            shared, &context, request_id, action, outcome, reason,
        ),
    )
}

fn runtime_local_rewrite_audit_unavailable_response() -> tiny_http::ResponseBox {
    build_runtime_proxy_json_error_response(
        503,
        "governance_audit_unavailable",
        "gateway governance audit is temporarily unavailable",
    )
}

pub(super) fn runtime_local_rewrite_prepare_constraints<'target, 'shared>(
    mut request: RuntimeLocalRewriteCapturedRequest<'target>,
    shared: &'shared RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewritePreparedRequest<'target, 'shared>> {
    if request.state.deadline_expired() {
        return Err(request
            .state
            .reject(runtime_local_rewrite_request_timeout_response()));
    }
    let mut constraints = match runtime_gateway_prepare_constraint_plan(
        request.state.request_id,
        &request.captured,
        shared,
    ) {
        Ok(plan) => plan,
        Err(response) => return Err(request.state.reject(response)),
    };
    let inspection = match crate::runtime_launch::proxy_startup::local_rewrite_classification_rules::apply_runtime_gateway_classification_to_request(
        request.state.request_id,
        &mut request.captured,
        shared,
        shared.gateway_guardrails.pii_redaction,
        request
            .state
            .application
            .as_ref()
            .and_then(|authorized| authorized.tenant_context())
            .map(|tenant| tenant.tenant_id),
    ) {
        Ok(inspection) => inspection,
        Err(_) => {
            if let Some(plan) = constraints.as_mut() {
                plan.reject_governance("presidio_redaction_failed");
            }
            match runtime_local_rewrite_material_audit(
                request.state.application.as_ref(),
                request.state.request_id,
                shared,
                "request_inspection_failed",
                AuditOutcome::Denied,
                "presidio_redaction_failed",
            ) {
                Some(Err(_)) => {
                    return Err(request
                        .state
                        .reject(runtime_local_rewrite_audit_unavailable_response()));
                }
                Some(Ok(())) => {}
                None => runtime_gateway_audit_data_plane_presidio_redaction_failed(
                    shared,
                    &request.captured.path_and_query,
                ),
            }
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "local_rewrite_presidio_redaction_failed",
                    [
                        runtime_proxy_log_field("request", request.state.request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field("reason", "presidio_redaction_failed"),
                        runtime_proxy_log_field(
                            "path",
                            path_without_query(&request.captured.path_and_query),
                        ),
                    ],
                ),
            );
            return Err(request.state.reject(build_runtime_proxy_text_response(
                502,
                "gateway PII redaction failed",
            )));
        }
    };
    if !inspection.masked_findings.is_empty()
        && runtime_local_rewrite_material_audit(
            request.state.application.as_ref(),
            request.state.request_id,
            shared,
            "request_transform",
            AuditOutcome::Success,
            "sensitive_fields_masked",
        )
        .is_some_and(|result| result.is_err())
    {
        return Err(request
            .state
            .reject(runtime_local_rewrite_audit_unavailable_response()));
    }
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
    if request.state.deadline_expired() {
        return Err(request.reject(runtime_local_rewrite_request_timeout_response()));
    }
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
    if request.state.deadline_expired() {
        return Err(request.reject(runtime_local_rewrite_request_timeout_response()));
    }
    if let Some(block) = runtime_proxy_crate::runtime_gateway_guardrail_block(
        &request.captured.body,
        &shared.gateway_guardrails,
    ) {
        let reason = block.kind.as_str();
        if let Some(plan) = request.constraints.as_mut() {
            plan.reject_governance(reason);
        }
        match runtime_local_rewrite_material_audit(
            request.state.application.as_ref(),
            request.state.request_id,
            shared,
            "admission_denied",
            AuditOutcome::Denied,
            reason,
        ) {
            Some(Err(_)) => {
                return Err(request
                    .state
                    .reject(runtime_local_rewrite_audit_unavailable_response()));
            }
            Some(Ok(())) => {}
            None => runtime_gateway_audit_data_plane_guardrail_blocked(
                shared,
                &request.captured.path_and_query,
                reason,
            ),
        }
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
    if request.state.deadline_expired() {
        return Err(request.reject(runtime_local_rewrite_request_timeout_response()));
    }
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
        request.state.request.network_zone(),
        application,
        &request.inspection,
    ) {
        Ok(admission) => admission,
        Err(failure) => {
            let rejection = failure.rejection;
            if let Some(plan) = request.constraints.as_mut() {
                plan.reject_virtual_key(rejection);
            }
            match runtime_local_rewrite_material_audit(
                request.state.application.as_ref(),
                request.state.request_id,
                shared,
                "admission_denied",
                AuditOutcome::Denied,
                rejection.code(),
            ) {
                Some(Err(_)) => {
                    return Err(request
                        .state
                        .reject(runtime_local_rewrite_audit_unavailable_response()));
                }
                Some(Ok(())) => {}
                None => runtime_gateway_audit_data_plane_virtual_key_rejected(
                    shared,
                    &request.captured.path_and_query,
                    rejection.code(),
                ),
            }
            runtime_local_rewrite_log_virtual_key_rejection(
                request.state.request_id,
                &request.captured.path_and_query,
                rejection.code(),
                shared,
            );
            let response = if let Some((approval_id, state)) = failure.approval {
                crate::runtime_launch::proxy_startup::local_rewrite_gateway_admin_response::runtime_gateway_admin_json_response(
                    rejection.status(),
                    serde_json::json!({
                        "error": {
                            "code": rejection.code(),
                            "message": "execution approval is required before dispatch",
                            "approval_id": approval_id.as_str(),
                            "approval_state": execution_approval_state_label(state),
                        }
                    }),
                )
            } else {
                build_runtime_proxy_json_error_response(
                    rejection.status(),
                    rejection.code(),
                    "gateway virtual key policy rejected this request",
                )
            };
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

fn execution_approval_state_label(state: prodex_domain::ApprovalState) -> &'static str {
    match state {
        prodex_domain::ApprovalState::Draft => "draft",
        prodex_domain::ApprovalState::PendingApproval => "pending_approval",
        prodex_domain::ApprovalState::Approved => "approved",
        prodex_domain::ApprovalState::Rejected => "rejected",
        prodex_domain::ApprovalState::Expired => "expired",
        prodex_domain::ApprovalState::Cancelled => "cancelled",
        prodex_domain::ApprovalState::Active => "active",
        prodex_domain::ApprovalState::Superseded => "superseded",
        prodex_domain::ApprovalState::RolledBack => "rolled_back",
    }
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
    if request.state.deadline_expired() {
        return Err(request.reject(runtime_local_rewrite_request_timeout_response()));
    }
    if let Some(block) = runtime_gateway_guardrail_webhook_block(
        "pre",
        request.state.request_id,
        &request.captured.body,
        shared,
    ) {
        if let Some(plan) = request.constraints.as_mut() {
            plan.reject_governance(&block.reason);
        }
        match runtime_local_rewrite_material_audit(
            request.state.application.as_ref(),
            request.state.request_id,
            shared,
            "admission_denied",
            AuditOutcome::Denied,
            &block.reason,
        ) {
            Some(Err(_)) => {
                return Err(request
                    .state
                    .reject(runtime_local_rewrite_audit_unavailable_response()));
            }
            Some(Ok(())) => {}
            None => runtime_gateway_audit_data_plane_guardrail_webhook_blocked(
                shared,
                &request.captured.path_and_query,
                "pre",
                &block.reason,
            ),
        }
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
        if runtime_local_rewrite_material_audit(
            request.state.application.as_ref(),
            request.state.request_id,
            shared,
            "admission_denied",
            AuditOutcome::Denied,
            reason,
        )
        .is_some_and(|result| result.is_err())
        {
            return Err(request
                .state
                .reject(runtime_local_rewrite_audit_unavailable_response()));
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
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewriteDispatchReadyRequest<'target>> {
    let RuntimeLocalRewriteReservedRequest {
        mut request,
        application_admission,
    } = reserved;
    if request.state.deadline_expired() {
        return Err(request.reject(runtime_local_rewrite_request_timeout_response()));
    }
    let (route_load, transformed) = match request.constraints.as_mut() {
        Some(plan) => match plan.apply(&mut request.captured) {
            Ok(result) => result,
            Err(response) => return Err(request.reject(response)),
        },
        None => (None, false),
    };
    request.state.guards.route_load = route_load;
    if transformed
        && runtime_local_rewrite_material_audit(
            request.state.application.as_ref(),
            request.state.request_id,
            shared,
            "request_transform",
            AuditOutcome::Success,
            "route_constraints_applied",
        )
        .is_some_and(|result| result.is_err())
    {
        return Err(request
            .state
            .reject(runtime_local_rewrite_audit_unavailable_response()));
    }
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
    runtime_local_rewrite_request_timeout_response, runtime_proxy_log, runtime_proxy_log_field,
    runtime_proxy_structured_log_message,
};
