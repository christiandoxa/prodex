//! DeepSeek request translation.

use super::{deepseek_passthrough_endpoint, request::deepseek_request_body_from_responses};
use crate::translator::{ProviderTransformInput, ProviderTransformResult};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};
use serde_json::Value;

pub(super) fn deepseek_transform_request(
    provider: ProviderId,
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    if deepseek_passthrough_endpoint(input.endpoint) {
        return ProviderTransformResult::lossless(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            input.body,
        );
    }
    if !matches!(
        input.endpoint,
        ProviderEndpoint::Responses | ProviderEndpoint::ResponsesCompact
    ) {
        return ProviderTransformResult::unsupported(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            format!(
                "DeepSeek translator does not support {}",
                input.endpoint.label()
            ),
        );
    }
    let value: Value = match serde_json::from_slice(&input.body) {
        Ok(value) => value,
        Err(error) => {
            return ProviderTransformResult::rejected(
                provider,
                input.endpoint,
                ProviderWireFormat::OpenAiResponses,
                ProviderWireFormat::OpenAiChatCompletions,
                format!("failed to parse Responses request JSON: {error}"),
            );
        }
    };
    let Some(obj) = value.as_object() else {
        return ProviderTransformResult::rejected(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            "DeepSeek request body must be a JSON object",
        );
    };
    if matches!(
        obj.get("parallel_tool_calls").and_then(Value::as_bool),
        Some(false)
    ) {
        return ProviderTransformResult::rejected(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            "DeepSeek does not expose a compatible parallel_tool_calls=false control",
        );
    }
    let (body, degraded) = match deepseek_request_body_from_responses(obj, &value) {
        Ok(result) => result,
        Err(reason) => {
            return ProviderTransformResult::rejected(
                provider,
                input.endpoint,
                ProviderWireFormat::OpenAiResponses,
                ProviderWireFormat::OpenAiChatCompletions,
                reason,
            );
        }
    };
    let result = if let Some(details) = degraded {
        ProviderTransformResult::degraded(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            body,
            "DeepSeek degrades JSON schema output to json_object",
            details,
        )
    } else {
        ProviderTransformResult::lossless(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            body,
        )
    };
    let mut metadata = serde_json::Map::new();
    for header in ["x-codex-turn-state", "session_id"] {
        if let Some(value) = input.headers.get(header) {
            metadata.insert(header.to_string(), Value::String(value.clone()));
        }
    }
    if let Some(previous) = obj.get("previous_response_id").and_then(Value::as_str) {
        metadata.insert(
            "previous_response_id".to_string(),
            Value::String(previous.to_string()),
        );
    }
    if metadata.is_empty() {
        result
    } else {
        result.with_metadata("continuation", Value::Object(metadata))
    }
}

#[cfg(test)]
mod tests {
    use super::deepseek_transform_request;
    use crate::translator::{ProviderTransformInput, ProviderTransformLoss};
    use crate::{ProviderEndpoint, ProviderId};
    use serde_json::json;

    #[test]
    fn request_transform_matches_oracle_and_preserves_boundary_metadata() {
        let mut input = ProviderTransformInput::new(
            ProviderEndpoint::Responses,
            serde_json::to_vec(&json!({
                "model": "deepseek-chat",
                "input": "call search then stop",
                "tools": [
                    {
                        "type": "function",
                        "function": {
                            "name": "search",
                            "description": "Search docs",
                            "parameters": {
                                "type": "object",
                                "properties": {"query": {"type": "string"}}
                            }
                        }
                    },
                    {"type": "web_search_preview"}
                ],
                "tool_choice": {
                    "type": "function",
                    "function": {"name": "search"}
                },
                "temperature": 0.2,
                "top_p": 0.9,
                "max_output_tokens": 128,
                "logprobs": true,
                "top_logprobs": 5,
                "stop_sequences": ["END"],
                "user": "user_123",
                "response_format": {
                    "type": "json_schema",
                    "schema": {"type": "object"},
                },
                "previous_response_id": "resp_1"
            }))
            .expect("request fixture serializes"),
        );
        input
            .headers
            .insert("session_id".to_string(), "sess_1".to_string());
        input
            .headers
            .insert("x-codex-turn-state".to_string(), "turn_1".to_string());

        let result = deepseek_transform_request(ProviderId::DeepSeek, input);
        match &result.loss {
            ProviderTransformLoss::DegradedButSafe { reason, details } => {
                assert_eq!(
                    reason,
                    "DeepSeek degrades JSON schema output to json_object"
                );
                assert_eq!(details["from"], "json_schema");
                assert_eq!(details["to"], "json_object");
            }
            loss => panic!("expected degraded request, got {loss:?}"),
        }
        assert_eq!(
            result.metadata["continuation"],
            json!({
                "previous_response_id": "resp_1",
                "session_id": "sess_1",
                "x-codex-turn-state": "turn_1"
            })
        );
        let body: serde_json::Value =
            serde_json::from_slice(result.body.as_ref().expect("request body")).unwrap();
        assert_eq!(
            body,
            json!({
                "model": "deepseek-chat",
                "stream": false,
                "messages": [{"role": "user", "content": "call search then stop"}],
                "tools": [{
                    "type": "function",
                    "function": {
                        "name": "search",
                        "description": "Search docs",
                        "parameters": {
                            "type": "object",
                            "properties": {"query": {"type": "string"}}
                        }
                    }
                }],
                "tool_choice": {
                    "type": "function",
                    "function": {"name": "search"}
                },
                "temperature": 0.2,
                "top_p": 0.9,
                "max_tokens": 128,
                "logprobs": true,
                "top_logprobs": 5,
                "stop": ["END"],
                "user_id": "user_123",
                "response_format": {"type": "json_object"}
            })
        );
    }

    #[test]
    fn request_transform_keeps_parallel_tool_call_rejection() {
        let result = deepseek_transform_request(
            ProviderId::DeepSeek,
            ProviderTransformInput::new(
                ProviderEndpoint::Responses,
                br#"{"input":"hello","parallel_tool_calls":false}"#.to_vec(),
            ),
        );

        assert!(matches!(
            result.loss,
            ProviderTransformLoss::Rejected { .. }
        ));
        assert!(result.body.is_none());
    }

    #[test]
    fn request_transform_rejects_invalid_parameters_with_stable_reasons() {
        for (request, expected) in [
            (
                json!({"input": "hello", "temperature": "warm"}),
                "DeepSeek temperature must be a number",
            ),
            (
                json!({"input": "hello", "max_tokens": 0}),
                "DeepSeek max_tokens must be a positive integer",
            ),
            (
                json!({"input": "hello", "top_logprobs": 21, "logprobs": true}),
                "DeepSeek top_logprobs must be <= 20",
            ),
            (
                json!({"input": "hello", "stop": ["END", 1]}),
                "DeepSeek stop sequences must be strings",
            ),
            (
                json!({"input": "hello", "user_id": "bad!"}),
                "DeepSeek user_id must use only letters, numbers, underscores, or dashes and be at most 512 bytes",
            ),
        ] {
            let result = deepseek_transform_request(
                ProviderId::DeepSeek,
                ProviderTransformInput::new(
                    ProviderEndpoint::Responses,
                    serde_json::to_vec(&request).expect("request serializes"),
                ),
            );
            let ProviderTransformLoss::Rejected { reason } = result.loss else {
                panic!("request should be rejected: {request}");
            };
            assert_eq!(reason, expected, "request: {request}");
        }
    }
}
