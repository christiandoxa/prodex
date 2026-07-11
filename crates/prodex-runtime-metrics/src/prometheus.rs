use prodex_runtime_broker as broker;

use crate::render_helpers::*;
use crate::snapshot::runtime_broker_prometheus_snapshot;
use crate::types::*;

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
        self.render_broker_counter(
            "prodex_runtime_broker_admission_wait_total_seconds",
            "Cumulative time spent waiting for runtime proxy admission.",
            self.snapshot.admission_wait.wait_total_ns as f64 / 1_000_000_000.0,
        );
        self.render_broker_counter(
            "prodex_runtime_broker_admission_waits_total",
            "Cumulative runtime proxy admission waits.",
            self.snapshot.admission_wait.wait_count as f64,
        );
        self.render_broker_gauge(
            "prodex_runtime_broker_admission_wait_max_seconds",
            "Maximum observed runtime proxy admission wait.",
            self.snapshot.admission_wait.wait_max_ns as f64 / 1_000_000_000.0,
        );
        self.render_broker_counter(
            "prodex_runtime_broker_long_lived_queue_wait_total_seconds",
            "Cumulative time spent waiting for long-lived request queue capacity.",
            self.snapshot.long_lived_queue_wait.wait_total_ns as f64 / 1_000_000_000.0,
        );
        self.render_broker_counter(
            "prodex_runtime_broker_long_lived_queue_waits_total",
            "Cumulative long-lived request queue waits.",
            self.snapshot.long_lived_queue_wait.wait_count as f64,
        );
        self.render_broker_gauge(
            "prodex_runtime_broker_long_lived_queue_wait_max_seconds",
            "Maximum observed long-lived request queue wait.",
            self.snapshot.long_lived_queue_wait.wait_max_ns as f64 / 1_000_000_000.0,
        );
        self.render_allocation_metrics();
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

    fn render_allocation_metrics(&mut self) {
        let Some(allocation) = self.snapshot.allocation else {
            return;
        };
        self.render_broker_counter(
            "prodex_runtime_broker_allocation_alloc_calls_total",
            "Cumulative successful allocation calls in an instrumented broker process.",
            allocation.alloc_calls as f64,
        );
        self.render_broker_counter(
            "prodex_runtime_broker_allocation_realloc_calls_total",
            "Cumulative successful reallocation calls in an instrumented broker process.",
            allocation.realloc_calls as f64,
        );
        self.render_broker_counter(
            "prodex_runtime_broker_allocation_dealloc_calls_total",
            "Cumulative deallocation calls in an instrumented broker process.",
            allocation.dealloc_calls as f64,
        );
        self.render_broker_counter(
            "prodex_runtime_broker_allocation_allocated_bytes_total",
            "Cumulative bytes requested by successful allocation calls in an instrumented broker process.",
            allocation.allocated_bytes as f64,
        );
        self.render_broker_counter(
            "prodex_runtime_broker_allocation_reallocated_bytes_total",
            "Cumulative bytes requested by successful reallocation calls in an instrumented broker process.",
            allocation.reallocated_bytes as f64,
        );
        self.render_broker_counter(
            "prodex_runtime_broker_allocation_deallocated_bytes_total",
            "Cumulative bytes released by deallocation calls in an instrumented broker process.",
            allocation.deallocated_bytes as f64,
        );
        self.render_broker_gauge(
            "prodex_runtime_broker_allocation_live_bytes",
            "Current live bytes observed by the instrumented broker allocator.",
            allocation.live_bytes as f64,
        );
        self.render_broker_gauge(
            "prodex_runtime_broker_allocation_peak_live_bytes",
            "Peak live bytes observed by the instrumented broker allocator.",
            allocation.peak_live_bytes as f64,
        );
    }

    fn push_help(&mut self, metric_name: &str, help: &str) {
        if self.options.include_help {
            push_help(&mut self.out, metric_name, help);
        }
    }
}
