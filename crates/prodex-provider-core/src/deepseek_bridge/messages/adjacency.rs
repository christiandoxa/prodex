//! DeepSeek tool-call/tool-output adjacency repair.

use std::collections::{BTreeMap, BTreeSet};

use super::deepseek_provider_core_normalize_assistant_tool_call_content;

pub fn deepseek_provider_core_repair_tool_call_adjacency(messages: &mut Vec<serde_json::Value>) {
    let tool_outputs = deepseek_provider_core_tool_output_messages(messages);
    if tool_outputs.is_empty() {
        deepseek_provider_core_drop_unanswered_tool_calls(messages);
        return;
    }

    let mut repaired = Vec::with_capacity(messages.len());
    let mut emitted_tool_output_call_ids = BTreeSet::new();
    for message in std::mem::take(messages) {
        if message.get("role").and_then(serde_json::Value::as_str) == Some("tool") {
            continue;
        }
        repaired.extend(deepseek_provider_core_repair_tool_call_message(
            message,
            &tool_outputs,
            &mut emitted_tool_output_call_ids,
        ));
    }

    *messages = repaired;
}

fn deepseek_provider_core_repair_tool_call_message(
    mut message: serde_json::Value,
    tool_outputs: &BTreeMap<String, serde_json::Value>,
    emitted_tool_output_call_ids: &mut BTreeSet<String>,
) -> Vec<serde_json::Value> {
    let Some(tool_calls) = message
        .get("tool_calls")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .filter(|tool_calls| !tool_calls.is_empty())
    else {
        return vec![message];
    };

    let (answered_tool_calls, adjacent_tool_outputs) = deepseek_provider_core_answered_tool_calls(
        tool_calls,
        tool_outputs,
        emitted_tool_output_call_ids,
    );
    if answered_tool_calls.is_empty() {
        if let Some(object) = message.as_object_mut() {
            object.remove("tool_calls");
        }
        return deepseek_provider_core_message_has_content(&message)
            .then_some(vec![message])
            .unwrap_or_default();
    }

    if let Some(object) = message.as_object_mut() {
        object.insert(
            "tool_calls".to_string(),
            serde_json::Value::Array(answered_tool_calls),
        );
    }
    let mut repaired = vec![deepseek_provider_core_normalize_assistant_tool_call_content(message)];
    repaired.extend(adjacent_tool_outputs);
    repaired
}

fn deepseek_provider_core_answered_tool_calls(
    tool_calls: Vec<serde_json::Value>,
    tool_outputs: &BTreeMap<String, serde_json::Value>,
    emitted_tool_output_call_ids: &mut BTreeSet<String>,
) -> (Vec<serde_json::Value>, Vec<serde_json::Value>) {
    let mut answered_tool_calls = Vec::new();
    let mut adjacent_tool_outputs = Vec::new();
    for tool_call in tool_calls {
        let Some(call_id) = tool_call
            .get("id")
            .and_then(serde_json::Value::as_str)
            .filter(|call_id| !call_id.trim().is_empty())
        else {
            continue;
        };
        if emitted_tool_output_call_ids.contains(call_id) {
            continue;
        }
        let Some(tool_output) = tool_outputs.get(call_id).cloned() else {
            continue;
        };
        emitted_tool_output_call_ids.insert(call_id.to_string());
        answered_tool_calls.push(tool_call);
        adjacent_tool_outputs.push(tool_output);
    }
    (answered_tool_calls, adjacent_tool_outputs)
}

fn deepseek_provider_core_drop_unanswered_tool_calls(messages: &mut Vec<serde_json::Value>) {
    messages.retain_mut(|message| {
        let has_tool_calls = message
            .get("tool_calls")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|tool_calls| !tool_calls.is_empty());
        if !has_tool_calls {
            return true;
        }
        if let Some(object) = message.as_object_mut() {
            object.remove("tool_calls");
        }
        deepseek_provider_core_message_has_content(message)
    });
}

fn deepseek_provider_core_tool_output_messages(
    messages: &[serde_json::Value],
) -> BTreeMap<String, serde_json::Value> {
    let mut tool_outputs = BTreeMap::new();
    for message in messages {
        if message.get("role").and_then(serde_json::Value::as_str) != Some("tool") {
            continue;
        }
        let Some(call_id) = message
            .get("tool_call_id")
            .and_then(serde_json::Value::as_str)
            .filter(|call_id| !call_id.trim().is_empty())
        else {
            continue;
        };
        tool_outputs
            .entry(call_id.to_string())
            .or_insert_with(|| message.clone());
    }
    tool_outputs
}

fn deepseek_provider_core_message_has_content(message: &serde_json::Value) -> bool {
    message
        .get("content")
        .is_some_and(deepseek_provider_core_value_has_content)
        || message
            .get("reasoning_content")
            .is_some_and(deepseek_provider_core_value_has_content)
}

fn deepseek_provider_core_value_has_content(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::String(text) => !text.trim().is_empty(),
        serde_json::Value::Array(items) => !items.is_empty(),
        serde_json::Value::Object(object) => !object.is_empty(),
        serde_json::Value::Bool(_) | serde_json::Value::Number(_) => true,
    }
}
