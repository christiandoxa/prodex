//! Gemini quota retry response-body parsing helpers.

pub(super) fn gemini_provider_core_google_quota_message_from_value(
    value: &serde_json::Value,
) -> Option<String> {
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            serde_json::Value::Object(object) => {
                let message = object
                    .get("message")
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| object.get("detail").and_then(serde_json::Value::as_str))
                    .or_else(|| object.get("error").and_then(serde_json::Value::as_str));
                let explicit_quota = ["status", "code", "reason"].into_iter().any(|key| {
                    object
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(gemini_provider_core_google_quota_code)
                });
                if explicit_quota {
                    return Some(
                        message
                            .unwrap_or("Gemini account quota was exhausted.")
                            .to_string(),
                    );
                }
                stack.extend(object.values());
            }
            serde_json::Value::Array(values) => stack.extend(values),
            _ => {}
        }
    }
    None
}

pub(in crate::gemini_bridge::errors) fn gemini_provider_core_plain_text_has_terminal_quota(
    body: &[u8],
) -> bool {
    let Ok(text) = std::str::from_utf8(body) else {
        return false;
    };
    let lower = text.to_ascii_lowercase();
    lower.contains("quota")
        && (lower.contains("exhausted")
            || lower.contains("exceeded")
            || lower.contains("insufficient"))
}

fn gemini_provider_core_google_quota_code(code: &str) -> bool {
    matches!(
        code.trim().to_ascii_lowercase().as_str(),
        "resource_exhausted"
            | "quota_exhausted"
            | "quota_exceeded"
            | "rate_limit_exceeded"
            | "rate_limit_exceeded_error"
    )
}

pub(super) fn gemini_provider_core_values_from_body(body: &[u8]) -> Vec<serde_json::Value> {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
        return vec![value];
    }
    let Ok(text) = std::str::from_utf8(body) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("data:"))
        .map(str::trim)
        .filter(|line| !line.is_empty() && *line != "[DONE]")
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .collect()
}
