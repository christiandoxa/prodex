//! DeepSeek Responses replay/history inspection helpers.

use std::collections::BTreeSet;

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
    let input_value = value.get("input")?;
    let input_json = serde_json::to_string(input_value).ok()?;
    let mut input = prodex_mojo_core::rich::DeepSeekKernelInput::new(
        prodex_mojo_core::rich::DeepSeekKernelOperation::ResponsesHistoryCallId,
    );
    input.input = Some(&input_json);
    let output = prodex_mojo_core::rich::deepseek_kernel(input).ok()?;
    serde_json::from_slice(&output).ok().flatten()
}

pub fn deepseek_provider_core_history_has_tool_call(
    history: &[serde_json::Value],
    call_id: &str,
) -> bool {
    let Ok(call_id_json) = serde_json::to_string(call_id) else {
        return false;
    };
    let Ok(history_json) = serde_json::to_string(history) else {
        return false;
    };
    let mut input = prodex_mojo_core::rich::DeepSeekKernelInput::new(
        prodex_mojo_core::rich::DeepSeekKernelOperation::ResponsesHistoryContainsCallId,
    );
    input.call_id = Some(&call_id_json);
    input.messages = Some(&history_json);
    prodex_mojo_core::rich::deepseek_kernel(input)
        .ok()
        .and_then(|output| serde_json::from_slice::<bool>(&output).ok())
        .unwrap_or(false)
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
