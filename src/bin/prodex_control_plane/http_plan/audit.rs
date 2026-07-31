use super::*;

type ControlPlaneHttpPlanStep<T> = Result<T, Result<String, String>>;

pub(super) fn control_plane_http_audit_json(
    plan: Option<&ApplicationControlPlaneAuditPlan>,
) -> serde_json::Value {
    let Some(plan) = plan else {
        return serde_json::json!({ "required": false });
    };
    let (audit_outcome, audit_action, audit_partition_tenant_id) = match &plan.decision {
        prodex_control_plane::ControlPlaneDecision::Authorized(action) => (
            action.audit_event.outcome,
            action.audit_event.action.as_str(),
            action.audit_write.tenant_partition_key,
        ),
        prodex_control_plane::ControlPlaneDecision::Denied {
            audit_event,
            audit_write,
            ..
        } => (
            audit_event.outcome,
            audit_event.action.as_str(),
            audit_write.tenant_partition_key,
        ),
    };
    serde_json::json!({
        "required": true,
        "storage_backend": application_control_plane_audit_storage_backend_label(&plan.audit_storage),
        "audit_action": audit_action,
        "audit_outcome": audit_outcome,
        "audit_partition_tenant_id": audit_partition_tenant_id,
    })
}

pub(super) fn plan_control_plane_http_audit_correlation(
    audit_plan: Option<&ApplicationControlPlaneAuditPlan>,
    request_id: RequestId,
    call_id: Option<CallId>,
    operation: ControlPlaneOperation,
    http: &GatewayHttpRequestMeta,
) -> ControlPlaneHttpPlanStep<serde_json::Value> {
    let Some(audit_plan) = audit_plan else {
        return Ok(serde_json::Value::Null);
    };
    let correlation_plan = match plan_application_control_plane_audit_correlation_from_http(
        prodex_application::ApplicationControlPlaneAuditCorrelationRequest {
            request_id,
            call_id,
            http_policy: GatewayHttpPolicy::production_default(),
            http: http.clone(),
            audit: audit_plan.clone(),
        },
    ) {
        Ok(plan) => plan,
        Err(error) => {
            let response = plan_application_control_plane_audit_correlation_error_response(&error);
            return Err(encode_control_plane_http_plan_operation_failure(
                operation,
                application_control_plane_audit_correlation_status_label(response.status),
                response.code,
                response.message,
                "control-plane audit correlation error",
                http,
            ));
        }
    };
    let emission = match plan_application_control_plane_audit_emission_span(
        prodex_application::ApplicationControlPlaneAuditEmissionSpanRequest {
            correlation: correlation_plan.correlation.clone(),
        },
    ) {
        Ok(plan) => serde_json::json!({
            "name": plan.span.descriptor.name,
            "kind": format!("{:?}", plan.span.descriptor.kind).to_ascii_lowercase(),
        }),
        Err(error) => {
            let response =
                plan_application_control_plane_audit_emission_span_error_response(&error);
            return Err(encode_control_plane_http_plan_operation_failure(
                operation,
                application_control_plane_audit_emission_span_status_label(response.status),
                response.code,
                response.message,
                "control-plane audit emission span error",
                http,
            ));
        }
    };
    let persistence = match plan_application_control_plane_audit_persistence_span(
        prodex_application::ApplicationControlPlaneAuditPersistenceSpanRequest {
            correlation: correlation_plan.correlation.clone(),
            audit_storage: audit_plan.audit_storage.clone(),
        },
    ) {
        Ok(plan) => serde_json::json!({
            "name": plan.span.descriptor.name,
            "kind": format!("{:?}", plan.span.descriptor.kind).to_ascii_lowercase(),
        }),
        Err(error) => {
            let response =
                plan_application_control_plane_audit_persistence_span_error_response(&error);
            return Err(encode_control_plane_http_plan_operation_failure(
                operation,
                application_control_plane_audit_persistence_span_status_label(response.status),
                response.code,
                response.message,
                "control-plane audit persistence span error",
                http,
            ));
        }
    };
    Ok(serde_json::json!({
        "request_id": correlation_plan.correlation.request_id,
        "call_id": correlation_plan.correlation.call_id,
        "trace_id": correlation_plan.correlation.trace_id.as_ref().map(|trace_id| trace_id.as_str()),
        "tenant_id": correlation_plan.correlation.tenant_id,
        "audit_event_id": correlation_plan.correlation.audit_event_id,
        "emission_span": emission,
        "persistence_span": persistence,
    }))
}
