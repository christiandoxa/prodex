use super::*;

pub(crate) fn runtime_broker_continuation_metrics(
    statuses: &RuntimeContinuationStatuses,
) -> RuntimeBrokerContinuationMetrics {
    let mut metrics = RuntimeBrokerContinuationMetrics {
        response_bindings: statuses.response.len(),
        turn_state_bindings: statuses.turn_state.len(),
        session_id_bindings: statuses.session_id.len(),
        warm: 0,
        verified: 0,
        suspect: 0,
        dead: 0,
    };
    for status in statuses
        .response
        .values()
        .chain(statuses.turn_state.values())
        .chain(statuses.session_id.values())
    {
        match status.state {
            RuntimeContinuationBindingLifecycle::Warm => metrics.warm += 1,
            RuntimeContinuationBindingLifecycle::Verified => metrics.verified += 1,
            RuntimeContinuationBindingLifecycle::Suspect => metrics.suspect += 1,
            RuntimeContinuationBindingLifecycle::Dead => metrics.dead += 1,
        }
    }
    metrics
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
        }
    }

    fn build_lane(lane: &RuntimeBrokerLaneMetrics) -> runtime_metrics::RuntimeBrokerLaneMetrics {
        runtime_metrics::RuntimeBrokerLaneMetrics {
            active: lane.active as u64,
            limit: lane.limit as u64,
        }
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
            responses: RuntimeBrokerLaneMetrics {
                active: shared
                    .lane_admission
                    .responses_active
                    .load(Ordering::SeqCst),
                limit: shared.lane_admission.limits.responses,
            },
            compact: RuntimeBrokerLaneMetrics {
                active: shared.lane_admission.compact_active.load(Ordering::SeqCst),
                limit: shared.lane_admission.limits.compact,
            },
            websocket: RuntimeBrokerLaneMetrics {
                active: shared
                    .lane_admission
                    .websocket_active
                    .load(Ordering::SeqCst),
                limit: shared.lane_admission.limits.websocket,
            },
            standard: RuntimeBrokerLaneMetrics {
                active: shared.lane_admission.standard_active.load(Ordering::SeqCst),
                limit: shared.lane_admission.limits.standard,
            },
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
        continuations: runtime_broker_continuation_metrics(&runtime.continuation_statuses),
    })
}
