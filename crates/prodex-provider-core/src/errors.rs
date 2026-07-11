mod body;

use self::body::provider_error_tokens;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderErrorClass {
    Auth,
    Quota,
    RateLimit,
    Transient,
    NotFound,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderErrorClassification {
    pub class: ProviderErrorClass,
    pub cooldown_ms: u64,
}

pub fn classify_provider_error(
    status: Option<u16>,
    code: Option<&str>,
    text: Option<&str>,
) -> ProviderErrorClassification {
    let normalized_code = code.unwrap_or_default().trim().to_ascii_lowercase();
    let normalized_text = text.unwrap_or_default().trim().to_ascii_lowercase();
    if matches!(status, Some(401 | 403))
        || matches!(
            normalized_code.as_str(),
            "unauthenticated" | "invalid_api_key" | "authentication_error"
        )
    {
        return ProviderErrorClassification {
            class: ProviderErrorClass::Auth,
            cooldown_ms: 0,
        };
    }
    if matches!(
        normalized_code.as_str(),
        "insufficient_quota" | "quota_exhausted" | "quota_exceeded" | "resource_exhausted"
    ) {
        return ProviderErrorClassification {
            class: ProviderErrorClass::Quota,
            cooldown_ms: 300_000,
        };
    }
    if matches!(
        normalized_code.as_str(),
        "rate_limit_exceeded" | "rate_limit_exceeded_error"
    ) || status == Some(429)
    {
        return ProviderErrorClassification {
            class: ProviderErrorClass::RateLimit,
            cooldown_ms: 60_000,
        };
    }
    if status == Some(404)
        || matches!(normalized_code.as_str(), "model_not_supported")
        || normalized_text.contains("model is not supported")
    {
        return ProviderErrorClassification {
            class: ProviderErrorClass::NotFound,
            cooldown_ms: 0,
        };
    }
    if matches!(status, Some(500 | 502 | 503 | 504)) || normalized_text.contains("overloaded") {
        return ProviderErrorClassification {
            class: ProviderErrorClass::Transient,
            cooldown_ms: 10_000,
        };
    }
    ProviderErrorClassification {
        class: ProviderErrorClass::Other,
        cooldown_ms: 0,
    }
}

pub fn classify_provider_error_body(
    status: u16,
    body: &[u8],
    mut classify: impl FnMut(Option<u16>, Option<&str>, Option<&str>) -> ProviderErrorClassification,
) -> ProviderErrorClassification {
    let text = std::str::from_utf8(body).ok();
    let mut best = classify(Some(status), None, text);
    for token in provider_error_tokens(body) {
        let candidate = classify(Some(status), Some(&token), Some(&token));
        if provider_error_classification_rank(candidate.class)
            < provider_error_classification_rank(best.class)
        {
            best = candidate;
        }
    }
    best
}

fn provider_error_classification_rank(class: ProviderErrorClass) -> u8 {
    match class {
        ProviderErrorClass::Auth => 0,
        ProviderErrorClass::Quota => 1,
        ProviderErrorClass::RateLimit => 2,
        ProviderErrorClass::NotFound => 3,
        ProviderErrorClass::Transient => 4,
        ProviderErrorClass::Other => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::{ProviderErrorClass, classify_provider_error, classify_provider_error_body};

    #[test]
    fn provider_error_body_prefers_structured_quota_over_generic_429() {
        let body = serde_json::to_vec(&serde_json::json!({
            "error": {
                "status": "RESOURCE_EXHAUSTED",
                "message": "Quota exceeded."
            }
        }))
        .unwrap();

        let classified = classify_provider_error_body(429, &body, classify_provider_error);

        assert_eq!(classified.class, ProviderErrorClass::Quota);
        assert_eq!(classified.cooldown_ms, 300_000);
    }

    #[test]
    fn provider_error_body_reads_sse_error_tokens() {
        let body = b"event: error\ndata: {\"error\":{\"code\":\"model_not_supported\"}}\n\n";

        let classified = classify_provider_error_body(400, body, classify_provider_error);

        assert_eq!(classified.class, ProviderErrorClass::NotFound);
    }
}
