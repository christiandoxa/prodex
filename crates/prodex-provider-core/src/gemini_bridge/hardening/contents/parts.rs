//! Gemini content part inspection helpers.

pub(super) fn gemini_provider_core_content_role(content: &serde_json::Value) -> Option<&str> {
    content.get("role").and_then(serde_json::Value::as_str)
}

pub(super) fn gemini_provider_core_function_calls_in_content(
    content: &serde_json::Value,
) -> Vec<(String, String)> {
    gemini_provider_core_parts_in_content(content)
        .into_iter()
        .filter_map(|part| {
            let function_call = part.get("functionCall")?;
            Some((
                function_call
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                function_call
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
            ))
        })
        .collect()
}

pub(super) fn gemini_provider_core_function_responses_in_content(
    content: &serde_json::Value,
) -> Vec<(String, String)> {
    gemini_provider_core_parts_in_content(content)
        .into_iter()
        .filter_map(|part| {
            let function_response = part.get("functionResponse")?;
            Some((
                function_response
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                function_response
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
            ))
        })
        .collect()
}

fn gemini_provider_core_parts_in_content(content: &serde_json::Value) -> Vec<&serde_json::Value> {
    content
        .get("parts")
        .and_then(serde_json::Value::as_array)
        .map(|parts| parts.iter().collect())
        .unwrap_or_default()
}
