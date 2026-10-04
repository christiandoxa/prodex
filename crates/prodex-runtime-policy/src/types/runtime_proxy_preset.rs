use super::{RuntimePolicyProxyPreset, RuntimePolicyProxyPresetSelection};
use prodex_mojo_core::runtime::{RuntimeTuningProxyPresetValues, runtime_tuning_proxy_preset_plan};
use serde::Deserialize;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyProxySettings {
    #[serde(default)]
    pub preset: RuntimePolicyProxyPresetSelection,
    pub worker_count: Option<usize>,
    pub long_lived_worker_count: Option<usize>,
    pub probe_refresh_worker_count: Option<usize>,
    pub async_worker_count: Option<usize>,
    pub long_lived_queue_capacity: Option<usize>,
    pub active_request_limit: Option<usize>,
    pub profile_inflight_soft_limit: Option<usize>,
    pub profile_inflight_hard_limit: Option<usize>,
    pub responses_active_limit: Option<usize>,
    pub compact_active_limit: Option<usize>,
    pub websocket_active_limit: Option<usize>,
    pub standard_active_limit: Option<usize>,
    pub http_connect_timeout_ms: Option<u64>,
    pub stream_idle_timeout_ms: Option<u64>,
    pub compact_request_timeout_ms: Option<u64>,
    pub sse_lookahead_timeout_ms: Option<u64>,
    pub prefetch_backpressure_retry_ms: Option<u64>,
    pub prefetch_backpressure_timeout_ms: Option<u64>,
    pub prefetch_max_buffered_bytes: Option<usize>,
    pub websocket_connect_timeout_ms: Option<u64>,
    pub websocket_happy_eyeballs_delay_ms: Option<u64>,
    pub websocket_precommit_progress_timeout_ms: Option<u64>,
    pub websocket_connect_worker_count: Option<usize>,
    pub websocket_connect_queue_capacity: Option<usize>,
    pub websocket_connect_overflow_capacity: Option<usize>,
    pub websocket_dns_worker_count: Option<usize>,
    pub websocket_dns_queue_capacity: Option<usize>,
    pub websocket_dns_overflow_capacity: Option<usize>,
    pub broker_ready_timeout_ms: Option<u64>,
    pub broker_health_connect_timeout_ms: Option<u64>,
    pub broker_health_read_timeout_ms: Option<u64>,
    pub websocket_previous_response_reuse_stale_ms: Option<u64>,
    pub admission_wait_budget_ms: Option<u64>,
    pub pressure_admission_wait_budget_ms: Option<u64>,
    pub long_lived_queue_wait_budget_ms: Option<u64>,
    pub pressure_long_lived_queue_wait_budget_ms: Option<u64>,
    pub sync_probe_pressure_pause_ms: Option<u64>,
    pub responses_critical_floor_percent: Option<i64>,
    pub startup_sync_probe_warm_limit: Option<usize>,
}

impl RuntimePolicyProxySettings {
    pub fn preset(&self) -> Option<RuntimePolicyProxyPreset> {
        self.preset.get()
    }

    pub(super) fn resolve_effective_preset(
        self,
        env_preset: Option<RuntimePolicyProxyPreset>,
    ) -> RuntimePolicyProxySettings {
        let plan = runtime_tuning_proxy_preset_plan(
            self.preset().map(RuntimePolicyProxyPreset::to_mojo),
            env_preset.map(RuntimePolicyProxyPreset::to_mojo),
            RuntimeTuningProxyPresetValues {
                worker_count: self.worker_count,
                long_lived_worker_count: self.long_lived_worker_count,
                probe_refresh_worker_count: self.probe_refresh_worker_count,
                async_worker_count: self.async_worker_count,
                long_lived_queue_capacity: self.long_lived_queue_capacity,
                active_request_limit: self.active_request_limit,
                profile_inflight_soft_limit: self.profile_inflight_soft_limit,
                profile_inflight_hard_limit: self.profile_inflight_hard_limit,
                responses_active_limit: self.responses_active_limit,
                compact_active_limit: self.compact_active_limit,
                websocket_active_limit: self.websocket_active_limit,
                standard_active_limit: self.standard_active_limit,
                http_connect_timeout_ms: self.http_connect_timeout_ms,
                stream_idle_timeout_ms: self.stream_idle_timeout_ms,
                compact_request_timeout_ms: self.compact_request_timeout_ms,
                sse_lookahead_timeout_ms: self.sse_lookahead_timeout_ms,
                prefetch_backpressure_retry_ms: self.prefetch_backpressure_retry_ms,
                prefetch_backpressure_timeout_ms: self.prefetch_backpressure_timeout_ms,
                prefetch_max_buffered_bytes: self.prefetch_max_buffered_bytes,
                websocket_connect_timeout_ms: self.websocket_connect_timeout_ms,
                websocket_happy_eyeballs_delay_ms: self.websocket_happy_eyeballs_delay_ms,
                websocket_precommit_progress_timeout_ms: self
                    .websocket_precommit_progress_timeout_ms,
                websocket_connect_worker_count: self.websocket_connect_worker_count,
                websocket_connect_queue_capacity: self.websocket_connect_queue_capacity,
                websocket_connect_overflow_capacity: self.websocket_connect_overflow_capacity,
                websocket_dns_worker_count: self.websocket_dns_worker_count,
                websocket_dns_queue_capacity: self.websocket_dns_queue_capacity,
                websocket_dns_overflow_capacity: self.websocket_dns_overflow_capacity,
                broker_ready_timeout_ms: self.broker_ready_timeout_ms,
                broker_health_connect_timeout_ms: self.broker_health_connect_timeout_ms,
                broker_health_read_timeout_ms: self.broker_health_read_timeout_ms,
                websocket_previous_response_reuse_stale_ms: self
                    .websocket_previous_response_reuse_stale_ms,
                admission_wait_budget_ms: self.admission_wait_budget_ms,
                pressure_admission_wait_budget_ms: self.pressure_admission_wait_budget_ms,
                long_lived_queue_wait_budget_ms: self.long_lived_queue_wait_budget_ms,
                pressure_long_lived_queue_wait_budget_ms: self
                    .pressure_long_lived_queue_wait_budget_ms,
                sync_probe_pressure_pause_ms: self.sync_probe_pressure_pause_ms,
                responses_critical_floor_percent: self.responses_critical_floor_percent,
                startup_sync_probe_warm_limit: self.startup_sync_probe_warm_limit,
            },
        )
        .expect("runtime proxy preset policy must be resolved by Mojo");
        RuntimePolicyProxySettings {
            preset: plan
                .effective_preset
                .map(|preset| {
                    RuntimePolicyProxyPresetSelection::selected(
                        RuntimePolicyProxyPreset::from_mojo(preset),
                    )
                })
                .unwrap_or_default(),
            worker_count: plan.values.worker_count,
            long_lived_worker_count: plan.values.long_lived_worker_count,
            probe_refresh_worker_count: plan.values.probe_refresh_worker_count,
            async_worker_count: plan.values.async_worker_count,
            long_lived_queue_capacity: plan.values.long_lived_queue_capacity,
            active_request_limit: plan.values.active_request_limit,
            profile_inflight_soft_limit: plan.values.profile_inflight_soft_limit,
            profile_inflight_hard_limit: plan.values.profile_inflight_hard_limit,
            responses_active_limit: plan.values.responses_active_limit,
            compact_active_limit: plan.values.compact_active_limit,
            websocket_active_limit: plan.values.websocket_active_limit,
            standard_active_limit: plan.values.standard_active_limit,
            http_connect_timeout_ms: plan.values.http_connect_timeout_ms,
            stream_idle_timeout_ms: plan.values.stream_idle_timeout_ms,
            compact_request_timeout_ms: plan.values.compact_request_timeout_ms,
            sse_lookahead_timeout_ms: plan.values.sse_lookahead_timeout_ms,
            prefetch_backpressure_retry_ms: plan.values.prefetch_backpressure_retry_ms,
            prefetch_backpressure_timeout_ms: plan.values.prefetch_backpressure_timeout_ms,
            prefetch_max_buffered_bytes: plan.values.prefetch_max_buffered_bytes,
            websocket_connect_timeout_ms: plan.values.websocket_connect_timeout_ms,
            websocket_happy_eyeballs_delay_ms: plan.values.websocket_happy_eyeballs_delay_ms,
            websocket_precommit_progress_timeout_ms: plan
                .values
                .websocket_precommit_progress_timeout_ms,
            websocket_connect_worker_count: plan.values.websocket_connect_worker_count,
            websocket_connect_queue_capacity: plan.values.websocket_connect_queue_capacity,
            websocket_connect_overflow_capacity: plan.values.websocket_connect_overflow_capacity,
            websocket_dns_worker_count: plan.values.websocket_dns_worker_count,
            websocket_dns_queue_capacity: plan.values.websocket_dns_queue_capacity,
            websocket_dns_overflow_capacity: plan.values.websocket_dns_overflow_capacity,
            broker_ready_timeout_ms: plan.values.broker_ready_timeout_ms,
            broker_health_connect_timeout_ms: plan.values.broker_health_connect_timeout_ms,
            broker_health_read_timeout_ms: plan.values.broker_health_read_timeout_ms,
            websocket_previous_response_reuse_stale_ms: plan
                .values
                .websocket_previous_response_reuse_stale_ms,
            admission_wait_budget_ms: plan.values.admission_wait_budget_ms,
            pressure_admission_wait_budget_ms: plan.values.pressure_admission_wait_budget_ms,
            long_lived_queue_wait_budget_ms: plan.values.long_lived_queue_wait_budget_ms,
            pressure_long_lived_queue_wait_budget_ms: plan
                .values
                .pressure_long_lived_queue_wait_budget_ms,
            sync_probe_pressure_pause_ms: plan.values.sync_probe_pressure_pause_ms,
            responses_critical_floor_percent: plan.values.responses_critical_floor_percent,
            startup_sync_probe_warm_limit: plan.values.startup_sync_probe_warm_limit,
        }
    }
}
