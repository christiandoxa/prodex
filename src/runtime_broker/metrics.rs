use super::*;
use std::io::{Read, Seek, SeekFrom};

pub(crate) fn runtime_broker_continuation_metrics(
    statuses: &RuntimeContinuationStatuses,
    now: i64,
) -> RuntimeBrokerContinuationMetrics {
    let mut metrics = RuntimeBrokerContinuationMetrics {
        response_bindings: statuses.response.len(),
        turn_state_bindings: statuses.turn_state.len(),
        session_id_bindings: statuses.session_id.len(),
        warm: 0,
        verified: 0,
        suspect: 0,
        dead: 0,
        failure_counts: RuntimeBrokerContinuationSignalMetrics::default(),
        not_found_streaks: RuntimeBrokerContinuationSignalMetrics::default(),
        stale_verified_bindings: RuntimeBrokerContinuationSignalMetrics::default(),
    };
    for (kind, status) in statuses
        .response
        .values()
        .map(|status| (RuntimeContinuationBindingKind::Response, status))
        .chain(
            statuses
                .turn_state
                .values()
                .map(|status| (RuntimeContinuationBindingKind::TurnState, status)),
        )
        .chain(
            statuses
                .session_id
                .values()
                .map(|status| (RuntimeContinuationBindingKind::SessionId, status)),
        )
    {
        match status.state {
            RuntimeContinuationBindingLifecycle::Warm => metrics.warm += 1,
            RuntimeContinuationBindingLifecycle::Verified => metrics.verified += 1,
            RuntimeContinuationBindingLifecycle::Suspect => metrics.suspect += 1,
            RuntimeContinuationBindingLifecycle::Dead => metrics.dead += 1,
        }
        add_continuation_signal(
            &mut metrics.failure_counts,
            kind,
            status.failure_count as usize,
        );
        add_continuation_signal(
            &mut metrics.not_found_streaks,
            kind,
            status.not_found_streak as usize,
        );
        if runtime_continuation_status_is_stale_verified(status, now) {
            add_continuation_signal(&mut metrics.stale_verified_bindings, kind, 1);
        }
    }
    metrics
}

fn add_continuation_signal(
    metrics: &mut RuntimeBrokerContinuationSignalMetrics,
    kind: RuntimeContinuationBindingKind,
    value: usize,
) {
    match kind {
        RuntimeContinuationBindingKind::Response => metrics.response += value,
        RuntimeContinuationBindingKind::TurnState => metrics.turn_state += value,
        RuntimeContinuationBindingKind::SessionId => metrics.session_id += value,
    }
}

fn runtime_broker_previous_response_continuity_metrics(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
) -> RuntimeBrokerPreviousResponseContinuityMetrics {
    const PREFIX: &str = "__previous_response_not_found__:";

    let mut metrics = RuntimeBrokerPreviousResponseContinuityMetrics::default();
    for (key, entry) in profile_health {
        let Some(rest) = key.strip_prefix(PREFIX) else {
            continue;
        };
        let Some((route, _)) = rest.split_once(':') else {
            continue;
        };
        let score = runtime_profile_effective_score(
            entry,
            now,
            RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_SECONDS,
        );
        if score == 0 {
            continue;
        }
        if add_route_continuity_signal(&mut metrics.negative_cache_entries, route, 1) {
            let _ = add_route_continuity_signal(
                &mut metrics.negative_cache_failures,
                route,
                score as usize,
            );
        }
    }
    metrics
}

fn add_route_continuity_signal(
    metrics: &mut RuntimeBrokerRouteContinuityMetrics,
    route: &str,
    value: usize,
) -> bool {
    match route {
        "responses" => metrics.responses += value,
        "compact" => metrics.compact += value,
        "websocket" => metrics.websocket += value,
        "standard" => metrics.standard += value,
        _ => return false,
    }
    true
}

const RUNTIME_BROKER_CONTINUITY_FAILURE_REASON_CACHE_LIMIT: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeBrokerContinuityFailureReasonCacheFingerprint {
    len: u64,
    modified_at: Duration,
}

#[derive(Debug, Clone)]
struct RuntimeBrokerContinuityFailureReasonCacheEntry {
    fingerprint: RuntimeBrokerContinuityFailureReasonCacheFingerprint,
    metrics: RuntimeBrokerContinuityFailureReasonMetrics,
    last_used_at: u64,
}

#[derive(Debug, Default)]
struct RuntimeBrokerContinuityFailureReasonCache {
    entries: BTreeMap<PathBuf, RuntimeBrokerContinuityFailureReasonCacheEntry>,
    next_touch: u64,
    #[cfg(test)]
    full_rebuilds: u64,
    #[cfg(test)]
    incremental_updates: u64,
    #[cfg(test)]
    hits: u64,
    #[cfg(test)]
    misses: u64,
}

impl RuntimeBrokerContinuityFailureReasonCache {
    fn touch(&mut self) -> u64 {
        self.next_touch = self.next_touch.wrapping_add(1);
        self.next_touch
    }

    fn get(
        &mut self,
        log_path: &Path,
        fingerprint: &RuntimeBrokerContinuityFailureReasonCacheFingerprint,
    ) -> Option<RuntimeBrokerContinuityFailureReasonMetrics> {
        let touched_at = self.touch();
        let cached = self
            .entries
            .get_mut(log_path)
            .filter(|entry| entry.fingerprint == *fingerprint)
            .map(|entry| {
                entry.last_used_at = touched_at;
                entry.metrics.clone()
            });
        #[cfg(test)]
        if cached.is_some() {
            self.hits += 1;
        } else {
            self.misses += 1;
        }
        cached
    }

    fn store(
        &mut self,
        log_path: &Path,
        fingerprint: RuntimeBrokerContinuityFailureReasonCacheFingerprint,
        metrics: RuntimeBrokerContinuityFailureReasonMetrics,
    ) {
        let touched_at = self.touch();
        self.entries.insert(
            log_path.to_path_buf(),
            RuntimeBrokerContinuityFailureReasonCacheEntry {
                fingerprint,
                metrics,
                last_used_at: touched_at,
            },
        );
        while self.entries.len() > RUNTIME_BROKER_CONTINUITY_FAILURE_REASON_CACHE_LIMIT {
            let Some(oldest_path) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used_at)
                .map(|(path, _)| path.clone())
            else {
                break;
            };
            self.entries.remove(&oldest_path);
        }
    }

    fn remove(&mut self, log_path: &Path) {
        self.entries.remove(log_path);
    }

    #[cfg(test)]
    fn clear(&mut self) {
        self.entries.clear();
        self.next_touch = 0;
        self.full_rebuilds = 0;
        self.incremental_updates = 0;
        self.hits = 0;
        self.misses = 0;
    }
}

static RUNTIME_BROKER_CONTINUITY_FAILURE_REASON_CACHE: OnceLock<
    Mutex<RuntimeBrokerContinuityFailureReasonCache>,
> = OnceLock::new();

fn runtime_broker_continuity_failure_reason_cache()
-> &'static Mutex<RuntimeBrokerContinuityFailureReasonCache> {
    RUNTIME_BROKER_CONTINUITY_FAILURE_REASON_CACHE
        .get_or_init(|| Mutex::new(RuntimeBrokerContinuityFailureReasonCache::default()))
}

fn runtime_broker_continuity_failure_reason_cache_fingerprint(
    metadata: &fs::Metadata,
) -> Option<RuntimeBrokerContinuityFailureReasonCacheFingerprint> {
    let modified_at = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(RuntimeBrokerContinuityFailureReasonCacheFingerprint {
        len: metadata.len(),
        modified_at,
    })
}

fn runtime_broker_continuity_failure_reason_metrics_from_bytes(
    log: &[u8],
) -> RuntimeBrokerContinuityFailureReasonMetrics {
    let summary = summarize_runtime_log_tail(log);
    RuntimeBrokerContinuityFailureReasonMetrics {
        chain_retried_owner: summary.chain_retried_owner_by_reason,
        chain_dead_upstream_confirmed: summary.chain_dead_upstream_confirmed_by_reason,
        stale_continuation: summary.stale_continuation_by_reason,
    }
}

fn runtime_broker_continuity_failure_reason_metrics_from_log_range(
    log_path: &Path,
    start: u64,
) -> Option<RuntimeBrokerContinuityFailureReasonMetrics> {
    let mut log = fs::File::open(log_path).ok()?;
    if start > 0 {
        log.seek(SeekFrom::Start(start)).ok()?;
    }
    let mut buffer = Vec::new();
    log.read_to_end(&mut buffer).ok()?;
    Some(runtime_broker_continuity_failure_reason_metrics_from_bytes(
        &buffer,
    ))
}

pub(crate) fn runtime_broker_continuity_failure_reason_metrics_from_log_file(
    log_path: &Path,
) -> Option<RuntimeBrokerContinuityFailureReasonMetrics> {
    runtime_broker_continuity_failure_reason_metrics_from_log_range(log_path, 0)
}

fn runtime_broker_merge_continuity_failure_reason_metrics(
    metrics: &mut RuntimeBrokerContinuityFailureReasonMetrics,
    delta: RuntimeBrokerContinuityFailureReasonMetrics,
) {
    for (reason, count) in delta.chain_retried_owner {
        *metrics.chain_retried_owner.entry(reason).or_insert(0) += count;
    }
    for (reason, count) in delta.chain_dead_upstream_confirmed {
        *metrics
            .chain_dead_upstream_confirmed
            .entry(reason)
            .or_insert(0) += count;
    }
    for (reason, count) in delta.stale_continuation {
        *metrics.stale_continuation.entry(reason).or_insert(0) += count;
    }
}

fn runtime_broker_subtract_reason_metrics(
    metrics: BTreeMap<String, usize>,
    delta: &BTreeMap<String, usize>,
) -> BTreeMap<String, usize> {
    metrics
        .into_iter()
        .filter_map(|(reason, count)| {
            let remaining = count.saturating_sub(delta.get(&reason).copied().unwrap_or_default());
            (remaining > 0).then_some((reason, remaining))
        })
        .collect()
}

fn runtime_broker_subtract_continuity_failure_reason_metrics(
    metrics: RuntimeBrokerContinuityFailureReasonMetrics,
    delta: &RuntimeBrokerContinuityFailureReasonMetrics,
) -> RuntimeBrokerContinuityFailureReasonMetrics {
    RuntimeBrokerContinuityFailureReasonMetrics {
        chain_retried_owner: runtime_broker_subtract_reason_metrics(
            metrics.chain_retried_owner,
            &delta.chain_retried_owner,
        ),
        chain_dead_upstream_confirmed: runtime_broker_subtract_reason_metrics(
            metrics.chain_dead_upstream_confirmed,
            &delta.chain_dead_upstream_confirmed,
        ),
        stale_continuation: runtime_broker_subtract_reason_metrics(
            metrics.stale_continuation,
            &delta.stale_continuation,
        ),
    }
}

fn runtime_broker_continuity_failure_reason_metrics(
    log_path: &Path,
) -> RuntimeBrokerContinuityFailureReasonMetrics {
    let Ok(metadata) = fs::metadata(log_path) else {
        runtime_broker_continuity_failure_reason_cache()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(log_path);
        return RuntimeBrokerContinuityFailureReasonMetrics::default();
    };
    let fingerprint = runtime_broker_continuity_failure_reason_cache_fingerprint(&metadata);

    let append_base = if let Some(fingerprint) = fingerprint.as_ref() {
        let mut cache = runtime_broker_continuity_failure_reason_cache()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(metrics) = cache.get(log_path, fingerprint) {
            return metrics;
        }
        cache.entries.get(log_path).cloned().filter(|entry| {
            entry.fingerprint.len < fingerprint.len
                && entry.fingerprint.modified_at <= fingerprint.modified_at
        })
    } else {
        None
    };

    if let (Some(fingerprint), Some(base)) = (fingerprint.as_ref(), append_base)
        && let Some(delta) = runtime_broker_continuity_failure_reason_metrics_from_log_range(
            log_path,
            base.fingerprint.len,
        )
    {
        let mut metrics = base.metrics.clone();
        runtime_broker_merge_continuity_failure_reason_metrics(&mut metrics, delta);
        let mut cache = runtime_broker_continuity_failure_reason_cache()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        cache.store(log_path, fingerprint.clone(), metrics.clone());
        #[cfg(test)]
        {
            cache.incremental_updates += 1;
        }
        return metrics;
    }

    let Some(metrics) =
        runtime_broker_continuity_failure_reason_metrics_from_log_range(log_path, 0)
    else {
        runtime_broker_continuity_failure_reason_cache()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(log_path);
        return RuntimeBrokerContinuityFailureReasonMetrics::default();
    };

    if let Some(fingerprint) = fingerprint {
        let mut cache = runtime_broker_continuity_failure_reason_cache()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        cache.store(log_path, fingerprint, metrics.clone());
        #[cfg(test)]
        {
            cache.full_rebuilds += 1;
        }
    }
    metrics
}

fn runtime_broker_live_continuity_failure_reason_metrics(
    log_path: &Path,
    parsed_metrics: &RuntimeBrokerContinuityFailureReasonMetrics,
) -> Option<RuntimeBrokerContinuityFailureReasonMetrics> {
    let snapshot = runtime_proxy_continuity_failure_reason_metrics_snapshot(log_path)?;
    let persisted_since_baseline = runtime_broker_subtract_continuity_failure_reason_metrics(
        parsed_metrics.clone(),
        &snapshot.baseline_metrics,
    );
    let pending_live = runtime_broker_subtract_continuity_failure_reason_metrics(
        snapshot.live_metrics,
        &persisted_since_baseline,
    );
    let mut merged = parsed_metrics.clone();
    runtime_broker_merge_continuity_failure_reason_metrics(&mut merged, pending_live);
    Some(merged)
}

pub(crate) fn runtime_broker_prometheus_snapshot(
    metadata: &RuntimeBrokerMetadata,
    metrics: &RuntimeBrokerMetrics,
) -> runtime_metrics::RuntimeBrokerSnapshot {
    RuntimeBrokerSnapshotBuilder::new(metadata, metrics).build()
}

struct RuntimeBrokerSnapshotBuilder<'a> {
    metadata: &'a RuntimeBrokerMetadata,
    metrics: &'a RuntimeBrokerMetrics,
}

impl<'a> RuntimeBrokerSnapshotBuilder<'a> {
    fn new(metadata: &'a RuntimeBrokerMetadata, metrics: &'a RuntimeBrokerMetrics) -> Self {
        Self { metadata, metrics }
    }

    fn build(&self) -> runtime_metrics::RuntimeBrokerSnapshot {
        runtime_metrics::RuntimeBrokerSnapshot {
            broker_key: self.metadata.broker_key.clone(),
            listen_addr: self.metadata.listen_addr.clone(),
            pid: self.metrics.health.pid,
            started_at_unix_seconds: self.metrics.health.started_at,
            current_profile: self.metrics.health.current_profile.clone(),
            include_code_review: self.metrics.health.include_code_review,
            persistence_role: self.metrics.health.persistence_role.clone(),
            prodex_version: self.metrics.health.prodex_version.clone(),
            executable_path: self.metrics.health.executable_path.clone(),
            executable_sha256: self.metrics.health.executable_sha256.clone(),
            active_requests: self.metrics.health.active_requests as u64,
            active_request_limit: self.metrics.active_request_limit as u64,
            local_overload_backoff_remaining_seconds: self
                .metrics
                .local_overload_backoff_remaining_seconds,
            runtime_state_lock_wait: runtime_metrics::RuntimeBrokerStateLockWaitMetrics {
                wait_total_ns: self.metrics.runtime_state_lock_wait.wait_total_ns,
                wait_count: self.metrics.runtime_state_lock_wait.wait_count,
                wait_max_ns: self.metrics.runtime_state_lock_wait.wait_max_ns,
            },
            traffic: self.build_traffic(),
            profile_inflight: self.build_profile_inflight(),
            retry_backoffs: self.metrics.retry_backoffs as u64,
            transport_backoffs: self.metrics.transport_backoffs as u64,
            route_circuits: self.metrics.route_circuits as u64,
            degraded_profiles: self.metrics.degraded_profiles as u64,
            degraded_routes: self.metrics.degraded_routes as u64,
            continuations: self.build_continuations(),
            previous_response_continuity: self.build_previous_response_continuity(),
            continuity_failure_reasons: self.build_continuity_failure_reasons(),
        }
    }

    fn build_traffic(&self) -> runtime_metrics::RuntimeBrokerTrafficMetrics {
        runtime_metrics::RuntimeBrokerTrafficMetrics {
            responses: Self::build_lane(&self.metrics.traffic.responses),
            compact: Self::build_lane(&self.metrics.traffic.compact),
            websocket: Self::build_lane(&self.metrics.traffic.websocket),
            standard: Self::build_lane(&self.metrics.traffic.standard),
        }
    }

    fn build_profile_inflight(&self) -> std::collections::BTreeMap<String, u64> {
        self.metrics
            .profile_inflight
            .iter()
            .map(|(profile, count)| (profile.clone(), *count as u64))
            .collect()
    }

    fn build_continuations(&self) -> runtime_metrics::RuntimeBrokerContinuationMetrics {
        runtime_metrics::RuntimeBrokerContinuationMetrics {
            response_bindings: self.metrics.continuations.response_bindings as u64,
            turn_state_bindings: self.metrics.continuations.turn_state_bindings as u64,
            session_id_bindings: self.metrics.continuations.session_id_bindings as u64,
            warm: self.metrics.continuations.warm as u64,
            verified: self.metrics.continuations.verified as u64,
            suspect: self.metrics.continuations.suspect as u64,
            dead: self.metrics.continuations.dead as u64,
            failure_counts: runtime_metrics::RuntimeBrokerContinuationSignalMetrics {
                response: self.metrics.continuations.failure_counts.response as u64,
                turn_state: self.metrics.continuations.failure_counts.turn_state as u64,
                session_id: self.metrics.continuations.failure_counts.session_id as u64,
            },
            not_found_streaks: runtime_metrics::RuntimeBrokerContinuationSignalMetrics {
                response: self.metrics.continuations.not_found_streaks.response as u64,
                turn_state: self.metrics.continuations.not_found_streaks.turn_state as u64,
                session_id: self.metrics.continuations.not_found_streaks.session_id as u64,
            },
            stale_verified_bindings: runtime_metrics::RuntimeBrokerContinuationSignalMetrics {
                response: self.metrics.continuations.stale_verified_bindings.response as u64,
                turn_state: self
                    .metrics
                    .continuations
                    .stale_verified_bindings
                    .turn_state as u64,
                session_id: self
                    .metrics
                    .continuations
                    .stale_verified_bindings
                    .session_id as u64,
            },
        }
    }

    fn build_previous_response_continuity(
        &self,
    ) -> runtime_metrics::RuntimeBrokerPreviousResponseContinuityMetrics {
        runtime_metrics::RuntimeBrokerPreviousResponseContinuityMetrics {
            negative_cache_entries: Self::build_route_continuity(
                &self
                    .metrics
                    .previous_response_continuity
                    .negative_cache_entries,
            ),
            negative_cache_failures: Self::build_route_continuity(
                &self
                    .metrics
                    .previous_response_continuity
                    .negative_cache_failures,
            ),
        }
    }

    fn build_continuity_failure_reasons(
        &self,
    ) -> runtime_metrics::RuntimeBrokerContinuityFailureReasonMetrics {
        runtime_metrics::RuntimeBrokerContinuityFailureReasonMetrics {
            chain_retried_owner: Self::build_reason_counts(
                &self.metrics.continuity_failure_reasons.chain_retried_owner,
            ),
            chain_dead_upstream_confirmed: Self::build_reason_counts(
                &self
                    .metrics
                    .continuity_failure_reasons
                    .chain_dead_upstream_confirmed,
            ),
            stale_continuation: Self::build_reason_counts(
                &self.metrics.continuity_failure_reasons.stale_continuation,
            ),
        }
    }

    fn build_lane(lane: &RuntimeBrokerLaneMetrics) -> runtime_metrics::RuntimeBrokerLaneMetrics {
        runtime_metrics::RuntimeBrokerLaneMetrics {
            active: lane.active as u64,
            limit: lane.limit as u64,
            admissions_total: lane.admissions_total,
            global_limit_rejections_total: lane.global_limit_rejections_total,
            lane_limit_rejections_total: lane.lane_limit_rejections_total,
        }
    }

    fn build_route_continuity(
        metrics: &RuntimeBrokerRouteContinuityMetrics,
    ) -> runtime_metrics::RuntimeBrokerRouteContinuityMetrics {
        runtime_metrics::RuntimeBrokerRouteContinuityMetrics {
            responses: metrics.responses as u64,
            compact: metrics.compact as u64,
            websocket: metrics.websocket as u64,
            standard: metrics.standard as u64,
        }
    }

    fn build_reason_counts(metrics: &BTreeMap<String, usize>) -> BTreeMap<String, u64> {
        metrics
            .iter()
            .map(|(reason, count)| (reason.clone(), *count as u64))
            .collect()
    }
}

fn runtime_broker_live_lane_metrics(
    shared: &RuntimeRotationProxyShared,
    lane: RuntimeRouteKind,
) -> RuntimeBrokerLaneMetrics {
    RuntimeBrokerLaneMetrics {
        active: shared
            .lane_admission
            .active_counter(lane)
            .load(Ordering::SeqCst),
        limit: shared.lane_admission.limit(lane),
        admissions_total: shared
            .lane_admission
            .admissions_total_counter(lane)
            .load(Ordering::Relaxed),
        global_limit_rejections_total: shared
            .lane_admission
            .global_limit_rejections_total_counter(lane)
            .load(Ordering::Relaxed),
        lane_limit_rejections_total: shared
            .lane_admission
            .lane_limit_rejections_total_counter(lane)
            .load(Ordering::Relaxed),
    }
}

pub(crate) fn runtime_broker_metrics_snapshot(
    shared: &RuntimeRotationProxyShared,
    metadata: &RuntimeBrokerMetadata,
) -> Result<RuntimeBrokerMetrics> {
    let now = Local::now().timestamp();
    let now_u64 = now.max(0) as u64;
    let runtime = shared
        .lock_runtime_state()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;

    let health = RuntimeBrokerHealth {
        pid: std::process::id(),
        started_at: metadata.started_at,
        current_profile: metadata.current_profile.clone(),
        include_code_review: metadata.include_code_review,
        active_requests: shared.active_request_count.load(Ordering::SeqCst),
        instance_token: metadata.instance_token.clone(),
        persistence_role: if runtime_proxy_persistence_enabled(shared) {
            "owner".to_string()
        } else {
            "follower".to_string()
        },
        prodex_version: metadata.prodex_version.clone(),
        executable_path: metadata.executable_path.clone(),
        executable_sha256: metadata.executable_sha256.clone(),
    };

    let degraded_profiles = runtime
        .profile_health
        .iter()
        .filter(|(key, entry)| {
            !key.starts_with("__") && runtime_profile_effective_health_score(entry, now) > 0
        })
        .count();
    let degraded_routes = runtime
        .profile_health
        .iter()
        .filter(|(key, entry)| {
            key.starts_with("__route_health__:")
                && runtime_profile_effective_health_score(entry, now) > 0
        })
        .count();
    let parsed_continuity_failure_reasons =
        runtime_broker_continuity_failure_reason_metrics(&shared.log_path);

    Ok(RuntimeBrokerMetrics {
        health,
        active_request_limit: shared.active_request_limit,
        local_overload_backoff_remaining_seconds: shared
            .local_overload_backoff_until
            .load(Ordering::SeqCst)
            .saturating_sub(now_u64),
        runtime_state_lock_wait: shared.runtime_state_lock_wait_metrics(),
        traffic: RuntimeBrokerTrafficMetrics {
            responses: runtime_broker_live_lane_metrics(shared, RuntimeRouteKind::Responses),
            compact: runtime_broker_live_lane_metrics(shared, RuntimeRouteKind::Compact),
            websocket: runtime_broker_live_lane_metrics(shared, RuntimeRouteKind::Websocket),
            standard: runtime_broker_live_lane_metrics(shared, RuntimeRouteKind::Standard),
        },
        profile_inflight: runtime.profile_inflight.clone(),
        retry_backoffs: runtime
            .profile_retry_backoff_until
            .values()
            .filter(|until| **until > now)
            .count(),
        transport_backoffs: runtime
            .profile_transport_backoff_until
            .values()
            .filter(|until| **until > now)
            .count(),
        route_circuits: runtime
            .profile_route_circuit_open_until
            .values()
            .filter(|until| **until > now)
            .count(),
        degraded_profiles,
        degraded_routes,
        continuations: runtime_broker_continuation_metrics(&runtime.continuation_statuses, now),
        previous_response_continuity: runtime_broker_previous_response_continuity_metrics(
            &runtime.profile_health,
            now,
        ),
        continuity_failure_reasons: runtime_broker_live_continuity_failure_reason_metrics(
            &shared.log_path,
            &parsed_continuity_failure_reasons,
        )
        .unwrap_or(parsed_continuity_failure_reasons),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct RuntimeBrokerContinuityFailureReasonCacheStats {
        full_rebuilds: u64,
        incremental_updates: u64,
        hits: u64,
        misses: u64,
        entries: usize,
    }

    fn temp_log_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "prodex-runtime-broker-metrics-{name}-{}-{}.log",
            std::process::id(),
            Local::now().timestamp_nanos_opt().unwrap_or_default()
        ))
    }

    fn clear_runtime_broker_continuity_failure_reason_cache() {
        runtime_broker_continuity_failure_reason_cache()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
    }

    fn runtime_broker_continuity_failure_reason_cache_stats()
    -> RuntimeBrokerContinuityFailureReasonCacheStats {
        let cache = runtime_broker_continuity_failure_reason_cache()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        RuntimeBrokerContinuityFailureReasonCacheStats {
            full_rebuilds: cache.full_rebuilds,
            incremental_updates: cache.incremental_updates,
            hits: cache.hits,
            misses: cache.misses,
            entries: cache.entries.len(),
        }
    }

    fn test_runtime_broker_shared(log_path: PathBuf) -> RuntimeRotationProxyShared {
        let root = std::env::temp_dir().join(format!(
            "prodex-runtime-broker-metrics-shared-{}",
            Local::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let runtime = RuntimeRotationState {
            paths: AppPaths {
                state_file: root.join("state.json"),
                managed_profiles_root: root.join("profiles"),
                shared_codex_root: root.join("shared"),
                legacy_shared_codex_root: root.join("legacy-shared"),
                root,
            },
            state: AppState {
                active_profile: Some("main".to_string()),
                profiles: BTreeMap::from([(
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: PathBuf::from("/tmp/prodex-runtime-broker-metrics-main"),
                        managed: true,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        };

        RuntimeRotationProxyShared {
            async_client: reqwest::Client::builder()
                .build()
                .expect("test async client"),
            async_runtime: Arc::new(
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .build()
                    .expect("test async runtime"),
            ),
            runtime: Arc::new(Mutex::new(runtime)),
            log_path,
            request_sequence: Arc::new(AtomicU64::new(1)),
            state_save_revision: Arc::new(AtomicU64::new(0)),
            local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
            active_request_count: Arc::new(AtomicUsize::new(0)),
            active_request_limit: 8,
            runtime_state_lock_wait_counters:
                RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
            lane_admission: RuntimeProxyLaneAdmission::new(runtime_proxy_lane_limits(8, 1, 1)),
        }
    }

    #[test]
    fn continuity_failure_reason_metrics_follow_runtime_log_summary() {
        let _guard = acquire_test_runtime_lock();
        clear_runtime_broker_continuity_failure_reason_cache();
        let log_path = temp_log_path("reasons");
        fs::write(
            &log_path,
            concat!(
                "[2026-04-22 10:00:00.000 +00:00] request=3 transport=websocket route=websocket websocket_session=sess-1 chain_retried_owner profile=second previous_response_id=resp-second delay_ms=20 reason=previous_response_not_found_locked_affinity via=-\n",
                "[2026-04-22 10:00:00.001 +00:00] request=3 websocket_session=sess-1 stale_continuation reason=websocket_reuse_watchdog_locked_affinity profile=second event=timeout\n",
                "[2026-04-22 10:00:00.002 +00:00] request=4 transport=http route=responses stale_continuation reason=previous_response_not_found profile=main\n",
                "[2026-04-22 10:00:00.003 +00:00] request=3 transport=websocket route=websocket websocket_session=sess-1 chain_dead_upstream_confirmed profile=second previous_response_id=resp-second reason=previous_response_not_found_locked_affinity via=- event=-\n"
            ),
        )
        .expect("test log should write");

        let metrics = runtime_broker_continuity_failure_reason_metrics(&log_path);

        assert_eq!(
            metrics.chain_retried_owner,
            BTreeMap::from([("previous_response_not_found_locked_affinity".to_string(), 1,)])
        );
        assert_eq!(
            metrics.chain_dead_upstream_confirmed,
            BTreeMap::from([("previous_response_not_found_locked_affinity".to_string(), 1,)])
        );
        assert_eq!(
            metrics.stale_continuation,
            BTreeMap::from([
                ("previous_response_not_found".to_string(), 1),
                ("websocket_reuse_watchdog_locked_affinity".to_string(), 1,),
            ])
        );

        fs::remove_file(&log_path).expect("test log should clean up");
    }

    #[test]
    fn continuity_failure_reason_metrics_reuse_cached_summary_for_unchanged_log() {
        let _guard = acquire_test_runtime_lock();
        clear_runtime_broker_continuity_failure_reason_cache();
        let log_path = temp_log_path("cache-hit");
        fs::write(
            &log_path,
            "[2026-04-22 10:00:00.000 +00:00] request=8 transport=http route=responses stale_continuation reason=previous_response_not_found profile=main\n",
        )
        .expect("test log should write");

        let first = runtime_broker_continuity_failure_reason_metrics(&log_path);
        assert_eq!(
            runtime_broker_continuity_failure_reason_cache_stats(),
            RuntimeBrokerContinuityFailureReasonCacheStats {
                full_rebuilds: 1,
                incremental_updates: 0,
                hits: 0,
                misses: 1,
                entries: 1,
            }
        );

        let second = runtime_broker_continuity_failure_reason_metrics(&log_path);
        assert_eq!(second, first);
        assert_eq!(
            runtime_broker_continuity_failure_reason_cache_stats(),
            RuntimeBrokerContinuityFailureReasonCacheStats {
                full_rebuilds: 1,
                incremental_updates: 0,
                hits: 1,
                misses: 1,
                entries: 1,
            }
        );

        fs::remove_file(&log_path).expect("test log should clean up");
    }

    #[test]
    fn continuity_failure_reason_metrics_invalidate_cache_when_log_changes() {
        let _guard = acquire_test_runtime_lock();
        clear_runtime_broker_continuity_failure_reason_cache();
        let log_path = temp_log_path("cache-invalidate");
        fs::write(
            &log_path,
            "[2026-04-22 10:00:00.000 +00:00] request=5 transport=http route=responses stale_continuation reason=previous_response_not_found profile=main\n",
        )
        .expect("test log should write");

        let first = runtime_broker_continuity_failure_reason_metrics(&log_path);
        assert_eq!(
            first.stale_continuation,
            BTreeMap::from([("previous_response_not_found".to_string(), 1)])
        );
        assert_eq!(
            runtime_broker_continuity_failure_reason_cache_stats(),
            RuntimeBrokerContinuityFailureReasonCacheStats {
                full_rebuilds: 1,
                incremental_updates: 0,
                hits: 0,
                misses: 1,
                entries: 1,
            }
        );

        let mut log = fs::OpenOptions::new()
            .append(true)
            .open(&log_path)
            .expect("test log should open");
        writeln!(
            log,
            "[2026-04-22 10:00:00.001 +00:00] request=5 transport=websocket route=websocket websocket_session=sess-5 chain_retried_owner profile=main previous_response_id=resp-5 delay_ms=20 reason=previous_response_not_found_locked_affinity via=-"
        )
        .expect("test log should append");
        drop(log);

        let second = runtime_broker_continuity_failure_reason_metrics(&log_path);
        assert_eq!(
            second.chain_retried_owner,
            BTreeMap::from([("previous_response_not_found_locked_affinity".to_string(), 1)])
        );
        assert_eq!(
            second.stale_continuation,
            BTreeMap::from([("previous_response_not_found".to_string(), 1)])
        );
        assert_eq!(
            runtime_broker_continuity_failure_reason_cache_stats(),
            RuntimeBrokerContinuityFailureReasonCacheStats {
                full_rebuilds: 1,
                incremental_updates: 1,
                hits: 0,
                misses: 2,
                entries: 1,
            }
        );

        fs::remove_file(&log_path).expect("test log should clean up");
    }

    #[test]
    fn runtime_broker_metrics_snapshot_merges_live_and_parsed_continuity_failure_counters() {
        let _guard = acquire_test_runtime_lock();
        clear_runtime_broker_continuity_failure_reason_cache();
        clear_all_runtime_proxy_continuity_failure_reason_metrics();
        let log_path = temp_log_path("live-counters");
        clear_runtime_proxy_continuity_failure_reason_metrics(&log_path);
        fs::write(
            &log_path,
            "[2026-04-22 10:00:00.000 +00:00] request=8 transport=http route=responses stale_continuation reason=previous_response_not_found profile=main\n",
        )
        .expect("test log should write");

        let shared = test_runtime_broker_shared(log_path.clone());
        runtime_proxy_record_continuity_failure_reason(
            &shared,
            "chain_dead_upstream_confirmed",
            "previous_response_not_found_locked_affinity",
        );
        runtime_proxy_record_continuity_failure_reason(
            &shared,
            "stale_continuation",
            "websocket_reuse_watchdog_locked_affinity",
        );

        let metrics = runtime_broker_metrics_snapshot(
            &shared,
            &RuntimeBrokerMetadata {
                broker_key: "broker".to_string(),
                listen_addr: "127.0.0.1:12345".to_string(),
                started_at: Local::now().timestamp(),
                current_profile: "main".to_string(),
                include_code_review: false,
                instance_token: "instance".to_string(),
                admin_token: "secret".to_string(),
                prodex_version: None,
                executable_path: None,
                executable_sha256: None,
            },
        )
        .expect("broker metrics snapshot should succeed");

        assert_eq!(
            metrics
                .continuity_failure_reasons
                .chain_dead_upstream_confirmed,
            BTreeMap::from([("previous_response_not_found_locked_affinity".to_string(), 1,)])
        );
        assert_eq!(
            metrics.continuity_failure_reasons.stale_continuation,
            BTreeMap::from([
                ("previous_response_not_found".to_string(), 1),
                ("websocket_reuse_watchdog_locked_affinity".to_string(), 1),
            ])
        );

        clear_runtime_proxy_continuity_failure_reason_metrics(&log_path);
        fs::remove_file(&log_path).expect("test log should clean up");
    }

    #[test]
    fn runtime_broker_metrics_snapshot_counts_pending_live_reasons_once() {
        let _guard = acquire_test_runtime_lock();
        clear_runtime_broker_continuity_failure_reason_cache();
        clear_all_runtime_proxy_continuity_failure_reason_metrics();
        let log_path = temp_log_path("live-pending-same-reason");
        clear_runtime_proxy_continuity_failure_reason_metrics(&log_path);
        fs::write(
            &log_path,
            "[2026-04-22 10:00:00.000 +00:00] request=8 transport=http route=responses stale_continuation reason=previous_response_not_found profile=main\n",
        )
        .expect("test log should write");

        let shared = test_runtime_broker_shared(log_path.clone());
        runtime_proxy_record_continuity_failure_reason(
            &shared,
            "stale_continuation",
            "previous_response_not_found",
        );

        let metrics = runtime_broker_metrics_snapshot(
            &shared,
            &RuntimeBrokerMetadata {
                broker_key: "broker".to_string(),
                listen_addr: "127.0.0.1:12345".to_string(),
                started_at: Local::now().timestamp(),
                current_profile: "main".to_string(),
                include_code_review: false,
                instance_token: "instance".to_string(),
                admin_token: "secret".to_string(),
                prodex_version: None,
                executable_path: None,
                executable_sha256: None,
            },
        )
        .expect("broker metrics snapshot should succeed");

        assert_eq!(
            metrics.continuity_failure_reasons.stale_continuation,
            BTreeMap::from([("previous_response_not_found".to_string(), 2)])
        );

        clear_runtime_proxy_continuity_failure_reason_metrics(&log_path);
        fs::remove_file(&log_path).expect("test log should clean up");
    }

    #[test]
    fn runtime_proxy_continuity_failure_reason_metrics_snapshot_prunes_missing_log_path() {
        let _guard = acquire_test_runtime_lock();
        clear_all_runtime_proxy_continuity_failure_reason_metrics();
        let log_path = temp_log_path("live-prune-missing");
        fs::write(&log_path, "").expect("test log should exist");
        let shared = test_runtime_broker_shared(log_path.clone());
        runtime_proxy_record_continuity_failure_reason(
            &shared,
            "stale_continuation",
            "previous_response_not_found",
        );
        assert!(
            runtime_proxy_continuity_failure_reason_metrics_snapshot(&log_path).is_some(),
            "live entry should exist before log removal"
        );

        fs::remove_file(&log_path).expect("test log should clean up");

        assert!(
            runtime_proxy_continuity_failure_reason_metrics_snapshot(&log_path).is_none(),
            "missing log path should evict the live entry"
        );
    }

    #[test]
    fn runtime_proxy_continuity_failure_reason_metrics_store_evicts_old_log_paths() {
        let _guard = acquire_test_runtime_lock();
        clear_all_runtime_proxy_continuity_failure_reason_metrics();
        let mut log_paths = Vec::new();

        for index in 0..20 {
            let log_path = temp_log_path(&format!("live-store-{index}"));
            fs::write(&log_path, "").expect("test log should exist");
            let shared = test_runtime_broker_shared(log_path.clone());
            runtime_proxy_record_continuity_failure_reason(
                &shared,
                "stale_continuation",
                "previous_response_not_found",
            );
            log_paths.push(log_path);
        }

        assert_eq!(
            runtime_proxy_continuity_failure_reason_metrics_store_entry_count(),
            16
        );
        assert!(
            runtime_proxy_continuity_failure_reason_metrics_snapshot(&log_paths[0]).is_none(),
            "oldest path should be evicted once the store exceeds its cap"
        );
        assert!(
            runtime_proxy_continuity_failure_reason_metrics_snapshot(
                log_paths.last().expect("live store path"),
            )
            .is_some(),
            "most recent path should stay resident"
        );

        for log_path in log_paths {
            clear_runtime_proxy_continuity_failure_reason_metrics(&log_path);
            let _ = fs::remove_file(&log_path);
        }
        clear_all_runtime_proxy_continuity_failure_reason_metrics();
    }
}
