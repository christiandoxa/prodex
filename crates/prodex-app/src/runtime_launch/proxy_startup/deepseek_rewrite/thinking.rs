pub(super) fn runtime_deepseek_normalize_thinking_tool_call_messages(
    messages: &mut [serde_json::Value],
) {
    for message in messages {
        let is_assistant =
            message.get("role").and_then(serde_json::Value::as_str) == Some("assistant");
        let has_tool_calls = message
            .get("tool_calls")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|tool_calls| !tool_calls.is_empty());
        if !is_assistant || !has_tool_calls {
            continue;
        }
        if message
            .get("reasoning_content")
            .and_then(serde_json::Value::as_str)
            .is_none()
        {
            message["reasoning_content"] = serde_json::Value::String(String::new());
        }
        if message
            .get("content")
            .is_none_or(serde_json::Value::is_null)
        {
            message["content"] = serde_json::Value::String(String::new());
        }
    }
}
