//! Gemini Live outbound client/setup messages.

use std::collections::HashMap;

mod setup;

pub use self::setup::{
    gemini_provider_core_live_function_declaration, gemini_provider_core_live_setup_message,
};

pub fn gemini_provider_core_live_client_content_message(
    value: &serde_json::Value,
) -> Option<serde_json::Value> {
    let item = value.get("item")?;
    if item.get("type").and_then(serde_json::Value::as_str) != Some("message") {
        return None;
    }
    let text = item
        .get("content")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .filter_map(|content| content.get("text").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then(|| {
        serde_json::json!({
            "client_content": {
                "turns": [{"role": "user", "parts": [{"text": text}]}],
                "turn_complete": true,
            }
        })
    })
}

pub fn gemini_provider_core_live_tool_response_message(
    value: &serde_json::Value,
    tool_names_by_call_id: &HashMap<String, String>,
) -> Option<(String, serde_json::Value)> {
    let item = value.get("item")?;
    if item.get("type").and_then(serde_json::Value::as_str) != Some("function_call_output") {
        return None;
    }
    let call_id = item.get("call_id").and_then(serde_json::Value::as_str)?;
    let output = item
        .get("output")
        .cloned()
        .unwrap_or_else(|| serde_json::Value::String(String::new()));
    let name = tool_names_by_call_id
        .get(call_id)
        .map(String::as_str)
        .unwrap_or(call_id);
    Some((
        call_id.to_string(),
        serde_json::json!({
            "tool_response": {
                "function_responses": [{
                    "id": call_id,
                    "name": name,
                    "response": {"output": output},
                }]
            }
        }),
    ))
}

pub fn gemini_provider_core_live_realtime_audio_message(
    data: String,
    mime_type: String,
) -> serde_json::Value {
    serde_json::json!({
        "realtime_input": {
            "audio": {
                "data": data,
                "mime_type": mime_type,
            }
        }
    })
}

pub fn gemini_provider_core_live_audio_stream_end_message() -> serde_json::Value {
    serde_json::json!({
        "realtime_input": {
            "audio_stream_end": true,
        }
    })
}
