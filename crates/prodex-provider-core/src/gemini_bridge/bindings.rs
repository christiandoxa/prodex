//! Gemini response and tool-call binding extraction helpers.

pub fn gemini_provider_core_tool_output_call_ids_from_request(
    value: &serde_json::Value,
) -> Vec<String> {
    value
        .get("input")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_object)
        .filter(|object| {
            matches!(
                object.get("type").and_then(serde_json::Value::as_str),
                Some(
                    "function_call_output"
                        | "custom_tool_call_output"
                        | "mcp_call_output"
                        | "mcp_tool_result",
                )
            )
        })
        .filter_map(|object| {
            ["call_id", "tool_call_id", "id"]
                .into_iter()
                .find_map(|key| {
                    object
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                })
        })
        .filter(|call_id| !call_id.trim().is_empty())
        .collect()
}

pub fn gemini_provider_core_response_id_from_responses_value(
    value: &serde_json::Value,
) -> Option<String> {
    value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .filter(|response_id| !response_id.trim().is_empty())
        .map(str::to_string)
}

pub fn gemini_provider_core_tool_call_ids_from_responses_value(
    value: &serde_json::Value,
) -> Vec<String> {
    value
        .get("output")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_object)
        .filter(|object| {
            object
                .get("type")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|kind| kind == "function_call" || kind == "custom_tool_call")
        })
        .filter_map(|object| {
            object
                .get("call_id")
                .or_else(|| object.get("id"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .filter(|call_id| !call_id.trim().is_empty())
        .collect()
}

pub fn gemini_provider_core_response_bindings_from_body(
    body: &[u8],
) -> Option<(String, Vec<String>)> {
    let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    let response_id =
        gemini_provider_core_response_id_from_responses_value(&value).unwrap_or_default();
    let tool_call_ids = gemini_provider_core_tool_call_ids_from_responses_value(&value);
    (!response_id.is_empty() || !tool_call_ids.is_empty()).then_some((response_id, tool_call_ids))
}
