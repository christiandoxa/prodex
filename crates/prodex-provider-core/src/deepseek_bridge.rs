//! DeepSeek bridge helpers used by app/runtime compatibility shims.
//!
//! These stay pure: no transport, auth, retry, or session state ownership.

use crate::{
    ProviderTransformResult, provider_core_rewritten_body,
    translators::{
        deepseek_rtk_wrapped_tool_arguments, deepseek_tool_call_thought_signature_object,
    },
};

mod input_items;
mod json;
mod messages;
mod request_messages;
mod request_params;
mod request_probe;
mod request_tools;

pub use self::input_items::{
    deepseek_provider_core_chat_role, deepseek_provider_core_first_function_call_output_call_id,
    deepseek_provider_core_history_has_system_message,
    deepseek_provider_core_history_has_tool_call, deepseek_provider_core_message_signatures,
    deepseek_provider_core_push_message_from_responses_item, deepseek_provider_core_system_message,
    deepseek_provider_core_tool_call_ids, deepseek_provider_core_tool_output_call_ids,
    deepseek_provider_core_user_message, deepseek_provider_core_validate_supported_input_item,
};
pub use self::json::{
    deepseek_provider_core_json_string, deepseek_provider_core_json_string_at_path,
    deepseek_provider_core_responses_content_text,
    deepseek_provider_core_responses_content_text_value,
};
pub use self::messages::{
    deepseek_provider_core_chat_assistant_messages_from_response_value,
    deepseek_provider_core_merge_response_metadata,
    deepseek_provider_core_normalize_assistant_tool_call_content,
    deepseek_provider_core_normalize_thinking_tool_call_messages,
    deepseek_provider_core_repair_tool_call_adjacency,
};
pub use self::request_messages::deepseek_provider_core_messages_from_responses_request;
pub use self::request_params::{
    deepseek_provider_core_apply_reasoning_from_responses_request,
    deepseek_provider_core_ensure_json_prompt_instruction,
    deepseek_provider_core_insert_primitive_request_fields,
    deepseek_provider_core_note_thinking_tool_choice_omission,
    deepseek_provider_core_reject_beta_completion_fields,
    deepseek_provider_core_reject_unsupported_request_fields,
    deepseek_provider_core_response_format_from_responses_request,
    deepseek_provider_core_response_metadata_from_responses_request,
    deepseek_provider_core_stop_from_responses_request, deepseek_provider_core_thinking_enabled,
    deepseek_provider_core_top_logprobs_from_responses_request,
    deepseek_provider_core_user_id_from_responses_request,
    deepseek_provider_core_validate_reasoning_shape,
};
pub use self::request_probe::deepseek_provider_core_simple_request;

pub use self::request_tools::{
    deepseek_provider_core_apply_strict_function_schema,
    deepseek_provider_core_dedup_and_validate_function_tools,
    deepseek_provider_core_function_tool_name, deepseek_provider_core_is_generic_function_tool,
    deepseek_provider_core_tool_name_from_tool_object,
    deepseek_provider_core_validate_function_name,
    deepseek_provider_core_validate_function_parameters,
    deepseek_provider_core_validate_tool_choice_name,
    deepseek_provider_core_validate_tool_choice_shape,
    deepseek_provider_core_validate_tool_choice_target,
    deepseek_provider_core_validate_tools_shape,
    deepseek_provider_core_validate_web_search_options,
    deepseek_provider_core_validate_web_search_tool_context_size,
};

pub fn deepseek_provider_core_request_body(result: &ProviderTransformResult) -> Option<Vec<u8>> {
    provider_core_rewritten_body(Some(result))
}

pub fn deepseek_provider_core_tool_call_thought_signature(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<String> {
    deepseek_tool_call_thought_signature_object(object)
}

pub fn deepseek_provider_core_rtk_wrapped_tool_arguments(name: &str, arguments: &str) -> String {
    deepseek_rtk_wrapped_tool_arguments(name, arguments)
}

#[cfg(test)]
mod tests;
