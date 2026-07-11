use super::provider_bridge::{RuntimeProviderBridgeKind, runtime_provider_label};
use prodex_domain::{CallId, RequestId, ReservationReconciliationReason};
use prodex_provider_core::{
    ProviderModelCost, calculate_cost_microusd, estimate_request_input_tokens,
    estimate_text_tokens, extract_usage_tokens, microusd_to_usd,
};
use runtime_proxy_crate::{
    path_without_query, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};
use serde::Serialize;
use std::fmt;

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
    let reserved_tokens = runtime_proxy_crate::runtime_gateway_estimated_tokens(request_body);
    let output_tokens =
        input_tokens.map(|input_tokens| reserved_tokens.saturating_sub(input_tokens));
    let cost_usd = calculate_cost_microusd(input_tokens, output_tokens, cost).map(microusd_to_usd);
    RuntimeProviderGatewaySpendEvent {
        event: "gateway_spend",
        phase: "request",
        request: request_id,
        key_name: None,
        tenant_id: None,
        request_id: runtime_provider_gateway_spend_request_id(),
        legacy_request_sequence: request_id,
        call_id: runtime_provider_gateway_spend_call_id(),
        provider: runtime_provider_label(kind).to_string(),
        path: path_without_query(path_and_query).to_string(),
        model,
        status,
        elapsed_ms,
        request_bytes,
        response_bytes: None,
        input_tokens,
        output_tokens,
        cost_usd,
        reconciliation_reason: None,
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
        key_name: None,
        tenant_id: None,
        request_id: runtime_provider_gateway_spend_request_id(),
        legacy_request_sequence: request_id,
        call_id: runtime_provider_gateway_spend_call_id(),
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
        reconciliation_reason: Some(ReservationReconciliationReason::Completed),
        sink: "runtime-log".to_string(),
    }
}

#[derive(Clone, Serialize)]
pub(super) struct RuntimeProviderGatewaySpendEvent {
    pub(super) event: &'static str,
    pub(super) phase: &'static str,
    #[serde(skip)]
    pub(super) request: u64,
    #[serde(skip)]
    pub(super) key_name: Option<String>,
    #[serde(skip)]
    pub(super) tenant_id: Option<String>,
    pub(super) request_id: String,
    pub(super) legacy_request_sequence: u64,
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
    pub(super) reconciliation_reason: Option<ReservationReconciliationReason>,
    pub(super) sink: String,
}

impl fmt::Debug for RuntimeProviderGatewaySpendEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeProviderGatewaySpendEvent")
            .field("event", &self.event)
            .field("phase", &self.phase)
            .field("request", &"<redacted>")
            .field("key_name", &redacted_option(&self.key_name))
            .field("tenant_id", &redacted_option(&self.tenant_id))
            .field("request_id", &"<redacted>")
            .field("legacy_request_sequence", &"<redacted>")
            .field("call_id", &"<redacted>")
            .field("provider", &self.provider)
            .field("path", &"<redacted>")
            .field("model", &"<redacted>")
            .field("status", &self.status)
            .field("elapsed_ms", &"<redacted>")
            .field("request_bytes", &"<redacted>")
            .field("response_bytes", &redacted_option(&self.response_bytes))
            .field("input_tokens", &redacted_option(&self.input_tokens))
            .field("output_tokens", &redacted_option(&self.output_tokens))
            .field("cost_usd", &redacted_option(&self.cost_usd))
            .field("reconciliation_reason", &self.reconciliation_reason)
            .field("sink", &self.sink)
            .finish()
    }
}

fn redacted_option<T>(value: &Option<T>) -> Option<&'static str> {
    value.as_ref().map(|_| "<redacted>")
}

impl RuntimeProviderGatewaySpendEvent {
    pub(super) fn log_message(&self) -> String {
        runtime_proxy_structured_log_message(
            "gateway_spend",
            [
                runtime_proxy_log_field("phase", self.phase),
                runtime_proxy_log_field("request", self.request.to_string()),
                runtime_proxy_log_field("request_id", self.request_id.as_str()),
                runtime_proxy_log_field(
                    "legacy_request_sequence",
                    self.legacy_request_sequence.to_string(),
                ),
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
                runtime_proxy_log_field(
                    "reconciliation_reason",
                    self.reconciliation_reason
                        .map(|reason| match reason {
                            ReservationReconciliationReason::Completed => "completed",
                            ReservationReconciliationReason::Cancelled => "cancelled",
                            ReservationReconciliationReason::StreamInterrupted => {
                                "stream_interrupted"
                            }
                        })
                        .unwrap_or("none"),
                ),
                runtime_proxy_log_field("sink", self.sink.as_str()),
            ],
        )
    }
}

fn runtime_provider_gateway_spend_call_id() -> String {
    format!("prodex-{}", CallId::new())
}

fn runtime_provider_gateway_spend_request_id() -> String {
    format!("prodex-{}", RequestId::new())
}

pub(super) fn runtime_provider_gateway_spend_apply_admission_ids(
    event: &mut RuntimeProviderGatewaySpendEvent,
    typed_request_id: Option<&str>,
    call_id: Option<&str>,
) {
    if let Some(typed_request_id) = typed_request_id {
        event.request_id = typed_request_id.to_string();
    }
    if let Some(call_id) = call_id {
        event.call_id = call_id.to_string();
    }
}

pub(super) fn runtime_provider_gateway_spend_apply_ledger_scope(
    event: &mut RuntimeProviderGatewaySpendEvent,
    key_name: Option<&str>,
    tenant_id: Option<&str>,
) {
    if let Some(key_name) = key_name {
        event.key_name = Some(key_name.to_string());
    }
    if let Some(tenant_id) = tenant_id {
        event.tenant_id = Some(tenant_id.to_string());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_gateway_spend_event_debug_output_redacts_sensitive_fields() {
        let event = RuntimeProviderGatewaySpendEvent {
            event: "gateway_spend",
            phase: "response",
            request: 42,
            key_name: Some("sk-provider-secret".to_string()),
            tenant_id: Some("tenant-provider-secret".to_string()),
            request_id: "prodex-request-provider-secret".to_string(),
            legacy_request_sequence: 42,
            call_id: "prodex-call-provider-secret".to_string(),
            provider: "openai-compatible".to_string(),
            path: "/v1/responses?api_key=secret".to_string(),
            model: "gpt-provider-secret".to_string(),
            status: 200,
            elapsed_ms: 1_234,
            request_bytes: 5_678,
            response_bytes: Some(9_012),
            input_tokens: Some(345),
            output_tokens: Some(678),
            cost_usd: Some(0.12345678),
            reconciliation_reason: Some(ReservationReconciliationReason::Completed),
            sink: "runtime-log".to_string(),
        };
        let rendered = format!("{event:?}");

        assert!(rendered.contains("RuntimeProviderGatewaySpendEvent"));
        assert!(rendered.contains("provider: \"openai-compatible\""));
        assert!(rendered.contains("status: 200"));
        assert!(rendered.contains("<redacted>"));
        for raw in [
            "sk-provider-secret",
            "tenant-provider-secret",
            "prodex-request-provider-secret",
            "prodex-call-provider-secret",
            "/v1/responses",
            "api_key=secret",
            "gpt-provider-secret",
            "0.12345678",
        ] {
            assert!(!rendered.contains(raw), "{rendered}");
        }
    }
}
