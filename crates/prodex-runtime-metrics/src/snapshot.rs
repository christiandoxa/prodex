use std::collections::BTreeMap;

use prodex_runtime_broker as broker;

use crate::types::*;

pub fn runtime_broker_prometheus_snapshot(
    metadata: &broker::RuntimeBrokerMetadata,
    metrics: &broker::RuntimeBrokerMetrics,
) -> RuntimeBrokerSnapshot {
    RuntimeBrokerSnapshotBuilder::new(metadata, metrics).build()
}

struct RuntimeBrokerSnapshotBuilder<'a> {
    metadata: &'a broker::RuntimeBrokerMetadata,
    metrics: &'a broker::RuntimeBrokerMetrics,
}

impl<'a> RuntimeBrokerSnapshotBuilder<'a> {
    fn new(
        metadata: &'a broker::RuntimeBrokerMetadata,
        metrics: &'a broker::RuntimeBrokerMetrics,
    ) -> Self {
        Self { metadata, metrics }
    }

    fn build(&self) -> RuntimeBrokerSnapshot {
        RuntimeBrokerSnapshot {
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
            runtime_state_lock_wait: Self::build_wait_metrics(self.metrics.runtime_state_lock_wait),
            admission_wait: Self::build_wait_metrics(self.metrics.admission_wait),
            long_lived_queue_wait: Self::build_wait_metrics(self.metrics.long_lived_queue_wait),
            traffic: self.build_traffic(),
            profile_inflight: self.build_profile_inflight(),
            active_request_release_underflows_total: self
                .metrics
                .active_request_release_underflows_total,
            profile_inflight_admissions_total: self.metrics.profile_inflight_admissions_total,
            profile_inflight_releases_total: self.metrics.profile_inflight_releases_total,
            profile_inflight_release_underflows_total: self
                .metrics
                .profile_inflight_release_underflows_total,
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

    fn build_traffic(&self) -> RuntimeBrokerTrafficMetrics {
        RuntimeBrokerTrafficMetrics {
            responses: Self::build_lane(&self.metrics.traffic.responses),
            compact: Self::build_lane(&self.metrics.traffic.compact),
            websocket: Self::build_lane(&self.metrics.traffic.websocket),
            standard: Self::build_lane(&self.metrics.traffic.standard),
        }
    }

    fn build_profile_inflight(&self) -> BTreeMap<String, u64> {
        self.metrics
            .profile_inflight
            .iter()
            .map(|(profile, count)| (profile.clone(), *count as u64))
            .collect()
    }

    fn build_continuations(&self) -> RuntimeBrokerContinuationMetrics {
        RuntimeBrokerContinuationMetrics {
            response_bindings: self.metrics.continuations.response_bindings as u64,
            turn_state_bindings: self.metrics.continuations.turn_state_bindings as u64,
            session_id_bindings: self.metrics.continuations.session_id_bindings as u64,
            warm: self.metrics.continuations.warm as u64,
            verified: self.metrics.continuations.verified as u64,
            suspect: self.metrics.continuations.suspect as u64,
            dead: self.metrics.continuations.dead as u64,
            failure_counts: RuntimeBrokerContinuationSignalMetrics {
                response: self.metrics.continuations.failure_counts.response as u64,
                turn_state: self.metrics.continuations.failure_counts.turn_state as u64,
                session_id: self.metrics.continuations.failure_counts.session_id as u64,
            },
            not_found_streaks: RuntimeBrokerContinuationSignalMetrics {
                response: self.metrics.continuations.not_found_streaks.response as u64,
                turn_state: self.metrics.continuations.not_found_streaks.turn_state as u64,
                session_id: self.metrics.continuations.not_found_streaks.session_id as u64,
            },
            stale_verified_bindings: RuntimeBrokerContinuationSignalMetrics {
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

    fn build_previous_response_continuity(&self) -> RuntimeBrokerPreviousResponseContinuityMetrics {
        RuntimeBrokerPreviousResponseContinuityMetrics {
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

    fn build_continuity_failure_reasons(&self) -> RuntimeBrokerContinuityFailureReasonMetrics {
        RuntimeBrokerContinuityFailureReasonMetrics {
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

    fn build_lane(lane: &broker::RuntimeBrokerLaneMetrics) -> RuntimeBrokerLaneMetrics {
        RuntimeBrokerLaneMetrics {
            active: lane.active as u64,
            limit: lane.limit as u64,
            admissions_total: lane.admissions_total,
            releases_total: lane.releases_total,
            global_limit_rejections_total: lane.global_limit_rejections_total,
            lane_limit_rejections_total: lane.lane_limit_rejections_total,
            release_underflows_total: lane.release_underflows_total,
        }
    }

    fn build_wait_metrics(
        metrics: broker::RuntimeWaitDurationMetrics,
    ) -> RuntimeBrokerWaitDurationMetrics {
        RuntimeBrokerWaitDurationMetrics {
            wait_total_ns: metrics.wait_total_ns,
            wait_count: metrics.wait_count,
            wait_max_ns: metrics.wait_max_ns,
        }
    }

    fn build_route_continuity(
        metrics: &broker::RuntimeBrokerRouteContinuityMetrics,
    ) -> RuntimeBrokerRouteContinuityMetrics {
        RuntimeBrokerRouteContinuityMetrics {
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

pub fn format_runtime_broker_snapshot_summary(snapshot: &RuntimeBrokerSnapshot) -> String {
    format!(
        "broker_key={} listen_addr={} profile={} active_requests={} limits={}/{}/{}/{} degraded_profiles={} degraded_routes={}",
        snapshot.broker_key,
        snapshot.listen_addr,
        snapshot.current_profile,
        snapshot.active_requests,
        snapshot.traffic.responses.limit,
        snapshot.traffic.compact.limit,
        snapshot.traffic.websocket.limit,
        snapshot.traffic.standard.limit,
        snapshot.degraded_profiles,
        snapshot.degraded_routes
    )
}
