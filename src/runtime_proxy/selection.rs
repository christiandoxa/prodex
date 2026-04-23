use super::*;

pub(crate) fn runtime_proxy_sync_probe_pressure_pause(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
) {
    if !runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind) {
        return;
    }
    let pause_ms = RUNTIME_PROXY_SYNC_PROBE_PRESSURE_PAUSE_MS;
    let observed_revision = runtime_probe_refresh_revision();
    let started_at = Instant::now();
    let wait_outcome = runtime_probe_refresh_wait_outcome_since(
        Duration::from_millis(pause_ms),
        observed_revision,
    );
    runtime_proxy_log(
        shared,
        format!(
            "runtime_proxy_sync_probe_pressure_pause route={} pause_ms={} waited_ms={} outcome={}",
            runtime_route_kind_label(route_kind),
            pause_ms,
            started_at.elapsed().as_millis(),
            runtime_profile_wait_outcome_label(wait_outcome),
        ),
    );
}

pub(crate) fn runtime_previous_response_affinity_is_trusted(
    shared: &RuntimeRotationProxyShared,
    previous_response_id: Option<&str>,
    bound_profile: Option<&str>,
) -> Result<bool> {
    let Some(previous_response_id) = previous_response_id else {
        return Ok(false);
    };
    let Some(bound_profile) = bound_profile else {
        return Ok(false);
    };

    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let Some(binding) = runtime
        .state
        .response_profile_bindings
        .get(previous_response_id)
    else {
        return Ok(false);
    };
    if binding.profile_name != bound_profile {
        return Ok(false);
    }
    Ok(runtime_continuation_status_map(
        &runtime.continuation_statuses,
        RuntimeContinuationBindingKind::Response,
    )
    .get(previous_response_id)
    .is_none_or(|status| {
        status.state == RuntimeContinuationBindingLifecycle::Verified
            || (status.state == RuntimeContinuationBindingLifecycle::Warm
                && status.last_verified_at.is_some())
    }))
}

pub(crate) fn runtime_previous_response_affinity_is_bound(
    shared: &RuntimeRotationProxyShared,
    previous_response_id: Option<&str>,
    bound_profile: Option<&str>,
) -> Result<bool> {
    let Some(previous_response_id) = previous_response_id else {
        return Ok(false);
    };
    let Some(bound_profile) = bound_profile else {
        return Ok(false);
    };

    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    Ok(runtime
        .state
        .response_profile_bindings
        .get(previous_response_id)
        .is_some_and(|binding| binding.profile_name == bound_profile))
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeSelectionProfileEntry {
    pub(crate) name: String,
    pub(crate) codex_home: PathBuf,
    pub(crate) provider: ProfileProvider,
    pub(crate) last_run_selected_at: Option<i64>,
}

impl ProfileSelectionProvider for RuntimeSelectionProfileEntry {
    fn runtime_pool_priority(&self) -> usize {
        self.provider.runtime_pool_priority()
    }
}

impl RuntimeSelectionProfileEntry {
    pub(crate) fn supports_codex_runtime(&self) -> bool {
        self.provider.supports_codex_runtime()
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeProfileSelectionCatalogView<'a> {
    entries: &'a [RuntimeSelectionProfileEntry],
}

impl ProfileSelectionRead for RuntimeProfileSelectionCatalogView<'_> {
    type Profile = RuntimeSelectionProfileEntry;

    fn profile_names(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|entry| entry.name.clone())
            .collect()
    }

    fn profile_entry(&self, name: &str) -> Option<&Self::Profile> {
        self.entries.iter().find(|entry| entry.name == name)
    }

    fn last_run_selected_at(&self, name: &str) -> Option<i64> {
        self.profile_entry(name)
            .and_then(|entry| entry.last_run_selected_at)
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeProfileSelectionCatalog {
    pub(crate) entries: Vec<RuntimeSelectionProfileEntry>,
}

impl RuntimeProfileSelectionCatalog {
    pub(crate) fn contains(&self, profile_name: &str) -> bool {
        self.entries.iter().any(|entry| entry.name == profile_name)
    }
}

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

#[derive(Debug, Clone)]
pub(crate) struct RuntimeRouteSelectionEntry {
    pub(crate) profile: RuntimeSelectionProfileEntry,
    pub(crate) cached_auth_summary: Option<AuthSummary>,
    pub(crate) cached_probe_entry: Option<RuntimeProfileProbeCacheEntry>,
    pub(crate) cached_usage_snapshot: Option<RuntimeProfileUsageSnapshot>,
    pub(crate) auth_failure_active: bool,
    pub(crate) in_selection_backoff: bool,
    pub(crate) backoff_sort_key: (usize, i64, i64, i64),
    pub(crate) inflight_count: usize,
    pub(crate) health_sort_key: u32,
}

impl RuntimeRouteSelectionEntry {
    pub(crate) fn supports_codex_runtime(&self) -> bool {
        self.profile.supports_codex_runtime()
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeRouteSelectionCatalogView<'a> {
    entries: &'a [RuntimeRouteSelectionEntry],
}

impl ProfileSelectionRead for RuntimeRouteSelectionCatalogView<'_> {
    type Profile = RuntimeSelectionProfileEntry;

    fn profile_names(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|entry| entry.profile.name.clone())
            .collect()
    }

    fn profile_entry(&self, name: &str) -> Option<&Self::Profile> {
        self.entries
            .iter()
            .find(|entry| entry.profile.name == name)
            .map(|entry| &entry.profile)
    }

    fn last_run_selected_at(&self, name: &str) -> Option<i64> {
        self.profile_entry(name)
            .and_then(|entry| entry.last_run_selected_at)
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeRouteSelectionCatalog {
    pub(crate) current_profile: String,
    pub(crate) include_code_review: bool,
    pub(crate) upstream_base_url: String,
    pub(crate) entries: Vec<RuntimeRouteSelectionEntry>,
}

impl RuntimeRouteSelectionCatalog {
    pub(crate) fn entry(&self, profile_name: &str) -> Option<&RuntimeRouteSelectionEntry> {
        self.entries
            .iter()
            .find(|entry| entry.profile.name == profile_name)
    }

    pub(crate) fn persisted_usage_snapshots(
        &self,
    ) -> BTreeMap<String, RuntimeProfileUsageSnapshot> {
        self.entries
            .iter()
            .filter_map(|entry| {
                entry
                    .cached_usage_snapshot
                    .clone()
                    .map(|snapshot| (entry.profile.name.clone(), snapshot))
            })
            .collect()
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

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeCandidateAffinity<'a> {
    pub(crate) route_kind: RuntimeRouteKind,
    pub(crate) candidate_name: &'a str,
    pub(crate) strict_affinity_profile: Option<&'a str>,
    pub(crate) pinned_profile: Option<&'a str>,
    pub(crate) turn_state_profile: Option<&'a str>,
    pub(crate) session_profile: Option<&'a str>,
    pub(crate) trusted_previous_response_affinity: bool,
}

impl<'a> RuntimeCandidateAffinity<'a> {
    #[allow(dead_code)]
    pub(crate) fn new(
        route_kind: RuntimeRouteKind,
        candidate_name: &'a str,
        strict_affinity_profile: Option<&'a str>,
        pinned_profile: Option<&'a str>,
        turn_state_profile: Option<&'a str>,
        session_profile: Option<&'a str>,
        trusted_previous_response_affinity: bool,
    ) -> Self {
        Self {
            route_kind,
            candidate_name,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            trusted_previous_response_affinity,
        }
    }
}

pub(crate) fn runtime_candidate_has_hard_affinity(affinity: RuntimeCandidateAffinity<'_>) -> bool {
    affinity
        .strict_affinity_profile
        .is_some_and(|profile_name| profile_name == affinity.candidate_name)
        || affinity
            .turn_state_profile
            .is_some_and(|profile_name| profile_name == affinity.candidate_name)
        || (affinity.trusted_previous_response_affinity
            && affinity
                .pinned_profile
                .is_some_and(|profile_name| profile_name == affinity.candidate_name))
        || (affinity.route_kind == RuntimeRouteKind::Compact
            && affinity
                .session_profile
                .is_some_and(|profile_name| profile_name == affinity.candidate_name))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeQuotaBlockedAffinityReleasePolicy {
    KeepAffinity,
    ReleaseAffinity,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeQuotaBlockedAffinityReleaseRequest<'a> {
    pub(crate) affinity: RuntimeCandidateAffinity<'a>,
    pub(crate) fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
}

pub(crate) fn runtime_quota_blocked_affinity_release_policy(
    request: RuntimeQuotaBlockedAffinityReleaseRequest<'_>,
) -> RuntimeQuotaBlockedAffinityReleasePolicy {
    if runtime_previous_response_fresh_fallback_policy(
        RuntimePreviousResponseFreshFallbackPolicyInput {
            has_previous_response_context: request.fresh_fallback_shape.is_some(),
            request_requires_locked_previous_response_affinity: false,
            fresh_fallback_shape: request.fresh_fallback_shape,
        },
    )
    .is_fail_closed()
    {
        // Fail closed: classified previous_response continuations stay chained to the owner.
        return RuntimeQuotaBlockedAffinityReleasePolicy::KeepAffinity;
    }

    if request
        .affinity
        .strict_affinity_profile
        .is_some_and(|profile_name| profile_name == request.affinity.candidate_name)
        || request
            .affinity
            .turn_state_profile
            .is_some_and(|profile_name| profile_name == request.affinity.candidate_name)
        || (request.affinity.route_kind == RuntimeRouteKind::Compact
            && request
                .affinity
                .session_profile
                .is_some_and(|profile_name| profile_name == request.affinity.candidate_name))
    {
        return RuntimeQuotaBlockedAffinityReleasePolicy::KeepAffinity;
    }

    RuntimeQuotaBlockedAffinityReleasePolicy::ReleaseAffinity
}

pub(crate) fn runtime_quota_blocked_affinity_is_releasable(
    affinity: RuntimeCandidateAffinity<'_>,
    _request_requires_previous_response_affinity: bool,
    fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> bool {
    matches!(
        runtime_quota_blocked_affinity_release_policy(RuntimeQuotaBlockedAffinityReleaseRequest {
            affinity,
            fresh_fallback_shape,
        }),
        RuntimeQuotaBlockedAffinityReleasePolicy::ReleaseAffinity
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RuntimePreviousResponseNotFoundFallbackPolicy {
    pub(crate) stale_continuation: RuntimePreviousResponseStaleContinuationPolicy,
    pub(crate) fresh_fallback: RuntimePreviousResponseFreshFallbackPolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimePreviousResponseStaleContinuationPolicy {
    NotApplicable,
    RetryWithTurnState,
    FailClosed,
}

impl RuntimePreviousResponseStaleContinuationPolicy {
    pub(crate) fn requires_stale_continuation(self) -> bool {
        matches!(self, Self::FailClosed)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimePreviousResponseNotFoundFallbackRequest<'a> {
    pub(crate) previous_response_id: Option<&'a str>,
    pub(crate) has_turn_state_retry: bool,
    pub(crate) request_requires_locked_previous_response_affinity: bool,
    pub(crate) previous_response_fresh_fallback_used: bool,
    pub(crate) fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
}

pub(crate) fn runtime_previous_response_not_found_fallback_policy(
    request: RuntimePreviousResponseNotFoundFallbackRequest<'_>,
) -> RuntimePreviousResponseNotFoundFallbackPolicy {
    let stale_continuation = match (request.previous_response_id, request.has_turn_state_retry) {
        (Some(_), false) => RuntimePreviousResponseStaleContinuationPolicy::FailClosed,
        (Some(_), true) => RuntimePreviousResponseStaleContinuationPolicy::RetryWithTurnState,
        (None, _) => RuntimePreviousResponseStaleContinuationPolicy::NotApplicable,
    };
    let fresh_fallback = runtime_previous_response_fresh_fallback_policy(
        RuntimePreviousResponseFreshFallbackPolicyInput {
            has_previous_response_context: request.previous_response_id.is_some()
                || request.previous_response_fresh_fallback_used,
            request_requires_locked_previous_response_affinity: request
                .request_requires_locked_previous_response_affinity,
            fresh_fallback_shape: request.fresh_fallback_shape,
        },
    );

    RuntimePreviousResponseNotFoundFallbackPolicy {
        stale_continuation,
        fresh_fallback,
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn runtime_quota_blocked_previous_response_fresh_fallback_allowed(
    previous_response_id: Option<&str>,
    trusted_previous_response_affinity: bool,
    previous_response_fresh_fallback_used: bool,
    fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> bool {
    runtime_previous_response_fresh_fallback_policy(
        RuntimePreviousResponseFreshFallbackPolicyInput {
            has_previous_response_context: previous_response_id.is_some()
                || trusted_previous_response_affinity
                || previous_response_fresh_fallback_used,
            request_requires_locked_previous_response_affinity: false,
            fresh_fallback_shape,
        },
    )
    .allows_fresh_fallback()
}

pub(crate) fn runtime_websocket_previous_response_reuse_is_nonreplayable(
    previous_response_id: Option<&str>,
    previous_response_fresh_fallback_used: bool,
    turn_state_override: Option<&str>,
) -> bool {
    previous_response_id.is_some()
        && !previous_response_fresh_fallback_used
        && turn_state_override.is_none()
}

pub(crate) fn runtime_websocket_previous_response_reuse_is_stale(
    nonreplayable_previous_response_reuse: bool,
    reuse_terminal_idle: Option<Duration>,
) -> bool {
    nonreplayable_previous_response_reuse
        && reuse_terminal_idle.is_some_and(|elapsed| {
            elapsed
                >= Duration::from_millis(runtime_proxy_websocket_previous_response_reuse_stale_ms())
        })
}

#[allow(dead_code)]
pub(crate) fn runtime_websocket_reuse_watchdog_previous_response_fresh_fallback_allowed(
    request: RuntimeWebsocketReuseWatchdogPreviousResponseFallback<'_>,
) -> bool {
    let profile_matches_bound_owner = request.bound_profile == Some(request.profile_name);
    let profile_matches_pinned_owner = request.pinned_profile == Some(request.profile_name);

    runtime_previous_response_fresh_fallback_policy(
        RuntimePreviousResponseFreshFallbackPolicyInput {
            has_previous_response_context: request.previous_response_id.is_some()
                || request.previous_response_fresh_fallback_used
                || request.trusted_previous_response_affinity
                || profile_matches_bound_owner
                || profile_matches_pinned_owner
                || request.request_turn_state.is_some(),
            request_requires_locked_previous_response_affinity: request
                .request_requires_previous_response_affinity,
            fresh_fallback_shape: request.fresh_fallback_shape,
        },
    )
    .allows_fresh_fallback()
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub(crate) struct RuntimeWebsocketReuseWatchdogPreviousResponseFallback<'a> {
    pub(crate) profile_name: &'a str,
    pub(crate) previous_response_id: Option<&'a str>,
    pub(crate) previous_response_fresh_fallback_used: bool,
    pub(crate) bound_profile: Option<&'a str>,
    pub(crate) pinned_profile: Option<&'a str>,
    pub(crate) request_requires_previous_response_affinity: bool,
    pub(crate) trusted_previous_response_affinity: bool,
    pub(crate) request_turn_state: Option<&'a str>,
    pub(crate) fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn runtime_websocket_previous_response_not_found_requires_stale_continuation(
    previous_response_id: Option<&str>,
    has_turn_state_retry: bool,
    request_requires_locked_previous_response_affinity: bool,
    fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> bool {
    runtime_previous_response_not_found_fallback_policy(
        RuntimePreviousResponseNotFoundFallbackRequest {
            previous_response_id,
            has_turn_state_retry,
            request_requires_locked_previous_response_affinity,
            previous_response_fresh_fallback_used: false,
            fresh_fallback_shape,
        },
    )
    .stale_continuation
    .requires_stale_continuation()
}

pub(crate) fn runtime_quota_precommit_floor_percent(route_kind: RuntimeRouteKind) -> i64 {
    match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => {
            runtime_proxy_responses_quota_critical_floor_percent()
        }
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => 1,
    }
}

pub(crate) fn runtime_quota_window_precommit_guard(
    window: RuntimeQuotaWindowSummary,
    floor_percent: i64,
) -> bool {
    matches!(
        window.status,
        RuntimeQuotaWindowStatus::Critical | RuntimeQuotaWindowStatus::Exhausted
    ) && window.remaining_percent <= floor_percent
}

pub(crate) fn runtime_quota_precommit_guard_reason(
    summary: RuntimeQuotaSummary,
    route_kind: RuntimeRouteKind,
) -> Option<&'static str> {
    let floor_percent = runtime_quota_precommit_floor_percent(route_kind);
    if summary.route_band == RuntimeQuotaPressureBand::Exhausted {
        return Some("quota_exhausted_before_send");
    }

    if matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) && (runtime_quota_window_precommit_guard(summary.five_hour, floor_percent)
        || runtime_quota_window_precommit_guard(summary.weekly, floor_percent))
    {
        return Some("quota_critical_floor_before_send");
    }

    None
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeResponseCandidateSelection<'a> {
    pub(crate) excluded_profiles: &'a BTreeSet<String>,
    pub(crate) strict_affinity_profile: Option<&'a str>,
    pub(crate) pinned_profile: Option<&'a str>,
    pub(crate) turn_state_profile: Option<&'a str>,
    pub(crate) session_profile: Option<&'a str>,
    pub(crate) discover_previous_response_owner: bool,
    pub(crate) previous_response_id: Option<&'a str>,
    pub(crate) route_kind: RuntimeRouteKind,
}

#[cfg_attr(not(test), allow(dead_code))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn select_runtime_response_candidate_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    strict_affinity_profile: Option<&str>,
    pinned_profile: Option<&str>,
    turn_state_profile: Option<&str>,
    session_profile: Option<&str>,
    discover_previous_response_owner: bool,
    previous_response_id: Option<&str>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    select_runtime_response_candidate_for_route_with_selection(
        shared,
        RuntimeResponseCandidateSelection {
            excluded_profiles,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            discover_previous_response_owner,
            previous_response_id,
            route_kind,
        },
    )
}

pub(crate) fn select_runtime_response_candidate_for_route_with_selection(
    shared: &RuntimeRotationProxyShared,
    selection: RuntimeResponseCandidateSelection<'_>,
) -> Result<Option<String>> {
    let RuntimeResponseCandidateSelection {
        excluded_profiles,
        strict_affinity_profile,
        pinned_profile,
        turn_state_profile,
        session_profile,
        discover_previous_response_owner,
        previous_response_id,
        route_kind,
    } = selection;

    if let Some(profile_name) = strict_affinity_profile {
        if excluded_profiles.contains(profile_name) {
            return Ok(None);
        }
        if runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
            route_kind,
            candidate_name: profile_name,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            trusted_previous_response_affinity: false,
        }) {
            return Ok(Some(profile_name.to_string()));
        }
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
        let compact_followup_owner_without_probe = matches!(
            route_kind,
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
        ) && quota_source.is_none();
        if runtime_quota_summary_allows_soft_affinity(quota_summary, quota_source, route_kind)
            || compact_followup_owner_without_probe
        {
            return Ok(Some(profile_name.to_string()));
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=compact_followup profile={} reason={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                profile_name,
                runtime_quota_soft_affinity_rejection_reason(
                    quota_summary,
                    quota_source,
                    route_kind,
                ),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(None);
    }

    if let Some(profile_name) = pinned_profile.filter(|name| !excluded_profiles.contains(*name)) {
        if runtime_previous_response_affinity_is_bound(
            shared,
            previous_response_id,
            pinned_profile,
        )? {
            return Ok(Some(profile_name.to_string()));
        }
        if runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
            route_kind,
            candidate_name: profile_name,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            trusted_previous_response_affinity: runtime_previous_response_affinity_is_trusted(
                shared,
                previous_response_id,
                pinned_profile,
            )?,
        }) {
            return Ok(Some(profile_name.to_string()));
        }
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
        if quota_summary.route_band <= RuntimeQuotaPressureBand::Critical
            && runtime_quota_precommit_guard_reason(quota_summary, route_kind).is_none()
        {
            return Ok(Some(profile_name.to_string()));
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=pinned profile={} reason={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                profile_name,
                runtime_quota_pressure_band_reason(quota_summary.route_band),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
    }

    if let Some(profile_name) = turn_state_profile.filter(|name| !excluded_profiles.contains(*name))
    {
        if runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
            route_kind,
            candidate_name: profile_name,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            trusted_previous_response_affinity: false,
        }) {
            return Ok(Some(profile_name.to_string()));
        }
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
        if quota_summary.route_band <= RuntimeQuotaPressureBand::Critical
            && runtime_quota_precommit_guard_reason(quota_summary, route_kind).is_none()
        {
            return Ok(Some(profile_name.to_string()));
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=turn_state profile={} reason={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                profile_name,
                runtime_quota_pressure_band_reason(quota_summary.route_band),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
    }

    if discover_previous_response_owner {
        return next_runtime_previous_response_candidate(
            shared,
            excluded_profiles,
            previous_response_id,
            route_kind,
        );
    }

    if let Some(profile_name) = session_profile.filter(|name| !excluded_profiles.contains(*name)) {
        if runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
            route_kind,
            candidate_name: profile_name,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            trusted_previous_response_affinity: false,
        }) {
            return Ok(Some(profile_name.to_string()));
        }
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
        let compact_session_owner_without_probe =
            route_kind == RuntimeRouteKind::Compact && quota_source.is_none();
        let websocket_unknown_current_profile_without_pool_fallback = route_kind
            == RuntimeRouteKind::Websocket
            && quota_source.is_none()
            && runtime_proxy_current_profile(shared)? == profile_name
            && !runtime_has_route_eligible_quota_fallback(
                shared,
                profile_name,
                excluded_profiles,
                route_kind,
            )?;
        if runtime_quota_summary_allows_soft_affinity(quota_summary, quota_source, route_kind)
            || compact_session_owner_without_probe
            || websocket_unknown_current_profile_without_pool_fallback
        {
            return Ok(Some(profile_name.to_string()));
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=session profile={} reason={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                profile_name,
                runtime_quota_soft_affinity_rejection_reason(
                    quota_summary,
                    quota_source,
                    route_kind,
                ),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
    }

    if let Some(profile_name) =
        runtime_proxy_optimistic_current_candidate_for_route(shared, excluded_profiles, route_kind)?
    {
        return Ok(Some(profile_name));
    }

    next_runtime_response_candidate_for_route(shared, excluded_profiles, route_kind)
}

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
        name: String,
        codex_home: PathBuf,
        cached_auth_summary: Option<AuthSummary>,
        auth_failure_active: bool,
        negative_cache_active: bool,
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
            .map(|profile| RuntimePreviousResponseDiscoveryEntry {
                name: profile.name.clone(),
                codex_home: profile.codex_home.clone(),
                cached_auth_summary: runtime_profile_cached_auth_summary_from_maps_for_selection(
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
            })
            .collect::<Vec<_>>();
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
            .iter()
            .find(|entry| entry.name == owner)
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
        let Some(entry) = discovery_entries.iter().find(|entry| entry.name == name) else {
            continue;
        };
        if let Some(previous_response_id) = previous_response_id
            && entry.negative_cache_active
        {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_affinity route={} affinity=previous_response_discovery profile={} reason=negative_cache response_id={}",
                    runtime_route_kind_label(route_kind),
                    name,
                    previous_response_id,
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
                format!(
                    "selection_skip_affinity route={} affinity=previous_response_discovery profile={} reason=auth_failure_backoff",
                    runtime_route_kind_label(route_kind),
                    name,
                ),
            );
            continue;
        }
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, &name, route_kind)?;
        if quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted
            || runtime_quota_precommit_guard_reason(quota_summary, route_kind).is_some()
        {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_affinity route={} affinity=previous_response_discovery profile={} reason={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    name,
                    runtime_quota_precommit_guard_reason(quota_summary, route_kind).unwrap_or_else(
                        || runtime_quota_pressure_band_reason(quota_summary.route_band)
                    ),
                    quota_source
                        .map(runtime_quota_source_label)
                        .unwrap_or("unknown"),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            continue;
        }
        return Ok(Some(name));
    }
    Ok(None)
}

pub(crate) fn runtime_proxy_optimistic_current_candidate_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let pressure_mode = runtime_proxy_pressure_mode_active(shared);
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
    let quota_evidence_required =
        has_alternative_quota_compatible_profile && quota_source.is_none();
    let live_quota_probe_required = has_alternative_quota_compatible_profile
        && matches!(
            route_kind,
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
        )
        && !matches!(quota_source, Some(RuntimeQuotaSource::LiveProbe));
    let unknown_quota_allowed = quota_summary.route_band == RuntimeQuotaPressureBand::Unknown
        && !has_alternative_quota_compatible_profile;
    let quota_band_blocks_current =
        quota_summary.route_band > RuntimeQuotaPressureBand::Healthy && !unknown_quota_allowed;

    if auth_failure_active
        || in_selection_backoff
        || circuit_open_until.is_some()
        || health_score > 0
        || performance_score > 0
        || quota_evidence_required
        || live_quota_probe_required
        || inflight_count >= inflight_soft_limit
        || quota_band_blocks_current
    {
        let reason = if auth_failure_active {
            "auth_failure_backoff"
        } else if in_selection_backoff {
            "selection_backoff"
        } else if circuit_open_until.is_some() {
            "route_circuit_open"
        } else if health_score > 0 {
            "profile_health"
        } else if performance_score > 0 {
            "profile_performance"
        } else if quota_evidence_required {
            "quota_probe_unavailable"
        } else if live_quota_probe_required {
            if matches!(quota_source, Some(RuntimeQuotaSource::PersistedSnapshot)) {
                "stale_persisted_quota"
            } else {
                "quota_probe_unavailable"
            }
        } else if quota_band_blocks_current {
            runtime_quota_pressure_band_reason(quota_summary.route_band)
        } else {
            "profile_inflight_soft_limit"
        };
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_current route={} profile={} reason={} inflight={} health={} performance={} soft_limit={} circuit_until={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                current_profile,
                reason,
                inflight_count,
                health_score,
                performance_score,
                inflight_soft_limit,
                circuit_open_until.unwrap_or_default(),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(None);
    }
    if !current_profile_quota_compatible {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_current route={} profile={} reason=auth_not_quota_compatible",
                runtime_route_kind_label(route_kind),
                current_profile
            ),
        );
        return Ok(None);
    }

    runtime_proxy_log(
        shared,
        format!(
            "selection_keep_current route={} profile={} inflight={} health={} performance={} quota_source={} {}",
            runtime_route_kind_label(route_kind),
            current_profile,
            inflight_count,
            health_score,
            performance_score,
            quota_source
                .map(runtime_quota_source_label)
                .unwrap_or("unknown"),
            runtime_quota_summary_log_fields(quota_summary),
        ),
    );
    if !reserve_runtime_profile_route_circuit_half_open_probe(shared, &current_profile, route_kind)?
    {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_current route={} profile={} reason=route_circuit_half_open_probe_wait inflight={} health={} performance={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                current_profile,
                inflight_count,
                health_score,
                performance_score,
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(None);
    }
    Ok(Some(current_profile))
}

pub(crate) fn next_runtime_response_candidate_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
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
            format!(
                "selection_skip_sync_probe route={} reason=pressure_mode cold_start_jobs={}",
                runtime_route_kind_label(route_kind),
                skip_jobs,
            ),
        );
    }

    let mut reports = probe_plan.reports;
    let mut ready_candidates = probe_plan.ready_candidates;
    if probe_plan.should_sync_probe_cold_start {
        let base_url = Some(selection_state.upstream_base_url.clone());
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
                fetch_usage(&job.codex_home, base_url.as_deref()).map_err(|err| err.to_string())
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
                format!(
                    "selection_skip_sync_probe route={} reason=pressure_mode cold_start_profiles={}",
                    runtime_route_kind_label(route_kind),
                    skip_profiles
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
        |name| runtime_profile_selection_jitter(shared, name, route_kind),
    );

    for candidate in candidate_plan.ready_candidates {
        if let Some(reason) = candidate.ready_skip_reason() {
            if reason == "profile_inflight_soft_limit" {
                runtime_proxy_log(
                    shared,
                    format!(
                        "selection_skip_current route={} profile={} reason=profile_inflight_soft_limit inflight={} soft_limit={} health={} quota_source={} {}",
                        runtime_route_kind_label(route_kind),
                        candidate.name,
                        candidate.inflight_count,
                        candidate.inflight_soft_limit,
                        candidate.health_sort_key,
                        runtime_quota_source_label(candidate.quota_source),
                        runtime_quota_summary_log_fields(candidate.quota_summary),
                    ),
                );
            } else {
                runtime_proxy_log(
                    shared,
                    format!(
                        "selection_skip_current route={} profile={} reason={} inflight={} health={} quota_source={} {}",
                        runtime_route_kind_label(route_kind),
                        candidate.name,
                        reason,
                        candidate.inflight_count,
                        candidate.health_sort_key,
                        runtime_quota_source_label(candidate.quota_source),
                        runtime_quota_summary_log_fields(candidate.quota_summary),
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
                format!(
                    "selection_skip_current route={} profile={} reason=route_circuit_half_open_probe_wait inflight={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    candidate.inflight_count,
                    candidate.health_sort_key,
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(candidate.quota_summary),
                ),
            );
            continue;
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_pick route={} profile={} mode=ready inflight={} health={} order={} {}",
                runtime_route_kind_label(route_kind),
                candidate.name,
                candidate.inflight_count,
                candidate.health_sort_key,
                candidate.order_index,
                format_args!(
                    "quota_source={} {}",
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(candidate.quota_summary)
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
                format!(
                    "selection_skip_current route={} profile={} reason={} inflight={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    reason,
                    candidate.inflight_count,
                    candidate.health_sort_key,
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(candidate.quota_summary),
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
                format!(
                    "selection_skip_current route={} profile={} reason=route_circuit_half_open_probe_wait inflight={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    candidate.inflight_count,
                    candidate.health_sort_key,
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(candidate.quota_summary),
                ),
            );
            continue;
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_pick route={} profile={} mode=backoff inflight={} health={} backoff={:?} order={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                candidate.name,
                candidate.inflight_count,
                candidate.health_sort_key,
                candidate.backoff_sort_key,
                candidate.order_index,
                runtime_quota_source_label(candidate.quota_source),
                runtime_quota_summary_log_fields(candidate.quota_summary),
            ),
        );
        fallback = Some(candidate.name);
        break;
    }

    if fallback.is_none() {
        runtime_proxy_log(
            shared,
            format!(
                "selection_pick route={} profile=none mode=exhausted excluded_count={}",
                runtime_route_kind_label(route_kind),
                excluded_profiles.len()
            ),
        );
    }

    Ok(fallback)
}

pub(crate) fn runtime_remaining_sync_probe_cold_start_profiles_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<usize> {
    let allow_disk_auth_fallback =
        !runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind);
    let now = Local::now().timestamp();
    let state = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        runtime_route_selection_catalog(&runtime, route_kind, now)
    };

    Ok(active_profile_selection_order_with_view(
        runtime_route_selection_view(&state),
        &state.current_profile,
    )
    .into_iter()
    .filter(|name| !excluded_profiles.contains(name))
    .filter(|name| {
        state.entry(name).is_some_and(|entry| {
            entry
                .cached_auth_summary
                .clone()
                .or_else(|| {
                    allow_disk_auth_fallback.then(|| read_auth_summary(&entry.profile.codex_home))
                })
                .is_some_and(|summary| summary.quota_compatible)
        })
    })
    .filter(|name| {
        state
            .entry(name)
            .is_some_and(|entry| entry.cached_probe_entry.is_none())
    })
    .filter(|name| {
        !state.entry(name).is_some_and(|entry| {
            entry
                .cached_usage_snapshot
                .as_ref()
                .is_some_and(|snapshot| {
                    runtime_snapshot_blocks_same_request_cold_start_probe(snapshot, route_kind, now)
                })
        })
    })
    .count())
}

pub(crate) fn runtime_waitable_inflight_candidates_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
    wait_affinity_owner: Option<&str>,
) -> Result<BTreeSet<String>> {
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let state = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        runtime_route_selection_catalog(&runtime, route_kind, now)
    };
    let cached_usage_snapshots = state.persisted_usage_snapshots();

    let mut waitable_profiles = BTreeSet::new();
    let mut reports = Vec::new();
    for (order_index, name) in active_profile_selection_order_with_view(
        runtime_route_selection_view(&state),
        &state.current_profile,
    )
    .into_iter()
    .enumerate()
    {
        if excluded_profiles.contains(&name) {
            continue;
        }
        if wait_affinity_owner.is_some_and(|owner| owner != name) {
            continue;
        }
        let Some(entry) = state.entry(&name) else {
            continue;
        };
        let Some(probe_entry) = entry.cached_probe_entry.as_ref() else {
            continue;
        };
        reports.push(RunProfileProbeReport {
            name,
            order_index,
            auth: probe_entry.auth.clone(),
            result: probe_entry.result.clone(),
        });
    }

    for candidate in ready_profile_candidates_with_view(
        &reports,
        state.include_code_review,
        Some(state.current_profile.as_str()),
        runtime_route_selection_view(&state),
        Some(&cached_usage_snapshots),
    ) {
        if excluded_profiles.contains(&candidate.name) {
            continue;
        }
        let Some(entry) = state.entry(&candidate.name) else {
            continue;
        };
        if entry.in_selection_backoff || entry.auth_failure_active {
            continue;
        }
        if runtime_quota_precommit_guard_reason(
            runtime_quota_summary_for_route(&candidate.usage, route_kind),
            route_kind,
        )
        .is_some()
        {
            continue;
        }
        if entry.inflight_count >= inflight_soft_limit {
            waitable_profiles.insert(candidate.name.clone());
        }
    }

    Ok(waitable_profiles)
}

pub(crate) fn runtime_any_waited_candidate_relieved(
    shared: &RuntimeRotationProxyShared,
    waited_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<bool> {
    if waited_profiles.is_empty() {
        return Ok(false);
    }
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let state = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        runtime_route_selection_catalog(&runtime, route_kind, now)
    };
    let cached_usage_snapshots = state.persisted_usage_snapshots();

    let mut reports = Vec::new();
    for (order_index, name) in active_profile_selection_order_with_view(
        runtime_route_selection_view(&state),
        &state.current_profile,
    )
    .into_iter()
    .enumerate()
    {
        if !waited_profiles.contains(&name) {
            continue;
        }
        let Some(entry) = state.entry(&name) else {
            continue;
        };
        let Some(probe_entry) = entry.cached_probe_entry.as_ref() else {
            continue;
        };
        reports.push(RunProfileProbeReport {
            name,
            order_index,
            auth: probe_entry.auth.clone(),
            result: probe_entry.result.clone(),
        });
    }

    for candidate in ready_profile_candidates_with_view(
        &reports,
        state.include_code_review,
        Some(state.current_profile.as_str()),
        runtime_route_selection_view(&state),
        Some(&cached_usage_snapshots),
    ) {
        if !waited_profiles.contains(&candidate.name) {
            continue;
        }
        let Some(entry) = state.entry(&candidate.name) else {
            continue;
        };
        if entry.in_selection_backoff || entry.auth_failure_active {
            continue;
        }
        if runtime_quota_precommit_guard_reason(
            runtime_quota_summary_for_route(&candidate.usage, route_kind),
            route_kind,
        )
        .is_some()
        {
            continue;
        }
        if entry.inflight_count < inflight_soft_limit {
            return Ok(true);
        }
    }

    Ok(false)
}

pub(crate) struct RuntimeInflightReliefWait<'a> {
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &'a RuntimeRotationProxyShared,
    excluded_profiles: &'a BTreeSet<String>,
    route_kind: RuntimeRouteKind,
    selection_started_at: Instant,
    continuation: bool,
    wait_affinity_owner: Option<&'a str>,
}

impl<'a> RuntimeInflightReliefWait<'a> {
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        request_id: u64,
        request: &'a RuntimeProxyRequest,
        shared: &'a RuntimeRotationProxyShared,
        excluded_profiles: &'a BTreeSet<String>,
        route_kind: RuntimeRouteKind,
        selection_started_at: Instant,
        continuation: bool,
        wait_affinity_owner: Option<&'a str>,
    ) -> Self {
        Self {
            request_id,
            request,
            shared,
            excluded_profiles,
            route_kind,
            selection_started_at,
            continuation,
            wait_affinity_owner,
        }
    }
}

pub(crate) fn runtime_proxy_maybe_wait_for_interactive_inflight_relief(
    wait: RuntimeInflightReliefWait<'_>,
) -> Result<bool> {
    let RuntimeInflightReliefWait {
        request_id,
        request,
        shared,
        excluded_profiles,
        route_kind,
        selection_started_at,
        continuation,
        wait_affinity_owner,
    } = wait;

    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let wait_budget = runtime_proxy_request_inflight_wait_budget(request, pressure_mode);
    if wait_budget.is_zero() {
        return Ok(false);
    }
    let waited_profiles = runtime_waitable_inflight_candidates_for_route(
        shared,
        excluded_profiles,
        route_kind,
        wait_affinity_owner,
    )?;
    if waited_profiles.is_empty() {
        return Ok(false);
    }

    let (_, precommit_budget) = runtime_proxy_precommit_budget(continuation, pressure_mode);
    let remaining_budget = precommit_budget.saturating_sub(selection_started_at.elapsed());
    let total_wait_budget = wait_budget.min(remaining_budget);
    if total_wait_budget.is_zero() {
        return Ok(false);
    }
    let wait_deadline = Instant::now() + total_wait_budget;

    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http inflight_wait_started route={} wait_ms={}",
            runtime_route_kind_label(route_kind),
            total_wait_budget.as_millis()
        ),
    );
    let started_at = Instant::now();
    let mut observed_revision = runtime_profile_inflight_release_revision(shared);
    let mut signaled = false;
    let mut useful_relief = false;
    let mut wake_source = RuntimeProfileInFlightWaitOutcome::Timeout;
    loop {
        let remaining_wait = wait_deadline.saturating_duration_since(Instant::now());
        if remaining_wait.is_zero() {
            break;
        }
        match runtime_profile_inflight_wait_outcome_since(shared, remaining_wait, observed_revision)
        {
            RuntimeProfileInFlightWaitOutcome::InflightRelease => {
                signaled = true;
                wake_source = RuntimeProfileInFlightWaitOutcome::InflightRelease;
                observed_revision = runtime_profile_inflight_release_revision(shared);
                useful_relief =
                    runtime_any_waited_candidate_relieved(shared, &waited_profiles, route_kind)?;
                if useful_relief {
                    break;
                }
            }
            RuntimeProfileInFlightWaitOutcome::OtherNotify => {
                signaled = true;
                wake_source = RuntimeProfileInFlightWaitOutcome::OtherNotify;
                observed_revision = runtime_profile_inflight_release_revision(shared);
            }
            RuntimeProfileInFlightWaitOutcome::Timeout => {
                if !signaled {
                    wake_source = RuntimeProfileInFlightWaitOutcome::Timeout;
                }
                break;
            }
        }
    }
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http inflight_wait_finished route={} waited_ms={} signaled={signaled} useful={} wake_source={}",
            runtime_route_kind_label(route_kind),
            started_at.elapsed().as_millis(),
            useful_relief,
            runtime_profile_inflight_wait_outcome_label(wake_source),
        ),
    );
    Ok(useful_relief)
}
