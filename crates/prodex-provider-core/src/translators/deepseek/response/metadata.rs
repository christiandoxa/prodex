//! DeepSeek buffered response metadata extraction.

use serde_json::Value;

#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::{DeepSeekKernelInput, DeepSeekKernelOperation};
#[cfg(not(feature = "mojo"))]
use serde_json::json;

pub(super) fn deepseek_response_metadata(value: &Value, message: Option<&Value>) -> Option<Value> {
    let mut metadata = serde_json::Map::new();
    if let Some(logprobs) = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("logprobs"))
        .filter(|logprobs| !logprobs.is_null())
    {
        metadata.insert("logprobs".to_string(), logprobs.clone());
    }
    if let Some(reasoning_content) = message
        .and_then(|message| message.get("reasoning_content"))
        .and_then(Value::as_str)
        .filter(|reasoning_content| !reasoning_content.is_empty())
    {
        metadata.insert(
            "reasoning_content".to_string(),
            Value::String(reasoning_content.to_string()),
        );
    }
    if let Some(refusal) = message
        .and_then(|message| message.get("refusal"))
        .and_then(Value::as_str)
        .filter(|refusal| !refusal.is_empty())
    {
        metadata.insert("refusal".to_string(), Value::String(refusal.to_string()));
    }
    if let Some(annotations) = message
        .and_then(|message| message.get("annotations"))
        .and_then(Value::as_array)
        .filter(|annotations| !annotations.is_empty())
    {
        metadata.insert("annotations".to_string(), Value::Array(annotations.clone()));
    }
    if let Some(finish_reason) = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("finish_reason"))
        .and_then(Value::as_str)
    {
        metadata.insert(
            "finish_reason".to_string(),
            Value::String(finish_reason.to_string()),
        );
    }
    if let Some(system_fingerprint) = value
        .get("system_fingerprint")
        .and_then(Value::as_str)
        .filter(|system_fingerprint| !system_fingerprint.is_empty())
    {
        metadata.insert(
            "system_fingerprint".to_string(),
            Value::String(system_fingerprint.to_string()),
        );
    }
    if metadata.is_empty() {
        return None;
    }
    #[cfg(feature = "mojo")]
    {
        let metadata = serde_json::to_string(&Value::Object(metadata))
            .expect("DeepSeek response metadata serializes");
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::ResponseMetadata);
        input.role = Some("deepseek");
        input.metadata = Some(&metadata);
        Some(crate::translators::deepseek::deepseek_mojo_value(input))
    }
    #[cfg(not(feature = "mojo"))]
    Some(json!({ "deepseek": metadata }))
}
