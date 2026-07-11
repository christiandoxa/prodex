//! Provider error body token extraction.

pub(super) fn provider_error_tokens(body: &[u8]) -> Vec<String> {
    let mut tokens = Vec::new();
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
        provider_error_tokens_from_value(&value, &mut tokens);
    } else if let Ok(text) = std::str::from_utf8(body) {
        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(payload) = trimmed.strip_prefix("data:")
                && let Ok(value) = serde_json::from_str::<serde_json::Value>(payload.trim())
            {
                provider_error_tokens_from_value(&value, &mut tokens);
                continue;
            }
            provider_push_error_token(&mut tokens, trimmed);
        }
    }
    tokens
}

fn provider_error_tokens_from_value(value: &serde_json::Value, output: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, value) in object {
                if matches!(
                    key.as_str(),
                    "code" | "status" | "reason" | "type" | "message" | "detail" | "error"
                ) {
                    match value {
                        serde_json::Value::String(text) => provider_push_error_token(output, text),
                        serde_json::Value::Number(number) => {
                            provider_push_error_token(output, &number.to_string())
                        }
                        _ => provider_error_tokens_from_value(value, output),
                    }
                } else {
                    provider_error_tokens_from_value(value, output);
                }
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                provider_error_tokens_from_value(value, output);
            }
        }
        serde_json::Value::String(text) => provider_push_error_token(output, text),
        _ => {}
    }
}

fn provider_push_error_token(output: &mut Vec<String>, value: &str) {
    let token = value.trim().to_ascii_lowercase();
    if token.is_empty() {
        return;
    }
    output.push(token);
}
