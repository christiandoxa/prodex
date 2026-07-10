//! Gemini bridge helpers used by app/runtime compatibility shims.
//!
//! These stay pure: no transport, auth, retry, or session state.

mod bindings;
mod compact;
mod errors;
mod hardening;
mod history;
mod leaks;
mod live;
mod media;
mod precommit;
mod request;
mod response;
mod tool_io;
mod tooling;
mod util;
pub use self::bindings::{
    gemini_provider_core_response_bindings_from_body,
    gemini_provider_core_response_id_from_responses_value,
    gemini_provider_core_tool_call_ids_from_responses_value,
    gemini_provider_core_tool_output_call_ids_from_request,
};
pub use self::compact::{
    GEMINI_PROVIDER_CORE_LOCAL_COMPACT_SUMMARY_PREFIX, gemini_provider_core_compact_response_body,
    gemini_provider_core_local_compact_summary,
    gemini_provider_core_semantic_compact_continuation_summary,
    gemini_provider_core_semantic_compact_instructions,
    gemini_provider_core_semantic_compact_request_body,
    gemini_provider_core_semantic_compact_summary,
};
pub use self::errors::{
    GEMINI_PROVIDER_CORE_MAX_INLINE_RATE_LIMIT_RETRY_DELAY_MS,
    gemini_provider_core_body_has_terminal_quota, gemini_provider_core_google_quota_message,
    gemini_provider_core_invalid_stream_retry_delay_ms, gemini_provider_core_normalized_error_body,
    gemini_provider_core_response_retryable_quota, gemini_provider_core_retry_delay_ms,
    gemini_provider_core_retry_delay_ms_from_body,
    gemini_provider_core_should_inline_rate_limit_retry,
    gemini_provider_core_should_rotate_after_quota_response,
};
pub use self::hardening::{
    gemini_provider_core_harden_contents, gemini_provider_core_harden_tool_call_thought_signatures,
    gemini_provider_core_thought_signature,
};
pub use self::history::{
    gemini_provider_core_chat_message_text, gemini_provider_core_contextual_user_instruction_text,
    gemini_provider_core_import_contents_from_value,
    gemini_provider_core_is_contextual_user_fragment,
    gemini_provider_core_session_checkpoint_value,
    gemini_provider_core_system_instruction_from_chat, gemini_provider_core_truncate_to_bytes,
};
pub use self::leaks::{
    gemini_provider_core_internal_instruction_corpus,
    gemini_provider_core_internal_instruction_leak_text,
    gemini_provider_core_sanitize_internal_instruction_leak_text,
    gemini_provider_core_text_echoes_internal_instruction,
    gemini_provider_core_visible_text_from_part,
};
pub use self::live::{
    GEMINI_PROVIDER_CORE_LIVE_AUDIO_RATE, GeminiProviderCoreLiveAudioConfig,
    GeminiProviderCoreLiveAudioFormat, GeminiProviderCoreLiveAudioPayload,
    gemini_provider_core_live_audio_config_from_value,
    gemini_provider_core_live_audio_rate_from_mime,
    gemini_provider_core_live_audio_stream_end_message,
    gemini_provider_core_live_binary_frame_error, gemini_provider_core_live_client_content_message,
    gemini_provider_core_live_conversation_item_deleted_event,
    gemini_provider_core_live_conversation_item_truncated_event,
    gemini_provider_core_live_decode_alaw, gemini_provider_core_live_decode_ulaw,
    gemini_provider_core_live_error_event, gemini_provider_core_live_field,
    gemini_provider_core_live_function_call_done_event,
    gemini_provider_core_live_function_declaration,
    gemini_provider_core_live_input_audio_cleared_event,
    gemini_provider_core_live_input_audio_payload,
    gemini_provider_core_live_input_audio_transcription_completed_event,
    gemini_provider_core_live_input_audio_transcription_delta_event,
    gemini_provider_core_live_legacy_session_audio_config,
    gemini_provider_core_live_output_audio_delta_event,
    gemini_provider_core_live_output_audio_transcript_delta_event,
    gemini_provider_core_live_output_audio_transcript_done_event,
    gemini_provider_core_live_output_text_delta_event,
    gemini_provider_core_live_output_text_done_event,
    gemini_provider_core_live_provider_stream_error,
    gemini_provider_core_live_realtime_audio_message,
    gemini_provider_core_live_response_cancelled_event,
    gemini_provider_core_live_response_created_event,
    gemini_provider_core_live_response_done_event, gemini_provider_core_live_server_turn_complete,
    gemini_provider_core_live_session_audio_config,
    gemini_provider_core_live_session_updated_event, gemini_provider_core_live_setup_message,
    gemini_provider_core_live_tool_response_message, gemini_provider_core_live_transcript_delta,
    gemini_provider_core_live_transcription_text,
    gemini_provider_core_live_unsupported_event_error,
};
pub use self::media::{
    gemini_provider_core_append_media_parts_to_last_user_content,
    gemini_provider_core_collect_media_parts, gemini_provider_core_content,
    gemini_provider_core_data_url_parts, gemini_provider_core_image_url_value,
    gemini_provider_core_local_context_text_part,
    gemini_provider_core_media_part_from_content_object, gemini_provider_core_media_part_from_data,
    gemini_provider_core_media_part_from_uri_or_data_url, gemini_provider_core_mime_type_for_uri,
    gemini_provider_core_mime_type_is_text, gemini_provider_core_text_part,
};
pub use self::precommit::{
    GeminiProviderCorePrecommitDecision, GeminiProviderCorePrecommitProbe,
    gemini_provider_core_precommit_decision_for_data_lines,
};
pub use self::request::{
    gemini_provider_core_builtin_tools_from_request,
    gemini_provider_core_exact_output_generate_chunk, gemini_provider_core_exact_output_sse_stream,
    gemini_provider_core_function_declaration_from_openai_tool,
    gemini_provider_core_function_tools_from_chat,
    gemini_provider_core_generate_content_body_value,
    gemini_provider_core_generate_content_request,
    gemini_provider_core_generate_content_request_map,
    gemini_provider_core_generation_config_from_request,
    gemini_provider_core_native_request_body_with_project, gemini_provider_core_request_body,
    gemini_provider_core_request_body_without_tool, gemini_provider_core_sanitize_function_schema,
    gemini_provider_core_simple_request, gemini_provider_core_tool_config_from_request,
    gemini_provider_core_tools_from_requests, gemini_provider_core_unsupported_tool_fallback_body,
};
pub use self::response::{
    gemini_provider_core_buffered_responses_value,
    gemini_provider_core_buffered_responses_value_with_fallback_ids,
    gemini_provider_core_chat_assistant_messages,
    gemini_provider_core_chat_assistant_tool_call_item, gemini_provider_core_citation_text,
    gemini_provider_core_custom_tool_input_from_arguments, gemini_provider_core_finish_reason,
    gemini_provider_core_finish_reason_failure, gemini_provider_core_finish_reason_incomplete,
    gemini_provider_core_finish_reason_retryable_invalid,
    gemini_provider_core_image_generation_call_item_from_part,
    gemini_provider_core_media_content_item_from_part,
    gemini_provider_core_normalized_response_value,
    gemini_provider_core_preserve_tool_call_signatures,
    gemini_provider_core_prompt_feedback_failure, gemini_provider_core_response_metadata,
    gemini_provider_core_response_terminal_without_history,
    gemini_provider_core_response_tool_call_added_item,
    gemini_provider_core_response_tool_call_item, gemini_provider_core_response_tool_call_raw_item,
    gemini_provider_core_responses_usage, gemini_provider_core_runtime_responses_value,
    gemini_provider_core_runtime_responses_value_with_fallback_ids,
    gemini_provider_core_simple_response, gemini_provider_core_text_from_special_part,
    gemini_provider_core_web_search_call_from_grounding,
};
pub use self::tool_io::{
    gemini_provider_core_function_call_part,
    gemini_provider_core_function_call_part_from_tool_call,
    gemini_provider_core_function_response_content_part,
    gemini_provider_core_function_response_from_tool_message,
    gemini_provider_core_function_response_part,
    gemini_provider_core_mask_tool_response_for_history,
    gemini_provider_core_structured_command_tool_response,
    gemini_provider_core_tool_output_preview, gemini_provider_core_tool_response_output_string,
};
pub use self::tooling::{
    gemini_provider_core_apply_gemini3_tool_declaration_overrides,
    gemini_provider_core_blocked_tool_call_item,
    gemini_provider_core_conversation_requests_command_output_only,
    gemini_provider_core_forced_command_output, gemini_provider_core_gemini3_tool_description,
    gemini_provider_core_model_uses_gemini3_toolset,
    gemini_provider_core_non_actionable_wait_or_poll_text,
    gemini_provider_core_normalize_tool_name, gemini_provider_core_tool_aliases,
    gemini_provider_core_tool_call_command_text, gemini_provider_core_tool_intent_without_call,
    gemini_provider_core_tool_is_mutating, gemini_provider_core_unverified_success_claim,
};
pub use self::util::{
    gemini_provider_core_bool_str, gemini_provider_core_bool_value,
    gemini_provider_core_collect_input_texts, gemini_provider_core_collect_path_values,
    gemini_provider_core_collect_string_values, gemini_provider_core_parse_command_specific_tool,
    gemini_provider_core_skip_context_path_name, gemini_provider_core_stream_error,
};

#[cfg(test)]
mod tests;
