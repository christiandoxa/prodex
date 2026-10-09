//! DeepSeek Responses replay/history inspection helpers.

use std::collections::BTreeSet;

fn deepseek_provider_core_history_summary(
    history: &[serde_json::Value],
) -> serde_json::Map<String, serde_json::Value> {
    let source = serde_json::to_string(history).expect("DeepSeek history serializes");
    let mut input = prodex_mojo_core::rich::DeepSeekKernelInput::new(
        prodex_mojo_core::rich::DeepSeekKernelOperation::ResponsesHistorySummary,
    );
    input.messages = Some(&source);
    let output = prodex_mojo_core::rich::deepseek_kernel(input)
        .expect("Mojo DeepSeek history summary returned invalid output");
    serde_json::from_slice::<serde_json::Value>(&output)
        .expect("Mojo DeepSeek history summary returned invalid JSON")
        .as_object()
        .cloned()
        .expect("Mojo DeepSeek history summary returned a non-object")
}

pub fn deepseek_provider_core_chat_role(role: &str) -> &str {
    let mut input = prodex_mojo_core::rich::DeepSeekKernelInput::new(
        prodex_mojo_core::rich::DeepSeekKernelOperation::ChatRole,
    );
    input.role = Some(role);
    let output = prodex_mojo_core::rich::deepseek_kernel(input)
        .expect("Mojo DeepSeek role classifier returned invalid output");
    match serde_json::from_slice::<String>(&output)
        .expect("Mojo DeepSeek role classifier returned invalid JSON")
        .as_str()
    {
        "assistant" => "assistant",
        "system" => "system",
        "tool" => "tool",
        _ => "user",
    }
}

pub fn deepseek_provider_core_history_has_system_message(
    history: &[serde_json::Value],
    content: &str,
) -> bool {
    deepseek_provider_core_history_summary(history)
        .get("system_messages")
        .cloned()
        .and_then(|messages| messages.as_array().cloned())
        .is_some_and(|messages| {
            messages
                .iter()
                .any(|message| message.as_str() == Some(content))
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
    deepseek_provider_core_history_summary(history)
        .get("tool_call_ids")
        .cloned()
        .and_then(|ids| ids.as_array().cloned())
        .map(|ids| {
            ids.into_iter()
                .filter_map(|id| id.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

pub fn deepseek_provider_core_tool_output_call_ids(
    history: &[serde_json::Value],
) -> BTreeSet<String> {
    deepseek_provider_core_history_summary(history)
        .get("tool_output_call_ids")
        .cloned()
        .and_then(|ids| ids.as_array().cloned())
        .map(|ids| {
            ids.into_iter()
                .filter_map(|id| id.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

pub fn deepseek_provider_core_message_signatures(
    history: &[serde_json::Value],
) -> BTreeSet<(String, String)> {
    deepseek_provider_core_history_summary(history)
        .get("signatures")
        .cloned()
        .and_then(|signatures| signatures.as_array().cloned())
        .map(|signatures| {
            signatures
                .into_iter()
                .filter_map(|signature| {
                    let object = signature.as_object()?;
                    Some((
                        object.get("role")?.as_str()?.to_string(),
                        object.get("content")?.as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}
