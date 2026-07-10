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
    if let Some((namespace, tool_name)) = name.rsplit_once("--")
        && !namespace.trim().is_empty()
        && !tool_name.trim().is_empty()
    {
        return (Some(namespace.to_string()), tool_name.to_string());
    }
    let Some(rest) = name.strip_prefix("mcp__") else {
        return (None, name.to_string());
    };
    let Some(index) = rest.rfind("__") else {
        return (None, name.to_string());
    };
    let namespace = format!("mcp__{}", &rest[..index]);
    let tool_name = rest[index + 2..].to_string();
    if tool_name.trim().is_empty() {
        return (None, name.to_string());
    }
    (Some(namespace), tool_name)
}

pub(super) fn provider_core_chat_compatible_responses_tool_call_item(
    tool_call: &serde_json::Value,
    provider_adapter_label: &str,
) -> Result<Option<serde_json::Value>, String> {
    let call_id = tool_call
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("call_0");
    let Some(function) = tool_call
        .get("function")
        .and_then(serde_json::Value::as_object)
    else {
        return Err(format!(
            "{provider_adapter_label} returned a tool call without a function object"
        ));
    };
    let Some(name) = function
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.trim().is_empty())
    else {
        return Err(format!(
            "{provider_adapter_label} returned a tool call without a function name"
        ));
    };
    let arguments = function
        .get("arguments")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("{}");
    provider_core_chat_compatible_validate_tool_call_arguments(
        name,
        arguments,
        provider_adapter_label,
    )?;
    if name == "tool_search" {
        let arguments = serde_json::from_str::<serde_json::Value>(arguments).map_err(|error| {
            provider_core_chat_compatible_tool_call_arguments_error(
                name,
                error,
                provider_adapter_label,
            )
        })?;
        return Ok(Some(serde_json::json!({
            "type": "tool_search_call",
            "call_id": call_id,
            "execution": "client",
            "arguments": arguments,
        })));
    }
    if name == "apply_patch" {
        return Ok(Some(serde_json::json!({
            "type": "custom_tool_call",
            "call_id": call_id,
            "name": name,
            "input": crate::gemini_bridge::gemini_provider_core_custom_tool_input_from_arguments(arguments),
        })));
    }
    let arguments = provider_core_chat_compatible_rtk_wrapped_tool_arguments(name, arguments);
    let (namespace, name) = provider_core_split_flat_namespace_tool_name(name);
    let mut item = serde_json::json!({
        "type": "function_call",
        "call_id": call_id,
        "name": name,
        "arguments": arguments,
    });
    if let Some(namespace) = namespace {
        item["namespace"] = serde_json::Value::String(namespace);
    }
    if let Some(signature) = provider_core_chat_compatible_tool_call_thought_signature(tool_call) {
        item["gemini_thought_signature"] = serde_json::Value::String(signature);
    }
    Ok(Some(item))
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
