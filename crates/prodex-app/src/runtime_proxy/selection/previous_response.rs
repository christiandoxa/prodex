use super::*;

pub(crate) fn next_runtime_previous_response_candidate(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    previous_response_id: Option<&str>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let allow_disk_auth_fallback =
        !runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind);
    let now = Local::now().timestamp();
    struct RuntimePreviousResponseDiscoveryEntry {
        codex_home: PathBuf,
        cached_auth_summary: Option<AuthSummary>,
        auth_failure_active: bool,
        negative_cache_active: bool,
        quota_summary: RuntimeQuotaSummary,
        quota_source: Option<RuntimeQuotaSource>,
    }

    let (selection_catalog, current_profile, previous_response_owner, discovery_entries) = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        let selection_catalog = runtime_profile_selection_catalog(&runtime);
        let discovery_entries = selection_catalog
            .entries
            .iter()
            .map(|profile| {
                let (quota_summary, quota_source) =
                    runtime_profile_quota_summary_for_route_from_state(
                        &runtime,
                        &profile.name,
                        route_kind,
                        now,
                    );
                (
                    profile.name.clone(),
                    RuntimePreviousResponseDiscoveryEntry {
                        codex_home: profile.codex_home.clone(),
                        cached_auth_summary:
                            runtime_profile_cached_auth_summary_from_maps_for_selection(
                                &profile.name,
                                &runtime.profile_usage_auth,
                                &runtime.profile_probe_cache,
                            ),
                        auth_failure_active: runtime_profile_auth_failure_active_with_auth_cache(
                            &runtime.profile_health,
                            &runtime.profile_usage_auth,
                            &profile.name,
                            now,
                        ),
                        negative_cache_active: previous_response_id.is_some_and(|response_id| {
                            runtime_previous_response_negative_cache_active(
                                &runtime.profile_health,
                                response_id,
                                &profile.name,
                                route_kind,
                                now,
                            )
                        }),
                        quota_summary,
                        quota_source,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        (
            selection_catalog,
            runtime.current_profile.clone(),
            previous_response_id.and_then(|response_id| {
                runtime
                    .state
                    .response_profile_bindings
                    .get(response_id)
                    .map(|binding| binding.profile_name.clone())
            }),
            discovery_entries,
        )
    };
    if let Some(owner) = previous_response_owner.as_deref()
        && !excluded_profiles.contains(owner)
        && selection_catalog.contains(owner)
        && !discovery_entries
            .get(owner)
            .is_some_and(|entry| entry.negative_cache_active)
    {
        return Ok(Some(owner.to_string()));
    }

    for name in active_profile_selection_order_with_view(
        runtime_profile_selection_view(&selection_catalog),
        &current_profile,
    ) {
        if excluded_profiles.contains(&name) {
            continue;
        }
        let Some(entry) = discovery_entries.get(&name) else {
            continue;
        };
        if let Some(previous_response_id) = previous_response_id
            && entry.negative_cache_active
        {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "selection_skip_affinity",
                    [
                        runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                        runtime_proxy_log_field("affinity", "previous_response_discovery"),
                        runtime_proxy_log_field("profile", name.as_str()),
                        runtime_proxy_log_field("reason", "negative_cache"),
                        runtime_proxy_log_field("response_id", previous_response_id),
                    ],
                ),
            );
            continue;
        }
        if !entry
            .cached_auth_summary
            .clone()
            .or_else(|| allow_disk_auth_fallback.then(|| read_auth_summary(&entry.codex_home)))
            .is_some_and(|summary| summary.quota_compatible)
        {
            continue;
        }
        if entry.auth_failure_active {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "selection_skip_affinity",
                    [
                        runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                        runtime_proxy_log_field("affinity", "previous_response_discovery"),
                        runtime_proxy_log_field("profile", name.as_str()),
                        runtime_proxy_log_field("reason", "auth_failure_backoff"),
                    ],
                ),
            );
            continue;
        }
        if entry.quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted
            || runtime_quota_precommit_guard_reason(entry.quota_summary, route_kind).is_some()
        {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "selection_skip_affinity",
                    runtime_selection_log_fields_with_quota(
                        [
                            runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                            runtime_proxy_log_field("affinity", "previous_response_discovery"),
                            runtime_proxy_log_field("profile", name.as_str()),
                            runtime_proxy_log_field(
                                "reason",
                                runtime_quota_precommit_guard_reason(
                                    entry.quota_summary,
                                    route_kind,
                                )
                                .unwrap_or_else(|| {
                                    runtime_quota_pressure_band_reason(
                                        entry.quota_summary.route_band,
                                    )
                                }),
                            ),
                            runtime_proxy_log_field(
                                "quota_source",
                                runtime_selection_quota_source_label(entry.quota_source),
                            ),
                        ],
                        entry.quota_summary,
                    ),
                ),
            );
            continue;
        }
        return Ok(Some(name));
    }
    Ok(None)
}
