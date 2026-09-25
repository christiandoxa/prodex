pub(crate) use self::response_tool_calls_apply_patch::gemini_custom_apply_patch_input;
use serde_json::{Value, json};

#[path = "response_tool_calls/chat.rs"]
mod chat;
mod response_tool_calls_apply_patch;
#[path = "response_tool_calls/rtk.rs"]
mod rtk;

pub(crate) use self::chat::gemini_chat_assistant_tool_call_item_with_call_id;
pub(crate) use self::chat::gemini_chat_assistant_tool_call_with_call_id;
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
    let signature = part
        .get("thoughtSignature")
        .and_then(Value::as_str)
        .or_else(|| {
            function_call
                .get("thoughtSignature")
                .and_then(Value::as_str)
        });
    let mut input = prodex_mojo_core::rich::GeminiResponseKernelInput::new(
        prodex_mojo_core::rich::GeminiResponseKernelOperation::AddedFunctionCallItem,
    );
    input.call_id = Some(call_id);
    input.name = Some(flat_name);
    input.signature = signature;
    Some(super::super::stream::gemini_mojo_value(input))
}

pub(crate) fn gemini_response_tool_call_raw_item_with_call_id(
    part: &Value,
    flat_name: &str,
    arguments: &str,
    call_id_override: Option<&str>,
) -> Value {
    let call_id = call_id_override.unwrap_or("call_1");
    let signature = part.get("thoughtSignature").and_then(Value::as_str);
    let mut input = prodex_mojo_core::rich::GeminiResponseKernelInput::new(
        prodex_mojo_core::rich::GeminiResponseKernelOperation::RawFunctionCallItem,
    );
    input.call_id = Some(call_id);
    input.name = Some(flat_name);
    input.arguments = Some(arguments);
    input.signature = signature;
    super::super::stream::gemini_mojo_value(input)
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
        let arguments = serde_json::to_string(&args_value).unwrap_or_else(|_| "{}".to_string());
        let mut input = prodex_mojo_core::rich::GeminiResponseKernelInput::new(
            prodex_mojo_core::rich::GeminiResponseKernelOperation::ToolSearchCallItem,
        );
        input.call_id = Some(call_id);
        input.arguments = Some(&arguments);
        return super::super::stream::gemini_mojo_value(input);
    }
    if flat_name == "apply_patch" {
        let input_value = gemini_custom_apply_patch_input(&args_value);
        let mut input = prodex_mojo_core::rich::GeminiResponseKernelInput::new(
            prodex_mojo_core::rich::GeminiResponseKernelOperation::CustomToolCallItem,
        );
        input.call_id = Some(call_id);
        input.name = Some(flat_name);
        input.arguments = Some(&input_value);
        return super::super::stream::gemini_mojo_value(input);
    }
    let args = serde_json::to_string(&args_value).unwrap_or_else(|_| "{}".to_string());
    let args = gemini_rtk_wrapped_tool_arguments(flat_name, &args);
    let signature = part
        .get("thoughtSignature")
        .and_then(Value::as_str)
        .or_else(|| {
            function_call
                .get("thoughtSignature")
                .and_then(Value::as_str)
        });
    let mut input = prodex_mojo_core::rich::GeminiResponseKernelInput::new(
        prodex_mojo_core::rich::GeminiResponseKernelOperation::FunctionCallItem,
    );
    input.call_id = Some(call_id);
    input.name = Some(flat_name);
    input.arguments = Some(&args);
    input.signature = signature;
    super::super::stream::gemini_mojo_value(input)
}
