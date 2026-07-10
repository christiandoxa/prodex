//! Gateway spend event logging and optional external observability sinks.

use super::super::local_rewrite::{
    RuntimeLocalRewriteProxyShared, schedule_runtime_gateway_billing_ledger_reconcile,
};
use super::super::provider_bridge::RuntimeProviderGatewaySpendEvent;
use crate::runtime_proxy_log;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::io::Write;

pub(in crate::runtime_launch::proxy_startup) fn emit_runtime_gateway_spend_event(
    shared: &RuntimeLocalRewriteProxyShared,
    event: RuntimeProviderGatewaySpendEvent,
) {
    runtime_proxy_log(&shared.runtime_shared, event.log_message());
    schedule_runtime_gateway_billing_ledger_reconcile(shared, event.clone());
    if shared.gateway_observability.sink_enabled("jsonl")
        && let Some(path) = shared.gateway_observability.jsonl_path.as_ref()
    {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            Ok(mut file) => {
                if let Ok(payload) = serde_json::to_string(&event) {
                    let _ = writeln!(file, "{payload}");
                }
            }
            Err(err) => runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_observability_jsonl_failed",
                    [
                        runtime_proxy_log_field("request", event.request.to_string()),
                        runtime_proxy_log_field("path", path.display().to_string()),
                        runtime_proxy_log_field("error", err.to_string()),
                    ],
                ),
            ),
        }
    }
    if shared.gateway_observability.sink_enabled("http")
        && let Some(endpoint) = shared.gateway_observability.http_endpoint.as_deref()
    {
        let payload = runtime_gateway_observability_http_payload(
            &event,
            shared.gateway_observability.http_schema.as_str(),
        );
        let mut request = shared.client.post(endpoint).json(&payload);
        if let Some(token) = shared.gateway_observability.http_bearer_token.as_deref() {
            request = request.bearer_auth(token);
        }
        if let Err(err) = request.send() {
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_observability_http_failed",
                    [
                        runtime_proxy_log_field("request", event.request.to_string()),
                        runtime_proxy_log_field("endpoint", endpoint),
                        runtime_proxy_log_field("error", err.to_string()),
                    ],
                ),
            );
        }
    }
}

fn runtime_gateway_observability_http_payload(
    event: &RuntimeProviderGatewaySpendEvent,
    schema: &str,
) -> serde_json::Value {
    match schema.trim().to_ascii_lowercase().as_str() {
        "otel" | "opentelemetry" => serde_json::json!({
            "resourceLogs": [{
                "resource": {"attributes": [
                    {"key": "service.name", "value": {"stringValue": "prodex-gateway"}}
                ]},
                "scopeLogs": [{
                    "scope": {"name": "prodex.gateway"},
                    "logRecords": [{
                        "severityText": "INFO",
                        "body": {"stringValue": event.event},
                        "attributes": runtime_gateway_otel_attributes(event)
                    }]
                }]
            }]
        }),
        "datadog" => serde_json::json!([{
            "ddsource": "prodex",
            "service": "prodex-gateway",
            "message": event.event,
            "status": "info",
            "prodex": event
        }]),
        "langfuse" => serde_json::json!({
            "batch": [{
                "id": event.call_id,
                "type": "trace-create",
                "body": {
                    "id": event.call_id,
                    "name": "prodex-gateway-call",
                    "metadata": event
                }
            }]
        }),
        _ => serde_json::to_value(event).unwrap_or_else(|_| serde_json::json!({})),
    }
}

fn runtime_gateway_otel_attributes(
    event: &RuntimeProviderGatewaySpendEvent,
) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({"key": "prodex.call_id", "value": {"stringValue": event.call_id}}),
        serde_json::json!({"key": "prodex.phase", "value": {"stringValue": event.phase}}),
        serde_json::json!({"key": "prodex.provider", "value": {"stringValue": event.provider}}),
        serde_json::json!({"key": "prodex.path", "value": {"stringValue": event.path}}),
        serde_json::json!({"key": "prodex.model", "value": {"stringValue": event.model}}),
        serde_json::json!({"key": "http.response.status_code", "value": {"intValue": event.status.to_string()}}),
        serde_json::json!({"key": "prodex.elapsed_ms", "value": {"intValue": event.elapsed_ms.to_string()}}),
        serde_json::json!({"key": "prodex.request_bytes", "value": {"intValue": event.request_bytes.to_string()}}),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::super::provider_bridge::{
        RuntimeProviderBridgeKind, runtime_provider_gateway_spend_event,
    };
    use super::*;

    #[test]
    fn gateway_observability_http_payload_supports_vendor_schemas() {
        let event = runtime_provider_gateway_spend_event(
            7,
            RuntimeProviderBridgeKind::OpenAiResponses,
            "/v1/responses",
            Some("gpt-5-mini"),
            200,
            42,
            128,
            br#"{"model":"gpt-5-mini","input":"hello from prodex"}"#,
            prodex_provider_core::ProviderModelCost::default(),
        );

        let generic = runtime_gateway_observability_http_payload(&event, "generic");
        assert_eq!(generic["event"], "gateway_spend");
        assert_eq!(generic["call_id"], "prodex-7");

        let otel = runtime_gateway_observability_http_payload(&event, "otel");
        assert_eq!(
            otel["resourceLogs"][0]["scopeLogs"][0]["scope"]["name"],
            "prodex.gateway"
        );

        let datadog = runtime_gateway_observability_http_payload(&event, "datadog");
        assert_eq!(datadog[0]["service"], "prodex-gateway");

        let langfuse = runtime_gateway_observability_http_payload(&event, "langfuse");
        assert_eq!(langfuse["batch"][0]["type"], "trace-create");
    }
}
