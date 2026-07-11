pub(crate) use self::response_tool_calls_apply_patch::gemini_custom_apply_patch_input;
use serde_json::{Value, json};

#[path = "response_tool_calls/chat.rs"]
mod chat;
mod response_tool_calls_apply_patch;
#[path = "response_tool_calls/rtk.rs"]
mod rtk;

pub(crate) use self::chat::gemini_chat_assistant_tool_call_item_with_call_id;
use self::rtk::gemini_rtk_wrapped_tool_arguments;

pub(super) fn gemini_response_tool_call_item(part: &Value, function_call: &Value) -> Value {
    gemini_response_tool_call_item_with_call_id(part, function_call, None)
}

pub(crate) fn gemini_response_tool_call_added_item_with_call_id(
    part: &Value,
    function_call: &Value,
    call_id_override: Option<&str>,
) -> Option<Value> {
    let call_id = function_call
        .get("id")
        .and_then(Value::as_str)
        .or(call_id_override)
        .unwrap_or("call_1");
    let flat_name = function_call
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("tool_call");
    if matches!(flat_name, "tool_search" | "apply_patch") {
        return None;
    }
    let (namespace, name) = gemini_split_flat_namespace_tool_name(flat_name);
    let mut item = json!({
        "type": "function_call",
        "call_id": call_id,
        "name": name,
    });
    if let Some(namespace) = namespace {
        item["namespace"] = Value::String(namespace);
    }
    if let Some(signature) = part
        .get("thoughtSignature")
        .and_then(Value::as_str)
        .or_else(|| {
            function_call
                .get("thoughtSignature")
                .and_then(Value::as_str)
        })
    {
        item["gemini_thought_signature"] = Value::String(signature.to_string());
    }
    Some(item)
}

pub(crate) fn gemini_response_tool_call_raw_item_with_call_id(
    part: &Value,
    flat_name: &str,
    arguments: &str,
    call_id_override: Option<&str>,
) -> Value {
    let call_id = call_id_override.unwrap_or("call_1");
    let (namespace, name) = gemini_split_flat_namespace_tool_name(flat_name);
    let mut item = json!({
        "type": "function_call",
        "call_id": call_id,
        "name": name,
        "arguments": arguments,
    });
    if let Some(namespace) = namespace {
        item["namespace"] = Value::String(namespace);
    }
    if let Some(signature) = part.get("thoughtSignature").and_then(Value::as_str) {
        item["gemini_thought_signature"] = Value::String(signature.to_string());
    }
    item
}

pub(crate) fn gemini_response_tool_call_item_with_call_id(
    part: &Value,
    function_call: &Value,
    call_id_override: Option<&str>,
) -> Value {
    let call_id = function_call
        .get("id")
        .and_then(Value::as_str)
        .or(call_id_override)
        .unwrap_or("call_1");
    let flat_name = function_call
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("tool_call");
    let args_value = function_call
        .get("args")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if flat_name == "tool_search" {
        return json!({
            "type": "tool_search_call",
            "call_id": call_id,
            "execution": "client",
            "arguments": args_value,
        });
    }
    if let Some(item) = gemini_custom_tool_call_item(call_id, flat_name, &args_value) {
        return item;
    }
    let args = serde_json::to_string(&args_value).unwrap_or_else(|_| "{}".to_string());
    let (namespace, name) = gemini_split_flat_namespace_tool_name(flat_name);
    let mut item = json!({
        "type": "function_call",
        "call_id": call_id,
        "name": name,
        "arguments": gemini_rtk_wrapped_tool_arguments(flat_name, &args),
    });
    if let Some(namespace) = namespace {
        item["namespace"] = Value::String(namespace);
    }
    if let Some(signature) = part
        .get("thoughtSignature")
        .and_then(Value::as_str)
        .or_else(|| {
            function_call
                .get("thoughtSignature")
                .and_then(Value::as_str)
        })
    {
        item["gemini_thought_signature"] = Value::String(signature.to_string());
    }
    item
}

fn gemini_custom_tool_call_item(
    call_id: &str,
    flat_name: &str,
    args_value: &Value,
) -> Option<Value> {
    if flat_name != "apply_patch" {
        return None;
    }
    Some(json!({
        "type": "custom_tool_call",
        "call_id": call_id,
        "name": flat_name,
        "input": gemini_custom_apply_patch_input(args_value),
    }))
}

fn gemini_split_flat_namespace_tool_name(name: &str) -> (Option<String>, String) {
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
