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

fn cached_auth_summary_from_mojo_source(
    usage_auth_present: bool,
    probe_auth: Option<&AuthSummary>,
) -> Option<AuthSummary> {
    let source = prodex_mojo_core::runtime::profile_cached_auth_source(
        usage_auth_present,
        probe_auth.is_some(),
    )
    .expect("compiled Mojo cached-auth source selection failed");
    match source {
        prodex_mojo_core::runtime::ProfileCachedAuthSource::None => None,
        prodex_mojo_core::runtime::ProfileCachedAuthSource::UsageAuth => Some(AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        }),
        prodex_mojo_core::runtime::ProfileCachedAuthSource::Probe => Some(
            probe_auth
                .expect("Mojo selected absent probe-auth summary")
                .clone(),
        ),
    }
}

pub(crate) fn runtime_profile_cached_auth_summary_for_selection(
    usage_auth_entry: Option<RuntimeProfileUsageAuthCacheEntry>,
    probe_entry: Option<RuntimeProfileProbeCacheEntry>,
) -> Option<AuthSummary> {
    cached_auth_summary_from_mojo_source(
        usage_auth_entry.is_some(),
        probe_entry.as_ref().map(|entry| &entry.auth),
    )
}

pub(crate) fn runtime_profile_cached_auth_summary_from_maps_for_selection(
    profile_name: &str,
    profile_usage_auth: &BTreeMap<String, RuntimeProfileUsageAuthCacheEntry>,
    profile_probe_cache: &BTreeMap<String, RuntimeProfileProbeCacheEntry>,
) -> Option<AuthSummary> {
    cached_auth_summary_from_mojo_source(
        profile_usage_auth.contains_key(profile_name),
        profile_probe_cache
            .get(profile_name)
            .map(|entry| &entry.auth),
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_cached_auth_source_precedence_keeps_probe_and_chatgpt_shapes() {
        let probe = AuthSummary {
            label: "external-provider".to_string(),
            quota_compatible: false,
        };
        assert!(cached_auth_summary_from_mojo_source(false, None).is_none());

        let source = cached_auth_summary_from_mojo_source(false, Some(&probe))
            .expect("probe source should be selected");
        assert_eq!(source.label, "external-provider");
        assert!(!source.quota_compatible);

        let source = cached_auth_summary_from_mojo_source(true, None)
            .expect("usage auth should supply ChatGPT source");
        assert_eq!(source.label, "chatgpt");
        assert!(source.quota_compatible);

        let source = cached_auth_summary_from_mojo_source(true, Some(&probe))
            .expect("usage auth should take precedence over probe auth");
        assert_eq!(source.label, "chatgpt");
        assert!(source.quota_compatible);
    }
}
