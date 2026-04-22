use super::*;

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

fn runtime_broker_continuity_failure_reason_metrics(
    log_path: &Path,
) -> RuntimeBrokerContinuityFailureReasonMetrics {
    // Admin metrics scrapes can afford a full-run log scan so Prometheus can
    // expose cumulative reason counters without adding hot-path bookkeeping.
    let Ok(log) = fs::read(log_path) else {
        return RuntimeBrokerContinuityFailureReasonMetrics::default();
    };
    let summary = summarize_runtime_log_tail(&log);
    RuntimeBrokerContinuityFailureReasonMetrics {
        chain_retried_owner: summary.chain_retried_owner_by_reason,
        chain_dead_upstream_confirmed: summary.chain_dead_upstream_confirmed_by_reason,
        stale_continuation: summary.stale_continuation_by_reason,
    }
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
        .runtime
        .lock()
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

    Ok(RuntimeBrokerMetrics {
        health,
        active_request_limit: shared.active_request_limit,
        local_overload_backoff_remaining_seconds: shared
            .local_overload_backoff_until
            .load(Ordering::SeqCst)
            .saturating_sub(now_u64),
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
        continuity_failure_reasons: runtime_broker_continuity_failure_reason_metrics(
            &shared.log_path,
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_log_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "prodex-runtime-broker-metrics-{name}-{}-{}.log",
            std::process::id(),
            Local::now().timestamp_nanos_opt().unwrap_or_default()
        ))
    }

    #[test]
    fn continuity_failure_reason_metrics_follow_runtime_log_summary() {
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
}
