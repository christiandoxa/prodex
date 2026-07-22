use super::super::anthropic_messages_sse_reader::RuntimeAnthropicMessagesSseReader;
use super::super::deepseek_rewrite::{
    RuntimeDeepSeekPendingRequest, runtime_deepseek_merge_response_metadata,
    runtime_deepseek_store_conversation,
};
use super::super::local_rewrite::{RUNTIME_LOCAL_REWRITE_PROFILE, RuntimeLocalRewriteProxyShared};
use super::super::local_rewrite_rate_limits::{
    append_binary_rate_limit_headers, append_text_rate_limit_headers,
    runtime_provider_codex_rate_limit_headers,
};
use super::super::local_rewrite_request::RuntimeLocalRewriteRequest;
use super::super::local_rewrite_response_spend::{
    emit_runtime_gateway_response_spend_event_for_body, runtime_gateway_spend_stream_body,
};
use super::super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_log_response_conformance,
};
use super::{
    RuntimeGatewayResponseGovernance, respond_runtime_local_rewrite_stream,
    runtime_local_rewrite_append_call_id_header,
    runtime_local_rewrite_buffered_response_from_response,
    runtime_local_rewrite_governed_response_with_call_id, runtime_local_rewrite_invalid_response,
};
use crate::{RuntimeProxyRequest, RuntimeStreamingResponse};
use prodex_provider_core::{
    ProviderEndpoint, ProviderTransformInput, anthropic_messages_translator,
    provider_core_rewritten_json_value,
};
use serde_json::{Value, json};
use std::io::Read;
use std::time::Instant;

pub(super) struct RuntimeAnthropicMessagesRewriteContext<'a> {
    pub(super) status: u16,
    pub(super) content_type: &'a str,
    pub(super) shared: &'a RuntimeLocalRewriteProxyShared,
    pub(super) captured: &'a RuntimeProxyRequest,
    pub(super) provider_kind: RuntimeProviderBridgeKind,
    pub(super) pending_request: RuntimeDeepSeekPendingRequest,
    pub(super) response_governance: RuntimeGatewayResponseGovernance,
}

pub(super) fn respond_runtime_anthropic_messages_rewrite(
    request_id: u64,
    request: RuntimeLocalRewriteRequest,
    response: reqwest::blocking::Response,
    context: RuntimeAnthropicMessagesRewriteContext<'_>,
) {
    let RuntimeAnthropicMessagesRewriteContext {
        status,
        content_type,
        shared,
        captured,
        provider_kind,
        pending_request: pending,
        response_governance,
    } = context;
    let conversations = shared.deepseek_conversations_for_request(captured);
    let rate_limit_headers =
        runtime_provider_codex_rate_limit_headers(provider_kind, response.headers());
    if content_type.contains("text/event-stream") {
        let mut headers = vec![(
            "content-type".to_string(),
            "text/event-stream; charset=utf-8".to_string(),
        )];
        append_text_rate_limit_headers(&mut headers, rate_limit_headers);
        runtime_local_rewrite_append_call_id_header(&mut headers, request_id, shared);
        let body: Box<dyn Read + Send> =
            Box::new(RuntimeAnthropicMessagesSseReader::new_with_provider(
                response,
                provider_kind,
                request_id,
                pending.messages,
                pending.response_metadata,
                conversations.clone(),
            ));
        let body = runtime_gateway_spend_stream_body(
            body,
            request_id,
            status,
            captured,
            shared,
            response_governance.spend_termination.clone(),
        );
        respond_runtime_local_rewrite_stream(
            request,
            RuntimeStreamingResponse {
                status,
                headers,
                body,
                request_id,
                profile_name: RUNTIME_LOCAL_REWRITE_PROFILE.to_string(),
                log_path: shared.runtime_shared.log_path.clone(),
                shared: shared.runtime_shared.clone(),
                _inflight_guard: None,
            },
            shared,
            response_governance,
        );
        return;
    }

    let response_started_at = Instant::now();
    let translated = (|| -> anyhow::Result<_> {
        let mut parts = runtime_local_rewrite_buffered_response_from_response(response)?;
        let native: Value = serde_json::from_slice(&parts.body)?;
        let mut input =
            ProviderTransformInput::new(ProviderEndpoint::Responses, parts.body.to_vec());
        input.status = Some(status);
        let result = anthropic_messages_translator().transform_response(input);
        runtime_provider_log_response_conformance(
            &shared.runtime_shared,
            request_id,
            provider_kind,
            &result,
        );
        let mut value = provider_core_rewritten_json_value(Some(&result))
            .ok_or_else(|| anyhow::anyhow!("native Anthropic response translation failed"))?;
        runtime_deepseek_merge_response_metadata(&mut value, pending.response_metadata);
        if let Some(response_id) = value.get("id").and_then(Value::as_str) {
            runtime_deepseek_store_conversation(
                &conversations,
                response_id,
                pending.messages,
                anthropic_chat_assistant_messages(&native),
            );
        }
        parts.body = serde_json::to_vec(&value)?.into();
        parts.headers = vec![(
            "content-type".to_string(),
            b"application/json; charset=utf-8".to_vec(),
        )];
        append_binary_rate_limit_headers(&mut parts.headers, rate_limit_headers);
        Ok(parts)
    })();

    let response = translated
        .map(|parts| {
            emit_runtime_gateway_response_spend_event_for_body(
                request_id,
                captured,
                shared,
                parts.status,
                response_started_at.elapsed().as_millis(),
                parts.body.as_slice(),
            );
            runtime_local_rewrite_governed_response_with_call_id(
                parts,
                request_id,
                shared,
                response_governance,
            )
        })
        .unwrap_or_else(|error| runtime_local_rewrite_invalid_response(request_id, shared, &error));
    let _ = request.respond(response);
}

fn anthropic_chat_assistant_messages(value: &Value) -> Vec<Value> {
    let Some(content) = value.get("content").and_then(Value::as_array) else {
        return Vec::new();
    };
    let text = content
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("");
    let tool_calls = content
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
        .filter_map(|block| {
            Some(json!({
                "id": block.get("id")?.as_str()?,
                "type": "function",
                "function": {
                    "name": block.get("name")?.as_str()?,
                    "arguments": serde_json::to_string(block.get("input").unwrap_or(&json!({}))).ok()?,
                }
            }))
        })
        .collect::<Vec<_>>();
    if text.is_empty() && tool_calls.is_empty() {
        return Vec::new();
    }
    vec![json!({
        "role": "assistant",
        "content": text,
        "tool_calls": tool_calls,
    })]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_anthropic_response_history_preserves_tool_calls() {
        let messages = anthropic_chat_assistant_messages(&json!({
            "content": [
                {"type": "text", "text": "checking"},
                {"type": "tool_use", "id": "call_test", "name": "read_file", "input": {"path": "/tmp/test"}}
            ]
        }));
        assert_eq!(messages[0]["content"], "checking");
        assert_eq!(messages[0]["tool_calls"][0]["id"], "call_test");
        assert_eq!(
            messages[0]["tool_calls"][0]["function"]["arguments"],
            r#"{"path":"/tmp/test"}"#
        );
    }
}
