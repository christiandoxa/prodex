pub(super) fn default_api_key_profile_name(openai_base_url: Option<&str>) -> String {
    openai_base_url
        .and_then(|base_url| reqwest::Url::parse(base_url).ok())
        .and_then(|url| url.host_str().map(ToOwned::to_owned))
        .map(|host| sanitize_profile_slug(&format!("api_key_{host}")))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "api_key".to_string())
}

pub(super) fn unique_profile_name_for_slug(
    slug: &str,
    is_available: impl FnMut(&str) -> bool,
) -> String {
    let base = sanitize_profile_slug(slug);
    prodex_profile_identity::unique_profile_name_from_base(&base, "api_key", is_available)
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

    #[test]
    fn unique_profile_name_for_slug_uses_mojo_candidate_planner() {
        let mut checked = Vec::new();
        let name = unique_profile_name_for_slug(" A/B ", |candidate| {
            checked.push(candidate.to_string());
            candidate == "a-b-3"
        });

        assert_eq!(name, "a-b-3");
        assert_eq!(checked, ["a-b", "a-b-2", "a-b-3"]);
    }
}
