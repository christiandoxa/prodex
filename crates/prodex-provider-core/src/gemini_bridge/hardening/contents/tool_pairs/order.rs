//! Gemini tool-response ordering repair.

use super::super::parts::{
    gemini_provider_core_content_role, gemini_provider_core_function_calls_in_content,
};
use prodex_mojo_core::provider_constraints::gemini_tool_response_part_order;

pub(in crate::gemini_bridge::hardening::contents) fn gemini_provider_core_refine_tool_response_order(
    contents: &mut [serde_json::Value],
) {
    for index in 1..contents.len() {
        if gemini_provider_core_content_role(&contents[index]) != Some("user")
            || gemini_provider_core_content_role(&contents[index - 1]) != Some("model")
        {
            continue;
        }
        let calls = gemini_provider_core_function_calls_in_content(&contents[index - 1]);
        let call_ids = calls.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>();
        let Some(parts) = contents[index]
            .get_mut("parts")
            .and_then(serde_json::Value::as_array_mut)
        else {
            continue;
        };
        let response_ids = parts
            .iter()
            .map(|part| {
                part.get("functionResponse").map(|response| {
                    response
                        .get("id")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                })
            })
            .collect::<Vec<_>>();
        let order = gemini_tool_response_part_order(&call_ids, &response_ids)
            .expect("Mojo Gemini tool-response ordering should accept Rust-owned views");
        let mut original_parts = std::mem::take(parts)
            .into_iter()
            .map(Some)
            .collect::<Vec<_>>();
        let mut ordered_parts = Vec::with_capacity(original_parts.len());
        for part_index in order {
            ordered_parts.push(
                original_parts[part_index]
                    .take()
                    .expect("Mojo order was validated as a permutation"),
            );
        }
        *parts = ordered_parts;
    }
}

#[cfg(test)]
mod tests {
    use super::gemini_provider_core_refine_tool_response_order;
    use serde_json::json;

    #[test]
    fn rust_caller_keeps_duplicate_unmatched_and_other_part_order() {
        let mut contents = vec![
            json!({"role": "model", "parts": [
                {"functionCall": {"id": "call-a", "name": "tool"}},
                {"functionCall": {"id": "call-b", "name": "tool"}},
                {"functionCall": {"id": "call-a", "name": "tool"}}
            ]}),
            json!({"role": "user", "parts": [
                {"text": "before"},
                {"functionResponse": {"id": "missing-a", "response": {"item": 1}}},
                {"functionResponse": {"id": "call-b", "response": {"item": 2}}},
                {"inlineData": {"mimeType": "image/png", "data": "fixture"}},
                {"functionResponse": {"id": "call-a", "response": {"item": 3}}},
                {"functionResponse": {"id": "call-a", "response": {"item": 4}}},
                {"functionResponse": {"id": "missing-b", "response": {"item": 5}}},
                {"functionResponse": {"response": {"item": 6}}},
                {"text": "after"}
            ]}),
        ];

        gemini_provider_core_refine_tool_response_order(&mut contents);

        assert_eq!(
            contents[1]["parts"],
            json!([
                {"functionResponse": {"id": "call-a", "response": {"item": 3}}},
                {"functionResponse": {"id": "call-a", "response": {"item": 4}}},
                {"functionResponse": {"id": "call-b", "response": {"item": 2}}},
                {"functionResponse": {"id": "missing-a", "response": {"item": 1}}},
                {"functionResponse": {"id": "missing-b", "response": {"item": 5}}},
                {"functionResponse": {"response": {"item": 6}}},
                {"text": "before"},
                {"inlineData": {"mimeType": "image/png", "data": "fixture"}},
                {"text": "after"}
            ])
        );
    }

    #[test]
    fn rust_caller_keeps_parts_unchanged_without_call_ids() {
        let mut contents = vec![
            json!({"role": "model", "parts": [{"text": "no calls"}]}),
            json!({"role": "user", "parts": [
                {"text": "before"},
                {"functionResponse": {"id": "call-a"}},
                {"text": "after"}
            ]}),
        ];

        gemini_provider_core_refine_tool_response_order(&mut contents);

        assert_eq!(
            contents[1]["parts"],
            json!([
                {"text": "before"},
                {"functionResponse": {"id": "call-a"}},
                {"text": "after"}
            ])
        );
    }
}
