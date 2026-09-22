//! Public provider tool API backed by the complete Mojo transformation.
use super::mojo::{transform_bytes, transform_value};
use prodex_mojo_core::json::ChatToolOperation as Operation;
use serde_json::Value;

pub fn provider_core_chat_tools_from_responses_request(value: &Value) -> Option<Vec<Value>> {
    transform_value(value, Operation::Tools, false).map(|value| match value {
        Value::Array(tools) => tools,
        _ => panic!("Mojo provider tools must return an array"),
    })
}

pub fn provider_core_chat_tool_choice_from_responses_request(
    value: &Value,
    thinking_enabled: bool,
) -> Option<Value> {
    transform_value(value, Operation::Choice, thinking_enabled)
}

pub fn provider_core_chat_web_search_options_from_responses_request(
    value: &Value,
) -> Option<Value> {
    transform_value(value, Operation::WebSearchOptions, false)
}

pub fn provider_core_chat_request_body_without_web_search_options(body: &[u8]) -> Option<Vec<u8>> {
    let value: Value = serde_json::from_slice(body).ok()?;
    transform_bytes(&value, Operation::WithoutWebSearch, false)
}

pub fn provider_core_flatten_namespace_tool_name(namespace: &str, name: &str) -> String {
    let value = serde_json::json!([namespace, name]);
    match transform_value(&value, Operation::FlattenName, false) {
        Some(Value::String(name)) => name,
        _ => panic!("Mojo namespace name planner must return a string"),
    }
}
