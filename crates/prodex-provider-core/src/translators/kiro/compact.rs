//! Kiro remote compact request/response helpers.

use serde_json::{Value, json};

const KIRO_PROVIDER_CORE_SEMANTIC_COMPACT_INSTRUCTIONS: &str = "\
Compact the supplied coding-agent transcript into one durable continuation summary. \
Preserve the user's goals, repository instructions, decisions, files changed, exact identifiers, \
commands and test results, unresolved failures, current worktree state, and the next concrete steps. \
Remove redundant narration and obsolete intermediate reasoning. Do not call tools. \
Return only the continuation summary, with no preamble or completion claim.";

pub fn kiro_provider_core_semantic_compact_instructions() -> &'static str {
    KIRO_PROVIDER_CORE_SEMANTIC_COMPACT_INSTRUCTIONS
}

pub fn kiro_provider_core_semantic_compact_request_body(body: &[u8]) -> Result<Vec<u8>, String> {
    let mut value = serde_json::from_slice::<Value>(body)
        .map_err(|err| format!("failed to parse Kiro compact request JSON: {err}"))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| "Kiro compact request must be a JSON object".to_string())?;
    let input = object
        .get_mut("input")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "Kiro compact request must contain an input array".to_string())?;
    input.push(json!({
        "type": "message",
        "role": "user",
        "content": [{
            "type": "input_text",
            "text": KIRO_PROVIDER_CORE_SEMANTIC_COMPACT_INSTRUCTIONS,
        }],
    }));
    object.insert("stream".to_string(), Value::Bool(false));
    object.insert("store".to_string(), Value::Bool(false));
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
        .map_err(|err| format!("failed to serialize Kiro compact request: {err}"))
}

pub fn kiro_provider_core_compact_summary_from_response(
    response: &Value,
) -> Result<String, String> {
    let output = response
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(|| "Kiro compact response is missing output".to_string())?;
    let summary = output
        .iter()
        .find(|item| item.get("type").and_then(Value::as_str) == Some("message"))
        .and_then(|item| item.get("content"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find_map(|item| item.get("text").and_then(Value::as_str))
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| "Kiro compact response returned no summary text".to_string())?;
    Ok(summary.to_string())
}
