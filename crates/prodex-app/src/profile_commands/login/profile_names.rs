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
    prodex_mojo_core::profile_identity::sanitize_profile_slug(value)
        .expect("Mojo profile slug sanitizer should accept Rust strings")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_slug_sanitizer_uses_mojo_kernel() {
        assert_eq!(
            sanitize_profile_slug("  API_KEY_User@EXAMPLE.com  "),
            "api_key_user_example.com"
        );
        assert_eq!(sanitize_profile_slug("雪@EXAMPLE.com"), "example.com");
        assert_eq!(sanitize_profile_slug("---"), "api_key");
        assert_eq!(sanitize_profile_slug(" A/B "), "a-b");
    }
}
