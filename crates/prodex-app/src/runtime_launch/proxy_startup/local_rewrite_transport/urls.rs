use super::{RuntimeProviderBridgeKind, path_without_query};
use prodex_provider_core::provider_adapter;

pub(in crate::runtime_launch::proxy_startup) fn runtime_local_rewrite_upstream_url(
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

pub(in crate::runtime_launch::proxy_startup) fn runtime_local_rewrite_log_url(
    value: &str,
) -> String {
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

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_anthropic_messages_upstream_url(
    base_url: &str,
) -> String {
    let mut base_url = base_url.trim_end_matches('/');
    match prodex_mojo_core::provider_upstream::provider_suffix_plan(0, base_url)
        .expect("Mojo DeepSeek upstream suffix policy returned invalid output")
    {
        0 => format!("{base_url}/messages"),
        1 => format!("{base_url}/v1/messages"),
        2 => {
            base_url = base_url
                .strip_suffix("/v1")
                .or_else(|| base_url.strip_suffix("/beta"))
                .expect("Mojo DeepSeek suffix policy selected a missing suffix");
            format!("{base_url}/anthropic/v1/messages")
        }
        3 => format!("{base_url}/anthropic/v1/messages"),
        _ => unreachable!("validated Mojo DeepSeek upstream suffix policy"),
    }
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_openai_standard_provider_upstream_url(
    provider_kind: RuntimeProviderBridgeKind,
    base_url: &str,
    mount_path: &str,
    path_and_query: &str,
) -> String {
    let adapter = provider_adapter(provider_kind.provider_id());
    let path = path_without_query(path_and_query);
    let use_chat_path = prodex_mojo_core::provider_upstream::standard_path_uses_chat(
        adapter.upstream_request_format() as i64,
        path.ends_with("/responses"),
    )
    .expect("Mojo provider upstream path policy returned invalid output");
    if use_chat_path {
        return runtime_local_rewrite_upstream_url(base_url, mount_path, "/chat/completions");
    }
    runtime_local_rewrite_upstream_url(base_url, mount_path, path_and_query)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_anthropic_messages_upstream_url(
    base_url: &str,
    mount_path: &str,
) -> String {
    runtime_local_rewrite_upstream_url(base_url, mount_path, "/messages")
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gemini_openai_compatible_upstream_url(
    base_url: &str,
) -> String {
    let base_url = base_url.trim_end_matches('/');
    match prodex_mojo_core::provider_upstream::provider_suffix_plan(1, base_url)
        .expect("Mojo Gemini upstream suffix policy returned invalid output")
    {
        0 => format!("{base_url}/chat/completions"),
        1 => format!("{base_url}/openai/chat/completions"),
        _ => unreachable!("validated Mojo Gemini upstream suffix policy"),
    }
}
