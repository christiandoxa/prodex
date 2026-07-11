use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_transport::runtime_gateway_with_outbound_secret;
use super::*;
use base64::Engine;
use prodex_domain::{CallId, RequestId};
use std::time::Duration;

const RUNTIME_GATEWAY_GUARDRAIL_WEBHOOK_MAX_RESPONSE_BYTES: usize = 64 * 1024;
const RUNTIME_GATEWAY_GUARDRAIL_WEBHOOK_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) struct RuntimeGatewayGuardrailWebhookBlock {
    pub(super) reason: String,
}

fn runtime_gateway_audit_post_guardrail_webhook_blocked(
    shared: &RuntimeLocalRewriteProxyShared,
    reason: &str,
) {
    let default_log_dir = shared
        .runtime_shared
        .log_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let path = prodex_audit_log::audit_log_path(default_log_dir);
    let _ = prodex_audit_log::append_audit_event(
        &path,
        "gateway_data_plane",
        "response_guardrail_webhook_blocked",
        "failure",
        serde_json::json!({
            "state_backend": shared.gateway_state_store.label(),
            "details": {
                "phase": "post",
                "reason": reason,
            },
        }),
    );
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

fn runtime_gateway_guardrail_webhook_failure(
    shared: &RuntimeLocalRewriteProxyShared,
    phase: &str,
    request_id: u64,
    error_kind: &str,
) -> Option<RuntimeGatewayGuardrailWebhookBlock> {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "gateway_guardrail_webhook_failed",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("phase", phase),
                runtime_proxy_log_field("endpoint", "redacted"),
                runtime_proxy_log_field("error_kind", error_kind),
            ],
        ),
    );
    runtime_gateway_guardrail_webhook_failure_block(shared.gateway_guardrail_webhook.fail_closed)
}

fn runtime_gateway_guardrail_webhook_failure_block(
    fail_closed: bool,
) -> Option<RuntimeGatewayGuardrailWebhookBlock> {
    fail_closed.then(|| RuntimeGatewayGuardrailWebhookBlock {
        reason: "webhook_error".to_string(),
    })
}

pub(super) fn runtime_gateway_guardrail_webhook_block(
    phase: &str,
    request_id: u64,
    body: &[u8],
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<RuntimeGatewayGuardrailWebhookBlock> {
    let block = runtime_gateway_guardrail_webhook_decision(phase, request_id, body, shared);
    if phase == "post"
        && let Some(block) = block.as_ref()
    {
        // The response layer does not retain the webhook phase, so record it here.
        runtime_gateway_audit_post_guardrail_webhook_blocked(shared, &block.reason);
    }
    block
}

fn runtime_gateway_guardrail_webhook_decision(
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
    let mut request = shared
        .client
        .post(url)
        .timeout(RUNTIME_GATEWAY_GUARDRAIL_WEBHOOK_TIMEOUT)
        .json(&payload);
    if let Some(secret) = shared.gateway_guardrail_webhook.bearer_token.as_ref() {
        request = match runtime_gateway_with_outbound_secret(secret, |token| {
            Ok(request.bearer_auth(token))
        }) {
            Ok(request) => request,
            Err(_) => {
                return runtime_gateway_guardrail_webhook_failure(
                    shared,
                    phase,
                    request_id,
                    "credential_resolution",
                );
            }
        };
    }
    let response = match request.send() {
        Ok(response) => response,
        Err(err) => {
            let err = err.without_url();
            return runtime_gateway_guardrail_webhook_failure(
                shared,
                phase,
                request_id,
                runtime_gateway_guardrail_webhook_error_kind(&err),
            );
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
            let _ = err;
            return shared.gateway_guardrail_webhook.fail_closed.then(|| {
                RuntimeGatewayGuardrailWebhookBlock {
                    reason: "webhook_body".to_string(),
                }
            });
        }
    };
    if !status.is_success() {
        return shared.gateway_guardrail_webhook.fail_closed.then(|| {
            RuntimeGatewayGuardrailWebhookBlock {
                reason: "webhook_status".to_string(),
            }
        });
    }
    let value = match serde_json::from_slice::<serde_json::Value>(&body) {
        Ok(value) => value,
        Err(err) => {
            let _ = err;
            return shared.gateway_guardrail_webhook.fail_closed.then(|| {
                RuntimeGatewayGuardrailWebhookBlock {
                    reason: "webhook_response".to_string(),
                }
            });
        }
    };
    let Some(allow) = value.get("allow").and_then(serde_json::Value::as_bool) else {
        return shared.gateway_guardrail_webhook.fail_closed.then(|| {
            RuntimeGatewayGuardrailWebhookBlock {
                reason: "webhook_response".to_string(),
            }
        });
    };
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

    #[test]
    fn guardrail_webhook_failure_respects_fail_closed() {
        assert!(runtime_gateway_guardrail_webhook_failure_block(false).is_none());
        assert_eq!(
            runtime_gateway_guardrail_webhook_failure_block(true)
                .unwrap()
                .reason,
            "webhook_error"
        );
    }
}
