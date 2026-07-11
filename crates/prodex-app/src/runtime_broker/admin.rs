use super::{
    RuntimeRotationProxyShared, audit_log_event_best_effort,
    build_runtime_proxy_json_error_response, path_without_query,
    runtime_broker_metadata_by_log_path, runtime_broker_metadata_for_log_path,
    runtime_broker_metrics_snapshot, runtime_proxy_log, runtime_proxy_persistence_enabled,
};
use anyhow::{Context, Result};
use prodex_runtime_broker::{RuntimeBrokerHealth, RuntimeBrokerMetadata};
use std::io::Read;
use std::path::Path;
use std::sync::atomic::Ordering;
use tiny_http::{Header as TinyHeader, Response as TinyResponse};

const RUNTIME_BROKER_ACTIVATION_MAX_BODY_BYTES: usize = 64 * 1024;

#[derive(Debug)]
struct RuntimeBrokerActivationBodyTooLarge {
    limit: usize,
}

impl std::fmt::Display for RuntimeBrokerActivationBodyTooLarge {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "runtime broker activation body exceeds {} bytes",
            self.limit
        )
    }
}

impl std::error::Error for RuntimeBrokerActivationBodyTooLarge {}

pub(crate) fn runtime_proxy_admin_token(request: &tiny_http::Request) -> Option<&str> {
    request
        .headers()
        .iter()
        .find(|header| {
            header
                .field
                .equiv(prodex_runtime_broker::RUNTIME_BROKER_ADMIN_TOKEN_HEADER)
        })
        .map(|header| header.value.as_str().trim())
        .filter(|value| !value.is_empty())
}

fn runtime_broker_admin_error_response(
    error: prodex_runtime_broker::RuntimeBrokerAdminError,
) -> tiny_http::ResponseBox {
    build_runtime_proxy_json_error_response(error.status, error.code, &error.message)
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
    let body = runtime_metrics::render_runtime_broker_prometheus_from_metrics(metadata, &metrics);
    Some(build_runtime_proxy_prometheus_response(200, body))
}

fn runtime_broker_health_response(
    shared: &RuntimeRotationProxyShared,
    metadata: RuntimeBrokerMetadata,
) -> Option<tiny_http::ResponseBox> {
    let health = RuntimeBrokerHealth::from_metadata(
        &metadata,
        std::process::id(),
        shared.active_request_count.load(Ordering::SeqCst),
        runtime_proxy_persistence_enabled(shared),
    );
    let body = serde_json::to_string(&health).ok()?;
    Some(build_runtime_proxy_json_response(200, body))
}

fn runtime_broker_activation_profile(
    request: &mut tiny_http::Request,
) -> std::result::Result<String, tiny_http::ResponseBox> {
    if let Err(error) =
        prodex_runtime_broker::runtime_broker_validate_activation_method(request.method().as_str())
    {
        return Err(runtime_broker_admin_error_response(error));
    }
    let body = read_runtime_broker_activation_body_limited(
        request.as_reader(),
        RUNTIME_BROKER_ACTIVATION_MAX_BODY_BYTES,
    )
    .map_err(runtime_broker_activation_body_error_response)?;
    prodex_runtime_broker::runtime_broker_activation_profile_from_json(&body)
        .map_err(runtime_broker_admin_error_response)
}

fn runtime_broker_activation_body_error_response(err: anyhow::Error) -> tiny_http::ResponseBox {
    if err
        .downcast_ref::<RuntimeBrokerActivationBodyTooLarge>()
        .is_some()
    {
        build_runtime_proxy_json_error_response(413, "request_too_large", &err.to_string())
    } else {
        build_runtime_proxy_json_error_response(400, "invalid_request", &err.to_string())
    }
}

fn read_runtime_broker_activation_body_limited(reader: impl Read, limit: usize) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    let mut reader = reader.take((limit as u64).saturating_add(1));
    reader
        .read_to_end(&mut body)
        .context("failed to read runtime broker activation body")?;
    if body.len() > limit {
        return Err(RuntimeBrokerActivationBodyTooLarge { limit }.into());
    }
    Ok(body)
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
        serde_json::to_string(&prodex_runtime_broker::runtime_broker_activation_success(
            current_profile,
        ))
        .unwrap_or_else(|_| "{\"ok\":true}".to_string()),
    ))
}

pub(crate) fn handle_runtime_proxy_admin_request(
    request: &mut tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
) -> Option<tiny_http::ResponseBox> {
    let path = path_without_query(request.url());
    let route = prodex_runtime_broker::RuntimeBrokerAdminRoute::from_path(path)?;

    let Some(metadata) = runtime_broker_metadata_for_log_path(&shared.log_path) else {
        return Some(runtime_broker_admin_error_response(
            prodex_runtime_broker::runtime_broker_admin_not_enabled_error(),
        ));
    };
    if let Err(error) = prodex_runtime_broker::runtime_broker_validate_admin_token(
        runtime_proxy_admin_token(request),
        metadata.admin_token.as_ref(),
    ) {
        return Some(runtime_broker_admin_error_response(error));
    }

    match route {
        prodex_runtime_broker::RuntimeBrokerAdminRoute::Health => {
            runtime_broker_health_response(shared, metadata)
        }
        prodex_runtime_broker::RuntimeBrokerAdminRoute::Metrics => {
            runtime_broker_metrics_json_response(shared, &metadata)
        }
        prodex_runtime_broker::RuntimeBrokerAdminRoute::MetricsPrometheus => {
            runtime_broker_metrics_prometheus_response(shared, &metadata)
        }
        prodex_runtime_broker::RuntimeBrokerAdminRoute::Activate => {
            runtime_broker_activation_response(request, shared, metadata)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_broker_activation_body_limit_allows_exact_limit() {
        let body = vec![b'a'; 4];

        let captured =
            read_runtime_broker_activation_body_limited(std::io::Cursor::new(body.clone()), 4)
                .unwrap();

        assert_eq!(captured, body);
    }

    #[test]
    fn runtime_broker_activation_body_limit_rejects_limit_plus_one() {
        let err =
            read_runtime_broker_activation_body_limited(std::io::Cursor::new(vec![b'a'; 5]), 4)
                .expect_err("body above limit should be rejected");

        assert!(
            err.downcast_ref::<RuntimeBrokerActivationBodyTooLarge>()
                .is_some()
        );
    }
}
