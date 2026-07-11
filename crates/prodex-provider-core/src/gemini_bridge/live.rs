//! Gemini Live / realtime translation helpers.

mod audio;
mod events;
mod messages;

pub use self::audio::{
    GEMINI_PROVIDER_CORE_LIVE_AUDIO_RATE, GeminiProviderCoreLiveAudioConfig,
    GeminiProviderCoreLiveAudioFormat, GeminiProviderCoreLiveAudioPayload,
    gemini_provider_core_live_audio_config_from_value,
    gemini_provider_core_live_audio_rate_from_mime, gemini_provider_core_live_decode_alaw,
    gemini_provider_core_live_decode_ulaw, gemini_provider_core_live_input_audio_payload,
    gemini_provider_core_live_legacy_session_audio_config,
    gemini_provider_core_live_session_audio_config,
};

pub use self::messages::{
    gemini_provider_core_live_audio_stream_end_message,
    gemini_provider_core_live_client_content_message,
    gemini_provider_core_live_function_declaration,
    gemini_provider_core_live_realtime_audio_message, gemini_provider_core_live_setup_message,
    gemini_provider_core_live_tool_response_message,
};

pub use self::events::{
    gemini_provider_core_live_binary_frame_error,
    gemini_provider_core_live_conversation_item_deleted_event,
    gemini_provider_core_live_conversation_item_truncated_event,
    gemini_provider_core_live_error_event, gemini_provider_core_live_function_call_done_event,
    gemini_provider_core_live_input_audio_cleared_event,
    gemini_provider_core_live_input_audio_transcription_completed_event,
    gemini_provider_core_live_input_audio_transcription_delta_event,
    gemini_provider_core_live_output_audio_delta_event,
    gemini_provider_core_live_output_audio_transcript_delta_event,
    gemini_provider_core_live_output_audio_transcript_done_event,
    gemini_provider_core_live_output_text_delta_event,
    gemini_provider_core_live_output_text_done_event,
    gemini_provider_core_live_provider_stream_error,
    gemini_provider_core_live_response_cancelled_event,
    gemini_provider_core_live_response_created_event,
    gemini_provider_core_live_response_done_event, gemini_provider_core_live_session_updated_event,
    gemini_provider_core_live_unsupported_event_error,
};

pub fn gemini_provider_core_live_field<'a>(
    value: &'a serde_json::Value,
    camel: &str,
    snake: &str,
) -> Option<&'a serde_json::Value> {
    value.get(camel).or_else(|| value.get(snake))
}

pub fn gemini_provider_core_live_server_turn_complete(value: &serde_json::Value) -> bool {
    gemini_provider_core_live_field(value, "serverContent", "server_content")
        .and_then(|content| {
            gemini_provider_core_live_field(content, "turnComplete", "turn_complete")
        })
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

pub fn gemini_provider_core_live_transcription_text<'a>(
    content: &'a serde_json::Value,
    camel: &str,
    snake: &str,
) -> Option<&'a str> {
    gemini_provider_core_live_field(content, camel, snake)?
        .get("text")
        .and_then(serde_json::Value::as_str)
}

pub fn gemini_provider_core_live_transcript_delta<'a>(previous: &str, current: &'a str) -> &'a str {
    current.strip_prefix(previous).unwrap_or(current)
}
