//! Responses input/history conversion helpers for the DeepSeek chat bridge.

use std::collections::BTreeSet;

mod history;
mod validation;

pub use self::history::{
    deepseek_provider_core_chat_role, deepseek_provider_core_first_function_call_output_call_id,
    deepseek_provider_core_history_has_system_message,
    deepseek_provider_core_history_has_tool_call, deepseek_provider_core_message_signatures,
    deepseek_provider_core_tool_call_ids, deepseek_provider_core_tool_output_call_ids,
};
use self::validation::deepseek_provider_core_validate_supported_input_item as validate_input_item_mojo;
use super::deepseek_provider_core_responses_content_text;

pub fn deepseek_provider_core_validate_supported_input_item(
    item: &serde_json::Value,
    gemini_compat: bool,
    provider_label: &str,
) -> Result<(), String> {
    validate_input_item_mojo(item, gemini_compat, provider_label)
}

pub fn deepseek_provider_core_system_message(content: &str) -> serde_json::Value {
    let mut input = prodex_mojo_core::rich::DeepSeekKernelInput::new(
        prodex_mojo_core::rich::DeepSeekKernelOperation::SystemMessage,
    );
    input.content = Some(content);
    deepseek_provider_core_mojo_value(input)
}

pub fn deepseek_provider_core_user_message(content: &str) -> serde_json::Value {
    let mut input = prodex_mojo_core::rich::DeepSeekKernelInput::new(
        prodex_mojo_core::rich::DeepSeekKernelOperation::UserMessage,
    );
    input.content = Some(content);
    deepseek_provider_core_mojo_value(input)
}

pub fn deepseek_provider_core_push_message_from_responses_item(
    item: &serde_json::Value,
    messages: &mut Vec<serde_json::Value>,
    replayed_tool_call_ids: &BTreeSet<String>,
    replayed_tool_output_call_ids: &BTreeSet<String>,
    replayed_message_signatures: &BTreeSet<(String, String)>,
) {
    let Some(object) = item.as_object() else {
        return;
    };
    let item_type = object.get("type").and_then(serde_json::Value::as_str);
    let call_id = object
        .get("call_id")
        .or_else(|| object.get("tool_call_id"))
        .or_else(|| object.get("id"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("call_0");
    let kind = prodex_mojo_core::rich::deepseek_replay_item_kind(item_type)
        .expect("compiled Mojo DeepSeek replay item classification failed");
    // The host resolves role/content text and history-key membership; the
    // compiled planner owns which messages may be re-emitted on a continuation.
    let (text_blank, text_duplicate) =
        if kind == prodex_mojo_core::rich::DeepSeekReplayItemKind::Text {
            let role = deepseek_provider_core_chat_role(
                object
                    .get("role")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("user"),
            );
            let text = deepseek_provider_core_responses_content_text(object.get("content"));
            (
                text.trim().is_empty(),
                replayed_message_signatures.contains(&(role.to_string(), text)),
            )
        } else {
            (false, false)
        };
    let output_field_mask = ["output", "content", "result", "error"]
        .iter()
        .enumerate()
        .fold(0_u8, |mask, (index, key)| {
            mask | (u8::from(object.contains_key(*key)) << index)
        });
    let action = prodex_mojo_core::rich::deepseek_replay_item_action(
        kind,
        prodex_mojo_core::rich::DeepSeekReplayItemFacts {
            replayed_call: replayed_tool_call_ids.contains(call_id),
            replayed_output: replayed_tool_output_call_ids.contains(call_id),
            output_field_mask,
            text_blank,
            text_duplicate,
        },
    )
    .expect("compiled Mojo DeepSeek replay item emission decision failed");
    let Some(action) = action else {
        return;
    };

    let source = serde_json::to_string(item).expect("DeepSeek input item serializes");
    let mut input = prodex_mojo_core::rich::DeepSeekKernelInput::new(
        prodex_mojo_core::rich::DeepSeekKernelOperation::RawBridgeInputItem,
    );
    input.item = Some(&source);
    input.stream = action.stream;
    input.sequence_number = u64::from(action.emit_output);
    let body = prodex_mojo_core::rich::deepseek_kernel(input)
        .unwrap_or_else(|error| panic!("Mojo DeepSeek input-item shaping failed: {error:?}"));
    let mapped: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap_or_else(|error| {
        panic!("Mojo DeepSeek input-item shaping returned invalid JSON: {error}")
    });
    messages.extend(mapped);
}

fn deepseek_provider_core_mojo_value(
    input: prodex_mojo_core::rich::DeepSeekKernelInput<'_>,
) -> serde_json::Value {
    let body = prodex_mojo_core::rich::deepseek_kernel(input)
        .unwrap_or_else(|error| panic!("DeepSeek Mojo kernel failed: {error:?}"));
    serde_json::from_slice(&body)
        .unwrap_or_else(|error| panic!("DeepSeek Mojo kernel returned invalid JSON: {error}"))
}
