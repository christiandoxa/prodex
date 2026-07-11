//! Gateway spend event logging and optional external observability sinks.

use super::super::local_rewrite::{
    RuntimeLocalRewriteProxyShared, runtime_gateway_try_reserve_background_task,
    schedule_runtime_gateway_billing_ledger_reconcile,
};
use super::super::provider_bridge::{
    RuntimeProviderGatewaySpendEvent, runtime_provider_gateway_spend_apply_admission_ids,
    runtime_provider_gateway_spend_apply_ledger_scope,
};
use super::runtime_local_rewrite_log_url;
use crate::runtime_proxy_log;
use redaction::redaction_redact_secret_like_text;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::io::Write;
use std::path::Path;
use std::time::Duration;

const RUNTIME_GATEWAY_OBSERVABILITY_HTTP_TIMEOUT: Duration = Duration::from_secs(10);

pub(in crate::runtime_launch::proxy_startup) fn emit_runtime_gateway_spend_event(
    shared: &RuntimeLocalRewriteProxyShared,
    mut event: RuntimeProviderGatewaySpendEvent,
) {
    let typed_request_id = shared
        .gateway_usage
        .typed_request_ids
        .lock()
        .ok()
        .and_then(|typed_request_ids| typed_request_ids.get(&event.request).cloned());
    let call_id = shared
        .gateway_usage
        .call_ids
        .lock()
        .ok()
        .and_then(|call_ids| call_ids.get(&event.request).cloned());
    let ledger_scope = shared
        .gateway_usage
        .ledger_scopes
        .lock()
        .ok()
        .and_then(|ledger_scopes| ledger_scopes.get(&event.request).cloned());
    runtime_provider_gateway_spend_apply_admission_ids(
        &mut event,
        typed_request_id.as_deref(),
        call_id.as_deref(),
    );
    runtime_provider_gateway_spend_apply_ledger_scope(
        &mut event,
        ledger_scope.as_ref().map(|scope| scope.key_name.as_str()),
        ledger_scope
            .as_ref()
            .and_then(|scope| scope.tenant_id.as_deref()),
    );
    runtime_proxy_log(&shared.runtime_shared, event.log_message());
    schedule_runtime_gateway_billing_ledger_reconcile(shared, event.clone());
    if !shared.gateway_observability.sink_enabled("jsonl")
        && !shared.gateway_observability.sink_enabled("http")
    {
        return;
    }
    let Some(permit) =
        runtime_gateway_try_reserve_background_task(&shared.gateway_observability_slots)
    else {
        runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_observability_dropped",
                [
                    runtime_proxy_log_field("request", event.request.to_string()),
                    runtime_proxy_log_field("reason", "task_limit"),
                ],
            ),
        );
        return;
    };
    let shared = shared.clone();
    drop(
        shared
            .runtime_shared
            .async_runtime
            .clone()
            .spawn_blocking(move || {
                let _permit = permit;
                emit_runtime_gateway_observability_sinks_blocking(&shared, &event);
            }),
    );
}

fn emit_runtime_gateway_observability_sinks_blocking(
    shared: &RuntimeLocalRewriteProxyShared,
    event: &RuntimeProviderGatewaySpendEvent,
) {
    if shared.gateway_observability.sink_enabled("jsonl")
        && let Some(path) = shared.gateway_observability.jsonl_path.as_ref()
        && let Err(err) = runtime_gateway_observability_write_jsonl(path, event)
    {
        runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_observability_jsonl_failed",
                [
                    runtime_proxy_log_field("request", event.request.to_string()),
                    runtime_proxy_log_field(
                        "path",
                        runtime_gateway_observability_redacted_log_text(
                            &path.display().to_string(),
                        ),
                    ),
                    runtime_proxy_log_field(
                        "error",
                        runtime_gateway_observability_redacted_log_text(&err.to_string()),
                    ),
                ],
            ),
        );
    }
    if shared.gateway_observability.sink_enabled("http")
        && let Some(endpoint) = shared.gateway_observability.http_endpoint.as_deref()
    {
        let payload = runtime_gateway_observability_http_payload(
            event,
            shared.gateway_observability.http_schema.as_str(),
        );
        let mut request = shared
            .client
            .post(endpoint)
            .timeout(RUNTIME_GATEWAY_OBSERVABILITY_HTTP_TIMEOUT)
            .json(&payload);
        if let Some(token) = shared.gateway_observability.http_bearer_token.as_deref() {
            request = request.bearer_auth(token);
        }
        if let Err(err) = request.send() {
            let error = runtime_gateway_observability_redacted_log_text(
                &reqwest::Error::without_url(err).to_string(),
            );
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_observability_http_failed",
                    [
                        runtime_proxy_log_field("request", event.request.to_string()),
                        runtime_proxy_log_field(
                            "endpoint",
                            runtime_gateway_observability_redacted_log_text(
                                &runtime_local_rewrite_log_url(endpoint),
                            ),
                        ),
                        runtime_proxy_log_field("error", error),
                    ],
                ),
            );
        }
    }
}

fn runtime_gateway_observability_redacted_log_text(value: &str) -> String {
    redaction_redact_secret_like_text(value)
}

fn runtime_gateway_observability_write_jsonl(
    path: &Path,
    event: &RuntimeProviderGatewaySpendEvent,
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let payload = serde_json::to_string(event).map_err(std::io::Error::other)?;
    writeln!(file, "{payload}")
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
        serde_json::json!({"key": "prodex.request_id", "value": {"stringValue": event.request_id}}),
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
    use std::str::FromStr;

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
        let call_id = generic["call_id"].as_str().expect("call_id is a string");
        let uuid = call_id
            .strip_prefix("prodex-")
            .and_then(|value| prodex_domain::CallId::from_str(value).ok())
            .expect("call_id uses a prodex-scoped uuidv7");
        assert_eq!(uuid.as_uuid().get_version_num(), 7);

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

    #[test]
    fn gateway_observability_jsonl_write_preserves_event_payload() {
        let root = std::env::temp_dir().join(format!(
            "prodex-gateway-jsonl-{}",
            prodex_domain::RequestId::new()
        ));
        let path = root.join("spend.jsonl");
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

        runtime_gateway_observability_write_jsonl(&path, &event).unwrap();

        let line = std::fs::read_to_string(&path).unwrap();
        let json: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(json["event"], "gateway_spend");
        assert_eq!(json["legacy_request_sequence"], 7);
        assert!(json.get("request").is_none());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_observability_failure_log_text_redacts_endpoint_secrets() {
        let endpoint =
            "https://telemetry.example.test/ingest?api_key=sk-live-fixture-notreal-123456";
        let error = "failed to send Authorization: Bearer fixture_http_notreal_12345";
        let path = "/tmp/prodex/sk-live-fixture-notreal-path/spend.jsonl";

        let redacted_endpoint = runtime_gateway_observability_redacted_log_text(endpoint);
        let redacted_error = runtime_gateway_observability_redacted_log_text(error);
        let redacted_path = runtime_gateway_observability_redacted_log_text(path);

        assert!(redacted_endpoint.contains("api_key=<redacted>"));
        assert!(!redacted_endpoint.contains("sk-live-fixture-notreal-123456"));
        assert!(redacted_error.contains("Authorization: Bearer <redacted>"));
        assert!(!redacted_error.contains("fixture_http_notreal_12345"));
        assert!(redacted_path.contains("sk-live-<redacted>"));
        assert!(!redacted_path.contains("sk-live-fixture-notreal-path"));
    }
}
