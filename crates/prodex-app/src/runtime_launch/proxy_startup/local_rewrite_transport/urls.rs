use super::*;

pub(super) fn runtime_local_rewrite_upstream_url(
    base_url: &str,
    mount_path: &str,
    path_and_query: &str,
) -> String {
    let base_url = base_url.trim_end_matches('/');
    let mount_path = mount_path.trim_end_matches('/');
    let path_and_query = runtime_proxy_crate::runtime_escape_url_path_dot_segments(path_and_query);
    let (path, query) = path_and_query
        .as_ref()
        .split_once('?')
        .map(|(path, query)| (path, Some(query)))
        .unwrap_or((path_and_query.as_ref(), None));
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

pub(super) fn runtime_local_rewrite_log_url(value: &str) -> String {
    if let Ok(mut url) = reqwest::Url::parse(value) {
        let _ = url.set_username("");
        let _ = url.set_password(None);
        url.set_query(None);
        url.set_fragment(None);
        return url.to_string();
    }
    value
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(value)
        .to_string()
}

pub(super) fn runtime_deepseek_upstream_url(
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

pub(super) fn runtime_deepseek_anthropic_messages_upstream_url(base_url: &str) -> String {
    let mut base_url = base_url.trim_end_matches('/');
    if base_url.ends_with("/anthropic/v1") {
        return format!("{base_url}/messages");
    }
    if base_url.ends_with("/anthropic") {
        return format!("{base_url}/v1/messages");
    }
    for suffix in ["/v1", "/beta"] {
        if let Some(root) = base_url.strip_suffix(suffix) {
            base_url = root;
            break;
        }
    }
    format!("{base_url}/anthropic/v1/messages")
}

pub(super) fn runtime_openai_standard_provider_upstream_url(
    provider_kind: RuntimeProviderBridgeKind,
    base_url: &str,
    mount_path: &str,
    path_and_query: &str,
) -> String {
    let adapter = provider_adapter(provider_kind.provider_id());
    let path = path_without_query(path_and_query);
    if path.ends_with("/responses")
        && matches!(
            adapter.upstream_request_format(),
            ProviderWireFormat::OpenAiChatCompletions
        )
    {
        return runtime_local_rewrite_upstream_url(base_url, mount_path, "/chat/completions");
    }
    runtime_local_rewrite_upstream_url(base_url, mount_path, path_and_query)
}

pub(super) fn runtime_anthropic_messages_upstream_url(base_url: &str, mount_path: &str) -> String {
    runtime_local_rewrite_upstream_url(base_url, mount_path, "/messages")
}

pub(super) fn runtime_gemini_openai_compatible_upstream_url(base_url: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    if base_url.ends_with("/openai") {
        format!("{base_url}/chat/completions")
    } else {
        format!("{base_url}/openai/chat/completions")
    }
}
