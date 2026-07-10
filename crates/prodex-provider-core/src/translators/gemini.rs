pub(crate) use self::request::gemini_builtin_tools_from_request;
pub(crate) use self::request::gemini_function_declaration_from_openai_tool;
pub(crate) use self::request::gemini_generation_config_from_request;
pub(crate) use self::request::gemini_preserve_tool_call_signatures;
pub(crate) use self::request::gemini_request_body_without_tool;
pub(crate) use self::request::gemini_tool_config_from_request;
pub(crate) use self::request::sanitize_function_schema as gemini_sanitize_function_schema;
pub(crate) use self::request_contents::gemini_contents_from_request;
pub(crate) use self::request_contents::{
    gemini_contextual_user_instruction_text, gemini_is_contextual_user_fragment,
};
pub(crate) use self::response::gemini_normalized_response_value;
pub(crate) use self::response::{
    gemini_chat_assistant_messages_from_generate_value,
    gemini_chat_assistant_tool_call_item_with_call_id, gemini_citation_text,
    gemini_custom_apply_patch_input, gemini_finish_reason, gemini_finish_reason_failure,
    gemini_finish_reason_incomplete, gemini_prompt_feedback_failure, gemini_response_metadata,
    gemini_response_tool_call_added_item_with_call_id, gemini_response_tool_call_item_with_call_id,
    gemini_response_tool_call_raw_item_with_call_id, gemini_responses_usage,
    gemini_runtime_responses_value_from_generate_value, gemini_web_search_call_from_grounding,
};
pub(crate) use self::response::{
    gemini_image_generation_call_item_from_part, gemini_media_content_item_from_part,
    gemini_text_from_special_part,
};
pub use self::stream::{
    GeminiProviderCoreStreamChunkMetadata, GeminiProviderCoreStreamFunctionCallDelta,
    GeminiProviderCoreStreamToolCall, gemini_provider_core_function_call_arguments_delta_event,
    gemini_provider_core_function_call_arguments_delta_event_with_thought_signature,
    gemini_provider_core_output_item_added_event, gemini_provider_core_output_item_done_event,
    gemini_provider_core_output_text_delta_event,
    gemini_provider_core_reasoning_summary_part_added_event,
    gemini_provider_core_reasoning_summary_text_delta_event,
    gemini_provider_core_response_completed_event, gemini_provider_core_response_created_event,
    gemini_provider_core_response_incomplete_event, gemini_provider_core_response_metadata_event,
    gemini_provider_core_stream_candidate_parts,
    gemini_provider_core_stream_chat_assistant_message, gemini_provider_core_stream_chunk_metadata,
    gemini_provider_core_stream_citation_item_id,
    gemini_provider_core_stream_completed_tool_call_arguments,
    gemini_provider_core_stream_completed_tool_call_item,
    gemini_provider_core_stream_fallback_response_id,
    gemini_provider_core_stream_fallback_tool_call_id,
    gemini_provider_core_stream_function_call_arguments_delta_source,
    gemini_provider_core_stream_function_call_delta, gemini_provider_core_stream_media_item_id,
    gemini_provider_core_stream_message_item, gemini_provider_core_stream_output_items,
    gemini_provider_core_stream_output_message_item,
    gemini_provider_core_stream_output_text_content,
    gemini_provider_core_stream_output_text_item_id,
    gemini_provider_core_stream_part_function_call,
    gemini_provider_core_stream_part_has_video_metadata,
    gemini_provider_core_stream_part_is_thought, gemini_provider_core_stream_part_text,
    gemini_provider_core_stream_reasoning_delta_source,
    gemini_provider_core_stream_response_id_from_chunk, gemini_provider_core_stream_response_value,
    gemini_provider_core_stream_should_emit_function_call_arguments_delta,
    gemini_provider_core_stream_text_delta_source, gemini_provider_core_stream_tool_call,
    gemini_provider_core_stream_tool_call_added_item,
    gemini_provider_core_stream_tool_call_arguments_value,
    gemini_provider_core_stream_tool_call_ids,
};
use crate::translator::{
    ProviderParamSupport, ProviderTransformInput, ProviderTransformResult, ProviderTranslator,
    ProviderUnsupportedReason,
};
use crate::{
    ProviderEndpoint, ProviderId, ProviderTokenUsage, ProviderWireFormat, extract_usage_tokens,
};
use serde_json::Value;

mod request;
mod request_contents;
mod request_transform;
mod response;
mod response_transform;
mod stream;

use self::request_transform::gemini_transform_request;
use self::response_transform::gemini_transform_response;
use self::stream::gemini_transform_stream_event;

#[derive(Clone, Copy)]
pub struct GeminiTranslator;

impl ProviderTranslator for GeminiTranslator {
    fn provider(&self) -> ProviderId {
        ProviderId::Gemini
    }

    fn client_wire_format(&self) -> ProviderWireFormat {
        ProviderWireFormat::OpenAiResponses
    }

    fn upstream_wire_format(&self) -> ProviderWireFormat {
        ProviderWireFormat::GeminiGenerateContent
    }

    fn supported_params(&self, endpoint: ProviderEndpoint, _model: &str) -> ProviderParamSupport {
        if endpoint == ProviderEndpoint::Models {
            return ProviderParamSupport::full();
        }
        if matches!(
            endpoint,
            ProviderEndpoint::Responses
                | ProviderEndpoint::ChatCompletions
                | ProviderEndpoint::Messages
                | ProviderEndpoint::Embeddings
        ) {
            return ProviderParamSupport {
                        supported: true,
                        unsupported: vec![
                    ProviderUnsupportedReason {
                        field: "response_format.type".to_string(),
                        reason:
                            "Gemini v1 translator supports only text, json_object, and json_schema response formats"
                                .to_string(),
                    },
                ],
            };
        }
        let mut support = ProviderParamSupport::full();
        if endpoint != ProviderEndpoint::Responses {
            support.supported = false;
            support.unsupported.push(ProviderUnsupportedReason {
                field: endpoint.label().to_string(),
                reason: "v1 translator currently models only responses compatibility".to_string(),
            });
        }
        support
    }

    fn transform_request(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        gemini_transform_request(input)
    }

    fn transform_response(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        gemini_transform_response(input)
    }

    fn transform_stream_event(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        gemini_transform_stream_event(input)
    }

    fn extract_usage(&self, body: &[u8]) -> ProviderTokenUsage {
        let Ok(value) = serde_json::from_slice::<Value>(body) else {
            return extract_usage_tokens(body);
        };
        let value = gemini_normalized_response_value(&value);
        extract_usage_tokens(&serde_json::to_vec(value.as_ref()).unwrap_or_else(|_| body.to_vec()))
    }
}

fn gemini_passthrough_endpoint(endpoint: ProviderEndpoint) -> bool {
    matches!(
        endpoint,
        ProviderEndpoint::ChatCompletions
            | ProviderEndpoint::Messages
            | ProviderEndpoint::Embeddings
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn gemini_provider_core_shapes_response_stream_events() {
        let response = json!({"id": "resp_1"});
        let item = json!({"type": "message"});
        let metadata = json!({"gemini": {"finishReason": "STOP"}});

        assert_eq!(
            gemini_provider_core_response_created_event(1, 123, "resp_1"),
            json!({
                "type": "response.created",
                "sequence_number": 1,
                "created_at": 123,
                "response": {"id": "resp_1"},
            })
        );
        assert_eq!(
            gemini_provider_core_response_completed_event(2, 123, &response),
            json!({
                "type": "response.completed",
                "sequence_number": 2,
                "created_at": 123,
                "response": response,
            })
        );
        assert_eq!(
            gemini_provider_core_response_incomplete_event(
                3,
                123,
                "resp_1",
                "max_output_tokens",
                "truncated"
            ),
            json!({
                "type": "response.incomplete",
                "sequence_number": 3,
                "created_at": 123,
                "response": {
                    "id": "resp_1",
                    "status": "incomplete",
                    "incomplete_details": {
                        "reason": "max_output_tokens",
                        "message": "truncated",
                    },
                },
            })
        );
        assert_eq!(
            gemini_provider_core_response_metadata_event(4, 123, "resp_1", metadata.clone()),
            json!({
                "type": "response.metadata",
                "sequence_number": 4,
                "created_at": 123,
                "response_id": "resp_1",
                "metadata": metadata,
            })
        );
        assert_eq!(
            gemini_provider_core_output_item_added_event(5, &item),
            json!({
                "type": "response.output_item.added",
                "sequence_number": 5,
                "item": item,
            })
        );
        assert_eq!(
            gemini_provider_core_output_item_done_event(6, Some("resp_1"), &item),
            json!({
                "type": "response.output_item.done",
                "sequence_number": 6,
                "response_id": "resp_1",
                "item": item,
            })
        );
        assert_eq!(
            gemini_provider_core_output_item_done_event(
                7,
                None,
                &json!({"type": "web_search_call"})
            ),
            json!({
                "type": "response.output_item.done",
                "sequence_number": 7,
                "item": {"type": "web_search_call"},
            })
        );
        assert_eq!(
            gemini_provider_core_output_text_delta_event(8, 123, "resp_1", "hello"),
            json!({
                "type": "response.output_text.delta",
                "sequence_number": 8,
                "created_at": 123,
                "response_id": "resp_1",
                "delta": "hello",
            })
        );
        assert_eq!(
            gemini_provider_core_reasoning_summary_part_added_event(8, "resp_1", 0),
            json!({
                "type": "response.reasoning_summary_part.added",
                "sequence_number": 8,
                "response_id": "resp_1",
                "summary_index": 0,
            })
        );
        assert_eq!(
            gemini_provider_core_reasoning_summary_text_delta_event(9, "resp_1", 0, "thinking"),
            json!({
                "type": "response.reasoning_summary_text.delta",
                "sequence_number": 9,
                "response_id": "resp_1",
                "summary_index": 0,
                "delta": "thinking",
            })
        );
        assert_eq!(
            gemini_provider_core_stream_output_message_item(vec![
                gemini_provider_core_stream_output_text_content("visible")
            ]),
            json!({
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "visible"}],
            })
        );
    }

    #[test]
    fn gemini_provider_core_shapes_stream_delta_sources() {
        assert_eq!(
            gemini_provider_core_stream_output_text_item_id(9),
            "msg_gemini_9"
        );
        assert_eq!(
            gemini_provider_core_stream_media_item_id(9),
            "msg_gemini_media_9"
        );
        assert_eq!(
            gemini_provider_core_stream_citation_item_id(9),
            "msg_gemini_citations_9"
        );
        assert_eq!(
            gemini_provider_core_stream_fallback_response_id(9),
            "resp_gemini_9"
        );
        assert_eq!(
            gemini_provider_core_stream_fallback_tool_call_id(9, 2),
            "call_gemini_9_2"
        );
        assert_eq!(
            gemini_provider_core_stream_text_delta_source("hello"),
            json!({
                "candidates": [{
                    "content": {
                        "parts": [{"text": "hello"}]
                    }
                }]
            })
        );
        assert_eq!(
            gemini_provider_core_stream_reasoning_delta_source("thinking"),
            json!({
                "candidates": [{
                    "content": {
                        "parts": [{"text": "thinking", "thought": true}]
                    }
                }]
            })
        );
    }

    #[test]
    fn gemini_provider_core_shapes_stream_tool_calls() {
        let parsed = gemini_provider_core_stream_function_call_delta(&json!({
            "id": "call_1",
            "name": "shell",
            "args": {"cmd": "pwd"},
        }));
        assert_eq!(parsed.explicit_call_id.as_deref(), Some("call_1"));
        assert_eq!(parsed.name, "shell");
        assert_eq!(parsed.arguments, "{\"cmd\":\"pwd\"}");

        let defaults = gemini_provider_core_stream_function_call_delta(&json!({"id": "  "}));
        assert_eq!(defaults.explicit_call_id, None);
        assert_eq!(defaults.name, "tool_call");
        assert_eq!(defaults.arguments, "{}");

        let fallback =
            gemini_provider_core_stream_tool_call(9, 2, None, None, "not-json", Some("sig"));
        assert_eq!(fallback.call_id, "call_gemini_9_2");
        assert_eq!(fallback.name, "tool_call");
        assert_eq!(fallback.arguments, "not-json");
        assert_eq!(fallback.thought_signature.as_deref(), Some("sig"));

        let explicit =
            gemini_provider_core_stream_tool_call(9, 3, Some("call_1"), Some("shell"), "{}", None);
        assert_eq!(explicit.call_id, "call_1");
        assert_eq!(explicit.name, "shell");
        assert_eq!(
            gemini_provider_core_stream_tool_call_ids(&[
                fallback.clone(),
                GeminiProviderCoreStreamToolCall {
                    call_id: " ".to_string(),
                    name: "empty".to_string(),
                    arguments: String::new(),
                    thought_signature: None,
                },
                explicit.clone(),
            ]),
            vec!["call_gemini_9_2".to_string(), "call_1".to_string()]
        );
    }

    #[test]
    fn gemini_provider_core_shapes_stream_function_call_arguments_delta() {
        assert!(gemini_provider_core_stream_should_emit_function_call_arguments_delta("shell"));
        assert!(
            !gemini_provider_core_stream_should_emit_function_call_arguments_delta("tool_search")
        );
        assert!(
            !gemini_provider_core_stream_should_emit_function_call_arguments_delta("apply_patch")
        );
        assert_eq!(
            gemini_provider_core_stream_function_call_arguments_delta_source(
                "call_1",
                "shell",
                "{\"cmd\":\"pwd\"}",
            ),
            json!({
                "candidates": [{
                    "content": {
                        "parts": [{
                            "functionCall": {
                                "id": "call_1",
                                "name": "shell",
                                "args": {"cmd": "pwd"},
                            }
                        }]
                    }
                }]
            })
        );
        assert_eq!(
            gemini_provider_core_stream_function_call_arguments_delta_source(
                "call_raw", "raw_tool", "not-json",
            )["candidates"][0]["content"]["parts"][0]["functionCall"]["args"],
            json!({})
        );
        assert_eq!(
            gemini_provider_core_function_call_arguments_delta_event(8, "call_1", "{\"x\":1}"),
            json!({
                "type": "response.function_call_arguments.delta",
                "sequence_number": 8,
                "call_id": "call_1",
                "delta": "{\"x\":1}",
            })
        );
        assert_eq!(
            gemini_provider_core_function_call_arguments_delta_event_with_thought_signature(
                gemini_provider_core_function_call_arguments_delta_event(8, "call_1", "{\"x\":1}"),
                Some("sig_delta"),
            )["thought_signature"],
            "sig_delta"
        );
    }

    #[test]
    fn gemini_provider_core_shapes_completed_stream_tool_call_items() {
        assert_eq!(
            gemini_provider_core_stream_completed_tool_call_arguments(
                "apply_patch",
                "*** Begin Patch"
            ),
            "*** Begin Patch"
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(
                &gemini_provider_core_stream_completed_tool_call_arguments(
                    "shell",
                    r#"{"cmd":"cd /repo && cargo check -q"}"#,
                )
            )
            .unwrap()["cmd"],
            "cd /repo && rtk cargo check -q"
        );
        assert_eq!(
            gemini_provider_core_stream_tool_call_arguments_value("{\"cmd\":\"pwd\"}")["cmd"],
            "pwd"
        );
        assert_eq!(
            gemini_provider_core_stream_tool_call_arguments_value("not-json"),
            json!("not-json")
        );

        let added = gemini_provider_core_stream_tool_call_added_item(
            "call_added",
            "plain_tool",
            Some("sig_added"),
        )
        .unwrap();
        assert_eq!(added["type"], "function_call");
        assert_eq!(added["call_id"], "call_added");
        assert_eq!(added["name"], "plain_tool");
        assert_eq!(added["gemini_thought_signature"], "sig_added");
        assert_eq!(
            gemini_provider_core_stream_tool_call_added_item("call_search", "tool_search", None),
            None
        );

        let blocked = gemini_provider_core_stream_completed_tool_call_item(
            "call_blocked",
            "shell",
            "blocked by policy",
            None,
            true,
        );
        assert_eq!(blocked["type"], "message");
        assert_eq!(blocked["content"][0]["text"], "blocked by policy");

        let item = gemini_provider_core_stream_completed_tool_call_item(
            "call_1",
            "plain_tool",
            "{\"path\":\"README.md\"}",
            Some("sig"),
            false,
        );
        assert_eq!(item["type"], "function_call");
        assert_eq!(item["call_id"], "call_1");
        assert_eq!(item["name"], "plain_tool");
        assert_eq!(item["arguments"], "{\"path\":\"README.md\"}");
        assert_eq!(item["gemini_thought_signature"], "sig");

        let raw = gemini_provider_core_stream_completed_tool_call_item(
            "call_raw",
            "raw_tool",
            "not-json",
            Some("sig_raw"),
            false,
        );
        assert_eq!(raw["call_id"], "call_raw");
        assert_eq!(raw["arguments"], "not-json");
        assert_eq!(raw["gemini_thought_signature"], "sig_raw");

        let search = gemini_provider_core_stream_completed_tool_call_item(
            "call_search",
            "tool_search",
            "{\"query\":\"sqz tools\"}",
            None,
            false,
        );
        assert_eq!(search["type"], "tool_search_call");
        assert_eq!(search["call_id"], "call_search");
        assert_eq!(search["arguments"]["query"], "sqz tools");
    }

    #[test]
    fn gemini_provider_core_shapes_stream_message_items() {
        assert_eq!(
            gemini_provider_core_stream_output_text_content("citation"),
            json!({"type": "output_text", "text": "citation"})
        );
        assert_eq!(
            gemini_provider_core_stream_message_item(
                "msg_1",
                vec![gemini_provider_core_stream_output_text_content("citation")],
            ),
            json!({
                "id": "msg_1",
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "citation"}],
            })
        );
    }

    #[test]
    fn gemini_provider_core_shapes_stream_chat_assistant_message() {
        assert_eq!(
            gemini_provider_core_stream_chat_assistant_message(
                "",
                "",
                &[],
                &[json!({"videoMetadata": {}})],
                &[],
                Some(&json!({"gemini": "metadata-only"})),
                &[],
            ),
            None
        );

        let message = gemini_provider_core_stream_chat_assistant_message(
            "visible",
            "thinking",
            &[json!({"type": "input_image"})],
            &[json!({"videoMetadata": {}})],
            &[json!({"type": "image_generation_call"})],
            Some(&json!({"gemini": {"finishReason": "STOP"}})),
            &[GeminiProviderCoreStreamToolCall {
                call_id: "call_1".to_string(),
                name: "shell".to_string(),
                arguments: "{\"cmd\":\"pwd\"}".to_string(),
                thought_signature: Some("sig".to_string()),
            }],
        )
        .unwrap();

        assert_eq!(message["role"], "assistant");
        assert_eq!(message["content"], "visible");
        assert_eq!(message["reasoning_content"], "thinking");
        assert_eq!(message["gemini_media_content"][0]["type"], "input_image");
        assert_eq!(
            message["gemini_native_parts"][0]["videoMetadata"],
            json!({})
        );
        assert_eq!(
            message["gemini_image_generation"][0]["type"],
            "image_generation_call"
        );
        assert_eq!(message["gemini_metadata"]["gemini"]["finishReason"], "STOP");
        assert_eq!(message["tool_calls"][0]["id"], "call_1");
        assert_eq!(message["tool_calls"][0]["function"]["name"], "shell");
        assert_eq!(message["tool_calls"][0]["gemini_thought_signature"], "sig");

        assert_eq!(
            gemini_provider_core_stream_chat_assistant_message(
                "",
                "",
                &[],
                &[],
                &[],
                None,
                &[GeminiProviderCoreStreamToolCall {
                    call_id: "call_2".to_string(),
                    name: "tool_call".to_string(),
                    arguments: String::new(),
                    thought_signature: None,
                }],
            )
            .unwrap()["content"],
            ""
        );
    }

    #[test]
    fn gemini_provider_core_shapes_stream_output_items() {
        let output = gemini_provider_core_stream_output_items(
            Some(&json!({"type": "web_search_call"})),
            &[json!({"type": "image_generation_call"})],
            "visible",
            &[json!({"type": "input_image"})],
            Some("citation"),
            &[
                GeminiProviderCoreStreamToolCall {
                    call_id: "call_1".to_string(),
                    name: "plain_tool".to_string(),
                    arguments: "{\"path\":\"README.md\"}".to_string(),
                    thought_signature: Some("sig".to_string()),
                },
                GeminiProviderCoreStreamToolCall {
                    call_id: "call_2".to_string(),
                    name: "raw_tool".to_string(),
                    arguments: "not-json".to_string(),
                    thought_signature: None,
                },
                GeminiProviderCoreStreamToolCall {
                    call_id: "call_3".to_string(),
                    name: "blocked_tool".to_string(),
                    arguments: "{\"cmd\":\"rm -rf /\"}".to_string(),
                    thought_signature: None,
                },
            ],
            |name, _args| (name == "blocked_tool").then(|| "blocked by policy".to_string()),
        );

        assert_eq!(output[0]["type"], "web_search_call");
        assert_eq!(output[1]["type"], "image_generation_call");
        assert_eq!(output[2]["content"][0]["text"], "visible");
        assert_eq!(output[2]["content"][1]["type"], "input_image");
        assert_eq!(output[3]["content"][0]["text"], "citation");
        assert_eq!(output[4]["type"], "function_call");
        assert_eq!(output[4]["call_id"], "call_1");
        assert_eq!(output[4]["name"], "plain_tool");
        assert_eq!(output[4]["arguments"], "{\"path\":\"README.md\"}");
        assert_eq!(output[4]["gemini_thought_signature"], "sig");
        assert_eq!(output[5]["call_id"], "call_2");
        assert_eq!(output[5]["arguments"], "not-json");
        assert_eq!(output[6]["content"][0]["text"], "blocked by policy");
    }

    #[test]
    fn gemini_provider_core_shapes_stream_response_value() {
        assert_eq!(
            gemini_provider_core_stream_response_id_from_chunk(
                "resp_gemini_9",
                &json!({"responseId": "resp_real"})
            ),
            Some("resp_real".to_string())
        );
        assert_eq!(
            gemini_provider_core_stream_response_id_from_chunk(
                "resp_real",
                &json!({"responseId": "resp_new"})
            ),
            None
        );
        assert_eq!(
            gemini_provider_core_stream_response_value(
                "resp_1",
                vec![json!({"type": "message"})],
                Some("gemini-test"),
                Some(json!({"input_tokens": 1, "output_tokens": 2})),
                Some(json!({"gemini": {"finishReason": "STOP"}})),
            ),
            json!({
                "id": "resp_1",
                "output": [{"type": "message"}],
                "model": "gemini-test",
                "usage": {"input_tokens": 1, "output_tokens": 2},
                "metadata": {"gemini": {"finishReason": "STOP"}},
            })
        );
    }

    #[test]
    fn gemini_provider_core_extracts_stream_chunk_metadata() {
        let metadata = gemini_provider_core_stream_chunk_metadata(
            "resp_gemini_9",
            &json!({
                "responseId": "resp_real",
                "modelVersion": "gemini-test",
                "usageMetadata": {
                    "promptTokenCount": 3,
                    "candidatesTokenCount": 5,
                    "totalTokenCount": 8,
                    "thoughtsTokenCount": 2
                },
                "candidates": [{
                    "finishReason": "STOP",
                    "avgLogprobs": -0.5,
                    "content": {"parts": [{"text": "hi"}]}
                }]
            }),
        );

        assert_eq!(metadata.response_id.as_deref(), Some("resp_real"));
        assert_eq!(metadata.model.as_deref(), Some("gemini-test"));
        assert_eq!(metadata.finish_reason.as_deref(), Some("STOP"));
        assert_eq!(metadata.usage.as_ref().unwrap()["input_tokens"], json!(3));
        assert_eq!(
            metadata.usage.as_ref().unwrap()["output_tokens_details"]["reasoning_tokens"],
            json!(2)
        );
        assert_eq!(
            metadata.response_metadata.as_ref().unwrap()["gemini"]["avgLogprobs"],
            json!(-0.5)
        );

        assert_eq!(
            gemini_provider_core_stream_chunk_metadata("resp_real", &json!({"responseId": "new"}))
                .response_id,
            None
        );
    }

    #[test]
    fn gemini_provider_core_extracts_stream_candidate_parts() {
        let value = json!({
            "candidates": [{
                "content": {
                    "parts": [
                        {"text": "thinking", "thought": true},
                        {"text": "visible"},
                        {"functionCall": {"name": "shell"}},
                        {"videoMetadata": {"fps": 24}}
                    ]
                }
            }]
        });
        let parts = gemini_provider_core_stream_candidate_parts(&value).unwrap();

        assert_eq!(parts.len(), 4);
        assert_eq!(
            gemini_provider_core_stream_part_text(&parts[0]),
            Some("thinking")
        );
        assert!(gemini_provider_core_stream_part_is_thought(&parts[0]));
        assert_eq!(
            gemini_provider_core_stream_part_text(&json!({"text": ""})),
            None
        );
        assert_eq!(
            gemini_provider_core_stream_part_function_call(&parts[2])
                .and_then(|function_call| function_call.get("name"))
                .and_then(serde_json::Value::as_str),
            Some("shell")
        );
        assert!(gemini_provider_core_stream_part_has_video_metadata(
            &parts[3]
        ));
        assert!(gemini_provider_core_stream_candidate_parts(&json!({})).is_none());
    }
}
