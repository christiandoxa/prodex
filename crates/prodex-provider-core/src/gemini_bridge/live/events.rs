//! Gemini Live OpenAI-compatible event builders.

#[path = "events/error.rs"]
mod error;
#[path = "events/output.rs"]
mod output;

pub use self::error::{
    gemini_provider_core_live_binary_frame_error, gemini_provider_core_live_error_event,
    gemini_provider_core_live_provider_stream_error,
    gemini_provider_core_live_unsupported_event_error,
};
pub use self::output::{
    gemini_provider_core_live_input_audio_transcription_completed_event,
    gemini_provider_core_live_input_audio_transcription_delta_event,
    gemini_provider_core_live_output_audio_delta_event,
    gemini_provider_core_live_output_audio_transcript_delta_event,
    gemini_provider_core_live_output_audio_transcript_done_event,
    gemini_provider_core_live_output_text_delta_event,
    gemini_provider_core_live_output_text_done_event,
};

pub fn gemini_provider_core_live_input_audio_cleared_event() -> serde_json::Value {
    serde_json::json!({
        "type": "input_audio_buffer.cleared",
    })
}

pub fn gemini_provider_core_live_response_created_event(response_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "response.created",
        "response": {"id": response_id},
    })
}

pub fn gemini_provider_core_live_response_cancelled_event(response_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "response.cancelled",
        "response": {"id": response_id},
    })
}

pub fn gemini_provider_core_live_conversation_item_truncated_event(
    value: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "type": "conversation.item.truncated",
        "item_id": value.get("item_id").cloned().unwrap_or(serde_json::Value::Null),
        "content_index": value.get("content_index").cloned().unwrap_or(serde_json::Value::Null),
        "audio_end_ms": value.get("audio_end_ms").cloned().unwrap_or(serde_json::Value::Null),
    })
}

pub fn gemini_provider_core_live_conversation_item_deleted_event(
    value: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "type": "conversation.item.deleted",
        "item_id": value.get("item_id").cloned().unwrap_or(serde_json::Value::Null),
    })
}

pub fn gemini_provider_core_live_session_updated_event() -> serde_json::Value {
    serde_json::json!({
        "type": "session.updated",
        "session": {
            "id": "sess_gemini_live",
            "type": "realtime",
        }
    })
}

pub fn gemini_provider_core_live_function_call_done_event(
    call_id: &str,
    name: &str,
    args: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "type": "conversation.item.done",
        "item": {
            "id": call_id,
            "type": "function_call",
            "name": name,
            "call_id": call_id,
            "arguments": serde_json::to_string(args).unwrap_or_else(|_| "{}".to_string()),
        }
    })
}

pub fn gemini_provider_core_live_response_done_event(response_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "response.done",
        "response": {"id": response_id},
    })
}
