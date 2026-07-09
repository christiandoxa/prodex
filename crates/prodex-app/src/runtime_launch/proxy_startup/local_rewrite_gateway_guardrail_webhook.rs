use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_transport::runtime_local_rewrite_log_url;
use super::*;
use base64::Engine;

const RUNTIME_GATEWAY_GUARDRAIL_WEBHOOK_MAX_RESPONSE_BYTES: usize = 64 * 1024;

pub(super) struct RuntimeGatewayGuardrailWebhookBlock {
    pub(super) reason: String,
    pub(super) value: String,
}

pub(super) fn runtime_gateway_guardrail_webhook_block(
    phase: &str,
    request_id: u64,
    body: &[u8],
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<RuntimeGatewayGuardrailWebhookBlock> {
    if !shared.gateway_guardrail_webhook.enabled_for(phase) {
        return None;
    }
    let url = shared.gateway_guardrail_webhook.url.as_deref()?;
    let payload = serde_json::json!({
        "phase": phase,
        "request_id": request_id,
        "call_id": format!("prodex-{request_id}"),
        "body_base64": base64::engine::general_purpose::STANDARD.encode(body),
    });
    let mut request = shared.client.post(url).json(&payload);
    if let Some(token) = shared.gateway_guardrail_webhook.bearer_token.as_deref() {
        request = request.bearer_auth(token);
    }
    let response = match request.send() {
        Ok(response) => response,
        Err(err) => {
            let err = err.without_url();
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_guardrail_webhook_failed",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("phase", phase),
                        runtime_proxy_log_field("url", runtime_local_rewrite_log_url(url)),
                        runtime_proxy_log_field("error", format!("{err:#}")),
                    ],
                ),
            );
            return shared.gateway_guardrail_webhook.fail_closed.then(|| {
                RuntimeGatewayGuardrailWebhookBlock {
                    reason: "webhook_error".to_string(),
                    value: err.to_string(),
                }
            });
        }
    };
    let status = response.status();
    let body = match read_runtime_buffered_response_body_with_limit(
        response,
        RUNTIME_GATEWAY_GUARDRAIL_WEBHOOK_MAX_RESPONSE_BYTES,
        "failed to read gateway guardrail webhook response body",
    ) {
        Ok(body) => body,
        Err(err) => {
            return shared.gateway_guardrail_webhook.fail_closed.then(|| {
                RuntimeGatewayGuardrailWebhookBlock {
                    reason: "webhook_body".to_string(),
                    value: err.to_string(),
                }
            });
        }
    };
    if !status.is_success() {
        return shared.gateway_guardrail_webhook.fail_closed.then(|| {
            RuntimeGatewayGuardrailWebhookBlock {
                reason: "webhook_status".to_string(),
                value: status.as_u16().to_string(),
            }
        });
    }
    let value = match serde_json::from_slice::<serde_json::Value>(&body) {
        Ok(value) => value,
        Err(err) => {
            return shared.gateway_guardrail_webhook.fail_closed.then(|| {
                RuntimeGatewayGuardrailWebhookBlock {
                    reason: "webhook_response".to_string(),
                    value: err.to_string(),
                }
            });
        }
    };
    let Some(allow) = value.get("allow").and_then(serde_json::Value::as_bool) else {
        return shared.gateway_guardrail_webhook.fail_closed.then(|| {
            RuntimeGatewayGuardrailWebhookBlock {
                reason: "webhook_response".to_string(),
                value: "missing allow field".to_string(),
            }
        });
    };
    (!allow).then(|| RuntimeGatewayGuardrailWebhookBlock {
        reason: value
            .get("reason")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("webhook_denied")
            .to_string(),
        value: value
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("blocked")
            .to_string(),
    })
}
