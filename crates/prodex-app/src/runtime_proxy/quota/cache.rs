use super::*;

pub(crate) fn runtime_profile_usage_cache_is_fresh(
    entry: &RuntimeProfileProbeCacheEntry,
    now: i64,
) -> bool {
    prodex_runtime_quota::runtime_profile_usage_cache_is_fresh(
        entry,
        now,
        RUNTIME_PROFILE_USAGE_CACHE_FRESH_SECONDS,
    )
}

pub(crate) fn runtime_profile_probe_cache_freshness(
    entry: &RuntimeProfileProbeCacheEntry,
    now: i64,
) -> RuntimeProbeCacheFreshness {
    prodex_runtime_quota::runtime_profile_probe_cache_freshness(
        entry,
        now,
        RUNTIME_PROFILE_USAGE_CACHE_FRESH_SECONDS,
        RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS,
    )
}

pub(crate) fn update_runtime_profile_probe_cache_with_usage(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    usage: UsageResponse,
) -> Result<()> {
    let auth = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .state
        .profiles
        .get(profile_name)
        .map(|profile| profile.provider.auth_summary(&profile.codex_home))
        .unwrap_or(AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        });
    apply_runtime_profile_probe_result(shared, profile_name, auth, Ok(usage))
}

pub(crate) fn runtime_usage_snapshot_is_usable(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
) -> bool {
    prodex_runtime_quota::runtime_usage_snapshot_is_usable(
        snapshot,
        now,
        RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS,
    )
}

pub(crate) fn runtime_profile_codex_home(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<Option<PathBuf>> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    Ok(runtime
        .state
        .profiles
        .get(profile_name)
        .map(|profile| profile.codex_home.clone()))
}

pub(crate) fn runtime_profile_cached_auth_summary_for_selection(
    usage_auth_entry: Option<RuntimeProfileUsageAuthCacheEntry>,
    probe_entry: Option<RuntimeProfileProbeCacheEntry>,
) -> Option<AuthSummary> {
    if let Some(entry) = usage_auth_entry {
        match runtime_profile_usage_auth_cache_entry_freshness(&entry) {
            RuntimeProfileUsageAuthCacheFreshness::Fresh => {
                return Some(AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                });
            }
            RuntimeProfileUsageAuthCacheFreshness::Stale
            | RuntimeProfileUsageAuthCacheFreshness::Unknown => {}
        }
    }
    probe_entry.map(|entry| entry.auth)
}

pub(crate) fn runtime_profile_cached_auth_summary_from_maps_for_selection(
    profile_name: &str,
    profile_usage_auth: &BTreeMap<String, RuntimeProfileUsageAuthCacheEntry>,
    profile_probe_cache: &BTreeMap<String, RuntimeProfileProbeCacheEntry>,
) -> Option<AuthSummary> {
    if profile_usage_auth.get(profile_name).is_some_and(|entry| {
        runtime_profile_usage_auth_cache_entry_freshness(entry)
            == RuntimeProfileUsageAuthCacheFreshness::Fresh
    }) {
        return Some(AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        });
    }
    profile_probe_cache
        .get(profile_name)
        .map(|entry| entry.auth.clone())
}

pub(crate) fn runtime_snapshot_blocks_same_request_cold_start_probe(
    snapshot: &RuntimeProfileUsageSnapshot,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    prodex_runtime_quota::runtime_snapshot_blocks_same_request_cold_start_probe(
        snapshot,
        route_kind,
        now,
        RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS,
        runtime_proxy_responses_quota_critical_floor_percent(),
    )
}
