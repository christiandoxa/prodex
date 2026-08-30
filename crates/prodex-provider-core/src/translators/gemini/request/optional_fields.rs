//! Gemini optional request field passthrough.

use serde_json::Value;

#[cfg(feature = "mojo")]
pub(crate) fn gemini_apply_optional_request_fields(
    source: &serde_json::Map<String, Value>,
    request: &mut serde_json::Map<String, Value>,
) {
    use super::super::request::GeminiRequestFieldScope;

    for field in super::super::request::gemini_request_field_plan(
        source,
        GeminiRequestFieldScope::OptionalRequest,
    ) {
        let Some(value) = super::super::request::gemini_request_source_value(source, field) else {
            continue;
        };
        if !matches!(
            field.target,
            prodex_mojo_core::provider_constraints::GeminiRequestFieldTarget::SafetySettings
        ) && value.is_null()
        {
            continue;
        }
        request.insert(
            super::super::request::gemini_request_target_name(field.target).to_string(),
            value.clone(),
        );
    }
}

#[cfg(not(feature = "mojo"))]
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
