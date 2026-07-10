//! Provider fallback request-body helpers.

pub fn provider_model_from_request_body(body: &[u8]) -> Option<String> {
    let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    value
        .get("model")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .map(str::to_string)
}

pub fn provider_request_body_with_model(body: &[u8], model: &str) -> Vec<u8> {
    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return body.to_vec();
    };
    let Some(object) = value.as_object_mut() else {
        return body.to_vec();
    };
    object.insert(
        "model".to_string(),
        serde_json::Value::String(model.to_string()),
    );
    serde_json::to_vec(&value).unwrap_or_else(|_| body.to_vec())
}
