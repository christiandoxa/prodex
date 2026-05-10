use super::*;

#[cfg(test)]
pub(crate) fn next_runtime_response_candidate_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    next_runtime_response_candidate_for_route_with_prompt_cache_key(
        shared,
        excluded_profiles,
        route_kind,
        None,
    )
}

pub(super) fn next_runtime_response_candidate_for_route_with_prompt_cache_key(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
    prompt_cache_key: Option<&str>,
) -> Result<Option<String>> {
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let sync_probe_pressure_mode =
        runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind);
    let allow_disk_auth_fallback = !sync_probe_pressure_mode;
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let mut selection_state = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        runtime_route_selection_catalog(&runtime, route_kind, now)
    };
    let probe_plan = build_runtime_response_probe_plan(
        &selection_state,
        excluded_profiles,
        route_kind,
        allow_disk_auth_fallback,
        sync_probe_pressure_mode,
        inflight_soft_limit,
        now,
    );
    for refresh in &probe_plan.stale_probe_refreshes {
        schedule_runtime_probe_refresh(shared, &refresh.name, &refresh.codex_home);
    }
    if let Some(skip_jobs) = probe_plan.sync_probe_skip_jobs_count {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "selection_skip_sync_probe",
                [
                    runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                    runtime_proxy_log_field("reason", "pressure_mode"),
                    runtime_proxy_log_field("cold_start_jobs", skip_jobs.to_string()),
                ],
            ),
        );
    }

    let mut reports = probe_plan.reports;
    let mut ready_candidates = probe_plan.ready_candidates;
    if probe_plan.should_sync_probe_cold_start {
        let base_url = Some(selection_state.upstream_base_url.clone());
        let upstream_no_proxy = shared.upstream_no_proxy;
        let sync_jobs = probe_plan
            .sync_probe_jobs
            .iter()
            .take(RUNTIME_PROFILE_SYNC_PROBE_FALLBACK_LIMIT)
            .cloned()
            .collect::<Vec<_>>();
        let probed_names = sync_jobs
            .iter()
            .map(|job| job.name.clone())
            .collect::<BTreeSet<_>>();
        let fresh_reports = map_parallel(sync_jobs, |job| {
            let auth = job.provider.auth_summary(&job.codex_home);
            let result = if auth.quota_compatible {
                fetch_usage_with_proxy_policy(
                    &job.codex_home,
                    base_url.as_deref(),
                    upstream_no_proxy,
                )
                .map_err(|err| err.to_string())
            } else {
                Err("auth mode is not quota-compatible".to_string())
            };

            RunProfileProbeReport {
                name: job.name,
                order_index: job.order_index,
                auth,
                result,
            }
        });

        for report in &fresh_reports {
            apply_runtime_profile_probe_result(
                shared,
                &report.name,
                report.auth.clone(),
                report.result.clone(),
            )?;
        }
        selection_state = {
            let mut runtime = shared
                .runtime
                .lock()
                .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
            prune_runtime_profile_selection_backoff(&mut runtime, now);
            runtime_route_selection_catalog(&runtime, route_kind, now)
        };
        let cached_usage_snapshots = selection_state.persisted_usage_snapshots();

        for fresh_report in fresh_reports {
            if let Some(existing) = reports
                .iter_mut()
                .find(|report| report.name == fresh_report.name)
            {
                *existing = fresh_report;
            }
        }
        reports.sort_by_key(|report| report.order_index);
        ready_candidates = ready_profile_candidates_with_view(
            &reports,
            selection_state.include_code_review,
            Some(selection_state.current_profile.as_str()),
            runtime_route_selection_view(&selection_state),
            Some(&cached_usage_snapshots),
        );
        for job in probe_plan
            .cold_start_probe_jobs
            .into_iter()
            .filter(|job| !probed_names.contains(&job.name))
        {
            schedule_runtime_probe_refresh(shared, &job.name, &job.codex_home);
        }
    } else {
        if let Some(skip_profiles) = probe_plan.sync_probe_skip_profiles_count {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "selection_skip_sync_probe",
                    [
                        runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                        runtime_proxy_log_field("reason", "pressure_mode"),
                        runtime_proxy_log_field("cold_start_profiles", skip_profiles.to_string()),
                    ],
                ),
            );
        }
        for job in probe_plan.cold_start_probe_jobs {
            schedule_runtime_probe_refresh(shared, &job.name, &job.codex_home);
        }
    }
    let candidate_plan = build_runtime_response_candidate_execution_plan(
        &selection_state,
        excluded_profiles,
        route_kind,
        inflight_soft_limit,
        ready_candidates,
        runtime_response_candidate_execution_options(
            prompt_cache_key,
            runtime_prompt_cache_bound_profile(prompt_cache_key).as_deref(),
            |name| runtime_profile_selection_jitter(shared, name, route_kind),
        ),
    );

    for candidate in candidate_plan.ready_candidates {
        if let Some(reason) = candidate.ready_skip_reason() {
            if reason == "profile_inflight_soft_limit" {
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "selection_skip_current",
                        runtime_selection_log_fields_with_quota(
                            [
                                runtime_proxy_log_field(
                                    "route",
                                    runtime_route_kind_label(route_kind),
                                ),
                                runtime_proxy_log_field("profile", candidate.name.as_str()),
                                runtime_proxy_log_field("reason", "profile_inflight_soft_limit"),
                                runtime_proxy_log_field(
                                    "inflight",
                                    candidate.inflight_count.to_string(),
                                ),
                                runtime_proxy_log_field(
                                    "soft_limit",
                                    candidate.inflight_soft_limit.to_string(),
                                ),
                                runtime_proxy_log_field(
                                    "health",
                                    candidate.health_sort_key.to_string(),
                                ),
                                runtime_proxy_log_field(
                                    "quota_source",
                                    runtime_quota_source_label(candidate.quota_source),
                                ),
                            ],
                            candidate.quota_summary,
                        ),
                    ),
                );
            } else {
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "selection_skip_current",
                        runtime_selection_log_fields_with_quota(
                            [
                                runtime_proxy_log_field(
                                    "route",
                                    runtime_route_kind_label(route_kind),
                                ),
                                runtime_proxy_log_field("profile", candidate.name.as_str()),
                                runtime_proxy_log_field("reason", reason),
                                runtime_proxy_log_field(
                                    "inflight",
                                    candidate.inflight_count.to_string(),
                                ),
                                runtime_proxy_log_field(
                                    "health",
                                    candidate.health_sort_key.to_string(),
                                ),
                                runtime_proxy_log_field(
                                    "quota_source",
                                    runtime_quota_source_label(candidate.quota_source),
                                ),
                            ],
                            candidate.quota_summary,
                        ),
                    ),
                );
            }
            continue;
        }
        if !reserve_runtime_profile_route_circuit_half_open_probe(
            shared,
            &candidate.name,
            route_kind,
        )? {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "selection_skip_current",
                    runtime_selection_log_fields_with_quota(
                        [
                            runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                            runtime_proxy_log_field("profile", candidate.name.as_str()),
                            runtime_proxy_log_field("reason", "route_circuit_half_open_probe_wait"),
                            runtime_proxy_log_field(
                                "inflight",
                                candidate.inflight_count.to_string(),
                            ),
                            runtime_proxy_log_field(
                                "health",
                                candidate.health_sort_key.to_string(),
                            ),
                            runtime_proxy_log_field(
                                "quota_source",
                                runtime_quota_source_label(candidate.quota_source),
                            ),
                        ],
                        candidate.quota_summary,
                    ),
                ),
            );
            continue;
        }
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "selection_pick",
                runtime_selection_log_fields_with_quota(
                    [
                        runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                        runtime_proxy_log_field("profile", candidate.name.as_str()),
                        runtime_proxy_log_field("mode", "ready"),
                        runtime_proxy_log_field("inflight", candidate.inflight_count.to_string()),
                        runtime_proxy_log_field("health", candidate.health_sort_key.to_string()),
                        runtime_proxy_log_field("order", candidate.order_index.to_string()),
                        runtime_proxy_log_field(
                            "quota_source",
                            runtime_quota_source_label(candidate.quota_source),
                        ),
                    ],
                    candidate.quota_summary,
                ),
            ),
        );
        return Ok(Some(candidate.name.clone()));
    }

    let mut fallback = None;
    for candidate in candidate_plan.fallback_candidates {
        if let Some(reason) = candidate.fallback_skip_reason() {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "selection_skip_current",
                    runtime_selection_log_fields_with_quota(
                        [
                            runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                            runtime_proxy_log_field("profile", candidate.name.as_str()),
                            runtime_proxy_log_field("reason", reason),
                            runtime_proxy_log_field(
                                "inflight",
                                candidate.inflight_count.to_string(),
                            ),
                            runtime_proxy_log_field(
                                "health",
                                candidate.health_sort_key.to_string(),
                            ),
                            runtime_proxy_log_field(
                                "quota_source",
                                runtime_quota_source_label(candidate.quota_source),
                            ),
                        ],
                        candidate.quota_summary,
                    ),
                ),
            );
            continue;
        }
        if !reserve_runtime_profile_route_circuit_half_open_probe(
            shared,
            &candidate.name,
            route_kind,
        )? {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "selection_skip_current",
                    runtime_selection_log_fields_with_quota(
                        [
                            runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                            runtime_proxy_log_field("profile", candidate.name.as_str()),
                            runtime_proxy_log_field("reason", "route_circuit_half_open_probe_wait"),
                            runtime_proxy_log_field(
                                "inflight",
                                candidate.inflight_count.to_string(),
                            ),
                            runtime_proxy_log_field(
                                "health",
                                candidate.health_sort_key.to_string(),
                            ),
                            runtime_proxy_log_field(
                                "quota_source",
                                runtime_quota_source_label(candidate.quota_source),
                            ),
                        ],
                        candidate.quota_summary,
                    ),
                ),
            );
            continue;
        }
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "selection_pick",
                runtime_selection_log_fields_with_quota(
                    [
                        runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                        runtime_proxy_log_field("profile", candidate.name.as_str()),
                        runtime_proxy_log_field("mode", "backoff"),
                        runtime_proxy_log_field("inflight", candidate.inflight_count.to_string()),
                        runtime_proxy_log_field("health", candidate.health_sort_key.to_string()),
                        runtime_proxy_log_field(
                            "backoff",
                            format!("{:?}", candidate.backoff_sort_key),
                        ),
                        runtime_proxy_log_field("order", candidate.order_index.to_string()),
                        runtime_proxy_log_field(
                            "quota_source",
                            runtime_quota_source_label(candidate.quota_source),
                        ),
                    ],
                    candidate.quota_summary,
                ),
            ),
        );
        fallback = Some(candidate.name);
        break;
    }

    if fallback.is_none() {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "selection_pick",
                [
                    runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                    runtime_proxy_log_field("profile", "none"),
                    runtime_proxy_log_field("mode", "exhausted"),
                    runtime_proxy_log_field("excluded_count", excluded_profiles.len().to_string()),
                ],
            ),
        );
    }

    Ok(fallback)
}
