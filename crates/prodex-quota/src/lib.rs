mod auth;
mod models;
mod render;

pub use self::auth::*;
pub use self::models::*;
pub use self::render::*;

pub fn usage_url(base_url: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    if base_url.contains("/backend-api") {
        format!("{base_url}/wham/usage")
    } else {
        format!("{base_url}/api/codex/usage")
    }
}

pub fn format_response_body(body: &[u8]) -> String {
    if body.is_empty() {
        return String::new();
    }

    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
        return serde_json::to_string_pretty(&value)
            .unwrap_or_else(|_| String::from_utf8_lossy(body).trim().to_string());
    }

    String::from_utf8_lossy(body).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_url_matches_backend_shape() {
        assert_eq!(
            usage_url("https://chatgpt.com/backend-api"),
            "https://chatgpt.com/backend-api/wham/usage"
        );
        assert_eq!(
            usage_url("https://chatgpt.com"),
            "https://chatgpt.com/api/codex/usage"
        );
    }

    #[test]
    fn response_body_pretty_prints_json() {
        let formatted = format_response_body(br#"{"error":{"message":"quota"}}"#);
        assert!(formatted.contains("\"message\": \"quota\""));
    }

    #[test]
    fn quota_auth_filter_matches_labels_and_compatibility() {
        let no_auth = AuthSummary {
            label: "no-auth".to_string(),
            quota_compatible: false,
        };
        let chatgpt = AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        };

        assert!(QuotaAuthFilter::parse("no-auth").unwrap().matches(&no_auth));
        assert!(!QuotaAuthFilter::parse("no-auth").unwrap().matches(&chatgpt));
        assert!(
            QuotaAuthFilter::parse("quota-compatible")
                .unwrap()
                .matches(&chatgpt)
        );
        assert!(
            QuotaAuthFilter::parse("non-quota-compatible")
                .unwrap()
                .matches(&no_auth)
        );
    }
}
