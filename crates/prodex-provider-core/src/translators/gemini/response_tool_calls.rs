pub(crate) use self::response_tool_calls_apply_patch::gemini_custom_apply_patch_input;
use serde_json::{Value, json};

#[path = "response_tool_calls/chat.rs"]
mod chat;
mod response_tool_calls_apply_patch;
#[path = "response_tool_calls/rtk.rs"]
mod rtk;

pub(crate) use self::chat::gemini_chat_assistant_tool_call_item_with_call_id;
pub(crate) use self::rtk::gemini_rtk_wrapped_tool_arguments;

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
    let signature = gemini_response_tool_call_signature(part, function_call);
    let mut input = prodex_mojo_core::rich::GeminiResponseKernelInput::new(
        prodex_mojo_core::rich::GeminiResponseKernelOperation::StreamAddedToolCallItem,
    );
    input.call_id = Some(call_id);
    input.name = Some(flat_name);
    input.signature = signature.as_deref();
    let value = super::super::stream::gemini_mojo_value(input);
    (!value.is_null()).then_some(value)
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
        prodex_mojo_core::rich::GeminiResponseKernelOperation::StreamCompletedToolCallItem,
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
    let args = serde_json::to_string(&args_value).unwrap_or_else(|_| "{}".to_string());
    let args = gemini_rtk_wrapped_tool_arguments(flat_name, &args);
    let custom_input =
        (flat_name == "apply_patch").then(|| gemini_custom_apply_patch_input(&args_value));
    let signature = gemini_response_tool_call_signature(part, function_call);
    let mut input = prodex_mojo_core::rich::GeminiResponseKernelInput::new(
        prodex_mojo_core::rich::GeminiResponseKernelOperation::StreamCompletedToolCallItem,
    );
    input.call_id = Some(call_id);
    input.name = Some(flat_name);
    input.arguments = Some(&args);
    input.signature = signature.as_deref();
    input.response = custom_input.as_deref();
    input.created_at_present = true;
    super::super::stream::gemini_mojo_value(input)
}

pub(super) fn gemini_response_tool_call_signature(
    part: &Value,
    function_call: &Value,
) -> Option<String> {
    let values = [
        part.get("thoughtSignature"),
        function_call.get("thoughtSignature"),
    ];
    let candidates = [
        signature_candidate(values[0]),
        signature_candidate(values[1]),
        signature_candidate(None),
        signature_candidate(None),
        signature_candidate(None),
        signature_candidate(None),
        signature_candidate(None),
    ];
    let selected = prodex_mojo_core::provider_constraints::gemini_signature_choice(&candidates)
        .expect("Mojo Gemini response thought-signature precedence failed")?;
    values
        .get(selected)
        .and_then(|value| value.and_then(Value::as_str))
        .map(str::to_string)
}

fn signature_candidate(
    value: Option<&Value>,
) -> prodex_mojo_core::provider_constraints::GeminiSignatureCandidate<'_> {
    prodex_mojo_core::provider_constraints::GeminiSignatureCandidate {
        present: value.is_some(),
        text: value.and_then(Value::as_str),
    }
}
