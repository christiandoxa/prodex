//! DeepSeek response tool-call normalization helpers.

use crate::translators::tool_args::{rtk_prefixed_noisy_shell_command, wrap_json_string_arg_with};
use serde_json::{Value, json};

pub(crate) fn deepseek_responses_tool_call_item(
    tool_call: &Value,
) -> Result<Option<Value>, String> {
    let call_id = tool_call
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("call_0");
    let Some(function) = tool_call.get("function").and_then(Value::as_object) else {
        return Err("DeepSeek returned a tool call without a function object".to_string());
    };
    let Some(name) = function
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
    else {
        return Err("DeepSeek returned a tool call without a function name".to_string());
    };
    let arguments = function
        .get("arguments")
        .and_then(Value::as_str)
        .unwrap_or("{}");
    deepseek_validate_tool_call_arguments(name, arguments)?;
    if name == "tool_search" {
        let arguments = serde_json::from_str::<Value>(arguments)
            .map_err(|error| deepseek_tool_call_arguments_error(name, error))?;
        return Ok(Some(json!({
            "type": "tool_search_call",
            "call_id": call_id,
            "execution": "client",
            "arguments": arguments,
        })));
    }
    if name == "apply_patch" {
        return Ok(Some(json!({
            "type": "custom_tool_call",
            "call_id": call_id,
            "name": name,
            "input": arguments,
        })));
    }
    let arguments = deepseek_rtk_wrapped_tool_arguments(name, arguments);
    let (namespace, name) = deepseek_split_flat_namespace_tool_name(name);
    let mut item = json!({
        "type":"function_call",
        "call_id": call_id,
        "name": name,
        "arguments": arguments,
    });
    if let Some(namespace) = namespace {
        item["namespace"] = Value::String(namespace);
    }
    if let Some(signature) = deepseek_chat_tool_call_thought_signature(tool_call) {
        item["gemini_thought_signature"] = Value::String(signature);
    }
    Ok(Some(item))
}

fn deepseek_validate_tool_call_arguments(name: &str, arguments: &str) -> Result<(), String> {
    if arguments.trim().is_empty() {
        return Ok(());
    }
    serde_json::from_str::<Value>(arguments)
        .map(|_| ())
        .map_err(|error| deepseek_tool_call_arguments_error(name, error))
}

fn deepseek_tool_call_arguments_error(name: &str, error: serde_json::Error) -> String {
    format!("DeepSeek returned malformed JSON arguments for tool call `{name}`: {error}")
}

fn deepseek_chat_tool_call_thought_signature(tool_call: &Value) -> Option<String> {
    tool_call
        .get("extra_content")
        .and_then(|value| value.get("google"))
        .and_then(|value| value.get("thought_signature"))
        .and_then(Value::as_str)
        .filter(|signature| !signature.trim().is_empty())
        .map(str::to_string)
}

pub(super) fn deepseek_split_flat_namespace_tool_name(name: &str) -> (Option<String>, String) {
    let trimmed = name.trim();
    for separator in ["__", ".", "/"] {
        if let Some((namespace, local)) = trimmed.split_once(separator)
            && !namespace.trim().is_empty()
            && !local.trim().is_empty()
        {
            return (Some(namespace.trim().to_string()), local.trim().to_string());
        }
    }
    (None, trimmed.to_string())
}

pub(crate) fn deepseek_rtk_wrapped_tool_arguments(name: &str, arguments: &str) -> String {
    if !matches!(name, "shell" | "exec" | "functions.exec_command") || arguments.trim().is_empty() {
        return arguments.to_string();
    }
    wrap_json_string_arg_with(arguments, &["cmd"], rtk_prefixed_noisy_shell_command)
}
