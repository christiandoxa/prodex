use super::*;

#[cfg(test)]
pub(crate) fn runtime_proxy_optimistic_current_candidate_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    runtime_proxy_optimistic_current_candidate_for_route_with_selection(
        shared,
        excluded_profiles,
        route_kind,
        None,
    )
}

pub(super) fn runtime_proxy_optimistic_current_candidate_for_route_with_selection(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
    prompt_cache_key: Option<&str>,
) -> Result<Option<String>> {
    let pressure_mode = runtime_proxy_pressure_mode_active(shared);
    let prompt_cache_owner_profile = runtime_prompt_cache_bound_profile(prompt_cache_key);
    let (
        current_profile,
        codex_home,
        in_selection_backoff,
        circuit_open_until,
        inflight_count,
        health_score,
        performance_score,
        auth_failure_active,
        cached_usage_auth_entry,
        probe_cache_entry,
    ) = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        let now = Local::now().timestamp();
        prune_runtime_profile_selection_backoff(&mut runtime, now);

        if excluded_profiles.contains(&runtime.current_profile) {
            return Ok(None);
        }

        let Some(profile) = runtime.state.profiles.get(&runtime.current_profile) else {
            return Ok(None);
        };
        (
            runtime.current_profile.clone(),
            profile.codex_home.clone(),
            runtime_profile_in_selection_backoff(
                &runtime,
                &runtime.current_profile,
                route_kind,
                now,
            ),
            runtime_profile_route_circuit_open_until(
                &runtime,
                &runtime.current_profile,
                route_kind,
                now,
            ),
            runtime_profile_inflight_count(&runtime, &runtime.current_profile),
            runtime_profile_health_score(&runtime, &runtime.current_profile, now, route_kind),
            runtime_profile_route_performance_score(
                &runtime.profile_health,
                &runtime.current_profile,
                now,
                route_kind,
            ),
            runtime_profile_auth_failure_active_with_auth_cache(
                &runtime.profile_health,
                &runtime.profile_usage_auth,
                &runtime.current_profile,
                now,
            ),
            runtime
                .profile_usage_auth
                .get(&runtime.current_profile)
                .cloned(),
            runtime
                .profile_probe_cache
                .get(&runtime.current_profile)
                .cloned(),
        )
    };
    let has_alternative_quota_compatible_profile = runtime_has_route_eligible_quota_fallback(
        shared,
        &current_profile,
        excluded_profiles,
        route_kind,
    )?;
    let allow_disk_auth_fallback =
        !runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind);
    let current_profile_quota_compatible = runtime_profile_cached_auth_summary_for_selection(
        cached_usage_auth_entry,
        probe_cache_entry,
    )
    .unwrap_or_else(|| {
        if allow_disk_auth_fallback {
            read_auth_summary(&codex_home)
        } else {
            AuthSummary {
                label: "uncached-auth".to_string(),
                quota_compatible: false,
            }
        }
    })
    .quota_compatible;
    let (quota_summary, quota_source) =
        runtime_profile_quota_summary_for_route(shared, &current_profile, route_kind)?;
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    if let RuntimeOptimisticCurrentCandidateDecision::Skip(skip) =
        runtime_optimistic_current_candidate_decision(
            RuntimeOptimisticCurrentCandidateSelectionInput {
                current_profile: current_profile.as_str(),
                route_kind,
                auth_failure_active,
                in_selection_backoff,
                circuit_open: circuit_open_until.is_some(),
                health_score,
                performance_score,
                current_profile_quota_compatible,
                has_alternative_quota_compatible_profile,
                quota_summary,
                quota_source,
                inflight_count,
                inflight_soft_limit,
                prompt_cache_key,
                prompt_cache_owner_profile: prompt_cache_owner_profile.as_deref(),
            },
        )
    {
        let reason = skip.reason_label();
        if skip.reason == RuntimeOptimisticCurrentCandidateSkipReason::PromptCacheAffinity {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "selection_skip_current",
                    runtime_selection_log_fields_with_quota(
                        [
                            runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                            runtime_proxy_log_field("profile", current_profile.as_str()),
                            runtime_proxy_log_field("reason", reason),
                            runtime_proxy_log_field("inflight", inflight_count.to_string()),
                            runtime_proxy_log_field("health", health_score.to_string()),
                            runtime_proxy_log_field("performance", performance_score.to_string()),
                            runtime_proxy_log_field(
                                "quota_source",
                                runtime_selection_quota_source_label(quota_source),
                            ),
                        ],
                        quota_summary,
                    ),
                ),
            );
        } else if skip.include_quota_fields() {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "selection_skip_current",
                    runtime_selection_log_fields_with_quota(
                        [
                            runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                            runtime_proxy_log_field("profile", current_profile.as_str()),
                            runtime_proxy_log_field("reason", reason),
                            runtime_proxy_log_field("inflight", inflight_count.to_string()),
                            runtime_proxy_log_field("health", health_score.to_string()),
                            runtime_proxy_log_field("performance", performance_score.to_string()),
                            runtime_proxy_log_field("soft_limit", inflight_soft_limit.to_string()),
                            runtime_proxy_log_field(
                                "circuit_until",
                                circuit_open_until.unwrap_or_default().to_string(),
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
        } else {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "selection_skip_current",
                    [
                        runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                        runtime_proxy_log_field("profile", current_profile.as_str()),
                        runtime_proxy_log_field("reason", reason),
                    ],
                ),
            );
        }
        return Ok(None);
    }

    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "selection_keep_current",
            runtime_selection_log_fields_with_quota(
                [
                    runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                    runtime_proxy_log_field("profile", current_profile.as_str()),
                    runtime_proxy_log_field("inflight", inflight_count.to_string()),
                    runtime_proxy_log_field("health", health_score.to_string()),
                    runtime_proxy_log_field("performance", performance_score.to_string()),
                    runtime_proxy_log_field(
                        "quota_source",
                        runtime_selection_quota_source_label(quota_source),
                    ),
                ],
                quota_summary,
            ),
        ),
    );
    if !reserve_runtime_profile_route_circuit_half_open_probe(shared, &current_profile, route_kind)?
    {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "selection_skip_current",
                runtime_selection_log_fields_with_quota(
                    [
                        runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                        runtime_proxy_log_field("profile", current_profile.as_str()),
                        runtime_proxy_log_field("reason", "route_circuit_half_open_probe_wait"),
                        runtime_proxy_log_field("inflight", inflight_count.to_string()),
                        runtime_proxy_log_field("health", health_score.to_string()),
                        runtime_proxy_log_field("performance", performance_score.to_string()),
                        runtime_proxy_log_field(
                            "quota_source",
                            runtime_selection_quota_source_label(quota_source),
                        ),
                    ],
                    quota_summary,
                ),
            ),
        );
        return Ok(None);
    }
    Ok(Some(current_profile))
}
