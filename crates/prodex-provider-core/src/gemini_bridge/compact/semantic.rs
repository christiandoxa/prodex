//! Gemini semantic compact request/summary helpers.

use crate::gemini_bridge::{
    gemini_provider_core_normalized_response_value, gemini_provider_core_runtime_responses_value,
};

const GEMINI_PROVIDER_CORE_SEMANTIC_COMPACT_INSTRUCTIONS: &str = "\
Compact the supplied coding-agent transcript into one durable continuation summary. \
Preserve the user's goals, repository instructions, decisions, files changed, exact identifiers, \
commands and test results, unresolved failures, current worktree state, and the next concrete steps. \
Remove redundant narration and obsolete intermediate reasoning. Do not call tools. \
Return only the continuation summary, with no preamble or completion claim.";

pub fn gemini_provider_core_semantic_compact_instructions() -> &'static str {
    GEMINI_PROVIDER_CORE_SEMANTIC_COMPACT_INSTRUCTIONS
}

pub fn gemini_provider_core_semantic_compact_request_body(body: &[u8]) -> Result<Vec<u8>, String> {
    let mut value = serde_json::from_slice::<serde_json::Value>(body)
        .map_err(|err| format!("failed to parse Gemini compact request JSON: {err}"))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| "Gemini compact request must be a JSON object".to_string())?;
    let input = object
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or_else(|| "Gemini compact request must contain an input array".to_string())?;
    input.push(serde_json::json!({
        "type": "message",
        "role": "user",
        "content": [{
            "type": "input_text",
            "text": GEMINI_PROVIDER_CORE_SEMANTIC_COMPACT_INSTRUCTIONS,
        }],
    }));

    object.insert(
        "instructions".to_string(),
        serde_json::Value::String(GEMINI_PROVIDER_CORE_SEMANTIC_COMPACT_INSTRUCTIONS.to_string()),
    );
    object.insert(
        "model".to_string(),
        serde_json::Value::String(crate::PRODEX_GEMINI_CHAT_COMPRESSION_MODEL.to_string()),
    );
    object.insert("stream".to_string(), serde_json::Value::Bool(false));
    object.insert("store".to_string(), serde_json::Value::Bool(false));
    object.insert(
        "parallel_tool_calls".to_string(),
        serde_json::Value::Bool(false),
    );
    object.insert(
        "prodex_gemini_compaction".to_string(),
        serde_json::Value::Bool(true),
    );
    for key in [
        "include",
        "previous_response_id",
        "prompt_cache_key",
        "text",
        "tool_choice",
        "tools",
    ] {
        object.remove(key);
    }

    serde_json::to_vec(&value)
        .map_err(|err| format!("failed to serialize Gemini semantic compact request: {err}"))
}

pub fn gemini_provider_core_semantic_compact_summary(
    value: &serde_json::Value,
    request_id: u64,
) -> String {
    if let Some(content) = value
        .get("choices")
        .and_then(serde_json::Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        return content.to_string();
    }

    let value = gemini_provider_core_normalized_response_value(value);
    let response = gemini_provider_core_runtime_responses_value(
        &value,
        request_id,
        crate::provider_core_chat_compatible_created_at(),
        crate::PRODEX_GEMINI_DEFAULT_MODEL,
        |_, _| None,
    );
    response
        .get("output")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| item.get("type").and_then(serde_json::Value::as_str) == Some("message"))
        .flat_map(|item| {
            item.get("content")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter(|item| {
            matches!(
                item.get("type").and_then(serde_json::Value::as_str),
                Some("output_text" | "input_text")
            )
        })
        .filter_map(|item| item.get("text").and_then(serde_json::Value::as_str))
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
