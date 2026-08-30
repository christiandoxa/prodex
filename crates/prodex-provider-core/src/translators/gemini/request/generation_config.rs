//! Gemini generation-config translation from OpenAI-compatible request fields.

#[path = "generation_config/thinking.rs"]
mod thinking;

use serde_json::Value;

pub use self::thinking::gemini_provider_core_model_uses_thinking_level;
pub(in crate::translators::gemini) use self::thinking::gemini_thinking_config_from_request;
use self::thinking::gemini_thinking_config_with_budget_from_request;

#[cfg(feature = "mojo")]
fn gemini_apply_mojo_generation_fields(
    obj: &serde_json::Map<String, Value>,
    generation_config: &mut serde_json::Map<String, Value>,
    scope: super::GeminiRequestFieldScope,
) {
    for field in super::gemini_request_field_plan(obj, scope) {
        let Some(value) = super::gemini_request_source_value(obj, field) else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        generation_config.insert(
            super::gemini_request_target_name(field.target).to_string(),
            value.clone(),
        );
    }
}

#[cfg(feature = "mojo")]
pub(in crate::translators::gemini) fn gemini_insert_basic_generation_config(
    obj: &serde_json::Map<String, Value>,
    generation_config: &mut serde_json::Map<String, Value>,
) {
    gemini_apply_mojo_generation_fields(
        obj,
        generation_config,
        super::GeminiRequestFieldScope::BasicGeneration,
    );
}

#[cfg(not(feature = "mojo"))]
pub(in crate::translators::gemini) fn gemini_insert_basic_generation_config(
    obj: &serde_json::Map<String, Value>,
    generation_config: &mut serde_json::Map<String, Value>,
) {
    for (from, to) in [
        ("temperature", "temperature"),
        ("top_p", "topP"),
        ("max_tokens", "maxOutputTokens"),
    ] {
        if let Some(value) = obj.get(from).filter(|value| !value.is_null()) {
            generation_config.insert(to.to_string(), value.clone());
        }
    }
    if let Some(stop) = obj
        .get("stop")
        .or_else(|| obj.get("stop_sequences"))
        .or_else(|| obj.get("stopSequences"))
        .filter(|value| !value.is_null())
    {
        generation_config.insert("stopSequences".to_string(), stop.clone());
    }
}

pub(crate) fn gemini_validate_candidate_count(value: &Value) -> Result<(), String> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    let snake = object
        .get("candidate_count")
        .filter(|value| !value.is_null());
    let camel = object
        .get("candidateCount")
        .filter(|value| !value.is_null());
    if let (Some(snake), Some(camel)) = (snake, camel)
        && snake != camel
    {
        return Err(
            "invalid_candidate_count: Gemini request fields `candidate_count` and `candidateCount` conflict"
                .to_string(),
        );
    }
    for (field, candidate_count) in [("candidate_count", snake), ("candidateCount", camel)] {
        let Some(candidate_count) = candidate_count else {
            continue;
        };
        if candidate_count.as_u64() != Some(1) {
            return Err(format!(
                "invalid_candidate_count: Gemini request field `{field}` must be omitted, null, or 1"
            ));
        }
    }
    Ok(())
}

pub(crate) fn gemini_generation_config_from_request(
    original: &Value,
    chat: &Value,
    model: &str,
    thinking_budget_tokens: Option<u64>,
) -> Value {
    let mut config = serde_json::Map::new();
    if let Some(chat) = chat.as_object() {
        for (from, to) in [
            ("temperature", "temperature"),
            ("top_p", "topP"),
            ("max_tokens", "maxOutputTokens"),
        ] {
            if let Some(value) = chat.get(from) {
                config.insert(to.to_string(), value.clone());
            }
        }
    }
    let Some(original) = original.as_object() else {
        return Value::Object(config);
    };
    gemini_insert_extended_generation_config(original, &mut config);
    if let Some(stop) = original
        .get("stop")
        .or_else(|| original.get("stop_sequences"))
        .or_else(|| original.get("stopSequences"))
        .filter(|value| !value.is_null())
    {
        config.insert("stopSequences".to_string(), stop.clone());
    }
    gemini_apply_text_format(original, &mut config);
    if let Some(thinking_config) =
        gemini_thinking_config_with_budget_from_request(original, model, thinking_budget_tokens)
    {
        config.insert("thinkingConfig".to_string(), thinking_config);
    }
    Value::Object(config)
}

#[cfg(feature = "mojo")]
pub(in crate::translators::gemini) fn gemini_insert_extended_generation_config(
    obj: &serde_json::Map<String, Value>,
    generation_config: &mut serde_json::Map<String, Value>,
) {
    gemini_apply_mojo_generation_fields(
        obj,
        generation_config,
        super::GeminiRequestFieldScope::ExtendedGeneration,
    );
}

#[cfg(not(feature = "mojo"))]
pub(in crate::translators::gemini) fn gemini_insert_extended_generation_config(
    obj: &serde_json::Map<String, Value>,
    generation_config: &mut serde_json::Map<String, Value>,
) {
    for (from, to) in [
        ("top_k", "topK"),
        ("topK", "topK"),
        ("seed", "seed"),
        ("presence_penalty", "presencePenalty"),
        ("presencePenalty", "presencePenalty"),
        ("frequency_penalty", "frequencyPenalty"),
        ("frequencyPenalty", "frequencyPenalty"),
        ("response_mime_type", "responseMimeType"),
        ("responseMimeType", "responseMimeType"),
        ("response_schema", "responseSchema"),
        ("responseSchema", "responseSchema"),
        ("response_json_schema", "responseJsonSchema"),
        ("responseJsonSchema", "responseJsonSchema"),
        ("response_modalities", "responseModalities"),
        ("responseModalities", "responseModalities"),
        ("media_resolution", "mediaResolution"),
        ("mediaResolution", "mediaResolution"),
        ("audio_timestamp", "audioTimestamp"),
        ("audioTimestamp", "audioTimestamp"),
        ("speech_config", "speechConfig"),
        ("speechConfig", "speechConfig"),
    ] {
        if let Some(value) = obj.get(from).filter(|value| !value.is_null()) {
            generation_config.insert(to.to_string(), value.clone());
        }
    }
    if let Some(value) = obj
        .get("candidateCount")
        .or_else(|| obj.get("candidate_count"))
        .filter(|value| !value.is_null() && value.as_u64() == Some(1))
    {
        generation_config.insert("candidateCount".to_string(), value.clone());
    }
}

pub(in crate::translators::gemini) fn gemini_apply_text_format(
    obj: &serde_json::Map<String, Value>,
    generation_config: &mut serde_json::Map<String, Value>,
) {
    let Some(text) = obj.get("text").and_then(Value::as_object) else {
        return;
    };
    let Some(format) = text.get("format").and_then(Value::as_object) else {
        return;
    };
    let format_type = format
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match format_type {
        "json_object" => {
            generation_config.insert(
                "responseMimeType".to_string(),
                Value::String("application/json".to_string()),
            );
        }
        "json_schema" => {
            generation_config.insert(
                "responseMimeType".to_string(),
                Value::String("application/json".to_string()),
            );
            if let Some(schema) = format.get("schema").or_else(|| format.get("json_schema")) {
                generation_config.insert("responseJsonSchema".to_string(), schema.clone());
            }
        }
        _ => {}
    }
}
