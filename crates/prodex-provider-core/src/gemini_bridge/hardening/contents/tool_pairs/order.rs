//! Gemini tool-response ordering repair.

use super::super::parts::{
    gemini_provider_core_content_role, gemini_provider_core_function_calls_in_content,
};

pub(in crate::gemini_bridge::hardening::contents) fn gemini_provider_core_refine_tool_response_order(
    contents: &mut [serde_json::Value],
) {
    for index in 1..contents.len() {
        if gemini_provider_core_content_role(&contents[index]) != Some("user")
            || gemini_provider_core_content_role(&contents[index - 1]) != Some("model")
        {
            continue;
        }
        let order = gemini_provider_core_function_calls_in_content(&contents[index - 1])
            .into_iter()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        if order.is_empty() {
            continue;
        }
        let Some(parts) = contents[index]
            .get_mut("parts")
            .and_then(serde_json::Value::as_array_mut)
        else {
            continue;
        };
        let mut response_parts = Vec::new();
        let mut other_parts = Vec::new();
        for part in parts.drain(..) {
            if part.get("functionResponse").is_some() {
                response_parts.push(part);
            } else {
                other_parts.push(part);
            }
        }
        response_parts.sort_by_key(|part| {
            let id = part
                .get("functionResponse")
                .and_then(|response| response.get("id"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            order
                .iter()
                .position(|candidate| candidate == id)
                .unwrap_or(usize::MAX)
        });
        parts.extend(response_parts);
        parts.extend(other_parts);
    }
}
