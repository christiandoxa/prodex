pub fn runtime_anthropic_shell_tool_input_from_output_item(
    item: &serde_json::Value,
) -> serde_json::Value {
    let mut input = serde_json::Map::new();
    if let Some(action) = item.get("action").and_then(serde_json::Value::as_object) {
        let commands = action
            .get("commands")
            .and_then(serde_json::Value::as_array)
            .map(|commands| {
                commands
                    .iter()
                    .filter_map(|command| {
                        command
                            .as_str()
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(|value| serde_json::Value::String(value.to_string()))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !commands.is_empty() {
            let command_text = commands
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
                .join("\n");
            input.insert(
                "command".to_string(),
                serde_json::Value::String(command_text),
            );
        } else if let Some(command) = action
            .get("command")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            input.insert(
                "command".to_string(),
                serde_json::Value::String(command.to_string()),
            );
        }
        if let Some(timeout_ms) = action.get("timeout_ms").and_then(serde_json::Value::as_u64) {
            input.insert(
                "timeout_ms".to_string(),
                serde_json::Value::Number(timeout_ms.into()),
            );
        }
        if let Some(max_output_length) = action
            .get("max_output_length")
            .and_then(serde_json::Value::as_u64)
        {
            input.insert(
                "max_output_length".to_string(),
                serde_json::Value::Number(max_output_length.into()),
            );
        }
    }
    serde_json::Value::Object(input)
}

pub fn runtime_anthropic_shell_tool_use_block_from_output_item(
    item: &serde_json::Value,
) -> serde_json::Value {
    let call_id = item
        .get("call_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("shell_call");
    serde_json::json!({
        "type": "tool_use",
        "id": call_id,
        "name": "bash",
        "input": runtime_anthropic_shell_tool_input_from_output_item(item),
    })
}
