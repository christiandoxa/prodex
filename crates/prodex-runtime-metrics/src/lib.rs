use std::collections::BTreeMap;

use prodex_runtime_broker as broker;

mod render_helpers;
use render_helpers::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeBrokerLaneMetrics {
    pub active: u64,
    pub limit: u64,
    pub admissions_total: u64,
    pub releases_total: u64,
    pub global_limit_rejections_total: u64,
    pub lane_limit_rejections_total: u64,
    pub release_underflows_total: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeBrokerTrafficMetrics {
    pub responses: RuntimeBrokerLaneMetrics,
    pub compact: RuntimeBrokerLaneMetrics,
    pub websocket: RuntimeBrokerLaneMetrics,
    pub standard: RuntimeBrokerLaneMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeBrokerContinuationMetrics {
    pub response_bindings: u64,
    pub turn_state_bindings: u64,
    pub session_id_bindings: u64,
    pub warm: u64,
    pub verified: u64,
    pub suspect: u64,
    pub dead: u64,
    pub failure_counts: RuntimeBrokerContinuationSignalMetrics,
    pub not_found_streaks: RuntimeBrokerContinuationSignalMetrics,
    pub stale_verified_bindings: RuntimeBrokerContinuationSignalMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeBrokerContinuationSignalMetrics {
    pub response: u64,
    pub turn_state: u64,
    pub session_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeBrokerContinuityFailureReasonMetrics {
    pub chain_retried_owner: BTreeMap<String, u64>,
    pub chain_dead_upstream_confirmed: BTreeMap<String, u64>,
    pub stale_continuation: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeBrokerRouteContinuityMetrics {
    pub responses: u64,
    pub compact: u64,
    pub websocket: u64,
    pub standard: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeBrokerPreviousResponseContinuityMetrics {
    pub negative_cache_entries: RuntimeBrokerRouteContinuityMetrics,
    pub negative_cache_failures: RuntimeBrokerRouteContinuityMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeBrokerSnapshot {
    pub broker_key: String,
    pub listen_addr: String,
    pub pid: u32,
    pub started_at_unix_seconds: i64,
    pub current_profile: String,
    pub include_code_review: bool,
    pub persistence_role: String,
    pub prodex_version: Option<String>,
    pub executable_path: Option<String>,
    pub executable_sha256: Option<String>,
    pub active_requests: u64,
    pub active_request_limit: u64,
    pub local_overload_backoff_remaining_seconds: u64,
    pub runtime_state_lock_wait: RuntimeBrokerStateLockWaitMetrics,
    pub traffic: RuntimeBrokerTrafficMetrics,
    pub profile_inflight: BTreeMap<String, u64>,
    pub active_request_release_underflows_total: u64,
    pub profile_inflight_admissions_total: u64,
    pub profile_inflight_releases_total: u64,
    pub profile_inflight_release_underflows_total: u64,
    pub retry_backoffs: u64,
    pub transport_backoffs: u64,
    pub route_circuits: u64,
    pub degraded_profiles: u64,
    pub degraded_routes: u64,
    pub continuations: RuntimeBrokerContinuationMetrics,
    pub previous_response_continuity: RuntimeBrokerPreviousResponseContinuityMetrics,
    pub continuity_failure_reasons: RuntimeBrokerContinuityFailureReasonMetrics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RuntimeBrokerStateLockWaitMetrics {
    pub wait_total_ns: u64,
    pub wait_count: u64,
    pub wait_max_ns: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrometheusTextOptions {
    pub include_help: bool,
}

impl Default for PrometheusTextOptions {
    fn default() -> Self {
        Self { include_help: true }
    }
}

pub fn runtime_broker_prometheus_snapshot(
    metadata: &broker::RuntimeBrokerMetadata,
    metrics: &broker::RuntimeBrokerMetrics,
) -> RuntimeBrokerSnapshot {
    RuntimeBrokerSnapshotBuilder::new(metadata, metrics).build()
}

pub fn render_runtime_broker_prometheus_from_metrics(
    metadata: &broker::RuntimeBrokerMetadata,
    metrics: &broker::RuntimeBrokerMetrics,
) -> String {
    render_runtime_broker_prometheus(&runtime_broker_prometheus_snapshot(metadata, metrics))
}

pub fn render_runtime_broker_prometheus(snapshot: &RuntimeBrokerSnapshot) -> String {
    render_runtime_broker_prometheus_with_options(snapshot, PrometheusTextOptions::default())
}

pub fn render_runtime_broker_prometheus_with_options(
    snapshot: &RuntimeBrokerSnapshot,
    options: PrometheusTextOptions,
) -> String {
    RuntimeBrokerPrometheusRenderer::new(snapshot, options).render()
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
            runtime_state_lock_wait: RuntimeBrokerStateLockWaitMetrics {
                wait_total_ns: self.metrics.runtime_state_lock_wait.wait_total_ns,
                wait_count: self.metrics.runtime_state_lock_wait.wait_count,
                wait_max_ns: self.metrics.runtime_state_lock_wait.wait_max_ns,
            },
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

struct RuntimeBrokerPrometheusRenderer<'a> {
    snapshot: &'a RuntimeBrokerSnapshot,
    options: PrometheusTextOptions,
    out: String,
}

impl<'a> RuntimeBrokerPrometheusRenderer<'a> {
    fn new(snapshot: &'a RuntimeBrokerSnapshot, options: PrometheusTextOptions) -> Self {
        Self {
            snapshot,
            options,
            out: String::new(),
        }
    }

    fn render(mut self) -> String {
        self.render_info();
        self.render_lane_families();
        self.render_broker_gauge(
            "prodex_runtime_broker_active_requests",
            "Active runtime requests currently being served by the broker.",
            self.snapshot.active_requests as f64,
        );
        self.render_broker_gauge(
            "prodex_runtime_broker_active_request_limit",
            "Maximum active runtime requests allowed by the broker.",
            self.snapshot.active_request_limit as f64,
        );
        self.render_broker_gauge(
            "prodex_runtime_broker_local_overload_backoff_remaining_seconds",
            "Remaining backoff time for local overload shedding.",
            self.snapshot.local_overload_backoff_remaining_seconds as f64,
        );
        self.render_broker_counter(
            "prodex_runtime_broker_runtime_state_lock_wait_total_seconds",
            "Cumulative time spent waiting for the runtime state lock.",
            self.snapshot.runtime_state_lock_wait.wait_total_ns as f64 / 1_000_000_000.0,
        );
        self.render_broker_counter(
            "prodex_runtime_broker_runtime_state_lock_acquisitions_total",
            "Cumulative runtime state lock acquisitions observed by instrumented call sites.",
            self.snapshot.runtime_state_lock_wait.wait_count as f64,
        );
        self.render_broker_gauge(
            "prodex_runtime_broker_runtime_state_lock_wait_max_seconds",
            "Maximum observed wait for the runtime state lock.",
            self.snapshot.runtime_state_lock_wait.wait_max_ns as f64 / 1_000_000_000.0,
        );
        self.render_guard_counters();
        self.render_broker_gauge(
            "prodex_runtime_broker_retry_backoffs",
            "Profiles currently in retry backoff.",
            self.snapshot.retry_backoffs as f64,
        );
        self.render_broker_gauge(
            "prodex_runtime_broker_transport_backoffs",
            "Profiles currently in transport backoff.",
            self.snapshot.transport_backoffs as f64,
        );
        self.render_broker_gauge(
            "prodex_runtime_broker_route_circuits",
            "Profiles currently protected by an open circuit per route.",
            self.snapshot.route_circuits as f64,
        );
        self.render_broker_gauge(
            "prodex_runtime_broker_degraded_profiles",
            "Profiles with a non-zero effective health score.",
            self.snapshot.degraded_profiles as f64,
        );
        self.render_broker_gauge(
            "prodex_runtime_broker_degraded_routes",
            "Route-specific health scores that are still degraded.",
            self.snapshot.degraded_routes as f64,
        );
        render_continuation_binding_counts_family(
            &mut self.out,
            "prodex_runtime_broker_continuation_binding_counts",
            "Continuation binding counts grouped by binding kind.",
            &self.snapshot.continuations,
            self.snapshot.broker_key.as_str(),
            self.snapshot.listen_addr.as_str(),
        );
        render_continuation_lifecycle_family(
            &mut self.out,
            "prodex_runtime_broker_continuation_bindings",
            "Continuation bindings grouped by lifecycle.",
            &self.snapshot.continuations,
            self.snapshot.broker_key.as_str(),
            self.snapshot.listen_addr.as_str(),
        );
        render_continuation_signal_family(
            &mut self.out,
            "prodex_runtime_broker_continuation_failure_counts",
            "Current aggregate continuity failure counts stored on continuation statuses by binding kind.",
            &self.snapshot.continuations.failure_counts,
            self.snapshot.broker_key.as_str(),
            self.snapshot.listen_addr.as_str(),
        );
        render_continuation_signal_family(
            &mut self.out,
            "prodex_runtime_broker_continuation_not_found_streaks",
            "Current aggregate not-found streaks stored on continuation statuses by binding kind.",
            &self.snapshot.continuations.not_found_streaks,
            self.snapshot.broker_key.as_str(),
            self.snapshot.listen_addr.as_str(),
        );
        render_continuation_signal_family(
            &mut self.out,
            "prodex_runtime_broker_continuation_stale_verified_bindings",
            "Verified continuation statuses that have aged past the stale horizon by binding kind.",
            &self.snapshot.continuations.stale_verified_bindings,
            self.snapshot.broker_key.as_str(),
            self.snapshot.listen_addr.as_str(),
        );
        render_continuity_failure_reason_family(
            &mut self.out,
            "prodex_runtime_broker_continuity_failures_total",
            "Cumulative continuity failure events observed in the broker runtime log by event and reason.",
            &self.snapshot.continuity_failure_reasons,
            self.snapshot.broker_key.as_str(),
            self.snapshot.listen_addr.as_str(),
        );
        render_route_continuity_family(
            &mut self.out,
            "prodex_runtime_broker_previous_response_negative_cache_entries",
            "Current active previous_response not-found negative-cache entries by route.",
            &self
                .snapshot
                .previous_response_continuity
                .negative_cache_entries,
            self.snapshot.broker_key.as_str(),
            self.snapshot.listen_addr.as_str(),
        );
        render_route_continuity_family(
            &mut self.out,
            "prodex_runtime_broker_previous_response_negative_cache_failures",
            "Current effective previous_response not-found negative-cache failure score by route.",
            &self
                .snapshot
                .previous_response_continuity
                .negative_cache_failures,
            self.snapshot.broker_key.as_str(),
            self.snapshot.listen_addr.as_str(),
        );
        render_inflight_family(
            &mut self.out,
            "prodex_runtime_broker_profile_inflight",
            "Current per-profile inflight counts.",
            &self.snapshot.profile_inflight,
            self.snapshot.broker_key.as_str(),
            self.snapshot.listen_addr.as_str(),
        );
        self.out
    }

    fn render_info(&mut self) {
        self.push_help(
            "prodex_runtime_broker_info",
            "Static broker metadata and current health attributes.",
        );
        push_type(&mut self.out, "prodex_runtime_broker_info", "gauge");
        push_gauge(
            &mut self.out,
            "prodex_runtime_broker_info",
            labels(&[
                ("broker_key", self.snapshot.broker_key.as_str()),
                ("listen_addr", self.snapshot.listen_addr.as_str()),
                ("current_profile", self.snapshot.current_profile.as_str()),
                (
                    "include_code_review",
                    bool_label(self.snapshot.include_code_review),
                ),
                ("persistence_role", self.snapshot.persistence_role.as_str()),
                (
                    "prodex_version",
                    self.snapshot.prodex_version.as_deref().unwrap_or("-"),
                ),
                (
                    "executable_path",
                    self.snapshot.executable_path.as_deref().unwrap_or("-"),
                ),
                (
                    "executable_sha256",
                    self.snapshot.executable_sha256.as_deref().unwrap_or("-"),
                ),
            ]),
            1.0,
        );
    }

    fn render_lane_families(&mut self) {
        let lanes = [
            ("responses", &self.snapshot.traffic.responses),
            ("compact", &self.snapshot.traffic.compact),
            ("websocket", &self.snapshot.traffic.websocket),
            ("standard", &self.snapshot.traffic.standard),
        ];
        render_lane_family(
            &mut self.out,
            LaneFamilyDescriptor {
                metric_name: "prodex_runtime_broker_lane_active_requests",
                help: "Current active requests per broker lane.",
                metric_type: "gauge",
            },
            self.snapshot.broker_key.as_str(),
            self.snapshot.listen_addr.as_str(),
            &lanes,
            |lane| lane.active as f64,
        );
        render_lane_family(
            &mut self.out,
            LaneFamilyDescriptor {
                metric_name: "prodex_runtime_broker_lane_limits",
                help: "Configured admission limits per broker lane.",
                metric_type: "gauge",
            },
            self.snapshot.broker_key.as_str(),
            self.snapshot.listen_addr.as_str(),
            &lanes,
            |lane| lane.limit as f64,
        );
        render_lane_family(
            &mut self.out,
            LaneFamilyDescriptor {
                metric_name: "prodex_runtime_broker_lane_admissions_total",
                help: "Cumulative successful admissions per broker lane.",
                metric_type: "counter",
            },
            self.snapshot.broker_key.as_str(),
            self.snapshot.listen_addr.as_str(),
            &lanes,
            |lane| lane.admissions_total as f64,
        );
        render_lane_family(
            &mut self.out,
            LaneFamilyDescriptor {
                metric_name: "prodex_runtime_broker_lane_releases_total",
                help: "Cumulative active-request guard releases per broker lane.",
                metric_type: "counter",
            },
            self.snapshot.broker_key.as_str(),
            self.snapshot.listen_addr.as_str(),
            &lanes,
            |lane| lane.releases_total as f64,
        );
        render_lane_family(
            &mut self.out,
            LaneFamilyDescriptor {
                metric_name: "prodex_runtime_broker_lane_global_limit_rejections_total",
                help: "Cumulative broker-global admission rejections per lane.",
                metric_type: "counter",
            },
            self.snapshot.broker_key.as_str(),
            self.snapshot.listen_addr.as_str(),
            &lanes,
            |lane| lane.global_limit_rejections_total as f64,
        );
        render_lane_family(
            &mut self.out,
            LaneFamilyDescriptor {
                metric_name: "prodex_runtime_broker_lane_lane_limit_rejections_total",
                help: "Cumulative per-lane admission rejections per lane.",
                metric_type: "counter",
            },
            self.snapshot.broker_key.as_str(),
            self.snapshot.listen_addr.as_str(),
            &lanes,
            |lane| lane.lane_limit_rejections_total as f64,
        );
        render_lane_family(
            &mut self.out,
            LaneFamilyDescriptor {
                metric_name: "prodex_runtime_broker_lane_release_underflows_total",
                help: "Cumulative active-request guard release underflows per broker lane.",
                metric_type: "counter",
            },
            self.snapshot.broker_key.as_str(),
            self.snapshot.listen_addr.as_str(),
            &lanes,
            |lane| lane.release_underflows_total as f64,
        );
    }

    fn render_broker_gauge(&mut self, metric_name: &str, help: &str, value: f64) {
        self.push_help(metric_name, help);
        push_type(&mut self.out, metric_name, "gauge");
        push_gauge(
            &mut self.out,
            metric_name,
            labels(&[
                ("broker_key", self.snapshot.broker_key.as_str()),
                ("listen_addr", self.snapshot.listen_addr.as_str()),
            ]),
            value,
        );
    }

    fn render_broker_counter(&mut self, metric_name: &str, help: &str, value: f64) {
        self.push_help(metric_name, help);
        push_type(&mut self.out, metric_name, "counter");
        push_gauge(
            &mut self.out,
            metric_name,
            labels(&[
                ("broker_key", self.snapshot.broker_key.as_str()),
                ("listen_addr", self.snapshot.listen_addr.as_str()),
            ]),
            value,
        );
    }

    fn render_guard_counters(&mut self) {
        self.render_broker_counter(
            "prodex_runtime_broker_active_request_release_underflows_total",
            "Cumulative active-request guard release underflows across all lanes.",
            self.snapshot.active_request_release_underflows_total as f64,
        );
        self.render_broker_counter(
            "prodex_runtime_broker_profile_inflight_admissions_total",
            "Cumulative profile in-flight guard admissions.",
            self.snapshot.profile_inflight_admissions_total as f64,
        );
        self.render_broker_counter(
            "prodex_runtime_broker_profile_inflight_releases_total",
            "Cumulative profile in-flight guard releases.",
            self.snapshot.profile_inflight_releases_total as f64,
        );
        self.render_broker_counter(
            "prodex_runtime_broker_profile_inflight_release_underflows_total",
            "Cumulative profile in-flight guard release underflows.",
            self.snapshot.profile_inflight_release_underflows_total as f64,
        );
    }

    fn push_help(&mut self, metric_name: &str, help: &str) {
        if self.options.include_help {
            push_help(&mut self.out, metric_name, help);
        }
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

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
