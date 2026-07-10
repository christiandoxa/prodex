//! DeepSeek request input reconstruction helpers.

use crate::{
    deepseek_provider_core_history_has_system_message, deepseek_provider_core_message_signatures,
    deepseek_provider_core_push_message_from_responses_item, deepseek_provider_core_system_message,
    deepseek_provider_core_tool_call_ids, deepseek_provider_core_tool_output_call_ids,
    deepseek_provider_core_user_message, deepseek_provider_core_validate_supported_input_item,
};

pub fn deepseek_provider_core_messages_from_responses_request(
    value: &serde_json::Value,
    history_messages: &[serde_json::Value],
    gemini_compat: bool,
    provider_label: &str,
) -> Result<Option<Vec<serde_json::Value>>, String> {
    let mut messages = Vec::new();
    messages.extend(history_messages.iter().cloned());

    if let Some(instructions) = value
        .get("instructions")
        .and_then(serde_json::Value::as_str)
        && !instructions.trim().is_empty()
        && !deepseek_provider_core_history_has_system_message(history_messages, instructions)
    {
        messages.insert(0, deepseek_provider_core_system_message(instructions));
    }

    let replayed_tool_call_ids = deepseek_provider_core_tool_call_ids(history_messages);
    let replayed_tool_output_call_ids =
        deepseek_provider_core_tool_output_call_ids(history_messages);
    let replayed_message_signatures = deepseek_provider_core_message_signatures(history_messages);

    match value.get("input") {
        Some(serde_json::Value::String(text)) => {
            messages.push(deepseek_provider_core_user_message(text));
        }
        Some(serde_json::Value::Array(items)) => {
            for item in items {
                if let Some(object) = item.as_object() {
                    let item = serde_json::Value::Object(object.clone());
                    deepseek_provider_core_validate_supported_input_item(
                        &item,
                        gemini_compat,
                        provider_label,
                    )?;
                    deepseek_provider_core_push_message_from_responses_item(
                        &item,
                        &mut messages,
                        &replayed_tool_call_ids,
                        &replayed_tool_output_call_ids,
                        &replayed_message_signatures,
                    );
                } else {
                    return Err(format!("{provider_label} input items must be objects"));
                }
            }
        }
        Some(_) => {
            return Err(format!(
                "{provider_label} input must be a string or array of input items"
            ));
        }
        None => {}
    }

    if messages.is_empty() {
        Ok(None)
    } else {
        Ok(Some(messages))
    }
}
