use super::provider_bridge::{RuntimeProviderBridgeKind, runtime_provider_label};
use prodex_provider_core::{
    ProviderModelCost, calculate_cost_microusd, estimate_request_input_tokens,
    estimate_text_tokens, extract_usage_tokens, microusd_to_usd,
};
use runtime_proxy_crate::{
    path_without_query, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};
use serde::Serialize;

#[allow(clippy::too_many_arguments)]
pub(super) fn runtime_provider_gateway_spend_event(
    request_id: u64,
    kind: RuntimeProviderBridgeKind,
    path_and_query: &str,
    model: Option<&str>,
    status: u16,
    elapsed_ms: u128,
    request_bytes: usize,
    request_body: &[u8],
    cost: ProviderModelCost,
) -> RuntimeProviderGatewaySpendEvent {
    let model = model
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
        .to_string();
    let input_tokens = Some(estimate_request_input_tokens(request_body));
    let cost_usd = calculate_cost_microusd(input_tokens, None, cost).map(microusd_to_usd);
    RuntimeProviderGatewaySpendEvent {
        event: "gateway_spend",
        phase: "request",
        request: request_id,
        call_id: format!("prodex-{request_id}"),
        provider: runtime_provider_label(kind).to_string(),
        path: path_without_query(path_and_query).to_string(),
        model,
        status,
        elapsed_ms,
        request_bytes,
        response_bytes: None,
        input_tokens,
        output_tokens: None,
        cost_usd,
        sink: "runtime-log".to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn runtime_provider_gateway_response_spend_event(
    request_id: u64,
    kind: RuntimeProviderBridgeKind,
    path_and_query: &str,
    model: Option<&str>,
    status: u16,
    elapsed_ms: u128,
    request_body: &[u8],
    response_body: &[u8],
    cost: ProviderModelCost,
) -> RuntimeProviderGatewaySpendEvent {
    let usage = extract_usage_tokens(response_body);
    let output_tokens = usage.output_tokens.or_else(|| {
        (!response_body.is_empty())
            .then(|| estimate_text_tokens(&String::from_utf8_lossy(response_body)))
    });
    runtime_provider_gateway_response_spend_event_from_tokens(
        request_id,
        kind,
        path_and_query,
        model,
        status,
        elapsed_ms,
        request_body,
        response_body.len(),
        usage.input_tokens,
        output_tokens,
        cost,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn runtime_provider_gateway_response_spend_event_from_tokens(
    request_id: u64,
    kind: RuntimeProviderBridgeKind,
    path_and_query: &str,
    model: Option<&str>,
    status: u16,
    elapsed_ms: u128,
    request_body: &[u8],
    response_bytes: usize,
    observed_input_tokens: Option<u64>,
    observed_output_tokens: Option<u64>,
    cost: ProviderModelCost,
) -> RuntimeProviderGatewaySpendEvent {
    let model = model
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
        .to_string();
    let input_tokens =
        observed_input_tokens.or_else(|| Some(estimate_request_input_tokens(request_body)));
    let output_tokens = observed_output_tokens;
    let cost_usd = calculate_cost_microusd(input_tokens, output_tokens, cost).map(microusd_to_usd);
    RuntimeProviderGatewaySpendEvent {
        event: "gateway_spend",
        phase: "response",
        request: request_id,
        call_id: format!("prodex-{request_id}"),
        provider: runtime_provider_label(kind).to_string(),
        path: path_without_query(path_and_query).to_string(),
        model,
        status,
        elapsed_ms,
        request_bytes: request_body.len(),
        response_bytes: Some(response_bytes),
        input_tokens,
        output_tokens,
        cost_usd,
        sink: "runtime-log".to_string(),
    }
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct RuntimeProviderGatewaySpendEvent {
    pub(super) event: &'static str,
    pub(super) phase: &'static str,
    pub(super) request: u64,
    pub(super) call_id: String,
    pub(super) provider: String,
    pub(super) path: String,
    pub(super) model: String,
    pub(super) status: u16,
    pub(super) elapsed_ms: u128,
    pub(super) request_bytes: usize,
    pub(super) response_bytes: Option<usize>,
    pub(super) input_tokens: Option<u64>,
    pub(super) output_tokens: Option<u64>,
    pub(super) cost_usd: Option<f64>,
    pub(super) sink: String,
}

impl RuntimeProviderGatewaySpendEvent {
    pub(super) fn log_message(&self) -> String {
        runtime_proxy_structured_log_message(
            "gateway_spend",
            [
                runtime_proxy_log_field("phase", self.phase),
                runtime_proxy_log_field("request", self.request.to_string()),
                runtime_proxy_log_field("call_id", self.call_id.as_str()),
                runtime_proxy_log_field("provider", self.provider.as_str()),
                runtime_proxy_log_field("path", self.path.as_str()),
                runtime_proxy_log_field("model", self.model.as_str()),
                runtime_proxy_log_field("status", self.status.to_string()),
                runtime_proxy_log_field("elapsed_ms", self.elapsed_ms.to_string()),
                runtime_proxy_log_field("request_bytes", self.request_bytes.to_string()),
                runtime_proxy_log_field(
                    "response_bytes",
                    optional_u64_label(self.response_bytes.map(|value| value as u64)),
                ),
                runtime_proxy_log_field("input_tokens", optional_u64_label(self.input_tokens)),
                runtime_proxy_log_field("output_tokens", optional_u64_label(self.output_tokens)),
                runtime_proxy_log_field("cost_usd", optional_f64_label(self.cost_usd)),
                runtime_proxy_log_field("sink", self.sink.as_str()),
            ],
        )
    }
}

fn optional_u64_label(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn optional_f64_label(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.8}"))
        .unwrap_or_else(|| "unknown".to_string())
}
