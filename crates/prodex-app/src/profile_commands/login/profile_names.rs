use crate::{AppPaths, AppState};

pub(super) fn default_api_key_profile_name(openai_base_url: Option<&str>) -> String {
    openai_base_url
        .and_then(|base_url| reqwest::Url::parse(base_url).ok())
        .and_then(|url| url.host_str().map(ToOwned::to_owned))
        .map(|host| sanitize_profile_slug(&format!("api_key_{host}")))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "api_key".to_string())
}

pub(super) fn unique_profile_name_for_slug(
    paths: &AppPaths,
    state: &AppState,
    slug: &str,
) -> String {
    let base = sanitize_profile_slug(slug);
    if crate::profile_name_is_available(paths, state, &base) {
        return base;
    }
    for suffix in 2.. {
        let candidate = format!("{base}-{suffix}");
        if crate::profile_name_is_available(paths, state, &candidate) {
            return candidate;
        }
    }
    unreachable!("unbounded profile suffix search should always return")
}

pub(super) fn sanitize_profile_slug(value: &str) -> String {
    let mut slug = String::new();
    for ch in value.trim().to_ascii_lowercase().chars() {
        match ch {
            'a'..='z' | '0'..='9' | '.' | '_' | '-' => slug.push(ch),
            '@' => slug.push('_'),
            _ => slug.push('-'),
        }
    }
    let slug = slug.trim_matches(|ch| matches!(ch, '.' | '_' | '-'));
    if slug.is_empty() || slug == "." || slug == ".." {
        "api_key".to_string()
    } else {
        slug.to_string()
    }
}
