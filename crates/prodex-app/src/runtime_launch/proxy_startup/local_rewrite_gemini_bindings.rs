use super::gemini_sse::RuntimeGeminiBindingRecorder;

pub(super) fn runtime_gemini_tool_output_call_ids_from_request(
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
                Some("function_call_output" | "mcp_call_output" | "mcp_tool_result")
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

pub(super) fn runtime_gemini_remember_bindings_from_responses_body(
    recorder: Option<&RuntimeGeminiBindingRecorder>,
    body: &[u8],
) {
    let Some(recorder) = recorder else {
        return;
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return;
    };
    let response_id = value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let tool_call_ids = runtime_gemini_tool_call_ids_from_responses_value(&value);
    if !response_id.trim().is_empty() || !tool_call_ids.is_empty() {
        recorder(response_id, tool_call_ids);
    }
}

fn runtime_gemini_tool_call_ids_from_responses_value(value: &serde_json::Value) -> Vec<String> {
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
                .is_some_and(|kind| kind == "function_call")
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
