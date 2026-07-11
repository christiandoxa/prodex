use std::collections::BTreeSet;

use crate::{ProviderEndpoint, ProviderReasoningEffort, ProviderRequestFeature};

pub(super) fn inferred_features(
    value: &serde_json::Value,
    endpoint: ProviderEndpoint,
) -> BTreeSet<ProviderRequestFeature> {
    let mut features = BTreeSet::new();
    let object = value
        .as_object()
        .expect("request root checked before feature inference");
    if object
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|tools| !tools.is_empty())
    {
        features.insert(ProviderRequestFeature::Tools);
    }
    if object.get("stream").and_then(serde_json::Value::as_bool) == Some(true) {
        features.insert(ProviderRequestFeature::Streaming);
    }
    if matches!(endpoint, ProviderEndpoint::ResponsesCompact) {
        features.insert(ProviderRequestFeature::Compact);
    }
    let reasoning_enabled = object.get("reasoning").is_some()
        && object
            .get("reasoning")
            .and_then(|reasoning| reasoning.get("effort"))
            .and_then(serde_json::Value::as_str)
            .map(ProviderReasoningEffort::parse)
            .is_none_or(ProviderReasoningEffort::reserves_tokens);
    let thinking_enabled = object
        .get("thinking")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|thinking| {
            thinking.get("type").and_then(serde_json::Value::as_str) != Some("disabled")
                && thinking.get("budget_tokens").is_some()
        });
    if reasoning_enabled || thinking_enabled {
        features.insert(ProviderRequestFeature::Reasoning);
    }
    if value_contains_key_or_type(value, "image_url", &["input_image", "image_url"]) {
        features.insert(ProviderRequestFeature::Vision);
    }
    if value_contains_key_or_type(value, "input_audio", &["input_audio", "audio"]) {
        features.insert(ProviderRequestFeature::Audio);
    }
    if object
        .get("response_format")
        .and_then(|format| format.get("type"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|kind| matches!(kind, "json_schema" | "json_object"))
        || object
            .get("text")
            .and_then(|text| text.get("format"))
            .and_then(|format| format.get("type"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|kind| matches!(kind, "json_schema" | "json_object"))
    {
        features.insert(ProviderRequestFeature::JsonSchema);
    }
    if object
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|tools| {
            tools.iter().any(|tool| {
                tool.get("type")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|kind| kind.starts_with("web_search"))
            })
        })
    {
        features.insert(ProviderRequestFeature::WebSearch);
    }
    features
}

fn value_contains_key_or_type(value: &serde_json::Value, key: &str, types: &[&str]) -> bool {
    match value {
        serde_json::Value::Array(values) => values
            .iter()
            .any(|value| value_contains_key_or_type(value, key, types)),
        serde_json::Value::Object(object) => {
            object.contains_key(key)
                || object
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|kind| types.contains(&kind))
                || object
                    .values()
                    .any(|value| value_contains_key_or_type(value, key, types))
        }
        _ => false,
    }
}
