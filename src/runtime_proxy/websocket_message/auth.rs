use super::*;

#[cfg(test)]
#[allow(dead_code)]
pub(in crate::runtime_proxy) fn runtime_profile_auth_summary_for_selection(
    profile_name: &str,
    codex_home: &Path,
    profile_usage_auth: &BTreeMap<String, RuntimeProfileUsageAuthCacheEntry>,
    profile_probe_cache: &BTreeMap<String, RuntimeProfileProbeCacheEntry>,
) -> AuthSummary {
    runtime_profile_auth_summary_for_selection_with_policy(
        profile_name,
        codex_home,
        profile_usage_auth,
        profile_probe_cache,
        true,
    )
    .unwrap_or_else(runtime_profile_uncached_auth_summary_for_selection)
}

pub(in crate::runtime_proxy) fn runtime_profile_uncached_auth_summary_for_selection() -> AuthSummary
{
    AuthSummary {
        label: "uncached-auth".to_string(),
        quota_compatible: false,
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::runtime_proxy) fn runtime_profile_auth_summary_for_selection_with_policy(
    profile_name: &str,
    codex_home: &Path,
    profile_usage_auth: &BTreeMap<String, RuntimeProfileUsageAuthCacheEntry>,
    profile_probe_cache: &BTreeMap<String, RuntimeProfileProbeCacheEntry>,
    allow_disk_fallback: bool,
) -> Option<AuthSummary> {
    runtime_profile_cached_auth_summary_from_maps_for_selection(
        profile_name,
        profile_usage_auth,
        profile_probe_cache,
    )
    .or_else(|| allow_disk_fallback.then(|| read_auth_summary(codex_home)))
}
