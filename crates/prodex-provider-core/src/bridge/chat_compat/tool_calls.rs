//! Chat-compatible tool-call conversion helpers.

pub fn provider_core_chat_compatible_rtk_wrapped_tool_arguments(
    name: &str,
    arguments: &str,
) -> String {
    crate::translators::chat_compatible_rtk_wrapped_tool_arguments(name, arguments)
}

pub fn provider_core_chat_compatible_tool_call_thought_signature(
    tool_call: &serde_json::Value,
) -> Option<String> {
    tool_call
        .get("extra_content")
        .and_then(|value| value.get("google"))
        .and_then(|value| value.get("thought_signature"))
        .and_then(serde_json::Value::as_str)
        .filter(|signature| !signature.trim().is_empty())
        .map(str::to_string)
}

pub fn provider_core_split_flat_namespace_tool_name(name: &str) -> (Option<String>, String) {
    match prodex_mojo_core::chat_tool_name_policy::split_flat_namespace_tool_name(name)
        .expect("compiled Mojo chat tool namespace decoder returned invalid output")
    {
        Some((namespace, tool)) => (Some(namespace.to_string()), tool.to_string()),
        None => (None, name.to_string()),
    }
}

pub(super) fn provider_core_chat_compatible_responses_tool_call_plan(
    tool_call: &serde_json::Value,
    provider_adapter_label: &str,
    fallback_call_id: &mut impl FnMut() -> String,
) -> Result<Option<serde_json::Value>, String> {
    let call_id = tool_call
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(fallback_call_id);
    let function = tool_call
        .get("function")
        .and_then(serde_json::Value::as_object);
    let name = function
        .and_then(|value| value.get("name"))
        .and_then(serde_json::Value::as_str);
    let kind = prodex_mojo_core::chat_tool_name_policy::chat_compatible_tool_call_kind(
        function.is_some(),
        name,
    )
    .expect("compiled Mojo chat tool-call classification returned invalid output");
    use prodex_mojo_core::chat_tool_name_policy::ChatCompatibleToolCallKind;
    match kind {
        ChatCompatibleToolCallKind::InvalidFunction => {
            return Err(format!(
                "{provider_adapter_label} returned a tool call without a function object"
            ));
        }
        ChatCompatibleToolCallKind::InvalidName => {
            return Err(format!(
                "{provider_adapter_label} returned a tool call without a function name"
            ));
        }
        ChatCompatibleToolCallKind::ToolSearch
        | ChatCompatibleToolCallKind::ApplyPatch
        | ChatCompatibleToolCallKind::Function => {}
    }
    let name = name.expect("compiled Mojo validated chat tool-call function name");
    let function = function.expect("compiled Mojo validated chat tool-call function object");
    let arguments = function
        .get("arguments")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("{}");
    provider_core_chat_compatible_validate_tool_call_arguments(
        name,
        arguments,
        provider_adapter_label,
    )?;
    if kind == ChatCompatibleToolCallKind::ToolSearch {
        serde_json::from_str::<serde_json::Value>(arguments).map_err(|error| {
            provider_core_chat_compatible_tool_call_arguments_error(
                name,
                error,
                provider_adapter_label,
            )
        })?;
        return Ok(Some(serde_json::json!({
            "kind": "tool_search",
            "call_id": call_id,
            "arguments": arguments,
        })));
    }
    if kind == ChatCompatibleToolCallKind::ApplyPatch {
        return Ok(Some(serde_json::json!({
            "kind": "apply_patch",
            "call_id": call_id,
            "name": name,
            "input": crate::gemini_bridge::gemini_provider_core_custom_tool_input_from_arguments(arguments),
        })));
    }
    let arguments = provider_core_chat_compatible_rtk_wrapped_tool_arguments(name, arguments);
    let mut plan = serde_json::json!({
        "kind": "function",
        "call_id": call_id,
        "arguments": arguments,
        "name": name,
    });
    if let Some(signature) = provider_core_chat_compatible_tool_call_thought_signature(tool_call) {
        plan["thought_signature"] = serde_json::Value::String(signature);
    }
    Ok(Some(plan))
}

fn provider_core_chat_compatible_validate_tool_call_arguments(
    name: &str,
    arguments: &str,
    provider_adapter_label: &str,
) -> Result<(), String> {
    if arguments.trim().is_empty() {
        return Ok(());
    }
    serde_json::from_str::<serde_json::Value>(arguments)
        .map(|_| ())
        .map_err(|error| {
            provider_core_chat_compatible_tool_call_arguments_error(
                name,
                error,
                provider_adapter_label,
            )
        })
}

fn provider_core_chat_compatible_tool_call_arguments_error(
    name: &str,
    error: serde_json::Error,
    provider_adapter_label: &str,
) -> String {
    format!(
        "{provider_adapter_label} returned malformed JSON arguments for tool call `{name}`: {error}"
    )
}
