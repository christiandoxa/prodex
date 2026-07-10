//! DeepSeek Responses replay/history inspection helpers.

use std::collections::BTreeSet;

use super::super::deepseek_provider_core_json_string;

pub fn deepseek_provider_core_chat_role(role: &str) -> &str {
    match role {
        "assistant" | "system" | "tool" => role,
        "developer" => "system",
        _ => "user",
    }
}

pub fn deepseek_provider_core_history_has_system_message(
    history: &[serde_json::Value],
    content: &str,
) -> bool {
    history.iter().any(|message| {
        message.get("role").and_then(serde_json::Value::as_str) == Some("system")
            && message.get("content").and_then(serde_json::Value::as_str) == Some(content)
    })
}

pub fn deepseek_provider_core_first_function_call_output_call_id(
    value: &serde_json::Value,
) -> Option<String> {
    value
        .get("input")?
        .as_array()?
        .iter()
        .filter_map(serde_json::Value::as_object)
        .find_map(|object| {
            matches!(
                object.get("type").and_then(serde_json::Value::as_str),
                Some(
                    "function_call_output"
                        | "custom_tool_call_output"
                        | "mcp_call_output"
                        | "mcp_tool_result",
                )
            )
            .then(|| deepseek_provider_core_json_string(object, &["call_id", "tool_call_id", "id"]))
            .flatten()
        })
        .filter(|call_id| !call_id.trim().is_empty())
}

pub fn deepseek_provider_core_history_has_tool_call(
    history: &[serde_json::Value],
    call_id: &str,
) -> bool {
    history.iter().any(|message| {
        message
            .get("tool_calls")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|tool_calls| {
                tool_calls.iter().any(|tool_call| {
                    tool_call.get("id").and_then(serde_json::Value::as_str) == Some(call_id)
                })
            })
    })
}

pub fn deepseek_provider_core_tool_call_ids(history: &[serde_json::Value]) -> BTreeSet<String> {
    history
        .iter()
        .filter_map(|message| {
            message
                .get("tool_calls")
                .and_then(serde_json::Value::as_array)
        })
        .flat_map(|tool_calls| tool_calls.iter())
        .filter_map(|tool_call| tool_call.get("id").and_then(serde_json::Value::as_str))
        .filter(|call_id| !call_id.trim().is_empty())
        .map(str::to_string)
        .collect()
}

pub fn deepseek_provider_core_tool_output_call_ids(
    history: &[serde_json::Value],
) -> BTreeSet<String> {
    history
        .iter()
        .filter(|message| message.get("role").and_then(serde_json::Value::as_str) == Some("tool"))
        .filter_map(|message| {
            message
                .get("tool_call_id")
                .and_then(serde_json::Value::as_str)
        })
        .filter(|call_id| !call_id.trim().is_empty())
        .map(str::to_string)
        .collect()
}

pub fn deepseek_provider_core_message_signatures(
    history: &[serde_json::Value],
) -> BTreeSet<(String, String)> {
    history
        .iter()
        .filter_map(|message| {
            let role = deepseek_provider_core_chat_role(
                message.get("role").and_then(serde_json::Value::as_str)?,
            );
            let content = message.get("content").and_then(serde_json::Value::as_str)?;
            (!content.trim().is_empty()).then(|| (role.to_string(), content.to_string()))
        })
        .collect()
}
