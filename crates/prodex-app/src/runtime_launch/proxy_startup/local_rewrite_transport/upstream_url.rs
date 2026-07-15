//! Provider upstream URL mapping for local rewrite routes.

use super::super::provider_bridge::{
    RuntimeProviderBridgeKind, RuntimeProviderRouteKind, RuntimeProviderWireFormat,
    runtime_provider_openai_contract, runtime_provider_route_kind,
};
use runtime_proxy_crate::path_without_query;

pub(in crate::runtime_launch::proxy_startup) fn runtime_local_rewrite_upstream_url(
    base_url: &str,
    mount_path: &str,
    path_and_query: &str,
) -> String {
    let base_url = base_url.trim_end_matches('/');
    let mount_path = mount_path.trim_end_matches('/');
    let (path, query) = path_and_query
        .split_once('?')
        .map(|(path, query)| (path, Some(query)))
        .unwrap_or((path_and_query, None));
    let suffix = path
        .strip_prefix(mount_path)
        .filter(|suffix| suffix.is_empty() || suffix.starts_with('/'))
        .unwrap_or(path);
    let mut upstream_url = if suffix.is_empty() {
        base_url.to_string()
    } else if suffix.starts_with('/') {
        format!("{base_url}{suffix}")
    } else {
        format!("{base_url}/{suffix}")
    };
    if let Some(query) = query {
        upstream_url.push('?');
        upstream_url.push_str(query);
    }
    upstream_url
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_upstream_url(
    base_url: &str,
    mount_path: &str,
    path_and_query: &str,
) -> String {
    runtime_openai_standard_provider_upstream_url(
        RuntimeProviderBridgeKind::DeepSeek,
        base_url,
        mount_path,
        path_and_query,
    )
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_openai_standard_provider_upstream_url(
    provider_kind: RuntimeProviderBridgeKind,
    base_url: &str,
    mount_path: &str,
    path_and_query: &str,
) -> String {
    let contract = runtime_provider_openai_contract(provider_kind);
    let path = path_without_query(path_and_query);
    if matches!(
        runtime_provider_route_kind(path),
        Some(RuntimeProviderRouteKind::Responses)
    ) && matches!(
        contract.upstream_request_format,
        RuntimeProviderWireFormat::OpenAiChatCompletions
    ) {
        return runtime_local_rewrite_upstream_url(base_url, mount_path, "/chat/completions");
    }
    runtime_local_rewrite_upstream_url(base_url, mount_path, path_and_query)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gemini_openai_compatible_upstream_url(
    base_url: &str,
) -> String {
    let base_url = base_url.trim_end_matches('/');
    if base_url.ends_with("/openai") {
        format!("{base_url}/chat/completions")
    } else {
        format!("{base_url}/openai/chat/completions")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_standard_provider_upstream_url_uses_contract_formats() {
        assert_eq!(
            runtime_openai_standard_provider_upstream_url(
                RuntimeProviderBridgeKind::OpenAiResponses,
                "https://upstream.test/v1",
                "/v1",
                "/v1/responses"
            ),
            "https://upstream.test/v1/responses"
        );
        assert_eq!(
            runtime_openai_standard_provider_upstream_url(
                RuntimeProviderBridgeKind::DeepSeek,
                "https://upstream.test/v1",
                "/v1",
                "/v1/responses"
            ),
            "https://upstream.test/v1/chat/completions"
        );
        assert_eq!(
            runtime_openai_standard_provider_upstream_url(
                RuntimeProviderBridgeKind::Copilot,
                "https://upstream.test/v1",
                "/v1",
                "/v1/chat/completions"
            ),
            "https://upstream.test/v1/chat/completions"
        );
        for path in [
            "/v1/embeddings",
            "/v1/images/generations",
            "/v1/audio/transcriptions",
            "/v1/batches",
            "/v1/rerank",
            "/v1/a2a",
            "/v1/messages",
        ] {
            assert_eq!(
                runtime_openai_standard_provider_upstream_url(
                    RuntimeProviderBridgeKind::OpenAiResponses,
                    "https://upstream.test/v1",
                    "/v1",
                    path
                ),
                format!("https://upstream.test{path}")
            );
        }
    }

    #[test]
    fn gemini_openai_compatible_url_uses_documented_chat_completions_endpoint() {
        assert_eq!(
            runtime_gemini_openai_compatible_upstream_url(
                "https://generativelanguage.googleapis.com/v1beta"
            ),
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
        );
        assert_eq!(
            runtime_gemini_openai_compatible_upstream_url(
                "https://generativelanguage.googleapis.com/v1beta/openai/"
            ),
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
        );
    }
}
