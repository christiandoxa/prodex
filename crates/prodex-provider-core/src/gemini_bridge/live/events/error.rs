//! Gemini Live error event builders.

pub fn gemini_provider_core_live_unsupported_event_error(message_type: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "error",
        "error": {
            "type": "invalid_request_error",
            "message": format!("Unsupported Codex realtime event type: {message_type}"),
        }
    })
}

pub fn gemini_provider_core_live_error_event(error: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "type": "error",
        "error": error,
    })
}

pub fn gemini_provider_core_live_provider_stream_error(
    message: impl Into<String>,
) -> serde_json::Value {
    serde_json::json!({
        "type": "error",
        "error": {
            "type": "provider_stream_error",
            "message": message.into(),
        }
    })
}

pub fn gemini_provider_core_live_binary_frame_error() -> serde_json::Value {
    serde_json::json!({
        "type": "error",
        "error": {
            "type": "invalid_request_error",
            "message": "Gemini Live bridge expects JSON text websocket frames.",
        }
    })
}
