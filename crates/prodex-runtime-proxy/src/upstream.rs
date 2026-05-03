use crate::runtime_proxy_normalize_openai_path;

pub fn runtime_proxy_upstream_url(base_url: &str, path_and_query: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    let normalized_path_and_query = runtime_proxy_normalize_openai_path(path_and_query);
    if base_url.contains("/backend-api")
        && let Some(suffix) = normalized_path_and_query
            .as_ref()
            .strip_prefix("/backend-api")
    {
        return format!("{base_url}{suffix}");
    }
    if normalized_path_and_query.starts_with('/') {
        return format!("{base_url}{normalized_path_and_query}");
    }
    format!("{base_url}/{normalized_path_and_query}")
}

pub fn should_skip_runtime_request_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "authorization"
            | "chatgpt-account-id"
            | "connection"
            | "content-length"
            | "host"
            | "transfer-encoding"
            | "upgrade"
    ) || lower.starts_with("sec-websocket-")
        || lower.starts_with("x-prodex-internal-")
}

pub fn runtime_proxy_effective_user_agent(headers: &[(String, String)]) -> Option<&str> {
    headers.iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("user-agent")
            .then_some(value.as_str())
            .filter(|value| !value.is_empty())
    })
}

#[cfg(test)]
#[path = "../tests/src/upstream.rs"]
mod tests;
