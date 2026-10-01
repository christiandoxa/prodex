use super::super::provider_models::{
    runtime_provider_model_catalog_json, runtime_provider_model_json_for,
};
use super::{RuntimeProviderBridgeKind, runtime_provider_label};
use crate::RuntimeHeapTrimmedBufferedResponseParts;
use prodex_mojo_core::{
    provider_constraints::provider_bridge_native_passthrough, rich::ascii_casefold_equal_exact,
};
use prodex_provider_core::{ProviderEndpoint, provider_adapter, provider_model_fallback_chain};
use runtime_proxy_crate::{
    path_without_query, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_native_passthrough(
    kind: RuntimeProviderBridgeKind,
    path_and_query: &str,
) -> bool {
    let route = runtime_provider_route_kind(path_and_query);
    let (route_kind, capability_status) = match route {
        Some(route) => (
            runtime_provider_route_kind_tag(route),
            provider_adapter(kind.provider_id())
                .capability_status(runtime_provider_route_endpoint(route)) as i64,
        ),
        None => (-1, -1),
    };
    provider_bridge_native_passthrough(kind as i64, route_kind, capability_status)
        .expect("Mojo provider native-passthrough policy returned invalid output")
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_models_buffered_response(
    kind: RuntimeProviderBridgeKind,
    dynamic_catalog: Option<&[serde_json::Value]>,
    method: &str,
    path_and_query: &str,
) -> Option<RuntimeHeapTrimmedBufferedResponseParts> {
    if !ascii_casefold_equal_exact(method, "GET")
        .expect("Mojo provider models-method comparison failed")
    {
        return None;
    }
    let route = runtime_provider_route_kind(path_and_query)?;
    if !matches!(
        route,
        RuntimeProviderRouteKind::ModelsList | RuntimeProviderRouteKind::ModelsSingle(_)
    ) {
        return None;
    }
    let models = match runtime_provider_model_catalog_json(kind, dynamic_catalog) {
        Ok(models) => models,
        Err(error) => {
            return Some(runtime_provider_json_response(
                503,
                serde_json::json!({
                    "error": {
                        "message": error.to_string(),
                        "type": "service_unavailable",
                        "code": "model_catalog_limit_exceeded"
                    }
                }),
            ));
        }
    };
    if models.is_empty() {
        return None;
    }
    match route {
        RuntimeProviderRouteKind::ModelsList => {
            let body = serde_json::json!({
                "object": "list",
                "data": models,
            });
            Some(runtime_provider_json_response(200, body))
        }
        RuntimeProviderRouteKind::ModelsSingle(model_id) => {
            let model = runtime_provider_model_json_for(kind, &models, model_id);
            let status = if model.is_some() { 200 } else { 404 };
            let body = model.unwrap_or_else(|| {
                serde_json::json!({
                    "error": {
                        "message": format!("model '{model_id}' is not available for {}", runtime_provider_label(kind)),
                        "type": "invalid_request_error",
                        "code": "model_not_found"
                    }
                })
            });
            Some(runtime_provider_json_response(status, body))
        }
        RuntimeProviderRouteKind::Responses
        | RuntimeProviderRouteKind::ResponsesCompact
        | RuntimeProviderRouteKind::ChatCompletions
        | RuntimeProviderRouteKind::Messages
        | RuntimeProviderRouteKind::Embeddings => None,
    }
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_request_body_with_model(
    body: &[u8],
    model: &str,
) -> Vec<u8> {
    prodex_provider_core::provider_request_body_with_model(body, model)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_model_fallback_chain(
    kind: RuntimeProviderBridgeKind,
    model: &str,
) -> Vec<String> {
    provider_model_fallback_chain(kind.provider_id(), model)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_canonical_model(
    kind: RuntimeProviderBridgeKind,
    model: &str,
) -> String {
    prodex_provider_core::provider_canonical_model(kind.provider_id(), model)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_request_ledger_message(
    request_id: u64,
    kind: RuntimeProviderBridgeKind,
    path_and_query: &str,
    model: Option<&str>,
    status: u16,
    elapsed_ms: u128,
    body_bytes: usize,
) -> String {
    let adapter = provider_adapter(kind.provider_id());
    runtime_proxy_structured_log_message(
        "local_rewrite_request_detail",
        [
            runtime_proxy_log_field("request", request_id.to_string()),
            runtime_proxy_log_field("provider", runtime_provider_label(kind)),
            runtime_proxy_log_field("path", path_without_query(path_and_query)),
            runtime_proxy_log_field(
                "model",
                model
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("unknown"),
            ),
            runtime_proxy_log_field("status", status.to_string()),
            runtime_proxy_log_field("elapsed_ms", elapsed_ms.to_string()),
            runtime_proxy_log_field("body_bytes", body_bytes.to_string()),
            runtime_proxy_log_field(
                "native_passthrough",
                runtime_provider_native_passthrough(kind, path_and_query).to_string(),
            ),
            runtime_proxy_log_field("client_format", adapter.client_request_format().label()),
            runtime_proxy_log_field("upstream_format", adapter.upstream_request_format().label()),
            runtime_proxy_log_field("response_format", adapter.response_format().label()),
        ],
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::runtime_launch::proxy_startup) enum RuntimeProviderRouteKind<'a> {
    Responses,
    ResponsesCompact,
    ChatCompletions,
    Messages,
    Embeddings,
    ModelsList,
    ModelsSingle(&'a str),
}

fn runtime_provider_route_kind_tag(route: RuntimeProviderRouteKind<'_>) -> i64 {
    match route {
        RuntimeProviderRouteKind::Responses => 0,
        RuntimeProviderRouteKind::ResponsesCompact => 1,
        RuntimeProviderRouteKind::ChatCompletions => 2,
        RuntimeProviderRouteKind::Messages => 3,
        RuntimeProviderRouteKind::Embeddings => 4,
        RuntimeProviderRouteKind::ModelsList => 5,
        RuntimeProviderRouteKind::ModelsSingle(_) => 6,
    }
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_route_endpoint(
    route: RuntimeProviderRouteKind<'_>,
) -> ProviderEndpoint {
    match route {
        RuntimeProviderRouteKind::Responses => ProviderEndpoint::Responses,
        RuntimeProviderRouteKind::ResponsesCompact => ProviderEndpoint::ResponsesCompact,
        RuntimeProviderRouteKind::ChatCompletions => ProviderEndpoint::ChatCompletions,
        RuntimeProviderRouteKind::Messages => ProviderEndpoint::Messages,
        RuntimeProviderRouteKind::Embeddings => ProviderEndpoint::Embeddings,
        RuntimeProviderRouteKind::ModelsList | RuntimeProviderRouteKind::ModelsSingle(_) => {
            ProviderEndpoint::Models
        }
    }
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_route_kind(
    path_and_query: &str,
) -> Option<RuntimeProviderRouteKind<'_>> {
    let plan = prodex_mojo_core::rich::runtime_provider_route_plan(path_and_query)
        .expect("Mojo provider-route planning returned invalid output");
    match plan.route_kind {
        -1 => None,
        0 => Some(RuntimeProviderRouteKind::Responses),
        1 => Some(RuntimeProviderRouteKind::ResponsesCompact),
        2 => Some(RuntimeProviderRouteKind::ChatCompletions),
        3 => Some(RuntimeProviderRouteKind::Messages),
        4 => Some(RuntimeProviderRouteKind::Embeddings),
        5 => Some(RuntimeProviderRouteKind::ModelsList),
        6 => {
            let (start, end) = plan
                .model_id_range
                .expect("Mojo model route requires model-id range");
            Some(RuntimeProviderRouteKind::ModelsSingle(
                &path_and_query[start..end],
            ))
        }
        _ => unreachable!("validated Mojo provider route tag"),
    }
}

fn runtime_provider_json_response(
    status: u16,
    body: serde_json::Value,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    let body = serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec());
    RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers: vec![(
            "content-type".to_string(),
            b"application/json; charset=utf-8".to_vec(),
        )],
        body: body.into(),
    }
}
