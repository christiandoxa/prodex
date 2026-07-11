//! Custom/freeform Responses tool translation.

use super::super::util::provider_core_json_string;

pub(super) fn provider_core_custom_tool_from_responses_tool(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<serde_json::Value> {
    if object.get("type").and_then(serde_json::Value::as_str) != Some("custom") {
        return None;
    }
    let name =
        provider_core_json_string(object, &["name"]).filter(|name| !name.trim().is_empty())?;
    let description = provider_core_json_string(object, &["description"])
        .filter(|description| !description.trim().is_empty())
        .unwrap_or_else(|| "Freeform custom tool input.".to_string());
    let input_hint = if name == "apply_patch" {
        "Call this custom/freeform tool with the exact Codex apply_patch grammar in the `input` string field. The first line must be `*** Begin Patch`, the last line must be `*** End Patch`, and hunks must use `*** Add File:`, `*** Delete File:`, or `*** Update File:`. For `*** Add File: path`, every new file content line must start with `+`; for example `+hello`, and a blank content line is `+`. Do not pass unified diff headers such as `--- a/...` or `+++ b/...` as the top-level input."
    } else {
        "Call this custom/freeform tool with the exact raw tool input in the `input` string field."
    };
    let input_description = if name == "apply_patch" {
        "Exact raw apply_patch input. For Add File, prefix every new file content line with '+'."
    } else {
        "Exact raw input for the custom/freeform tool."
    };
    let format_hint = object
        .get("format")
        .and_then(|format| serde_json::to_string(format).ok())
        .map(|format| format!("\n\nOriginal custom tool format JSON: {format}"))
        .unwrap_or_default();
    Some(serde_json::json!({
        "type": "function",
        "function": {
            "name": name,
            "description": format!("{description}\n\n{input_hint}{format_hint}"),
            "parameters": {
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": input_description
                    }
                },
                "required": ["input"],
                "additionalProperties": false
            }
        }
    }))
}
