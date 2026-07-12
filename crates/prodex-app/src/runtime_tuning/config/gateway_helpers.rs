use super::*;

pub(super) fn runtime_gateway_policy_provider(value: &str) -> Option<SuperExternalProvider> {
    match value.to_ascii_lowercase().as_str() {
        "anthropic" | "claude" => Some(SuperExternalProvider::Anthropic),
        "copilot" | "github-copilot" | "github_copilot" => Some(SuperExternalProvider::Copilot),
        "deepseek" => Some(SuperExternalProvider::DeepSeek),
        "gemini" => Some(SuperExternalProvider::Gemini),
        "kiro" => Some(SuperExternalProvider::Kiro),
        _ => None,
    }
}

pub(super) fn runtime_gateway_upstream_base_url(
    parser: &mut RuntimeConfigParser,
    cli: Option<&str>,
    policy: Option<&str>,
    provider: Option<SuperExternalProvider>,
) -> String {
    let (key, raw) = if let Some(value) = cli {
        ("gateway --base-url", value)
    } else if let Some(value) = policy {
        ("gateway.base_url", value)
    } else if let Some(provider) = provider {
        ("gateway provider base URL", provider.default_base_url())
    } else if let Some(value) = parser.environment.get("OPENAI_BASE_URL") {
        let Some(value) = value.to_str() else {
            parser.errors.push(ConfigError {
                key: "OPENAI_BASE_URL",
                message: "must be valid Unicode".to_string(),
            });
            return "https://api.openai.com/v1".to_string();
        };
        ("OPENAI_BASE_URL", value)
    } else {
        ("OPENAI_BASE_URL", "https://api.openai.com/v1")
    };
    if raw.is_empty() {
        parser.errors.push(ConfigError {
            key,
            message: "cannot be empty".to_string(),
        });
        return "https://api.openai.com/v1".to_string();
    }
    if raw.chars().any(char::is_whitespace) {
        parser.errors.push(ConfigError {
            key,
            message: "must not contain whitespace".to_string(),
        });
        return "https://api.openai.com/v1".to_string();
    }
    let normalized = raw.trim_end_matches('/');
    let parsed = reqwest::Url::parse(normalized);
    let valid = parsed.as_ref().is_ok_and(|url| {
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
    });
    if !valid {
        parser.errors.push(ConfigError {
            key,
            message: "must be an http(s) URL with host and no credentials, query, or fragment"
                .to_string(),
        });
        return "https://api.openai.com/v1".to_string();
    }
    if provider.is_none()
        && parsed
            .ok()
            .is_some_and(|url| url.path().trim_matches('/').is_empty())
    {
        format!("{normalized}/v1")
    } else {
        normalized.to_string()
    }
}

pub(super) fn runtime_gateway_capture<T>(
    parser: &mut RuntimeConfigParser,
    key: &'static str,
    result: anyhow::Result<T>,
    default: T,
) -> T {
    match result {
        Ok(value) => value,
        Err(error) => {
            let message = error.to_string();
            let message = message
                .strip_prefix(key)
                .and_then(|message| message.strip_prefix(' '))
                .unwrap_or(&message)
                .to_string();
            parser.errors.push(ConfigError { key, message });
            default
        }
    }
}

pub(super) struct ParsedWebsocketTuning {
    pub(super) connect_timeout_ms: u64,
    pub(super) happy_eyeballs_delay_ms: u64,
    pub(super) precommit_progress_timeout_ms: u64,
    pub(super) connect_worker_count: usize,
    pub(super) connect_queue_capacity: usize,
    pub(super) connect_overflow_capacity: usize,
    pub(super) dns_worker_count: usize,
    pub(super) dns_queue_capacity: usize,
    pub(super) dns_overflow_capacity: usize,
}

pub(super) fn nonzero(value: usize) -> Option<usize> {
    (value > 0).then_some(value)
}
