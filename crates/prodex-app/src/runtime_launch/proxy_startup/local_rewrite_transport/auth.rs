//! Auth attempt ordering and transport-local header filtering.

use super::super::anthropic_rewrite::{RuntimeAnthropicAuth, RuntimeAnthropicProviderAuth};
use super::super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::RuntimeLocalRewriteSelectedAnthropicAuth;
use crate::RuntimeProxyRequest;
use std::sync::atomic::Ordering;

pub(in crate::runtime_launch::proxy_startup) fn runtime_local_rewrite_api_key_attempts<'a>(
    shared: &RuntimeLocalRewriteProxyShared,
    api_keys: &'a [String],
) -> Vec<(String, &'a str)> {
    if api_keys.is_empty() {
        return Vec::new();
    }
    let start = if api_keys.len() == 1 {
        0
    } else {
        shared.api_key_cursor.fetch_add(1, Ordering::Relaxed) % api_keys.len()
    };
    runtime_local_rewrite_api_key_attempts_from_start(api_keys, start)
}

fn runtime_local_rewrite_api_key_attempts_from_start(
    api_keys: &[String],
    start: usize,
) -> Vec<(String, &str)> {
    (0..api_keys.len())
        .map(|offset| {
            let index = (start + offset) % api_keys.len();
            let label = if api_keys.len() == 1 {
                "api-key".to_string()
            } else {
                format!("api-key-{}", index + 1)
            };
            (label, api_keys[index].as_str())
        })
        .collect()
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_local_rewrite_anthropic_auth_attempts(
    shared: &RuntimeLocalRewriteProxyShared,
    auth: &RuntimeAnthropicProviderAuth,
) -> Vec<RuntimeLocalRewriteSelectedAnthropicAuth> {
    match auth {
        RuntimeAnthropicProviderAuth::ApiKeys { api_keys } => {
            runtime_local_rewrite_api_key_attempts(shared, api_keys)
                .into_iter()
                .map(
                    |(label, api_key)| RuntimeLocalRewriteSelectedAnthropicAuth {
                        label,
                        auth: RuntimeAnthropicAuth::ApiKey {
                            api_key: api_key.to_string(),
                        },
                    },
                )
                .collect()
        }
        RuntimeAnthropicProviderAuth::OAuthProfiles { profiles } => {
            if profiles.is_empty() {
                return Vec::new();
            }
            let start = if profiles.len() == 1 {
                0
            } else {
                shared.api_key_cursor.fetch_add(1, Ordering::Relaxed) % profiles.len()
            };
            (0..profiles.len())
                .map(|offset| {
                    let index = (start + offset) % profiles.len();
                    let profile = profiles[index].clone();
                    RuntimeLocalRewriteSelectedAnthropicAuth {
                        label: profile.profile_name.clone(),
                        auth: profile.auth(),
                    }
                })
                .collect()
        }
    }
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_local_rewrite_header<'a>(
    request: &'a RuntimeProxyRequest,
    expected_name: &str,
) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(expected_name))
        .map(|(_, value)| value.as_str())
}

pub(in crate::runtime_launch::proxy_startup) fn should_skip_runtime_local_rewrite_request_header(
    name: &str,
) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "connection"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_key_attempts_rotate_start_and_include_all_keys() {
        let api_keys = vec![
            "first".to_string(),
            "second".to_string(),
            "third".to_string(),
        ];

        let attempts = runtime_local_rewrite_api_key_attempts_from_start(&api_keys, 1);

        assert_eq!(
            attempts,
            vec![
                ("api-key-2".to_string(), "second"),
                ("api-key-3".to_string(), "third"),
                ("api-key-1".to_string(), "first"),
            ]
        );
    }
    #[test]
    fn single_api_key_attempt_uses_generic_label() {
        let api_keys = vec!["only".to_string()];

        let attempts = runtime_local_rewrite_api_key_attempts_from_start(&api_keys, 0);

        assert_eq!(attempts, vec![("api-key".to_string(), "only")]);
    }
}
