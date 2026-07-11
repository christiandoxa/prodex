use super::super::provider_models::{
    runtime_provider_model_catalog_json, runtime_provider_model_json_for,
};
use super::{RuntimeProviderBridgeKind, runtime_provider_label, runtime_provider_openai_contract};
use crate::RuntimeHeapTrimmedBufferedResponseParts;
use prodex_gateway_http::{GatewayHttpRouteKind, classify_route};
use prodex_provider_core::{
    ProviderAdapterContract, ProviderCapabilityStatus, ProviderEndpoint, ProviderModelCost,
    provider_adapter, provider_model_cost, provider_model_fallback_chain,
};
use runtime_proxy_crate::{
    path_without_query, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};
use std::collections::BTreeMap;

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_native_passthrough(
    kind: RuntimeProviderBridgeKind,
    path_and_query: &str,
) -> bool {
    let path = path_without_query(path_and_query);
    let Some(route) = runtime_provider_route_kind(path) else {
        return true;
    };
    if matches!(kind, RuntimeProviderBridgeKind::OpenAiResponses) {
        return true;
    }
    let Some(endpoint) = runtime_provider_route_endpoint(route) else {
        return false;
    };
    if matches!(endpoint, ProviderEndpoint::ResponsesCompact) {
        return false;
    }
    matches!(
        provider_adapter(kind.provider_id()).capability_status(endpoint),
        ProviderCapabilityStatus::Native | ProviderCapabilityStatus::Passthrough
    )
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_models_buffered_response(
    kind: RuntimeProviderBridgeKind,
    dynamic_catalog: Option<&[serde_json::Value]>,
    method: &str,
    path_and_query: &str,
) -> Option<RuntimeHeapTrimmedBufferedResponseParts> {
    if !method.eq_ignore_ascii_case("GET") {
        return None;
    }
    let models = runtime_provider_model_catalog_json(kind, dynamic_catalog);
    if models.is_empty() {
        return None;
    }
    let path = path_without_query(path_and_query);
    match runtime_provider_route_kind(path)? {
        RuntimeProviderRouteKind::ModelsList => {
            let body = serde_json::json!({
                "object": "list",
                "data": models,
            });
            Some(runtime_provider_json_response(200, body))
        }
        RuntimeProviderRouteKind::ModelsSingle(model_id) => {
            let model = runtime_provider_model_json_for(kind, dynamic_catalog, model_id);
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

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_gateway_cost_for_request(
    kind: RuntimeProviderBridgeKind,
    aliases: &[runtime_proxy_crate::RuntimeGatewayRouteAlias],
    model_state: &BTreeMap<String, runtime_proxy_crate::RuntimeGatewayRouteModelState>,
    request_id: u64,
    body: &[u8],
    model: &str,
) -> ProviderModelCost {
    let model = model.trim();
    let direct_metric_model = model
        .strip_prefix("combo:")
        .and_then(|combo| {
            combo
                .split(',')
                .map(str::trim)
                .find(|part| !part.is_empty())
        })
        .unwrap_or(model);
    if let Some(metrics) = aliases
        .iter()
        .find_map(|alias| alias.model_metrics.get(direct_metric_model))
    {
        return ProviderModelCost {
            input_cost_per_million_microusd: metrics.input_cost_per_million_microusd,
            output_cost_per_million_microusd: metrics.output_cost_per_million_microusd,
        };
    }
    if let Some(rewrite) = runtime_proxy_crate::runtime_gateway_rewrite_route_alias_with_state(
        body,
        aliases,
        request_id,
        model_state,
    ) && let Some(alias) = aliases
        .iter()
        .find(|alias| alias.alias.eq_ignore_ascii_case(model))
    {
        if let Some(metrics) = alias.model_metrics.get(&rewrite.model) {
            return ProviderModelCost {
                input_cost_per_million_microusd: metrics.input_cost_per_million_microusd,
                output_cost_per_million_microusd: metrics.output_cost_per_million_microusd,
            };
        }
        if matches!(
            rewrite.strategy,
            runtime_proxy_crate::RuntimeGatewayRouteStrategy::Fallback
        ) && let Some(first_model) = alias.models.first()
            && let Some(metrics) = alias.model_metrics.get(first_model)
        {
            return ProviderModelCost {
                input_cost_per_million_microusd: metrics.input_cost_per_million_microusd,
                output_cost_per_million_microusd: metrics.output_cost_per_million_microusd,
            };
        }
    }
    provider_model_cost(kind.provider_id(), model)
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
    let contract = runtime_provider_openai_contract(kind);
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
            runtime_proxy_log_field("client_format", contract.client_request_format.label()),
            runtime_proxy_log_field("upstream_format", contract.upstream_request_format.label()),
            runtime_proxy_log_field("response_format", contract.response_format.label()),
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

fn runtime_provider_route_endpoint(
    route: RuntimeProviderRouteKind<'_>,
) -> Option<ProviderEndpoint> {
    match route {
        RuntimeProviderRouteKind::Responses => Some(ProviderEndpoint::Responses),
        RuntimeProviderRouteKind::ResponsesCompact => Some(ProviderEndpoint::ResponsesCompact),
        RuntimeProviderRouteKind::ChatCompletions => Some(ProviderEndpoint::ChatCompletions),
        RuntimeProviderRouteKind::Messages => Some(ProviderEndpoint::Messages),
        RuntimeProviderRouteKind::Embeddings => Some(ProviderEndpoint::Embeddings),
        RuntimeProviderRouteKind::ModelsList | RuntimeProviderRouteKind::ModelsSingle(_) => None,
    }
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_route_kind(
    path: &str,
) -> Option<RuntimeProviderRouteKind<'_>> {
    let path = path_without_query(path);
    match classify_route(path) {
        GatewayHttpRouteKind::DataPlaneResponses => Some(RuntimeProviderRouteKind::Responses),
        GatewayHttpRouteKind::DataPlaneCompact => Some(RuntimeProviderRouteKind::ResponsesCompact),
        GatewayHttpRouteKind::DataPlaneChatCompletions => {
            Some(RuntimeProviderRouteKind::ChatCompletions)
        }
        GatewayHttpRouteKind::DataPlaneMessages => Some(RuntimeProviderRouteKind::Messages),
        GatewayHttpRouteKind::DataPlaneEmbeddings => Some(RuntimeProviderRouteKind::Embeddings),
        GatewayHttpRouteKind::DataPlaneModels => Some(RuntimeProviderRouteKind::ModelsList),
        GatewayHttpRouteKind::DataPlaneModel => ["/v1/models/", "/models/"]
            .into_iter()
            .find_map(|prefix| path.strip_prefix(prefix))
            .map(RuntimeProviderRouteKind::ModelsSingle),
        _ => None,
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
