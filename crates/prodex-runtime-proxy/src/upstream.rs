use crate::runtime_proxy_normalize_openai_path;
use std::borrow::Cow;

pub fn runtime_proxy_upstream_url(base_url: &str, path_and_query: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    let normalized_path_and_query = runtime_proxy_normalize_openai_path(path_and_query);
    let normalized_path_and_query =
        runtime_escape_url_path_dot_segments(normalized_path_and_query.as_ref());
    if base_url.ends_with("/backend-api")
        && let Some(suffix) = normalized_path_and_query
            .as_ref()
            .strip_prefix("/backend-api")
            .filter(|suffix| suffix.is_empty() || suffix.starts_with('/'))
    {
        return format!("{base_url}{suffix}");
    }
    if normalized_path_and_query.starts_with('/') {
        return format!("{base_url}{normalized_path_and_query}");
    }
    format!("{base_url}/{normalized_path_and_query}")
}

pub fn runtime_escape_url_path_dot_segments(path_and_query: &str) -> Cow<'_, str> {
    let (path, query) = path_and_query
        .split_once('?')
        .map(|(path, query)| (path, Some(query)))
        .unwrap_or((path_and_query, None));
    let mut escaped = String::new();
    let mut changed = false;
    for (index, segment) in path.split('/').enumerate() {
        if index > 0 {
            escaped.push('/');
        }
        match runtime_encoded_dot_segment_len(segment) {
            Some(1) => {
                escaped.push_str("%252e");
                changed = true;
            }
            Some(2) => {
                escaped.push_str("%252e%252e");
                changed = true;
            }
            _ => escaped.push_str(segment),
        }
    }
    if !changed {
        return Cow::Borrowed(path_and_query);
    }
    if let Some(query) = query {
        escaped.push('?');
        escaped.push_str(query);
    }
    Cow::Owned(escaped)
}

fn runtime_encoded_dot_segment_len(segment: &str) -> Option<usize> {
    let bytes = segment.as_bytes();
    let mut index = 0;
    let mut dots = 0;
    while index < bytes.len() {
        if bytes[index] == b'.' {
            dots += 1;
            index += 1;
        } else if index + 2 < bytes.len()
            && bytes[index] == b'%'
            && bytes[index + 1] == b'2'
            && bytes[index + 2].eq_ignore_ascii_case(&b'e')
        {
            dots += 1;
            index += 3;
        } else {
            return None;
        }
    }
    matches!(dots, 1 | 2).then_some(dots)
}

pub fn should_skip_runtime_request_header(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "authorization"
            | "chatgpt-account-id"
            | "connection"
            | "content-length"
            | "host"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    ) || lower.starts_with("sec-websocket-")
        || lower.starts_with("x-prodex-internal-")
}

pub fn runtime_forward_request_headers<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Vec<(&'a str, &'a str)> {
    let headers = headers.into_iter().collect::<Vec<_>>();
    let connection_headers = runtime_connection_header_tokens(headers.iter().copied());
    headers
        .into_iter()
        .filter(|(name, _)| {
            !runtime_header_name_matches_connection_token(name, &connection_headers)
        })
        .filter(|(name, _)| !should_skip_runtime_request_header(name))
        .collect()
}

pub fn runtime_connection_header_tokens<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Vec<String> {
    headers
        .into_iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("connection"))
        .flat_map(|(_, value)| value.split(','))
        .map(str::trim)
        .filter(|token| {
            !token.is_empty()
                && token.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric()
                        || matches!(
                            byte,
                            b'!' | b'#'
                                | b'$'
                                | b'%'
                                | b'&'
                                | b'\''
                                | b'*'
                                | b'+'
                                | b'-'
                                | b'.'
                                | b'^'
                                | b'_'
                                | b'`'
                                | b'|'
                                | b'~'
                        )
                })
        })
        .map(str::to_string)
        .collect()
}

pub fn runtime_header_name_matches_connection_token(name: &str, tokens: &[String]) -> bool {
    let name = name.trim();
    tokens.iter().any(|token| token.eq_ignore_ascii_case(name))
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
