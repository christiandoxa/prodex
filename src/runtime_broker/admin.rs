use super::*;

pub(crate) fn runtime_proxy_admin_token(request: &tiny_http::Request) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|header| header.field.equiv("X-Prodex-Admin-Token"))
        .map(|header| header.value.as_str().trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn build_runtime_proxy_json_response(
    status: u16,
    body: String,
) -> tiny_http::ResponseBox {
    let mut response = TinyResponse::from_string(body).with_status_code(status);
    if let Ok(header) = TinyHeader::from_bytes("Content-Type", "application/json") {
        response = response.with_header(header);
    }
    response.boxed()
}

pub(crate) fn build_runtime_proxy_string_response(
    status: u16,
    body: String,
    content_type: &str,
) -> tiny_http::ResponseBox {
    let mut response = TinyResponse::from_string(body).with_status_code(status);
    if let Ok(header) = TinyHeader::from_bytes("Content-Type", content_type) {
        response = response.with_header(header);
    }
    response.boxed()
}

pub(crate) fn build_runtime_proxy_prometheus_response(
    status: u16,
    body: String,
) -> tiny_http::ResponseBox {
    build_runtime_proxy_string_response(status, body, "text/plain; version=0.0.4; charset=utf-8")
}

pub(crate) fn update_runtime_broker_current_profile(log_path: &Path, current_profile: &str) {
    let mut metadata_by_path = runtime_broker_metadata_by_log_path()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(metadata) = metadata_by_path.get_mut(log_path) {
        metadata.current_profile = current_profile.to_string();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeBrokerAdminRoute {
    Health,
    Metrics,
    MetricsPrometheus,
    Activate,
}

impl RuntimeBrokerAdminRoute {
    fn from_path(path: &str) -> Option<Self> {
        match path {
            "/__prodex/runtime/health" => Some(Self::Health),
            "/__prodex/runtime/metrics" => Some(Self::Metrics),
            "/__prodex/runtime/metrics/prometheus" => Some(Self::MetricsPrometheus),
            "/__prodex/runtime/activate" => Some(Self::Activate),
            _ => None,
        }
    }
}

fn runtime_broker_metrics_json_response(
    shared: &RuntimeRotationProxyShared,
    metadata: &RuntimeBrokerMetadata,
) -> Option<tiny_http::ResponseBox> {
    let metrics = match runtime_broker_metrics_snapshot(shared, metadata) {
        Ok(metrics) => metrics,
        Err(err) => {
            return Some(build_runtime_proxy_json_error_response(
                500,
                "internal_error",
                &err.to_string(),
            ));
        }
    };
    let body = serde_json::to_string(&metrics).ok()?;
    Some(build_runtime_proxy_json_response(200, body))
}

fn runtime_broker_metrics_prometheus_response(
    shared: &RuntimeRotationProxyShared,
    metadata: &RuntimeBrokerMetadata,
) -> Option<tiny_http::ResponseBox> {
    let metrics = match runtime_broker_metrics_snapshot(shared, metadata) {
        Ok(metrics) => metrics,
        Err(err) => {
            return Some(build_runtime_proxy_json_error_response(
                500,
                "internal_error",
                &err.to_string(),
            ));
        }
    };
    let snapshot = runtime_broker_prometheus_snapshot(metadata, &metrics);
    let body = runtime_metrics::render_runtime_broker_prometheus(&snapshot);
    Some(build_runtime_proxy_prometheus_response(200, body))
}

fn runtime_broker_health_response(
    shared: &RuntimeRotationProxyShared,
    metadata: RuntimeBrokerMetadata,
) -> Option<tiny_http::ResponseBox> {
    let health = RuntimeBrokerHealth {
        pid: std::process::id(),
        started_at: metadata.started_at,
        current_profile: metadata.current_profile,
        include_code_review: metadata.include_code_review,
        active_requests: shared.active_request_count.load(Ordering::SeqCst),
        instance_token: metadata.instance_token,
        persistence_role: if runtime_proxy_persistence_enabled(shared) {
            "owner".to_string()
        } else {
            "follower".to_string()
        },
        prodex_version: metadata.prodex_version,
        executable_path: metadata.executable_path,
        executable_sha256: metadata.executable_sha256,
    };
    let body = serde_json::to_string(&health).ok()?;
    Some(build_runtime_proxy_json_response(200, body))
}

fn runtime_broker_activation_profile(
    request: &mut tiny_http::Request,
) -> std::result::Result<String, tiny_http::ResponseBox> {
    if request.method().as_str() != "POST" {
        return Err(build_runtime_proxy_json_error_response(
            405,
            "method_not_allowed",
            "runtime broker activation requires POST",
        ));
    }
    let mut body = Vec::new();
    if let Err(err) = request.as_reader().read_to_end(&mut body) {
        return Err(build_runtime_proxy_json_error_response(
            400,
            "invalid_request",
            &format!("failed to read runtime broker activation body: {err}"),
        ));
    }
    serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|value| {
            value
                .get("current_profile")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .map(str::to_string)
        })
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            build_runtime_proxy_json_error_response(
                400,
                "invalid_request",
                "runtime broker activation requires a non-empty current_profile",
            )
        })
}

fn apply_runtime_broker_activation(
    shared: &RuntimeRotationProxyShared,
    current_profile: &str,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    runtime.current_profile = current_profile.to_string();
    runtime.state.active_profile = Some(current_profile.to_string());
    Ok(())
}

fn runtime_broker_activation_response(
    request: &mut tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
    metadata: RuntimeBrokerMetadata,
) -> Option<tiny_http::ResponseBox> {
    let current_profile = match runtime_broker_activation_profile(request) {
        Ok(current_profile) => current_profile,
        Err(response) => return Some(response),
    };
    if let Err(err) = apply_runtime_broker_activation(shared, &current_profile) {
        return Some(build_runtime_proxy_json_error_response(
            500,
            "internal_error",
            &err.to_string(),
        ));
    }
    update_runtime_broker_current_profile(&shared.log_path, &current_profile);
    runtime_proxy_log(
        shared,
        format!("runtime_broker_activate current_profile={current_profile}"),
    );
    audit_log_event_best_effort(
        "runtime_broker",
        "activate_profile",
        "success",
        serde_json::json!({
            "broker_key": metadata.broker_key,
            "listen_addr": metadata.listen_addr,
            "current_profile": current_profile,
        }),
    );
    Some(build_runtime_proxy_json_response(
        200,
        serde_json::json!({
            "ok": true,
            "current_profile": current_profile,
        })
        .to_string(),
    ))
}

pub(crate) fn handle_runtime_proxy_admin_request(
    request: &mut tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
) -> Option<tiny_http::ResponseBox> {
    let path = path_without_query(request.url());
    let route = RuntimeBrokerAdminRoute::from_path(path)?;

    let Some(metadata) = runtime_broker_metadata_for_log_path(&shared.log_path) else {
        return Some(build_runtime_proxy_json_error_response(
            404,
            "not_found",
            "runtime broker admin endpoint is not enabled for this proxy",
        ));
    };
    if runtime_proxy_admin_token(request).as_deref() != Some(metadata.admin_token.as_str()) {
        return Some(build_runtime_proxy_json_error_response(
            403,
            "forbidden",
            "missing or invalid runtime broker admin token",
        ));
    }

    match route {
        RuntimeBrokerAdminRoute::Health => runtime_broker_health_response(shared, metadata),
        RuntimeBrokerAdminRoute::Metrics => runtime_broker_metrics_json_response(shared, &metadata),
        RuntimeBrokerAdminRoute::MetricsPrometheus => {
            runtime_broker_metrics_prometheus_response(shared, &metadata)
        }
        RuntimeBrokerAdminRoute::Activate => {
            runtime_broker_activation_response(request, shared, metadata)
        }
    }
}
