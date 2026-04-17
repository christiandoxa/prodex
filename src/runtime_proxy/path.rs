use super::*;

pub(crate) fn is_runtime_responses_path(path_and_query: &str) -> bool {
    let normalized_path_and_query = runtime_proxy_normalize_openai_path(path_and_query);
    path_without_query(normalized_path_and_query.as_ref()).ends_with("/codex/responses")
}

pub(crate) fn is_runtime_anthropic_messages_path(path_and_query: &str) -> bool {
    path_without_query(path_and_query).ends_with(RUNTIME_PROXY_ANTHROPIC_MESSAGES_PATH)
}

pub(crate) fn is_runtime_compact_path(path_and_query: &str) -> bool {
    let normalized_path_and_query = runtime_proxy_normalize_openai_path(path_and_query);
    path_without_query(normalized_path_and_query.as_ref()).ends_with("/responses/compact")
}

pub(crate) fn path_without_query(path_and_query: &str) -> &str {
    path_and_query
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(path_and_query)
}

pub(crate) fn runtime_proxy_openai_suffix(path: &str) -> Option<&str> {
    if let Some(suffix) = path.strip_prefix(LEGACY_RUNTIME_PROXY_OPENAI_MOUNT_PATH_PREFIX)
        && let Some(version_suffix_index) = suffix.find('/')
    {
        return Some(&suffix[version_suffix_index..]);
    }

    if let Some(suffix) = path.strip_prefix(RUNTIME_PROXY_OPENAI_MOUNT_PATH)
        && (suffix.is_empty() || suffix.starts_with('/'))
    {
        return Some(suffix);
    }

    None
}

pub(crate) fn runtime_proxy_normalize_openai_path(path_and_query: &str) -> Cow<'_, str> {
    let (path, query) = match path_and_query.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (path_and_query, None),
    };
    let Some(suffix) = runtime_proxy_openai_suffix(path) else {
        return Cow::Borrowed(path_and_query);
    };

    let mut normalized =
        String::with_capacity(path_and_query.len() + RUNTIME_PROXY_OPENAI_UPSTREAM_PATH.len());
    normalized.push_str(RUNTIME_PROXY_OPENAI_UPSTREAM_PATH);
    normalized.push_str(suffix);
    if let Some(query) = query {
        normalized.push('?');
        normalized.push_str(query);
    }
    Cow::Owned(normalized)
}
