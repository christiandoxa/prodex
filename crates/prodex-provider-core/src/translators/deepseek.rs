use crate::translator::{
    ProviderParamSupport, ProviderTransformInput, ProviderTransformResult, ProviderTranslator,
};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};

#[path = "deepseek/request.rs"]
mod request;
#[path = "deepseek/request_transform.rs"]
mod request_transform;
#[path = "deepseek/response.rs"]
mod response;
#[path = "deepseek/response_transform.rs"]
mod response_transform;
#[path = "deepseek/stream.rs"]
mod stream;
#[path = "deepseek/supported_params.rs"]
mod supported_params;
#[path = "deepseek/tooling.rs"]
mod tooling;
use self::response::deepseek_stream_event_from_chat_value;
pub use self::stream::{
    DeepSeekProviderCoreStreamChatToolCall, DeepSeekProviderCoreStreamChoiceDelta,
    DeepSeekProviderCoreStreamChoiceMetadata, DeepSeekProviderCoreStreamChunkMetadata,
    DeepSeekProviderCoreStreamToolCallDelta, deepseek_provider_core_chat_stream_error,
    deepseek_provider_core_function_call_arguments_delta_event,
    deepseek_provider_core_output_item_added_event, deepseek_provider_core_output_item_done_event,
    deepseek_provider_core_output_text_delta_event,
    deepseek_provider_core_response_completed_event, deepseek_provider_core_response_created_event,
    deepseek_provider_core_stream_chat_assistant_message,
    deepseek_provider_core_stream_choice_delta, deepseek_provider_core_stream_choice_metadata,
    deepseek_provider_core_stream_chunk_metadata,
    deepseek_provider_core_stream_fallback_response_id,
    deepseek_provider_core_stream_fallback_tool_call_id,
    deepseek_provider_core_stream_first_choice,
    deepseek_provider_core_stream_function_call_arguments_delta_source,
    deepseek_provider_core_stream_output_text_item,
    deepseek_provider_core_stream_output_text_item_id,
    deepseek_provider_core_stream_response_id_from_chunk,
    deepseek_provider_core_stream_response_metadata, deepseek_provider_core_stream_response_value,
    deepseek_provider_core_stream_text_delta_source,
    deepseek_provider_core_stream_tool_call_added_item,
    deepseek_provider_core_stream_tool_call_delta, deepseek_provider_core_stream_tool_call_item,
    deepseek_provider_core_validate_stream_tool_call_arguments,
    deepseek_provider_core_validate_stream_tool_call_delta,
};
pub(crate) use self::tooling::{
    deepseek_rtk_wrapped_tool_arguments, deepseek_tool_call_thought_signature_object,
};

#[derive(Clone, Copy)]
pub struct DeepSeekTranslator;

impl ProviderTranslator for DeepSeekTranslator {
    fn provider(&self) -> ProviderId {
        ProviderId::DeepSeek
    }

    fn client_wire_format(&self) -> ProviderWireFormat {
        ProviderWireFormat::OpenAiResponses
    }

    fn upstream_wire_format(&self) -> ProviderWireFormat {
        ProviderWireFormat::OpenAiChatCompletions
    }

    fn supported_params(&self, endpoint: ProviderEndpoint, _model: &str) -> ProviderParamSupport {
        supported_params::deepseek_supported_params(endpoint)
    }

    fn transform_request(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        request_transform::deepseek_transform_request(self.provider(), input)
    }

    fn transform_response(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        response_transform::deepseek_transform_response(self.provider(), input)
    }

    fn transform_stream_event(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        stream::deepseek_transform_stream_event(self.provider(), input)
    }
}

fn deepseek_passthrough_endpoint(endpoint: ProviderEndpoint) -> bool {
    matches!(
        endpoint,
        ProviderEndpoint::ChatCompletions | ProviderEndpoint::Messages
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deepseek_provider_core_shapes_response_completed_event() {
        assert_eq!(
            deepseek_provider_core_response_completed_event(
                3,
                123,
                &serde_json::json!({"id": "resp_1"}),
            ),
            serde_json::json!({
                "type": "response.completed",
                "sequence_number": 3,
                "created_at": 123,
                "response": {"id": "resp_1"},
            })
        );
    }

    #[test]
    fn deepseek_provider_core_shapes_response_created_event() {
        assert_eq!(
            deepseek_provider_core_response_created_event(2, 123, "resp_1"),
            serde_json::json!({
                "type": "response.created",
                "sequence_number": 2,
                "created_at": 123,
                "response": {"id": "resp_1"},
            })
        );
    }

    #[test]
    fn deepseek_provider_core_classifies_chat_stream_errors() {
        assert_eq!(
            deepseek_provider_core_chat_stream_error(&serde_json::json!({"error": "boom"})),
            Some(("provider_stream_error".to_string(), "boom".to_string()))
        );
        assert_eq!(
            deepseek_provider_core_chat_stream_error(&serde_json::json!({
                "error": {"code": 500, "detail": "bad"}
            })),
            Some(("500".to_string(), "bad".to_string()))
        );
        assert_eq!(
            deepseek_provider_core_chat_stream_error(&serde_json::json!({"error": {}})),
            Some((
                "provider_stream_error".to_string(),
                "Provider stream returned an embedded error".to_string(),
            ))
        );
        assert_eq!(
            deepseek_provider_core_chat_stream_error(&serde_json::json!({"ok": true})),
            None
        );
    }

    #[test]
    fn deepseek_provider_core_validates_stream_tool_call_arguments() {
        assert_eq!(
            deepseek_provider_core_validate_stream_tool_call_arguments(
                "DeepSeek",
                2,
                Some(""),
                "{}",
            ),
            Err("DeepSeek streamed a tool call without a function name at index 2".to_string())
        );
        assert_eq!(
            deepseek_provider_core_validate_stream_tool_call_arguments(
                "DeepSeek",
                3,
                Some("shell"),
                "",
            ),
            Ok(())
        );
        let error = deepseek_provider_core_validate_stream_tool_call_arguments(
            "DeepSeek",
            4,
            Some("shell"),
            "{bad",
        )
        .expect_err("malformed streamed arguments should be rejected");
        assert!(error.contains(
            "DeepSeek streamed malformed JSON arguments for tool call `shell` at index 4"
        ));
    }

    #[test]
    fn deepseek_provider_core_validates_stream_tool_call_delta_shape() {
        assert_eq!(
            deepseek_provider_core_validate_stream_tool_call_delta(
                "DeepSeek",
                false,
                &serde_json::json!({"id": "call_1"})
            ),
            Err("DeepSeek streamed a tool call without a function object".to_string())
        );
        assert_eq!(
            deepseek_provider_core_validate_stream_tool_call_delta(
                "DeepSeek",
                true,
                &serde_json::json!({"id": "call_1"})
            ),
            Ok(())
        );
        assert_eq!(
            deepseek_provider_core_validate_stream_tool_call_delta(
                "DeepSeek",
                false,
                &serde_json::json!({"function": {"name": "shell"}})
            ),
            Ok(())
        );
    }

    #[test]
    fn deepseek_provider_core_extracts_stream_tool_call_delta_fields() {
        assert_eq!(
            deepseek_provider_core_stream_tool_call_delta(&serde_json::json!({
                "index": 7,
                "id": "call_7",
                "function": {
                    "name": "shell",
                    "arguments": "{\"cmd\":\"echo hi\"}",
                },
                "extra_content": {
                    "google": {
                        "thought_signature": "sig_7",
                    }
                }
            })),
            DeepSeekProviderCoreStreamToolCallDelta {
                index: 7,
                call_id: Some("call_7".to_string()),
                name: Some("shell".to_string()),
                argument_delta: Some("{\"cmd\":\"echo hi\"}".to_string()),
                thought_signature: Some("sig_7".to_string()),
            }
        );
        assert_eq!(
            deepseek_provider_core_stream_tool_call_delta(&serde_json::json!({
                "index": "bad",
                "function": {
                    "arguments": "",
                },
            })),
            DeepSeekProviderCoreStreamToolCallDelta::default()
        );
    }

    #[test]
    fn deepseek_provider_core_shapes_stream_fallback_tool_call_id() {
        assert_eq!(
            deepseek_provider_core_stream_fallback_tool_call_id("deepseek", 7, 2),
            "call_deepseek_7_2"
        );
    }

    #[test]
    fn deepseek_provider_core_shapes_stream_output_text_item_id() {
        assert_eq!(
            deepseek_provider_core_stream_output_text_item_id("deepseek", 7),
            "msg_deepseek_7"
        );
    }

    #[test]
    fn deepseek_provider_core_shapes_stream_response_ids() {
        assert_eq!(
            deepseek_provider_core_stream_fallback_response_id("deepseek", 7),
            "resp_deepseek_7"
        );
        assert_eq!(
            deepseek_provider_core_stream_response_id_from_chunk(
                "deepseek",
                "resp_deepseek_7",
                &serde_json::json!({"id": "chatcmpl_1"}),
            ),
            Some("chatcmpl_1".to_string())
        );
        assert_eq!(
            deepseek_provider_core_stream_response_id_from_chunk(
                "deepseek",
                "chatcmpl_1",
                &serde_json::json!({"id": "chatcmpl_2"}),
            ),
            None
        );
        assert_eq!(
            deepseek_provider_core_stream_response_id_from_chunk(
                "deepseek",
                "resp_deepseek_7",
                &serde_json::json!({"choices": []}),
            ),
            None
        );
    }

    #[test]
    fn deepseek_provider_core_extracts_stream_chunk_metadata() {
        assert_eq!(
            deepseek_provider_core_stream_chunk_metadata(
                &serde_json::json!({
                    "model": "deepseek-v4-pro",
                    "created": 1782740700_u64,
                    "system_fingerprint": "fp_deepseek_stream",
                    "usage": {
                        "prompt_tokens": 11,
                        "completion_tokens": 5,
                        "total_tokens": 16,
                        "prompt_cache_hit_tokens": 3,
                        "prompt_cache_miss_tokens": 8,
                        "completion_tokens_details": {
                            "reasoning_tokens": 2,
                        }
                    },
                }),
                "deepseek",
            ),
            DeepSeekProviderCoreStreamChunkMetadata {
                model: Some("deepseek-v4-pro".to_string()),
                created_at: Some(1782740700),
                system_fingerprint: Some("fp_deepseek_stream".to_string()),
                usage: Some(serde_json::json!({
                    "input_tokens": 11,
                    "output_tokens": 5,
                    "total_tokens": 16,
                    "input_tokens_details": {
                        "cached_tokens": 3,
                    },
                    "output_tokens_details": {
                        "reasoning_tokens": 2,
                    },
                    "metadata": {
                        "deepseek": {
                            "prompt_cache_hit_tokens": 3,
                            "prompt_cache_miss_tokens": 8,
                        }
                    },
                })),
            }
        );
        assert_eq!(
            deepseek_provider_core_stream_chunk_metadata(
                &serde_json::json!({"system_fingerprint": ""}),
                "deepseek",
            ),
            DeepSeekProviderCoreStreamChunkMetadata::default()
        );
    }

    #[test]
    fn deepseek_provider_core_extracts_stream_choice_metadata() {
        let chunk = serde_json::json!({
            "choices": [{
                "delta": {},
                "finish_reason": "stop",
                "logprobs": {"content": [{"token": "done"}]},
            }]
        });
        let choice = deepseek_provider_core_stream_first_choice(&chunk)
            .expect("first streamed choice should be extracted");
        assert_eq!(
            deepseek_provider_core_stream_choice_metadata(choice),
            DeepSeekProviderCoreStreamChoiceMetadata {
                logprobs: Some(serde_json::json!({"content": [{"token": "done"}]})),
                finish_reason: Some("stop".to_string()),
            }
        );
        assert_eq!(
            deepseek_provider_core_stream_first_choice(&serde_json::json!({"choices": []})),
            None
        );
        assert_eq!(
            deepseek_provider_core_stream_choice_metadata(&serde_json::json!({"logprobs": null})),
            DeepSeekProviderCoreStreamChoiceMetadata::default()
        );
    }

    #[test]
    fn deepseek_provider_core_extracts_stream_choice_delta() {
        assert_eq!(
            deepseek_provider_core_stream_choice_delta(&serde_json::json!({
                "delta": {
                    "reasoning_content": "think",
                    "refusal": "no",
                    "annotations": [{"type": "url_citation"}],
                    "content": "done",
                    "tool_calls": [{
                        "index": 0,
                        "function": {"name": "shell"},
                    }],
                }
            })),
            DeepSeekProviderCoreStreamChoiceDelta {
                reasoning_content: Some("think".to_string()),
                refusal: Some("no".to_string()),
                annotations: vec![serde_json::json!({"type": "url_citation"})],
                content: Some("done".to_string()),
                tool_calls: vec![serde_json::json!({
                    "index": 0,
                    "function": {"name": "shell"},
                })],
            }
        );
        assert_eq!(
            deepseek_provider_core_stream_choice_delta(&serde_json::json!({
                "delta": {
                    "reasoning_content": "",
                    "refusal": "",
                    "content": "",
                }
            })),
            DeepSeekProviderCoreStreamChoiceDelta::default()
        );
        assert_eq!(
            deepseek_provider_core_stream_choice_delta(&serde_json::json!({})),
            DeepSeekProviderCoreStreamChoiceDelta::default()
        );
    }

    #[test]
    fn deepseek_provider_core_shapes_stream_response_metadata() {
        assert_eq!(
            deepseek_provider_core_stream_response_metadata(
                "deepseek",
                None,
                "",
                "",
                &[],
                None,
                None
            ),
            None
        );
        assert_eq!(
            deepseek_provider_core_stream_response_metadata(
                "deepseek",
                Some(serde_json::json!({"token": 0.7})),
                "thinking",
                "nope",
                &[serde_json::json!({"url": "https://example.test"})],
                Some("stop"),
                Some("fp_1"),
            ),
            Some(serde_json::json!({
                "deepseek": {
                    "logprobs": {"token": 0.7},
                    "reasoning_content": "thinking",
                    "refusal": "nope",
                    "annotations": [{"url": "https://example.test"}],
                    "finish_reason": "stop",
                    "system_fingerprint": "fp_1",
                }
            }))
        );
    }

    #[test]
    fn deepseek_provider_core_shapes_stream_response_value() {
        assert_eq!(
            deepseek_provider_core_stream_response_value("resp_1", vec![], None, None, None, None),
            serde_json::json!({
                "id": "resp_1",
                "output": [],
            })
        );
        assert_eq!(
            deepseek_provider_core_stream_response_value(
                "resp_2",
                vec![serde_json::json!({"type": "message"})],
                Some("deepseek-chat"),
                Some(serde_json::json!({"input_tokens": 1})),
                Some(serde_json::json!({"deepseek": {"finish_reason": "stop"}})),
                Some(serde_json::json!({"deepseek": {"request": "metadata"}, "app": {"id": 1}})),
            ),
            serde_json::json!({
                "id": "resp_2",
                "output": [{"type": "message"}],
                "model": "deepseek-chat",
                "usage": {"input_tokens": 1},
                "metadata": {
                    "deepseek": {
                        "finish_reason": "stop",
                        "request": "metadata",
                    },
                    "app": {"id": 1},
                },
            })
        );
    }

    #[test]
    fn deepseek_provider_core_shapes_stream_output_text_item() {
        assert_eq!(
            deepseek_provider_core_stream_output_text_item("hello"),
            serde_json::json!({
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": "hello",
                }],
            })
        );
    }

    #[test]
    fn deepseek_provider_core_shapes_stream_chat_assistant_message() {
        assert_eq!(
            deepseek_provider_core_stream_chat_assistant_message("", "", &[]),
            None
        );
        assert_eq!(
            deepseek_provider_core_stream_chat_assistant_message("done", "thought", &[]),
            Some(serde_json::json!({
                "role": "assistant",
                "content": "done",
                "reasoning_content": "thought",
            }))
        );
        assert_eq!(
            deepseek_provider_core_stream_chat_assistant_message(
                "",
                "",
                &[DeepSeekProviderCoreStreamChatToolCall {
                    call_id: "call_1".to_string(),
                    name: "tool".to_string(),
                    arguments: "{\"x\":1}".to_string(),
                    thought_signature: Some("sig_1".to_string()),
                }],
            ),
            Some(serde_json::json!({
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "tool",
                        "arguments": "{\"x\":1}",
                    },
                    "extra_content": {
                        "google": {
                            "thought_signature": "sig_1",
                        }
                    }
                }],
            }))
        );
    }

    #[test]
    fn deepseek_provider_core_shapes_stream_tool_call_items() {
        assert_eq!(
            deepseek_provider_core_stream_tool_call_added_item(
                "call_1",
                "mcp__prodex_sqz__sqz_read_file"
            ),
            Some(serde_json::json!({
                "type": "function_call",
                "call_id": "call_1",
                "name": "sqz_read_file",
                "namespace": "mcp__prodex_sqz",
            }))
        );
        assert_eq!(
            deepseek_provider_core_stream_tool_call_added_item("call_2", "tool_search"),
            None
        );
        assert_eq!(
            deepseek_provider_core_stream_tool_call_item(
                "call_3",
                "tool_search",
                "{\"query\":\"prodex\"}",
                None,
            ),
            serde_json::json!({
                "type": "tool_search_call",
                "call_id": "call_3",
                "execution": "client",
                "arguments": {"query": "prodex"},
            })
        );
        assert_eq!(
            deepseek_provider_core_stream_tool_call_item(
                "call_4",
                "mcp__prodex_sqz__sqz_read_file",
                "{\"path\":\"README.md\"}",
                Some("sig_1"),
            ),
            serde_json::json!({
                "type": "function_call",
                "call_id": "call_4",
                "name": "sqz_read_file",
                "namespace": "mcp__prodex_sqz",
                "arguments": "{\"path\":\"README.md\"}",
                "gemini_thought_signature": "sig_1",
            })
        );
    }

    #[test]
    fn deepseek_provider_core_shapes_stream_function_call_arguments_delta_source() {
        assert_eq!(
            deepseek_provider_core_stream_function_call_arguments_delta_source(
                "call_1",
                "{\"path\":\"README.md\"}",
            ),
            serde_json::json!({
                "choices": [{
                    "delta": {
                        "tool_calls": [{
                            "id": "call_1",
                            "function": {
                                "arguments": "{\"path\":\"README.md\"}",
                            }
                        }]
                    }
                }]
            })
        );
    }

    #[test]
    fn deepseek_provider_core_shapes_stream_text_delta_values() {
        assert_eq!(
            deepseek_provider_core_stream_text_delta_source("hello"),
            serde_json::json!({
                "choices": [{
                    "delta": {
                        "content": "hello",
                    }
                }]
            })
        );
        assert_eq!(
            deepseek_provider_core_output_text_delta_event(4, 123, "resp_1", "hello"),
            serde_json::json!({
                "type": "response.output_text.delta",
                "sequence_number": 4,
                "created_at": 123,
                "response_id": "resp_1",
                "delta": "hello",
            })
        );
    }

    #[test]
    fn deepseek_provider_core_shapes_stream_tool_call_events() {
        let item = serde_json::json!({"type": "function_call", "call_id": "call_1"});
        assert_eq!(
            deepseek_provider_core_output_item_added_event(7, &item),
            serde_json::json!({
                "type": "response.output_item.added",
                "sequence_number": 7,
                "item": {"type": "function_call", "call_id": "call_1"},
            })
        );
        assert_eq!(
            deepseek_provider_core_function_call_arguments_delta_event(8, "call_1", "{\"x\":1}",),
            serde_json::json!({
                "type": "response.function_call_arguments.delta",
                "sequence_number": 8,
                "call_id": "call_1",
                "delta": "{\"x\":1}",
            })
        );
        assert_eq!(
            deepseek_provider_core_output_item_done_event(9, &item),
            serde_json::json!({
                "type": "response.output_item.done",
                "sequence_number": 9,
                "item": {"type": "function_call", "call_id": "call_1"},
            })
        );
    }
}
