use super::*;

pub(crate) fn runtime_profile_selection_catalog(
    runtime: &RuntimeRotationState,
) -> RuntimeProfileSelectionCatalog {
    RuntimeProfileSelectionCatalog {
        entries: runtime
            .state
            .profiles
            .iter()
            .map(|(name, profile)| RuntimeSelectionProfileEntry {
                name: name.clone(),
                codex_home: profile.codex_home.clone(),
                provider: profile.provider.clone(),
                last_run_selected_at: runtime.state.last_run_selected_at.get(name).copied(),
            })
            .collect(),
    }
}

pub(crate) fn runtime_profile_selection_view(
    catalog: &RuntimeProfileSelectionCatalog,
) -> RuntimeProfileSelectionCatalogView<'_> {
    RuntimeProfileSelectionCatalogView {
        entries: &catalog.entries,
    }
}

pub(crate) fn runtime_route_selection_view(
    catalog: &RuntimeRouteSelectionCatalog,
) -> RuntimeRouteSelectionCatalogView<'_> {
    RuntimeRouteSelectionCatalogView {
        entries: &catalog.entries,
    }
}

pub(crate) fn runtime_route_selection_catalog(
    runtime: &RuntimeRotationState,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> RuntimeRouteSelectionCatalog {
    RuntimeRouteSelectionCatalog {
        current_profile: runtime.current_profile.clone(),
        include_code_review: runtime.include_code_review,
        upstream_base_url: runtime.upstream_base_url.clone(),
        entries: runtime_profile_selection_catalog(runtime)
            .entries
            .into_iter()
            .map(|profile| RuntimeRouteSelectionEntry {
                cached_auth_summary: runtime_profile_cached_auth_summary_from_maps_for_selection(
                    &profile.name,
                    &runtime.profile_usage_auth,
                    &runtime.profile_probe_cache,
                ),
                cached_probe_entry: runtime.profile_probe_cache.get(&profile.name).cloned(),
                cached_usage_snapshot: runtime.profile_usage_snapshots.get(&profile.name).cloned(),
                auth_failure_active: runtime_profile_auth_failure_active_with_auth_cache(
                    &runtime.profile_health,
                    &runtime.profile_usage_auth,
                    &profile.name,
                    now,
                ),
                in_selection_backoff: runtime_profile_name_in_selection_backoff(
                    &profile.name,
                    &runtime.profile_retry_backoff_until,
                    &runtime.profile_transport_backoff_until,
                    &runtime.profile_route_circuit_open_until,
                    route_kind,
                    now,
                ),
                backoff_sort_key: runtime_profile_backoff_sort_key(
                    &profile.name,
                    &runtime.profile_retry_backoff_until,
                    &runtime.profile_transport_backoff_until,
                    &runtime.profile_route_circuit_open_until,
                    route_kind,
                    now,
                ),
                inflight_count: runtime_profile_inflight_sort_key(
                    &profile.name,
                    &runtime.profile_inflight,
                ),
                health_sort_key: runtime_profile_health_sort_key(
                    &profile.name,
                    &runtime.profile_health,
                    now,
                    route_kind,
                ),
                profile,
            })
            .collect(),
    }
}
