//! Kiro message shaping through the bounded Mojo JSON ABI.

use prodex_mojo_core::rich::{KiroKernelInput, KiroKernelOperation, kiro_kernel};
use serde_json::Value;

pub fn kiro_provider_core_prompt_from_chat_messages(messages: &[Value]) -> String {
    let messages = serde_json::to_string(messages).expect("Kiro chat messages serialize for Mojo");
    let mut input = KiroKernelInput::new(KiroKernelOperation::PromptFromChatMessages);
    input.input = Some(&messages);
    let body = kiro_kernel(input)
        .unwrap_or_else(|error| panic!("Mojo Kiro prompt construction failed: {error:?}"));
    String::from_utf8(body).expect("Mojo Kiro prompt is UTF-8")
}

pub fn kiro_provider_core_responses_items_from_chat_message(message: &Value) -> Vec<Value> {
    let message =
        serde_json::to_string(message).expect("Kiro chat message serializes for Mojo rewrite");
    let mut input = prodex_mojo_core::rich::KiroKernelInput::new(
        prodex_mojo_core::rich::KiroKernelOperation::RawResponsesItemsFromChatMessage,
    );
    input.input = Some(&message);
    let body = prodex_mojo_core::rich::kiro_kernel(input)
        .unwrap_or_else(|error| panic!("Mojo Kiro chat-message rewrite failed: {error:?}"));
    serde_json::from_slice(&body).unwrap_or_else(|error| {
        panic!("Mojo Kiro chat-message rewrite returned invalid JSON: {error}")
    })
}

pub fn kiro_provider_core_tool_from_legacy_chat_function(function: &Value) -> Option<Value> {
    kiro_provider_core_legacy_json_value(function, KiroKernelOperation::LegacyFunctionTool)
}

pub fn kiro_provider_core_tool_choice_from_legacy_chat_function_call(
    function_call: &Value,
) -> Option<Value> {
    kiro_provider_core_legacy_json_value(function_call, KiroKernelOperation::LegacyToolChoice)
}

fn kiro_provider_core_legacy_json_value(
    value: &Value,
    operation: KiroKernelOperation,
) -> Option<Value> {
    let value = serde_json::to_string(value).expect("Kiro legacy function value serializes");
    let mut input = KiroKernelInput::new(operation);
    input.input = Some(&value);
    let body = kiro_kernel(input)
        .unwrap_or_else(|error| panic!("Mojo Kiro legacy function shaping failed: {error:?}"));
    if body.is_empty() {
        None
    } else {
        Some(serde_json::from_slice(&body).unwrap_or_else(|error| {
            panic!("Mojo Kiro legacy function shaping returned invalid JSON: {error}")
        }))
    }
}
