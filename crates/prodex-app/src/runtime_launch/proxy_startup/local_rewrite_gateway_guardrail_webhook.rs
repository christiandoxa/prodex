use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::*;
use base64::Engine;
use prodex_domain::{CallId, RequestId};

pub(super) struct RuntimeGatewayGuardrailWebhookBlock {
    pub(super) reason: String,
}

fn runtime_gateway_guardrail_webhook_error_kind(err: &reqwest::Error) -> &'static str {
    if err.is_timeout() {
        "timeout"
    } else if err.is_connect() {
        "connect"
    } else if err.is_request() {
        "request"
    } else if err.is_body() {
        "body"
    } else if err.is_decode() {
        "decode"
    } else {
        "other"
    }
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
    let payload = runtime_gateway_guardrail_webhook_payload(phase, request_id, body);
    let mut request = shared.client.post(url).json(&payload);
    if let Some(token) = shared.gateway_guardrail_webhook.bearer_token.as_deref() {
        request = request.bearer_auth(token);
    }
    let response = match request.send() {
        Ok(response) => response,
        Err(err) => {
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_guardrail_webhook_failed",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("phase", phase),
                        runtime_proxy_log_field("endpoint", "redacted"),
                        runtime_proxy_log_field(
                            "error_kind",
                            runtime_gateway_guardrail_webhook_error_kind(&err),
                        ),
                    ],
                ),
            );
            return shared.gateway_guardrail_webhook.fail_closed.then(|| {
                RuntimeGatewayGuardrailWebhookBlock {
                    reason: "webhook_error".to_string(),
                }
            });
        }
    };
    let status = response.status();
    let body = response.bytes().unwrap_or_default();
    if !status.is_success() {
        return shared.gateway_guardrail_webhook.fail_closed.then(|| {
            RuntimeGatewayGuardrailWebhookBlock {
                reason: "webhook_status".to_string(),
            }
        });
    }
    let value = serde_json::from_slice::<serde_json::Value>(&body).ok()?;
    let allow = value
        .get("allow")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    (!allow).then(|| RuntimeGatewayGuardrailWebhookBlock {
        reason: value
            .get("reason")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("webhook_denied")
            .to_string(),
    })
}

fn runtime_gateway_guardrail_webhook_payload(
    phase: &str,
    request_id: u64,
    body: &[u8],
) -> serde_json::Value {
    serde_json::json!({
        "phase": phase,
        "request_id": format!("prodex-{}", RequestId::new()),
        "legacy_request_sequence": request_id,
        "call_id": format!("prodex-{}", CallId::new()),
        "body_base64": base64::engine::general_purpose::STANDARD.encode(body),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guardrail_webhook_payload_call_id_uses_uuidv7() {
        let payload = runtime_gateway_guardrail_webhook_payload("pre", 42, b"hello");
        let call_id = payload["call_id"]
            .as_str()
            .and_then(|id| id.strip_prefix("prodex-"))
            .expect("guardrail webhook call id should keep prodex prefix");
        let request_id = payload["request_id"]
            .as_str()
            .and_then(|id| id.strip_prefix("prodex-"))
            .expect("guardrail webhook request id should keep prodex prefix");

        assert_eq!(payload["legacy_request_sequence"], 42);
        assert_eq!(payload["body_base64"], "aGVsbG8=");
        assert_eq!(
            request_id
                .parse::<prodex_domain::RequestId>()
                .unwrap()
                .as_uuid()
                .get_version_num(),
            7
        );
        assert_eq!(
            call_id
                .parse::<prodex_domain::CallId>()
                .unwrap()
                .as_uuid()
                .get_version_num(),
            7
        );
        assert_ne!(payload["call_id"].as_str(), Some("prodex-42"));
    }
}
