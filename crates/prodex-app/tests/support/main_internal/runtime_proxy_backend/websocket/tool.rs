pub(super) fn request_function_call_output_label(request: &str) -> (String, String) {
    serde_json::from_str::<serde_json::Value>(request)
        .ok()
        .and_then(|body_json| {
            body_json
                .get("input")
                .and_then(serde_json::Value::as_array)
                .and_then(|input| input.first())
                .map(|item| {
                    let call_id = item
                        .get("call_id")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("call_missing");
                    let item_label = item
                        .get("type")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("tool_call_output")
                        .replace('_', " ");
                    (call_id.to_string(), item_label)
                })
        })
        .unwrap_or_else(|| ("call_missing".to_string(), "tool call output".to_string()))
}
