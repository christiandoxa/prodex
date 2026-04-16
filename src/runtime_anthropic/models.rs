use super::*;

pub(crate) fn runtime_proxy_anthropic_models_list() -> serde_json::Value {
    let data = runtime_proxy_responses_model_descriptors()
        .iter()
        .map(|descriptor| runtime_proxy_anthropic_model_descriptor(descriptor.id))
        .collect::<Vec<_>>();
    let first_id = data
        .first()
        .and_then(|model| model.get("id"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let last_id = data
        .last()
        .and_then(|model| model.get("id"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    serde_json::json!({
        "data": data,
        "first_id": first_id,
        "has_more": false,
        "last_id": last_id,
    })
}

pub(crate) fn runtime_proxy_anthropic_model_descriptor(model_id: &str) -> serde_json::Value {
    let supported_effort_levels = runtime_proxy_responses_model_supported_effort_levels(model_id);
    serde_json::json!({
        "type": "model",
        "id": model_id,
        "display_name": runtime_proxy_anthropic_model_display_name(model_id),
        "created_at": RUNTIME_PROXY_ANTHROPIC_MODEL_CREATED_AT,
        "supportsEffort": true,
        "supportedEffortLevels": supported_effort_levels,
    })
}

pub(crate) fn runtime_proxy_anthropic_model_display_name(model_id: &str) -> String {
    runtime_proxy_responses_model_descriptor(model_id)
        .map(|descriptor| descriptor.display_name.to_string())
        .unwrap_or_else(|| model_id.to_string())
}

pub(crate) fn runtime_proxy_anthropic_model_id_from_path(path: &str) -> Option<&str> {
    path.strip_prefix(&format!("{RUNTIME_PROXY_ANTHROPIC_MODELS_PATH}/"))
        .filter(|model_id| !model_id.is_empty())
}
