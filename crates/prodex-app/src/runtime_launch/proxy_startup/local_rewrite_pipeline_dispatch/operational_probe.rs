use super::*;

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_operational_probe_response(
    method: &str,
    request_path: &str,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<tiny_http::ResponseBox> {
    let probe = match request_path.split('?').next().unwrap_or(request_path) {
        "/livez" => "livez",
        "/readyz" => "readyz",
        "/startupz" => "startupz",
        _ => return None,
    };
    if method != "GET" && method != "HEAD" {
        return Some(runtime_gateway_probe_method_rejection(probe));
    }
    let probe_state = runtime_gateway_operational_probe_state(shared, probe);
    let (probe_metric, result_metric) =
        runtime_gateway_operational_probe_metrics(probe, &probe_state);
    crate::record_runtime_health_probe_metric(probe_metric, result_metric);
    let body = serde_json::json!({
        "object": "gateway.health",
        "probe": probe,
        "status": probe_state.status,
        "ready": probe_state.ready,
        "local_overload": probe_state.overloaded,
        "draining": probe_state.draining,
        "credentials_stale": probe_state.credentials_stale,
        "governance_audit_available": probe_state.governance_audit_available,
        "governance_policy_available": probe_state.governance_policy_available,
        "policy_version": shared.gateway_policy_version,
        "active_requests": shared.runtime_shared.active_request_count.load(Ordering::SeqCst),
        "active_request_limit": shared.runtime_shared.active_request_limit,
    })
    .to_string();
    Some(build_runtime_proxy_response_from_parts(
        RuntimeHeapTrimmedBufferedResponseParts {
            status: if probe_state.ready { 200 } else { 503 },
            headers: vec![("content-type".to_string(), b"application/json".to_vec())],
            body: if method == "HEAD" {
                Vec::new().into()
            } else {
                body.into_bytes().into()
            },
        },
    ))
}

struct RuntimeGatewayOperationalProbeState {
    status: &'static str,
    ready: bool,
    overloaded: bool,
    draining: bool,
    credentials_stale: bool,
    governance_audit_available: bool,
    governance_policy_available: bool,
}

fn runtime_gateway_operational_probe_state(
    shared: &RuntimeLocalRewriteProxyShared,
    probe: &str,
) -> RuntimeGatewayOperationalProbeState {
    let overloaded = runtime_proxy_local_overload_pressure_active(&shared.runtime_shared);
    let draining = shared.gateway_draining.load(Ordering::SeqCst);
    let credentials_stale = shared.gateway_credentials.refresh_is_stale();
    let governance_policy_available = runtime_gateway_mandatory_governance_available(shared);
    let governance_audit_available =
        super::super::super::local_rewrite_governance_audit::runtime_governance_audit_is_available(
            shared,
        );
    let ready = probe != "readyz"
        || (!overloaded
            && !draining
            && !credentials_stale
            && governance_policy_available
            && governance_audit_available);
    let status = if ready {
        "ok"
    } else if draining {
        "draining"
    } else if credentials_stale {
        "credentials_stale"
    } else if !governance_audit_available {
        "governance_audit_unavailable"
    } else if !governance_policy_available {
        "governance_policy_unavailable"
    } else {
        "overloaded"
    };
    RuntimeGatewayOperationalProbeState {
        status,
        ready,
        overloaded,
        draining,
        credentials_stale,
        governance_audit_available,
        governance_policy_available,
    }
}

fn runtime_gateway_operational_probe_metrics(
    probe: &str,
    state: &RuntimeGatewayOperationalProbeState,
) -> (
    prodex_observability::HealthProbeKind,
    prodex_observability::HealthProbeResult,
) {
    let probe_metric = match probe {
        "livez" => prodex_observability::HealthProbeKind::Live,
        "readyz" => prodex_observability::HealthProbeKind::Ready,
        _ => prodex_observability::HealthProbeKind::Startup,
    };
    let result_metric = if state.draining {
        prodex_observability::HealthProbeResult::Draining
    } else if state.overloaded
        || state.credentials_stale
        || !state.governance_policy_available
        || !state.governance_audit_available
    {
        prodex_observability::HealthProbeResult::Degraded
    } else {
        prodex_observability::HealthProbeResult::Passing
    };
    (probe_metric, result_metric)
}

fn runtime_gateway_mandatory_governance_available(shared: &RuntimeLocalRewriteProxyShared) -> bool {
    if !shared
        .runtime_shared
        .runtime_config
        .governance
        .mode
        .is_enforcing()
    {
        return true;
    }
    let Some(authority) = shared.governance_authority.as_ref() else {
        return false;
    };
    let Ok(tenant_ids) = authority.tenant_ids() else {
        return false;
    };
    shared.governance.policies_are_servable(
        &tenant_ids,
        super::super::super::local_rewrite_gateway_util::runtime_gateway_unix_epoch_millis(),
    )
}

fn runtime_gateway_probe_method_rejection(probe: &str) -> tiny_http::ResponseBox {
    build_runtime_proxy_response_from_parts(RuntimeHeapTrimmedBufferedResponseParts {
        status: 405,
        headers: vec![
            ("content-type".to_string(), b"application/json".to_vec()),
            ("allow".to_string(), b"GET, HEAD".to_vec()),
        ],
        body: serde_json::json!({
            "object": "gateway.health",
            "probe": probe,
            "status": "method_not_allowed"
        })
        .to_string()
        .into_bytes()
        .into(),
    })
}
