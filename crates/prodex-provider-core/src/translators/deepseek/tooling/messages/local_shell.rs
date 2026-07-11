//! DeepSeek local-shell input call shaping.

use serde_json::{Value, json};

pub(super) fn deepseek_input_local_shell_call_message(item: &Value) -> Option<Value> {
    let call_id = item
        .get("call_id")
        .or_else(|| item.get("tool_call_id"))
        .or_else(|| item.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("call_1");
    let command = item
        .get("action")
        .and_then(|action| action.get("command"))
        .and_then(Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|command| !command.trim().is_empty())
        .or_else(|| {
            item.get("command")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .filter(|command| !command.trim().is_empty())?;
    let mut shell_arguments = serde_json::Map::new();
    shell_arguments.insert("command".to_string(), Value::String(command));
    deepseek_copy_shell_argument(item, &mut shell_arguments, "cwd");
    deepseek_copy_shell_argument(item, &mut shell_arguments, "timeout");
    deepseek_copy_shell_argument(item, &mut shell_arguments, "env");
    let arguments = serde_json::to_string(&Value::Object(shell_arguments)).ok()?;
    Some(json!({
        "role": "assistant",
        "content": "",
        "tool_calls": [{
            "id": call_id,
            "type": "function",
            "function": {
                "name": "shell_command",
                "arguments": arguments,
            },
        }],
    }))
}

fn deepseek_copy_shell_argument(
    item: &Value,
    arguments: &mut serde_json::Map<String, Value>,
    key: &str,
) {
    if let Some(value) = item
        .get(key)
        .or_else(|| item.get("action").and_then(|action| action.get(key)))
    {
        arguments.insert(key.to_string(), value.clone());
    }
}
