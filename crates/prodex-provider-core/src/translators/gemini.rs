pub(crate) use self::request::gemini_builtin_tools_from_request;
pub(crate) use self::request::gemini_function_declaration_from_openai_tool;
pub(crate) use self::request::gemini_generation_config_from_request;
pub(crate) use self::request::gemini_preserve_tool_call_signatures;
pub use self::request::gemini_provider_core_model_uses_thinking_level;
pub(crate) use self::request::gemini_request_body_without_tool;
pub(crate) use self::request::gemini_tool_config_from_request;
pub(crate) use self::request::sanitize_function_schema as gemini_sanitize_function_schema;
pub(crate) use self::request::{gemini_validate_candidate_count, gemini_validate_openai_tools};
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
    gemini_runtime_responses_value_from_generate_value,
    gemini_runtime_responses_value_from_generate_value_with_fallback_ids,
    gemini_web_search_call_from_grounding,
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
                | ProviderEndpoint::ResponsesCompact
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
#[path = "gemini_tests.rs"]
mod tests;
