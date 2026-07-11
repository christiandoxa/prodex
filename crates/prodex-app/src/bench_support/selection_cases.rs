use super::*;

#[doc(hidden)]
pub struct RuntimeProxyQuotaFallbackBenchCase {
    shared: RuntimeRotationProxyShared,
    excluded_profiles: BTreeSet<String>,
    profile_name: String,
}

impl RuntimeProxyQuotaFallbackBenchCase {
    pub fn new(profile_count: usize) -> Self {
        let profile_count = profile_count.max(2);
        let paths = bench_paths("quota-fallback");
        let now = Local::now().timestamp();
        let mut profiles = BTreeMap::new();
        let mut probe_cache = BTreeMap::new();
        let mut profile_inflight = BTreeMap::new();

        for index in 0..profile_count {
            let name = format!("profile-{index:03}");
            profiles.insert(name.clone(), bench_profile_entry(&paths, &name));
            probe_cache.insert(name.clone(), bench_quota_compatible_probe_entry(now));
            if index > 0 && index + 1 < profile_count {
                profile_inflight.insert(name, usize::MAX / 4);
            }
        }

        let profile_name = "profile-000".to_string();
        let state = RuntimeRotationState {
            paths,
            state: AppState {
                active_profile: Some(profile_name.clone()),
                profiles,
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: profile_name.clone(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: probe_cache,
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight,
            profile_health: BTreeMap::new(),
        };

        Self {
            shared: bench_runtime_shared("quota-fallback", state, 32),
            excluded_profiles: BTreeSet::new(),
            profile_name,
        }
    }

    pub fn has_route_eligible_quota_fallback(&self) -> bool {
        runtime_has_route_eligible_quota_fallback(
            &self.shared,
            &self.profile_name,
            &self.excluded_profiles,
            RuntimeRouteKind::Responses,
        )
        .expect("benchmark selection should succeed")
    }
}

#[doc(hidden)]
pub struct RuntimeProxyPreviousResponseBenchCase {
    shared: RuntimeRotationProxyShared,
    excluded_profiles: BTreeSet<String>,
    previous_response_id: String,
}

impl RuntimeProxyPreviousResponseBenchCase {
    pub fn new(profile_count: usize) -> Self {
        let profile_count = profile_count.max(16);
        let paths = bench_paths("previous-response-selection");
        let now = Local::now().timestamp();
        let mut profiles = BTreeMap::new();
        let mut probe_cache = BTreeMap::new();
        let mut last_run_selected_at = BTreeMap::new();
        let mut profile_health = BTreeMap::new();
        let previous_response_id = "resp-bench".to_string();

        for index in 0..profile_count {
            let name = format!("profile-{index:03}");
            profiles.insert(name.clone(), bench_profile_entry(&paths, &name));
            probe_cache.insert(
                name.clone(),
                if index % 7 == 0 {
                    bench_probe_entry(now, 96, 96)
                } else {
                    bench_ready_probe_entry(now)
                },
            );
            last_run_selected_at.insert(name.clone(), now - index as i64);
            if index < profile_count / 3 {
                profile_health.insert(
                    runtime_previous_response_negative_cache_key(
                        &previous_response_id,
                        &name,
                        RuntimeRouteKind::Responses,
                    ),
                    RuntimeProfileHealth {
                        score: 1,
                        updated_at: now,
                    },
                );
            }
        }

        let current_profile = "profile-000".to_string();
        let state = RuntimeRotationState {
            paths,
            state: AppState {
                active_profile: Some(current_profile.clone()),
                profiles,
                last_run_selected_at,
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile,
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: probe_cache,
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health,
        };

        Self {
            shared: bench_runtime_shared("previous-response-selection", state, 32),
            excluded_profiles: BTreeSet::new(),
            previous_response_id,
        }
    }

    pub fn next_previous_response_candidate(&self) -> Option<String> {
        next_runtime_previous_response_candidate(
            &self.shared,
            &self.excluded_profiles,
            Some(&self.previous_response_id),
            RuntimeRouteKind::Responses,
        )
        .expect("benchmark previous response selection should succeed")
    }
}

#[doc(hidden)]
pub struct RuntimeProxyMixedPoolSelectionBenchCase {
    shared: RuntimeRotationProxyShared,
    excluded_profiles: BTreeSet<String>,
}

impl RuntimeProxyMixedPoolSelectionBenchCase {
    pub fn new(profile_count: usize) -> Self {
        let profile_count = profile_count.max(32);
        let paths = bench_paths("mixed-pool-selection");
        let now = Local::now().timestamp();
        let mut profiles = BTreeMap::new();
        let mut probe_cache = BTreeMap::new();
        let mut last_run_selected_at = BTreeMap::new();
        let mut retry_backoff_until = BTreeMap::new();
        let mut transport_backoff_until = BTreeMap::new();
        let mut route_circuit_open_until = BTreeMap::new();
        let mut profile_inflight = BTreeMap::new();
        let mut profile_health = BTreeMap::new();
        let mut excluded_profiles = BTreeSet::new();

        for index in 0..profile_count {
            let name = format!("profile-{index:03}");
            profiles.insert(name.clone(), bench_profile_entry(&paths, &name));
            let (primary_used, weekly_used) = match index % 12 {
                1 | 2 => (97, 97),
                3 => (88, 94),
                _ => (10 + (index % 20) as i64, 20 + (index % 15) as i64),
            };
            probe_cache.insert(
                name.clone(),
                bench_probe_entry(now, primary_used, weekly_used),
            );
            last_run_selected_at.insert(name.clone(), now - index as i64);

            if index < profile_count / 8 {
                excluded_profiles.insert(name.clone());
            } else if index < profile_count / 3 {
                match index % 5 {
                    0 => {
                        retry_backoff_until.insert(name.clone(), now + 90);
                    }
                    1 => {
                        transport_backoff_until.insert(
                            runtime_profile_transport_backoff_key(
                                &name,
                                RuntimeRouteKind::Responses,
                            ),
                            now + 45,
                        );
                    }
                    2 => {
                        route_circuit_open_until.insert(
                            runtime_profile_route_circuit_key(&name, RuntimeRouteKind::Responses),
                            now + 30,
                        );
                    }
                    3 => {
                        profile_inflight.insert(name.clone(), usize::MAX / 4);
                    }
                    _ => {
                        profile_health.insert(
                            runtime_profile_route_health_key(&name, RuntimeRouteKind::Responses),
                            RuntimeProfileHealth {
                                score: 3,
                                updated_at: now,
                            },
                        );
                    }
                }
            } else if index % 17 == 0 {
                profile_inflight.insert(name.clone(), 2);
            }
        }

        let current_profile = "profile-000".to_string();
        let state = RuntimeRotationState {
            paths,
            state: AppState {
                active_profile: Some(current_profile.clone()),
                profiles,
                last_run_selected_at,
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile,
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: probe_cache,
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: retry_backoff_until,
            profile_transport_backoff_until: transport_backoff_until,
            profile_route_circuit_open_until: route_circuit_open_until,
            profile_inflight,
            profile_health,
        };

        Self {
            shared: bench_runtime_shared("mixed-pool-selection", state, 64),
            excluded_profiles,
        }
    }

    pub fn select_fresh_response_candidate(&self) -> Option<String> {
        select_runtime_response_candidate_for_route(
            &self.shared,
            RuntimeResponseCandidateSelection::fresh(
                &self.excluded_profiles,
                RuntimeRouteKind::Responses,
            ),
        )
        .expect("benchmark mixed-pool selection should succeed")
    }
}

#[doc(hidden)]
pub struct RuntimeProxyCompactSessionSelectionBenchCase {
    shared: RuntimeRotationProxyShared,
    excluded_profiles: BTreeSet<String>,
    session_id: String,
}

impl RuntimeProxyCompactSessionSelectionBenchCase {
    pub fn new(profile_count: usize) -> Self {
        let profile_count = profile_count.max(16);
        let paths = bench_paths("compact-session-selection");
        let now = Local::now().timestamp();
        let mut profiles = BTreeMap::new();
        let mut probe_cache = BTreeMap::new();
        let mut last_run_selected_at = BTreeMap::new();
        let mut profile_inflight = BTreeMap::new();
        let session_id = "session-bench".to_string();
        let session_profile = format!("profile-{:03}", profile_count / 2);

        for index in 0..profile_count {
            let name = format!("profile-{index:03}");
            profiles.insert(name.clone(), bench_profile_entry(&paths, &name));
            probe_cache.insert(name.clone(), bench_ready_probe_entry(now));
            last_run_selected_at.insert(name.clone(), now - index as i64);
            if name == session_profile {
                profile_inflight.insert(name, usize::MAX / 4);
            }
        }

        let binding = ResponseProfileBinding {
            profile_name: session_profile,
            bound_at: now,
        };
        let mut session_id_bindings = BTreeMap::new();
        session_id_bindings.insert(session_id.clone(), binding.clone());
        let mut session_profile_bindings = BTreeMap::new();
        session_profile_bindings.insert(session_id.clone(), binding);
        let mut continuation_statuses = RuntimeContinuationStatuses::default();
        continuation_statuses.session_id.insert(
            session_id.clone(),
            bench_verified_continuation_status(now, RuntimeRouteKind::Compact),
        );

        let current_profile = "profile-000".to_string();
        let state = RuntimeRotationState {
            paths,
            state: AppState {
                active_profile: Some(current_profile.clone()),
                profiles,
                last_run_selected_at,
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings,
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile,
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings,
            continuation_statuses,
            profile_probe_cache: probe_cache,
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight,
            profile_health: BTreeMap::new(),
        };

        Self {
            shared: bench_runtime_shared("compact-session-selection", state, 32),
            excluded_profiles: BTreeSet::new(),
            session_id,
        }
    }

    pub fn select_compact_session_candidate(&self) -> Option<String> {
        let session_profile = runtime_session_bound_profile(&self.shared, &self.session_id)
            .expect("benchmark compact session lookup should succeed");
        select_runtime_response_candidate_for_route(
            &self.shared,
            RuntimeResponseCandidateSelection {
                session_profile: session_profile.as_deref(),
                ..RuntimeResponseCandidateSelection::fresh(
                    &self.excluded_profiles,
                    RuntimeRouteKind::Compact,
                )
            },
        )
        .expect("benchmark compact session selection should succeed")
    }
}

#[doc(hidden)]
pub struct RuntimeProxyWebsocketStaleReuseBenchCase {
    shared: RuntimeRotationProxyShared,
    excluded_profiles: BTreeSet<String>,
    previous_response_id: String,
    stale_elapsed: Duration,
}

impl RuntimeProxyWebsocketStaleReuseBenchCase {
    pub fn new(profile_count: usize) -> Self {
        let profile_count = profile_count.max(16);
        let paths = bench_paths("websocket-stale-reuse");
        let now = Local::now().timestamp();
        let mut profiles = BTreeMap::new();
        let mut probe_cache = BTreeMap::new();
        let mut last_run_selected_at = BTreeMap::new();
        let previous_response_id = "resp-websocket-bench".to_string();
        let owner_profile = format!("profile-{:03}", profile_count / 3);

        for index in 0..profile_count {
            let name = format!("profile-{index:03}");
            profiles.insert(name.clone(), bench_profile_entry(&paths, &name));
            probe_cache.insert(name.clone(), bench_ready_probe_entry(now));
            last_run_selected_at.insert(name, now - index as i64);
        }

        let mut response_profile_bindings = BTreeMap::new();
        response_profile_bindings.insert(
            previous_response_id.clone(),
            ResponseProfileBinding {
                profile_name: owner_profile,
                bound_at: now,
            },
        );
        let mut continuation_statuses = RuntimeContinuationStatuses::default();
        continuation_statuses.response.insert(
            previous_response_id.clone(),
            bench_verified_continuation_status(now, RuntimeRouteKind::Websocket),
        );

        let current_profile = "profile-000".to_string();
        let state = RuntimeRotationState {
            paths,
            state: AppState {
                active_profile: Some(current_profile.clone()),
                profiles,
                last_run_selected_at,
                response_profile_bindings,
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile,
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses,
            profile_probe_cache: probe_cache,
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        };

        Self {
            shared: bench_runtime_shared("websocket-stale-reuse", state, 32),
            excluded_profiles: BTreeSet::new(),
            previous_response_id,
            stale_elapsed: Duration::from_millis(
                RUNTIME_PROXY_WEBSOCKET_PREVIOUS_RESPONSE_REUSE_STALE_MS.saturating_add(1),
            ),
        }
    }

    pub fn evaluate_stale_reuse_affinity(&self) -> usize {
        let bound_profile = runtime_response_bound_profile(
            &self.shared,
            &self.previous_response_id,
            RuntimeRouteKind::Websocket,
        )
        .expect("benchmark websocket response binding lookup should succeed");
        let trusted = runtime_previous_response_affinity_is_trusted(
            &self.shared,
            Some(&self.previous_response_id),
            bound_profile.as_deref(),
        )
        .expect("benchmark websocket affinity trust lookup should succeed");
        let nonreplayable = runtime_websocket_previous_response_reuse_is_nonreplayable(
            Some(&self.previous_response_id),
            false,
            None,
        );
        let stale = runtime_websocket_previous_response_reuse_is_stale(
            nonreplayable,
            Some(self.stale_elapsed),
        );
        let fallback_allowed =
            runtime_websocket_reuse_watchdog_previous_response_fresh_fallback_allowed(
                RuntimeWebsocketReuseWatchdogPreviousResponseFallback {
                    profile_name: bound_profile.as_deref().unwrap_or(""),
                    previous_response_id: Some(&self.previous_response_id),
                    previous_response_fresh_fallback_used: false,
                    bound_profile: bound_profile.as_deref(),
                    pinned_profile: bound_profile.as_deref(),
                    request_requires_previous_response_affinity: false,
                    trusted_previous_response_affinity: trusted,
                    request_turn_state: None,
                    fresh_fallback_shape: Some(
                        RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation,
                    ),
                },
            );
        let candidate = select_runtime_response_candidate_for_route(
            &self.shared,
            RuntimeResponseCandidateSelection {
                pinned_profile: bound_profile.as_deref(),
                discover_previous_response_owner: true,
                previous_response_id: Some(&self.previous_response_id),
                ..RuntimeResponseCandidateSelection::fresh(
                    &self.excluded_profiles,
                    RuntimeRouteKind::Websocket,
                )
            },
        )
        .expect("benchmark websocket stale reuse selection should succeed");

        usize::from(trusted)
            + usize::from(nonreplayable)
            + usize::from(stale)
            + usize::from(!fallback_allowed)
            + usize::from(candidate.as_deref() == bound_profile.as_deref())
    }
}
