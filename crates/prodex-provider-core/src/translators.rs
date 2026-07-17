mod anthropic;
mod copilot;
mod deepseek;
mod gemini;
mod kiro;
mod openai_chat_compat;
mod passthrough;
mod tool_args;

use crate::ProviderId;
use crate::translator::{ProviderConformanceCase, ProviderTransformResult, ProviderTranslator};
use crate::{ProviderEndpoint, ProviderWireFormat};

pub use anthropic::{
    AnthropicMessagesTranslator, AnthropicTranslator,
    translate_openai_chat_request_to_anthropic_messages,
};
pub use copilot::{
    CopilotTranslator, copilot_provider_core_request_body_with_canonical_model,
    copilot_provider_core_request_body_without_encrypted_content,
    copilot_provider_core_request_has_agent_input, copilot_provider_core_request_has_vision_input,
    copilot_provider_core_response_id_from_value,
};
pub use deepseek::{
    DeepSeekProviderCoreStreamChatToolCall, DeepSeekProviderCoreStreamChoiceDelta,
    DeepSeekProviderCoreStreamChoiceMetadata, DeepSeekProviderCoreStreamChunkMetadata,
    DeepSeekProviderCoreStreamToolCallDelta, DeepSeekTranslator,
    deepseek_provider_core_chat_stream_error,
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
pub(crate) use deepseek::{
    deepseek_rtk_wrapped_tool_arguments, deepseek_tool_call_thought_signature_object,
};
pub(crate) use gemini::gemini_builtin_tools_from_request;
pub(crate) use gemini::gemini_citation_text;
pub(crate) use gemini::gemini_contents_from_request;
pub(crate) use gemini::gemini_contextual_user_instruction_text;
pub(crate) use gemini::gemini_custom_apply_patch_input;
pub(crate) use gemini::gemini_function_declaration_from_openai_tool;
pub(crate) use gemini::gemini_generation_config_from_request;
pub(crate) use gemini::gemini_is_contextual_user_fragment;
pub(crate) use gemini::gemini_normalized_response_value;
pub(crate) use gemini::gemini_preserve_tool_call_signatures;
pub(crate) use gemini::gemini_request_body_without_tool;
pub(crate) use gemini::gemini_sanitize_function_schema;
pub(crate) use gemini::gemini_tool_config_from_request;
pub use gemini::{
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
pub use gemini::{GeminiTranslator, gemini_provider_core_model_uses_thinking_level};
pub(crate) use gemini::{
    gemini_chat_assistant_messages_from_generate_value,
    gemini_chat_assistant_tool_call_item_with_call_id, gemini_finish_reason,
    gemini_finish_reason_failure, gemini_finish_reason_incomplete,
    gemini_image_generation_call_item_from_part, gemini_media_content_item_from_part,
    gemini_prompt_feedback_failure, gemini_response_metadata,
    gemini_response_tool_call_added_item_with_call_id, gemini_response_tool_call_item_with_call_id,
    gemini_response_tool_call_raw_item_with_call_id, gemini_responses_usage,
    gemini_runtime_responses_value_from_generate_value,
    gemini_runtime_responses_value_from_generate_value_with_fallback_ids,
    gemini_text_from_special_part, gemini_web_search_call_from_grounding,
};
pub use kiro::{
    KiroProviderCoreRequestError, KiroTranslator, kiro_provider_core_acp_assistant_output_message,
    kiro_provider_core_acp_chat_assistant_message, kiro_provider_core_acp_chat_tool_call_item,
    kiro_provider_core_acp_error_value, kiro_provider_core_acp_incomplete_details,
    kiro_provider_core_acp_incomplete_details_value, kiro_provider_core_acp_initialize_request,
    kiro_provider_core_acp_mark_failed_response, kiro_provider_core_acp_mark_incomplete_response,
    kiro_provider_core_acp_metadata, kiro_provider_core_acp_model_value,
    kiro_provider_core_acp_plan_entry, kiro_provider_core_acp_response_value,
    kiro_provider_core_acp_responses_tool_call_item, kiro_provider_core_acp_session_info,
    kiro_provider_core_acp_session_new_request, kiro_provider_core_acp_session_prompt_request,
    kiro_provider_core_acp_stop_reason, kiro_provider_core_acp_usage_update_json,
    kiro_provider_core_anthropic_message_value_from_response,
    kiro_provider_core_apply_response_runtime_metadata, kiro_provider_core_chat_completion_chunk,
    kiro_provider_core_chat_completion_empty_delta,
    kiro_provider_core_chat_completion_finish_reason,
    kiro_provider_core_chat_completion_finish_reason_from_response,
    kiro_provider_core_chat_completion_role_delta, kiro_provider_core_chat_completion_text_delta,
    kiro_provider_core_chat_completion_tool_call_delta,
    kiro_provider_core_chat_completion_value_from_response,
    kiro_provider_core_chat_completions_request_body,
    kiro_provider_core_compact_summary_from_response,
    kiro_provider_core_invalid_request_error_value, kiro_provider_core_model_list_value,
    kiro_provider_core_model_value_or_not_found, kiro_provider_core_output_item_added_event,
    kiro_provider_core_output_item_done_event, kiro_provider_core_output_text_delta_event,
    kiro_provider_core_prompt_from_chat_messages, kiro_provider_core_response_completed_event,
    kiro_provider_core_response_created_event, kiro_provider_core_response_has_tool_calls,
    kiro_provider_core_responses_items_from_chat_message,
    kiro_provider_core_semantic_compact_instructions,
    kiro_provider_core_semantic_compact_request_body, kiro_provider_core_stream_content_text,
    kiro_provider_core_stream_tool_arguments, kiro_provider_core_stream_tool_call_item,
    kiro_provider_core_tool_call_arguments_delta_chat_value,
    kiro_provider_core_tool_choice_from_legacy_chat_function_call,
    kiro_provider_core_tool_from_legacy_chat_function,
    kiro_provider_core_unsupported_path_error_value,
};
pub use passthrough::PassthroughTranslator;
pub(crate) use tool_args::chat_compatible_rtk_wrapped_tool_arguments;

pub(super) fn unsupported_endpoint_result(
    provider: ProviderId,
    endpoint: ProviderEndpoint,
    from_format: ProviderWireFormat,
    to_format: ProviderWireFormat,
) -> ProviderTransformResult {
    ProviderTransformResult::unsupported(
        provider,
        endpoint,
        from_format,
        to_format,
        format!(
            "{} translator does not support {}",
            provider.label(),
            endpoint.label()
        ),
    )
}

pub(super) fn provider_declares_passthrough(
    provider: ProviderId,
    endpoint: ProviderEndpoint,
    from_format: ProviderWireFormat,
    to_format: ProviderWireFormat,
) -> bool {
    crate::provider_implementation_registry()
        .get(provider)
        .is_some_and(|descriptor| descriptor.declares_passthrough(endpoint, from_format, to_format))
}

static ANTHROPIC_MESSAGES_TRANSLATOR: AnthropicMessagesTranslator = AnthropicMessagesTranslator;

pub fn provider_translator(provider: ProviderId) -> &'static dyn ProviderTranslator {
    crate::provider_implementation_registry()
        .get(provider)
        .expect("built-in provider implementation must be registered")
        .translator()
}

pub fn anthropic_messages_translator() -> &'static dyn ProviderTranslator {
    &ANTHROPIC_MESSAGES_TRANSLATOR
}

pub fn provider_conformance_cases() -> &'static [ProviderConformanceCase] {
    static CASES: std::sync::OnceLock<Vec<ProviderConformanceCase>> = std::sync::OnceLock::new();
    CASES
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../tests/fixtures/provider_conformance_cases.json"
            ))
            .expect("provider conformance fixtures should parse")
        })
        .as_slice()
}
