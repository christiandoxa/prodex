mod body;
use self::body::provider_error_codes;

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
    let (class, cooldown_ms) = prodex_mojo_core::rich::provider_error_classify(status, code, text)
        .expect("Mojo provider error classifier returned invalid output");
    ProviderErrorClassification {
        class: match class {
            0 => ProviderErrorClass::Auth,
            1 => ProviderErrorClass::Quota,
            2 => ProviderErrorClass::RateLimit,
            3 => ProviderErrorClass::Transient,
            4 => ProviderErrorClass::NotFound,
            5 => ProviderErrorClass::Other,
            _ => unreachable!("validated Mojo provider error class"),
        },
        cooldown_ms,
    }
}

pub fn classify_provider_error_body(
    status: u16,
    body: &[u8],
    mut classify: impl FnMut(Option<u16>, Option<&str>, Option<&str>) -> ProviderErrorClassification,
) -> ProviderErrorClassification {
    let text = std::str::from_utf8(body).ok();
    let mut best = if status == 429 {
        ProviderErrorClassification {
            class: ProviderErrorClass::Other,
            cooldown_ms: 0,
        }
    } else {
        classify(Some(status), None, text)
    };
    for token in provider_error_codes(body) {
        // A bare 429 is deliberately omitted here: retry eligibility must come
        // from the structured provider code, never the status or message alone.
        let candidate = classify(
            (status != 429).then_some(status),
            Some(&token),
            (status != 429).then_some(token.as_str()),
        );
        if provider_error_classification_rank(candidate.class)
            < provider_error_classification_rank(best.class)
        {
            best = candidate;
        }
    }
    best
}

/// True only when the provider error explicitly identifies a request member as rejected.
pub fn provider_error_rejects_request_member(body: &[u8], member: &str) -> bool {
    fn normalized(value: &str) -> String {
        value
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect()
    }

    fn mentions_member(value: &str, member: &str) -> bool {
        normalized(value).contains(member)
    }

    fn has_rejection_marker(value: &str) -> bool {
        let value = value.to_ascii_lowercase();
        [
            "unsupported",
            "not supported",
            "does not support",
            "unknown_parameter",
            "unknown parameter",
            "unknown_field",
            "unknown field",
            "unknown name",
            "unrecognized",
            "unexpected",
            "not allowed",
            "invalid_argument",
            "invalid argument",
            "invalid_parameter",
            "invalid parameter",
            "extra inputs are not permitted",
        ]
        .into_iter()
        .any(|marker| value.contains(marker))
    }

    fn value_mentions_member(value: &serde_json::Value, member: &str) -> bool {
        match value {
            serde_json::Value::String(value) => mentions_member(value, member),
            serde_json::Value::Array(values) => values
                .iter()
                .any(|value| value_mentions_member(value, member)),
            _ => false,
        }
    }

    fn value_has_rejection_marker(value: &serde_json::Value) -> bool {
        match value {
            serde_json::Value::String(value) => has_rejection_marker(value),
            serde_json::Value::Array(values) => values.iter().any(value_has_rejection_marker),
            _ => false,
        }
    }

    fn explicitly_rejects(value: &serde_json::Value, member: &str) -> bool {
        match value {
            serde_json::Value::String(value) => {
                mentions_member(value, member) && has_rejection_marker(value)
            }
            serde_json::Value::Array(values) => {
                values.iter().any(|value| explicitly_rejects(value, member))
            }
            serde_json::Value::Object(values) => {
                let identifies_member = values.iter().any(|(key, value)| {
                    mentions_member(key, member)
                        || matches!(
                            key.to_ascii_lowercase().as_str(),
                            "param" | "parameter" | "field" | "name" | "path" | "loc" | "location"
                        ) && value_mentions_member(value, member)
                });
                let rejects = values.iter().any(|(key, value)| {
                    matches!(
                        key.to_ascii_lowercase().as_str(),
                        "code" | "status" | "type" | "message" | "detail" | "reason"
                    ) && value_has_rejection_marker(value)
                });
                (identifies_member && rejects)
                    || values
                        .values()
                        .any(|value| explicitly_rejects(value, member))
            }
            _ => false,
        }
    }

    let member = member
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    if member.is_empty() {
        return false;
    }
    serde_json::from_slice(body).map_or_else(
        |_| {
            let text = String::from_utf8_lossy(body);
            mentions_member(&text, &member) && has_rejection_marker(&text)
        },
        |value| explicitly_rejects(&value, &member),
    )
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
mod classifier_tests {
    use super::*;

    #[test]
    fn provider_error_classifier_mojo_matches_expected_policy() {
        let cases = [
            (Some(401), None, None, ProviderErrorClass::Auth, 0_u64),
            (Some(403), None, None, ProviderErrorClass::Auth, 0),
            (
                None,
                Some(" UNAUTHENTICATED "),
                None,
                ProviderErrorClass::Auth,
                0,
            ),
            (
                None,
                Some("\u{2003}INVALID_API_KEY\u{3000}"),
                None,
                ProviderErrorClass::Auth,
                0,
            ),
            (
                None,
                Some("organization_spend_limit_exceeded"),
                None,
                ProviderErrorClass::Quota,
                300_000,
            ),
            (
                Some(429),
                Some("rate_limit_exceeded"),
                None,
                ProviderErrorClass::RateLimit,
                60_000,
            ),
            (
                None,
                Some("slow_down"),
                None,
                ProviderErrorClass::RateLimit,
                60_000,
            ),
            (Some(404), None, None, ProviderErrorClass::NotFound, 0),
            (
                Some(401),
                Some("insufficient_quota"),
                None,
                ProviderErrorClass::Auth,
                0,
            ),
            (
                Some(503),
                Some("rate_limit_exceeded"),
                None,
                ProviderErrorClass::RateLimit,
                60_000,
            ),
            (
                Some(404),
                Some("quota_exhausted"),
                Some("backend overloaded"),
                ProviderErrorClass::Quota,
                300_000,
            ),
            (
                Some(404),
                None,
                Some("backend overloaded"),
                ProviderErrorClass::NotFound,
                0,
            ),
            (
                Some(500),
                Some("model_not_supported"),
                None,
                ProviderErrorClass::NotFound,
                0,
            ),
            (
                None,
                Some("model_not_supported"),
                None,
                ProviderErrorClass::NotFound,
                0,
            ),
            (
                None,
                None,
                Some("The selected MODEL IS NOT SUPPORTED here"),
                ProviderErrorClass::NotFound,
                0,
            ),
            (Some(503), None, None, ProviderErrorClass::Transient, 10_000),
            (
                None,
                None,
                Some("backend currently OVERLOADED"),
                ProviderErrorClass::Transient,
                10_000,
            ),
            (
                Some(429),
                None,
                Some("too many requests"),
                ProviderErrorClass::Other,
                0,
            ),
            (
                None,
                Some("unknown"),
                Some("ordinary error"),
                ProviderErrorClass::Other,
                0,
            ),
            (
                None,
                Some("{\"error\":\"unterminated"),
                Some("\0 malformed { json �"),
                ProviderErrorClass::Other,
                0,
            ),
        ];
        for (status, code, text, class, cooldown_ms) in cases {
            assert_eq!(
                classify_provider_error(status, code, text),
                ProviderErrorClassification { class, cooldown_ms },
                "status={status:?} code={code:?} text={text:?}"
            );
        }
    }

    #[test]
    fn provider_error_classifier_trims_unicode_whitespace() {
        for whitespace in [
            "\u{0085}", "\u{00a0}", "\u{1680}", "\u{2000}", "\u{2003}", "\u{200a}", "\u{2028}",
            "\u{2029}", "\u{202f}", "\u{205f}", "\u{3000}",
        ] {
            let code = format!("{whitespace}invalid_api_key{whitespace}");
            assert_eq!(
                classify_provider_error(None, Some(&code), None),
                super::ProviderErrorClassification {
                    class: ProviderErrorClass::Auth,
                    cooldown_ms: 0,
                },
                "whitespace={whitespace:?}"
            );
        }
    }

    #[test]
    fn provider_error_classifier_handles_large_inputs() {
        let large = "x".repeat(1 << 20);
        let large_text = format!("{large} overloaded");
        assert_eq!(
            classify_provider_error(None, Some(&large), None),
            super::ProviderErrorClassification {
                class: ProviderErrorClass::Other,
                cooldown_ms: 0,
            }
        );
        assert_eq!(
            classify_provider_error(None, None, Some(&large_text)),
            super::ProviderErrorClassification {
                class: ProviderErrorClass::Transient,
                cooldown_ms: 10_000,
            }
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ProviderErrorClass, classify_provider_error, classify_provider_error_body,
        provider_error_rejects_request_member,
    };

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
    fn generic_429_is_not_rotatable_without_a_structured_code() {
        for body in [
            b"too many requests".as_slice(),
            b"rate_limit_exceeded".as_slice(),
            b"server is overloaded".as_slice(),
            br#"{"error":{"message":"rate_limit_exceeded"}}"#,
            br#"{"error":{"message":"server is overloaded"}}"#,
            br#"{"error":{"code":"429"}}"#,
            br#"{"error":{"reason":"server is overloaded; try again"}}"#,
            br#"{"error":"rate_limit_exceeded"}"#,
            br#"{"error":"insufficient_quota"}"#,
            b"<html>rate limit exceeded</html>".as_slice(),
        ] {
            let classified = classify_provider_error_body(429, body, classify_provider_error);

            assert_eq!(classified.class, ProviderErrorClass::Other, "{body:?}");
            assert_eq!(classified.cooldown_ms, 0, "{body:?}");
        }
    }

    #[test]
    fn structured_rate_limit_codes_still_classify_a_429_as_rotatable() {
        for code in ["rate_limit_exceeded", "slow_down"] {
            let body = serde_json::to_vec(&serde_json::json!({
                "error": {"code": code}
            }))
            .unwrap();
            let classified = classify_provider_error_body(429, &body, classify_provider_error);

            assert_eq!(classified.class, ProviderErrorClass::RateLimit, "{code}");
            assert_eq!(classified.cooldown_ms, 60_000, "{code}");
        }
    }

    #[test]
    fn codex_0156_spend_limit_codes_classify_as_quota() {
        for code in [
            "credit_balance_exhausted",
            "organization_spend_limit_exceeded",
            "project_spend_limit_exceeded",
        ] {
            let body = serde_json::to_vec(&serde_json::json!({
                "error": {"code": code}
            }))
            .unwrap();
            let classified = classify_provider_error_body(429, &body, classify_provider_error);

            assert_eq!(classified.class, ProviderErrorClass::Quota, "{code}");
            assert_eq!(classified.cooldown_ms, 300_000, "{code}");
        }
    }

    #[test]
    fn provider_error_body_reads_sse_error_tokens() {
        let body = b"event: error\ndata: {\"error\":{\"code\":\"model_not_supported\"}}\n\n";

        let classified = classify_provider_error_body(400, body, classify_provider_error);

        assert_eq!(classified.class, ProviderErrorClass::NotFound);
    }

    #[test]
    fn request_member_rejection_requires_the_member_and_rejection_signal() {
        assert!(provider_error_rejects_request_member(
            br#"{"error":{"code":"unknown_parameter","param":"web_search_options"}}"#,
            "web_search_options",
        ));
        assert!(provider_error_rejects_request_member(
            br#"{"error":{"status":"INVALID_ARGUMENT","message":"Unknown name googleSearch"}}"#,
            "googleSearch",
        ));
        assert!(!provider_error_rejects_request_member(
            br#"{"error":{"code":"invalid_parameter","param":"temperature"}}"#,
            "web_search_options",
        ));
        assert!(!provider_error_rejects_request_member(
            br#"{"error":{"message":"web_search_options accepted"}}"#,
            "web_search_options",
        ));
        assert!(!provider_error_rejects_request_member(
            br#"{"error":{"code":"invalid_parameter","param":"temperature"},"request":{"web_search_options":{}}}"#,
            "web_search_options",
        ));
    }
}
