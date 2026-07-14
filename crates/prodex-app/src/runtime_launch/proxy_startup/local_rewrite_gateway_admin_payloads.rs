use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_openapi::runtime_gateway_openapi_spec_for_mount;
use prodex_provider_core::provider_contract_catalog;

pub(super) fn runtime_gateway_openapi_spec(
    shared: &RuntimeLocalRewriteProxyShared,
) -> serde_json::Value {
    runtime_gateway_openapi_spec_for_mount(&shared.mount_path)
}

pub(super) fn runtime_gateway_admin_observability_payload(
    shared: &RuntimeLocalRewriteProxyShared,
) -> serde_json::Value {
    serde_json::json!({
        "object": "gateway.observability",
        "call_id_header": shared.gateway_call_id_header,
        "sinks": shared.gateway_observability.sinks,
        "jsonl_path": shared.gateway_observability.jsonl_path.as_ref().map(|path| path.display().to_string()),
        "http_endpoint": shared.gateway_observability.http_endpoint,
        "http_schema": shared.gateway_observability.http_schema,
        "http_bearer_token_configured": shared.gateway_observability.http_bearer_token.is_some(),
    })
}

pub(super) fn runtime_gateway_admin_providers_payload(
    shared: &RuntimeLocalRewriteProxyShared,
) -> serde_json::Value {
    let catalog = provider_contract_catalog(shared.resolved_harness.effective);
    let mut payload = serde_json::to_value(catalog).expect("provider contract should serialize");
    payload
        .as_object_mut()
        .expect("provider contract should be an object")
        .insert("object".into(), "gateway.providers".into());
    payload
}

pub(super) fn runtime_gateway_admin_guardrails_payload(
    shared: &RuntimeLocalRewriteProxyShared,
) -> serde_json::Value {
    serde_json::json!({
        "object": "gateway.guardrails",
        "blocked_keywords_count": shared.gateway_guardrails.blocked_keywords.len(),
        "blocked_output_keywords_count": shared.gateway_guardrails.blocked_output_keywords.len(),
        "allowed_models": shared.gateway_guardrails.allowed_models,
        "prompt_injection_detection": shared.gateway_guardrails.prompt_injection_detection,
        "pii_redaction": shared.gateway_guardrails.pii_redaction,
        "webhook": {
            "configured": shared.gateway_guardrail_webhook.url.is_some(),
            "phases": shared.gateway_guardrail_webhook.phases,
            "bearer_token_configured": shared.gateway_guardrail_webhook.bearer_token.is_some(),
            "fail_closed": shared.gateway_guardrail_webhook.fail_closed,
        },
    })
}
