//! Responses input content guards for chat-compatible request validation.

use super::super::super::Value;

pub(super) fn responses_input_has_custom_tool_or_tool_search(value: &Value) -> bool {
    match value {
        Value::Array(items) => items
            .iter()
            .any(responses_input_has_custom_tool_or_tool_search),
        Value::Object(obj) => {
            matches!(
                obj.get("type").and_then(Value::as_str),
                Some("custom_tool_call") | Some("tool_search_call")
            ) || obj
                .get("content")
                .is_some_and(responses_input_has_custom_tool_or_tool_search)
        }
        _ => false,
    }
}

pub(super) fn responses_input_has_non_text_content(value: &Value) -> bool {
    match value {
        Value::Array(items) => items.iter().any(responses_input_has_non_text_content),
        Value::Object(obj) => {
            if let Some(item_type) = obj.get("type").and_then(Value::as_str)
                && !matches!(
                    item_type,
                    "message"
                        | "input_text"
                        | "output_text"
                        | "function_call"
                        | "function_call_output"
                )
            {
                return obj.contains_key("text")
                    || obj.contains_key("content")
                    || matches!(item_type, "input_image" | "input_audio");
            }
            obj.get("content")
                .is_some_and(responses_content_has_non_text_parts)
        }
        _ => false,
    }
}

fn responses_content_has_non_text_parts(value: &Value) -> bool {
    match value {
        Value::String(_) => false,
        Value::Array(items) => items.iter().any(|item| {
            let Some(obj) = item.as_object() else {
                return false;
            };
            !matches!(
                obj.get("type").and_then(Value::as_str),
                Some("input_text") | Some("output_text") | Some("text")
            )
        }),
        Value::Object(obj) => !matches!(
            obj.get("type").and_then(Value::as_str),
            Some("input_text") | Some("output_text") | Some("text")
        ),
        _ => false,
    }
}
