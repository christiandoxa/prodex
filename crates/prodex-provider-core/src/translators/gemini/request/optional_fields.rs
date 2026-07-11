//! Gemini optional request field passthrough.

use serde_json::Value;

pub(crate) fn gemini_apply_optional_request_fields(
    source: &serde_json::Map<String, Value>,
    request: &mut serde_json::Map<String, Value>,
) {
    if let Some(settings) = source
        .get("safety_settings")
        .or_else(|| source.get("safetySettings"))
    {
        request.insert("safetySettings".to_string(), settings.clone());
    }
    if let Some(cached_content) = source
        .get("cached_content")
        .or_else(|| source.get("cachedContent"))
        .filter(|value| !value.is_null())
    {
        request.insert("cachedContent".to_string(), cached_content.clone());
    }
    if let Some(labels) = source.get("labels").filter(|value| !value.is_null()) {
        request.insert("labels".to_string(), labels.clone());
    }
}
