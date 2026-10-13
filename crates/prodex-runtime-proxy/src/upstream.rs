use crate::runtime_proxy_normalize_openai_path;
use prodex_mojo_core::rich::{
    ascii_casefold_equal_exact, runtime_proxy_escape_dot_segments, runtime_proxy_header_skip_kind,
};
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
    runtime_proxy_escape_dot_segments(path_and_query)
        .expect("Mojo runtime dot-segment escaping returned invalid output")
}

pub fn is_runtime_transport_local_request_header(name: &str) -> bool {
    runtime_proxy_header_skip_kind(name)
        .expect("Mojo runtime header policy returned invalid output")
        == 1
}

pub fn is_prodex_internal_request_header(name: &str) -> bool {
    runtime_proxy_header_skip_kind(name)
        .expect("Mojo runtime header policy returned invalid output")
        == 2
}

pub fn should_skip_runtime_request_header(name: &str) -> bool {
    runtime_proxy_header_skip_kind(name)
        .expect("Mojo runtime header policy returned invalid output")
        != 0
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
        .filter(|(name, _)| {
            ascii_casefold_equal_exact(name, "connection")
                .expect("Mojo connection header-name comparison failed")
        })
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
    tokens.iter().any(|token| {
        ascii_casefold_equal_exact(token, name).expect("Mojo connection-token comparison failed")
    })
}

#[cfg(test)]
pub(crate) fn runtime_proxy_effective_user_agent(headers: &[(String, String)]) -> Option<&str> {
    headers.iter().find_map(|(name, value)| {
        ascii_casefold_equal_exact(name, "user-agent")
            .expect("Mojo user-agent header-name comparison failed")
            .then_some(value.as_str())
            .filter(|value| !value.is_empty())
    })
}

#[cfg(test)]
#[path = "../tests/src/upstream.rs"]
mod tests;
