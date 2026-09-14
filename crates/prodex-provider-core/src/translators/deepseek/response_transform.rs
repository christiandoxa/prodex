//! DeepSeek buffered response translation.

use super::{deepseek_passthrough_endpoint, response::deepseek_responses_value_from_chat_value};
use crate::translator::{ProviderTransformInput, ProviderTransformResult};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};
use serde_json::Value;

pub(super) fn deepseek_transform_response(
    provider: ProviderId,
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    if deepseek_passthrough_endpoint(input.endpoint) {
        return ProviderTransformResult::lossless(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
            input.body,
        );
    }
    if input.endpoint != ProviderEndpoint::Responses {
        return ProviderTransformResult::unsupported(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiChatCompletions,
            ProviderWireFormat::OpenAiResponses,
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
                ProviderWireFormat::OpenAiChatCompletions,
                ProviderWireFormat::OpenAiResponses,
                format!("failed to parse DeepSeek response JSON: {error}"),
            );
        }
    };
    let response = deepseek_responses_value_from_chat_value(&value);
    ProviderTransformResult::lossless(
        provider,
        input.endpoint,
        ProviderWireFormat::OpenAiChatCompletions,
        ProviderWireFormat::OpenAiResponses,
        serde_json::to_vec(&response).expect("deepseek response serializes"),
    )
}

#[cfg(test)]
mod tests {
    use super::deepseek_transform_response;
    use crate::translator::{ProviderTransformInput, ProviderTransformLoss};
    use crate::{ProviderEndpoint, ProviderId};
    use serde_json::json;

    #[test]
    fn response_transform_matches_oracle_with_tool_and_response_metadata() {
        let result = deepseek_transform_response(
            ProviderId::DeepSeek,
            ProviderTransformInput::new(
                ProviderEndpoint::Responses,
                serde_json::to_vec(&json!({
                    "id": "chatcmpl_cache_1",
                    "model": "deepseek-chat",
                    "created": 1700000000,
                    "choices": [{
                        "message": {
                            "role": "assistant",
                            "content": "cached hello",
                            "reasoning_content": "think",
                            "tool_calls": [{
                                "id": "call_1",
                                "function": {
                                    "name": "functions.exec_command",
                                    "arguments": "{\"cmd\":\"echo hi\"}"
                                },
                                "extra_content": {
                                    "google": {"thought_signature": "sig_1"}
                                }
                            }]
                        },
                        "finish_reason": "tool_calls",
                        "logprobs": {"tokens": []}
                    }],
                    "system_fingerprint": "fp_1",
                    "usage": {
                        "prompt_tokens": 11,
                        "completion_tokens": 7,
                        "total_tokens": 18,
                        "prompt_cache_hit_tokens": 5,
                        "prompt_cache_miss_tokens": 6,
                        "completion_tokens_details": {"reasoning_tokens": 2}
                    }
                }))
                .expect("response fixture serializes"),
            ),
        );

        assert!(matches!(result.loss, ProviderTransformLoss::Lossless));
        let body: serde_json::Value =
            serde_json::from_slice(result.body.as_ref().expect("response body")).unwrap();
        assert_eq!(body["created_at"], 1700000000);
        assert_eq!(body["output"][0]["content"][0]["text"], "cached hello");
        assert_eq!(body["output"][1]["namespace"], "functions");
        assert_eq!(body["output"][1]["name"], "exec_command");
        assert_eq!(body["output"][1]["arguments"], "{\"cmd\":\"rtk echo hi\"}");
        assert_eq!(body["output"][1]["gemini_thought_signature"], "sig_1");
        assert_eq!(body["usage"]["input_tokens"], 11);
        assert_eq!(
            body["usage"]["output_tokens_details"]["reasoning_tokens"],
            2
        );
        assert_eq!(body["metadata"]["deepseek"]["reasoning_content"], "think");
        assert_eq!(body["metadata"]["deepseek"]["finish_reason"], "tool_calls");
    }

    #[test]
    fn response_transform_keeps_invalid_json_rejection() {
        let result = deepseek_transform_response(
            ProviderId::DeepSeek,
            ProviderTransformInput::new(ProviderEndpoint::Responses, b"{bad".to_vec()),
        );

        assert!(matches!(
            result.loss,
            ProviderTransformLoss::Rejected { .. }
        ));
        assert!(result.body.is_none());
    }
}
