//! Gemini tool-call/tool-response sequence repair.

mod order;

pub(super) use self::order::gemini_provider_core_refine_tool_response_order;

use super::parts::{
    gemini_provider_core_content_role, gemini_provider_core_function_calls_in_content,
    gemini_provider_core_function_responses_in_content,
};

pub(super) fn gemini_provider_core_pair_tool_calls(contents: &mut Vec<serde_json::Value>) {
    let mut index = 0;
    while index < contents.len() {
        if gemini_provider_core_content_role(&contents[index]) != Some("model") {
            index += 1;
            continue;
        }
        let calls = gemini_provider_core_function_calls_in_content(&contents[index]);
        if calls.is_empty() {
            index += 1;
            continue;
        }
        let existing = contents
            .get(index + 1)
            .filter(|content| gemini_provider_core_content_role(content) == Some("user"))
            .map(gemini_provider_core_function_responses_in_content)
            .unwrap_or_default();
        let missing = calls
            .into_iter()
            .filter(|call| !existing.iter().any(|response| response == call))
            .collect::<Vec<_>>();
        if missing.is_empty() {
            index += 1;
            continue;
        }
        let sentinel_parts = missing
            .into_iter()
            .map(|(id, name)| {
                serde_json::json!({
                    "functionResponse": {
                        "id": id,
                        "name": name,
                        "response": {
                            "error": "The tool execution result was missing from the Codex conversation history. Treat this tool result as unavailable and recover by inspecting current workspace state before continuing."
                        }
                    }
                })
            })
            .collect::<Vec<_>>();
        if let Some(next) = contents.get_mut(index + 1)
            && gemini_provider_core_content_role(next) == Some("user")
            && let Some(parts) = next
                .get_mut("parts")
                .and_then(serde_json::Value::as_array_mut)
        {
            parts.extend(sentinel_parts);
        } else {
            contents.insert(
                index + 1,
                serde_json::json!({
                    "role": "user",
                    "parts": sentinel_parts,
                }),
            );
        }
        index += 2;
    }
}

pub(super) fn gemini_provider_core_repair_orphan_tool_responses(
    contents: &mut Vec<serde_json::Value>,
) {
    let mut index = 0;
    while index < contents.len() {
        if gemini_provider_core_content_role(&contents[index]) != Some("user") {
            index += 1;
            continue;
        }
        let responses = gemini_provider_core_function_responses_in_content(&contents[index]);
        if responses.is_empty() {
            index += 1;
            continue;
        }
        let previous_calls = index
            .checked_sub(1)
            .and_then(|previous| contents.get(previous))
            .filter(|content| gemini_provider_core_content_role(content) == Some("model"))
            .map(gemini_provider_core_function_calls_in_content)
            .unwrap_or_default();
        let orphaned = responses
            .into_iter()
            .filter(|response| !previous_calls.iter().any(|call| call == response))
            .collect::<Vec<_>>();
        if orphaned.is_empty() {
            index += 1;
            continue;
        }
        let synthetic_parts = orphaned
            .into_iter()
            .map(|(id, name)| {
                serde_json::json!({
                    "functionCall": {
                        "id": id,
                        "name": name,
                        "args": {}
                    },
                    "thoughtSignature": "skip_thought_signature_validator"
                })
            })
            .collect::<Vec<_>>();
        if index > 0 && gemini_provider_core_content_role(&contents[index - 1]) == Some("model") {
            if let Some(parts) = contents[index - 1]
                .get_mut("parts")
                .and_then(serde_json::Value::as_array_mut)
            {
                parts.extend(synthetic_parts);
            }
        } else {
            contents.insert(
                index,
                serde_json::json!({
                    "role": "model",
                    "parts": synthetic_parts,
                }),
            );
            index += 1;
        }
        index += 1;
    }
}
