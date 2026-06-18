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
    if status == Some(404) {
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
