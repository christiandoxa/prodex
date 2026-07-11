use super::{
    RuntimeProfileSelectionCatalog, RuntimeRotationProxyShared, RuntimeSelectionProfileEntry,
    read_auth_summary, runtime_previous_response_negative_cache_active,
    runtime_profile_auth_failure_active_with_auth_cache,
    runtime_profile_cached_auth_summary_from_maps_for_selection,
    runtime_profile_quota_summary_for_route_from_state, runtime_profile_selection_catalog,
    runtime_proxy_log, runtime_proxy_sync_probe_pressure_mode_active_for_route,
    runtime_quota_precommit_guard_reason, runtime_quota_pressure_band_reason,
    runtime_route_kind_label, runtime_selection_log_fields_with_quota,
    runtime_selection_quota_source_label,
};
use anyhow::Result;
use chrono::Local;
use prodex_app_reports::ProfileSelectionProvider;
use prodex_quota::RuntimeQuotaPressureBand;
use prodex_runtime_state::RuntimeRouteKind;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::collections::BTreeSet;
use std::path::PathBuf;

pub(crate) fn next_runtime_previous_response_candidate(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    previous_response_id: Option<&str>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let allow_disk_auth_fallback =
        !runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind);
    let now = Local::now().timestamp();
    struct RuntimePreviousResponseDiskFallbackEntry {
        name: String,
        codex_home: PathBuf,
    }

    let disk_fallback_entries = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        let selection_catalog = runtime_profile_selection_catalog(&runtime);
        if let Some(owner) = previous_response_id.and_then(|response_id| {
            runtime
                .state
                .response_profile_bindings
                .get(response_id)
                .map(|binding| binding.profile_name.as_str())
        }) && !excluded_profiles.contains(owner)
            && selection_catalog.contains(owner)
            && !previous_response_id.is_some_and(|response_id| {
                runtime_previous_response_negative_cache_active(
                    &runtime.profile_health,
                    response_id,
                    owner,
                    route_kind,
                    now,
                )
            })
        {
            return Ok(Some(owner.to_string()));
        }
        let mut disk_fallback_entries = Vec::new();
        for profile in
            runtime_previous_response_ordered_profiles(&selection_catalog, &runtime.current_profile)
        {
            let name = profile.name.as_str();
            if excluded_profiles.contains(name) {
                continue;
            }
            if let Some(previous_response_id) = previous_response_id
                && runtime_previous_response_negative_cache_active(
                    &runtime.profile_health,
                    previous_response_id,
                    name,
                    route_kind,
                    now,
                )
            {
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "selection_skip_affinity",
                        [
                            runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                            runtime_proxy_log_field("affinity", "previous_response_discovery"),
                            runtime_proxy_log_field("profile", name),
                            runtime_proxy_log_field("reason", "negative_cache"),
                            runtime_proxy_log_field("response_id", previous_response_id),
                        ],
                    ),
                );
                continue;
            }
            if runtime_profile_auth_failure_active_with_auth_cache(
                &runtime.profile_health,
                &runtime.profile_usage_auth,
                name,
                now,
            ) {
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "selection_skip_affinity",
                        [
                            runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                            runtime_proxy_log_field("affinity", "previous_response_discovery"),
                            runtime_proxy_log_field("profile", name),
                            runtime_proxy_log_field("reason", "auth_failure_backoff"),
                        ],
                    ),
                );
                continue;
            }
            let (quota_summary, quota_source) =
                runtime_profile_quota_summary_for_route_from_state(&runtime, name, route_kind, now);
            if quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted
                || runtime_quota_precommit_guard_reason(quota_summary, route_kind).is_some()
            {
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "selection_skip_affinity",
                        runtime_selection_log_fields_with_quota(
                            [
                                runtime_proxy_log_field(
                                    "route",
                                    runtime_route_kind_label(route_kind),
                                ),
                                runtime_proxy_log_field("affinity", "previous_response_discovery"),
                                runtime_proxy_log_field("profile", name),
                                runtime_proxy_log_field(
                                    "reason",
                                    runtime_quota_precommit_guard_reason(quota_summary, route_kind)
                                        .unwrap_or_else(|| {
                                            runtime_quota_pressure_band_reason(
                                                quota_summary.route_band,
                                            )
                                        }),
                                ),
                                runtime_proxy_log_field(
                                    "quota_source",
                                    runtime_selection_quota_source_label(quota_source),
                                ),
                            ],
                            quota_summary,
                        ),
                    ),
                );
                continue;
            }
            match runtime_profile_cached_auth_summary_from_maps_for_selection(
                name,
                &runtime.profile_usage_auth,
                &runtime.profile_probe_cache,
            ) {
                Some(summary) if summary.quota_compatible => return Ok(Some(profile.name.clone())),
                Some(_) => {}
                None if allow_disk_auth_fallback => {
                    disk_fallback_entries.push(RuntimePreviousResponseDiskFallbackEntry {
                        name: profile.name.clone(),
                        codex_home: profile.codex_home.clone(),
                    });
                }
                None => {}
            }
        }
        disk_fallback_entries
    };

    for entry in disk_fallback_entries {
        if read_auth_summary(&entry.codex_home).quota_compatible {
            return Ok(Some(entry.name));
        }
    }
    Ok(None)
}

fn runtime_previous_response_ordered_profiles<'a>(
    catalog: &'a RuntimeProfileSelectionCatalog,
    current_profile: &str,
) -> Vec<&'a RuntimeSelectionProfileEntry> {
    let current_index = catalog
        .entries
        .iter()
        .position(|entry| entry.name == current_profile);
    let mut ordered = Vec::with_capacity(catalog.entries.len());
    if let Some(index) = current_index {
        ordered.push((
            catalog.entries[index].runtime_pool_priority(),
            0,
            &catalog.entries[index],
        ));
        for (offset, entry) in catalog.entries.iter().skip(index + 1).enumerate() {
            ordered.push((entry.runtime_pool_priority(), offset + 1, entry));
        }
        let tail_len = catalog.entries.len().saturating_sub(index + 1);
        for (offset, entry) in catalog.entries.iter().take(index).enumerate() {
            ordered.push((entry.runtime_pool_priority(), tail_len + offset + 1, entry));
        }
    } else {
        for (index, entry) in catalog.entries.iter().enumerate() {
            if entry.name != current_profile {
                ordered.push((entry.runtime_pool_priority(), index, entry));
            }
        }
    }
    ordered.sort_by_key(|(provider_priority, order_index, _)| (*provider_priority, *order_index));
    ordered.into_iter().map(|(_, _, entry)| entry).collect()
}
