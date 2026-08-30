//! DeepSeek request input/message conversion helpers.

#[path = "messages/chat_items.rs"]
mod chat_items;
#[path = "messages/input_tool_calls.rs"]
mod input_tool_calls;
#[path = "messages/local_shell.rs"]
mod local_shell;
#[path = "messages/thought_signature.rs"]
mod thought_signature;

use crate::deepseek_provider_core_responses_content_text;
use chat_items::{deepseek_message_content_text, deepseek_message_tool_calls};
use input_tool_calls::{
    deepseek_input_custom_tool_call_message, deepseek_input_function_call_message,
    deepseek_input_mcp_call_messages,
};
use local_shell::deepseek_input_local_shell_call_message;
use serde_json::Value;
#[cfg(not(feature = "mojo"))]
use serde_json::json;

#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::{DeepSeekKernelInput, DeepSeekKernelOperation};
pub(crate) use thought_signature::deepseek_tool_call_thought_signature_object;

#[cfg(feature = "mojo")]
fn deepseek_mojo_message(
    role: &str,
    content: &str,
    tool_calls: Option<&str>,
    call_id: Option<&str>,
) -> Value {
    let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::Message);
    input.role = Some(role);
    input.content = Some(content);
    input.tool_calls = tool_calls;
    input.call_id = call_id;
    super::super::deepseek_mojo_value(input)
}

fn deepseek_message(role: &str, content: &str) -> Value {
    #[cfg(feature = "mojo")]
    return deepseek_mojo_message(role, content, None, None);
    #[cfg(not(feature = "mojo"))]
    json!({"role": role, "content": content})
}

pub(super) fn deepseek_tool_call_message(
    call_id: &str,
    name: &str,
    arguments: &str,
    signature: Option<&str>,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::ToolCallMessage);
        input.call_id = Some(call_id);
        input.name = Some(name);
        input.arguments = Some(arguments);
        input.signature = signature;
        return super::super::deepseek_mojo_value(input);
    }
    #[cfg(not(feature = "mojo"))]
    {
        let mut tool_call = json!({
            "id": call_id,
            "type": "function",
            "function": {
                "name": name,
                "arguments": arguments,
            },
        });
        if let Some(signature) = signature
            && let Some(tool_call_object) = tool_call.as_object_mut()
        {
            tool_call_object.insert(
                "gemini_thought_signature".to_string(),
                Value::String(signature.to_string()),
            );
        }
        json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [tool_call],
        })
    }
}

pub(super) fn deepseek_tool_message(call_id: &str, content: &str) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::ToolMessage);
        input.call_id = Some(call_id);
        input.content = Some(content);
        return super::super::deepseek_mojo_value(input);
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "role": "tool",
        "tool_call_id": call_id,
        "content": content,
    })
}

pub(super) fn deepseek_raw_tool_message(call_id: &str, content: &Value) -> Value {
    #[cfg(feature = "mojo")]
    {
        let content = serde_json::to_string(content).expect("DeepSeek tool output serializes");
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::ToolMessage);
        input.call_id = Some(call_id);
        input.input = Some(&content);
        return super::super::deepseek_mojo_value(input);
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "role": "tool",
        "tool_call_id": call_id,
        "content": content,
    })
}

fn deepseek_empty_assistant_message() -> Value {
    deepseek_message("assistant", "")
}

pub(crate) fn deepseek_messages_from_request(value: &Value) -> Vec<Value> {
    let mut messages = if let Some(messages) = value.get("messages").and_then(Value::as_array) {
        messages.clone()
    } else if let Some(input) = value.get("input") {
        match input {
            Value::String(text) => vec![deepseek_message("user", text)],
            Value::Array(items) => items
                .iter()
                .flat_map(deepseek_messages_from_input_item)
                .collect(),
            _ => vec![deepseek_message("user", "")],
        }
    } else {
        vec![deepseek_message("user", "")]
    };
    if let Some(instructions) = value
        .get("instructions")
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
    {
        messages.insert(0, deepseek_message("system", instructions));
    }
    messages
}

fn deepseek_messages_from_input_item(item: &Value) -> Vec<Value> {
    match item.get("type").and_then(Value::as_str) {
        Some("function_call") => {
            return vec![
                deepseek_input_function_call_message(item)
                    .unwrap_or_else(deepseek_empty_assistant_message),
            ];
        }
        Some("mcp_call") => {
            return deepseek_input_mcp_call_messages(item)
                .unwrap_or_else(|| vec![deepseek_empty_assistant_message()]);
        }
        Some("custom_tool_call") => {
            return vec![
                deepseek_input_custom_tool_call_message(item)
                    .unwrap_or_else(deepseek_empty_assistant_message),
            ];
        }
        Some("local_shell_call") => {
            return vec![
                deepseek_input_local_shell_call_message(item)
                    .unwrap_or_else(deepseek_empty_assistant_message),
            ];
        }
        Some("function_call_output") => {
            let call_id =
                deepseek_tool_output_call_id(item).unwrap_or_else(|| "call_1".to_string());
            return vec![deepseek_tool_message(
                &call_id,
                &deepseek_tool_output_content(item),
            )];
        }
        Some("custom_tool_call_output") => {
            let call_id =
                deepseek_tool_output_call_id(item).unwrap_or_else(|| "call_1".to_string());
            return vec![deepseek_tool_message(
                &call_id,
                &deepseek_tool_output_content(item),
            )];
        }
        Some("mcp_tool_result") | Some("mcp_call_output") => {
            let call_id =
                deepseek_tool_output_call_id(item).unwrap_or_else(|| "call_1".to_string());
            return vec![deepseek_tool_message(
                &call_id,
                &deepseek_tool_output_content(item),
            )];
        }
        _ => {}
    }
    let role = item.get("role").and_then(Value::as_str).unwrap_or("user");
    let content = item
        .get("content")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| deepseek_message_content_text(item.get("content")))
        .or_else(|| item.get("text").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default();
    if role == "tool" {
        return vec![deepseek_mojo_or_rust_message(
            role,
            &content,
            None,
            deepseek_tool_output_call_id(item).as_deref(),
        )];
    }
    let tool_calls = deepseek_message_tool_calls(item);
    let tool_calls = tool_calls.as_ref().map(|tool_calls| {
        serde_json::to_string(tool_calls).expect("DeepSeek tool calls serialize")
    });
    vec![deepseek_mojo_or_rust_message(
        role,
        &content,
        tool_calls.as_deref(),
        None,
    )]
}

fn deepseek_mojo_or_rust_message(
    role: &str,
    content: &str,
    tool_calls: Option<&str>,
    call_id: Option<&str>,
) -> Value {
    #[cfg(feature = "mojo")]
    return deepseek_mojo_message(role, content, tool_calls, call_id);
    #[cfg(not(feature = "mojo"))]
    {
        let mut message = json!({
            "role": role,
            "content": content,
        });
        if let Some(tool_calls) = tool_calls
            && let Ok(tool_calls) = serde_json::from_str::<Value>(tool_calls)
            && let Some(object) = message.as_object_mut()
        {
            object.insert("tool_calls".to_string(), tool_calls);
        }
        if let Some(call_id) = call_id
            && let Some(object) = message.as_object_mut()
        {
            object.insert(
                "tool_call_id".to_string(),
                Value::String(call_id.to_string()),
            );
        }
        message
    }
}

fn deepseek_tool_output_call_id(item: &Value) -> Option<String> {
    item.get("call_id")
        .or_else(|| item.get("tool_call_id"))
        .or_else(|| item.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn deepseek_tool_output_content(item: &Value) -> String {
    item.get("output")
        .or_else(|| item.get("content"))
        .or_else(|| item.get("result"))
        .or_else(|| item.get("error"))
        .map(|value| deepseek_provider_core_responses_content_text(Some(value)))
        .unwrap_or_default()
}
