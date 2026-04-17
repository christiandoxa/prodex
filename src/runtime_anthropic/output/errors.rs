use super::*;

pub(crate) fn runtime_anthropic_message_id() -> String {
    format!("msg_{}", runtime_random_token("claude").replace('-', ""))
}

pub(crate) fn runtime_anthropic_error_type_for_status(status: u16) -> &'static str {
    match status {
        400 => "invalid_request_error",
        401 => "authentication_error",
        403 => "permission_error",
        404 => "not_found_error",
        429 => "rate_limit_error",
        500 | 502 | 503 | 504 | 529 => "overloaded_error",
        _ => "api_error",
    }
}

pub(crate) fn runtime_anthropic_error_message_from_parts(
    parts: &RuntimeBufferedResponseParts,
) -> String {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&parts.body) {
        if let Some(message) = value
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return message.to_string();
        }
        if let Some(message) = value
            .get("message")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return message.to_string();
        }
    }
    let body = String::from_utf8_lossy(&parts.body).trim().to_string();
    if body.is_empty() {
        "Upstream runtime proxy request failed.".to_string()
    } else {
        body
    }
}

pub(crate) fn build_runtime_anthropic_error_parts(
    status: u16,
    error_type: &str,
    message: &str,
) -> RuntimeBufferedResponseParts {
    RuntimeBufferedResponseParts {
        status,
        headers: vec![("Content-Type".to_string(), b"application/json".to_vec())],
        body: serde_json::json!({
            "type": "error",
            "error": {
                "type": error_type,
                "message": message,
            }
        })
        .to_string()
        .into_bytes()
        .into(),
    }
}

pub(crate) fn runtime_anthropic_error_from_upstream_parts(
    parts: RuntimeBufferedResponseParts,
) -> RuntimeBufferedResponseParts {
    let status = parts.status;
    let message = runtime_anthropic_error_message_from_parts(&parts);
    build_runtime_anthropic_error_parts(
        status,
        runtime_anthropic_error_type_for_status(status),
        &message,
    )
}
