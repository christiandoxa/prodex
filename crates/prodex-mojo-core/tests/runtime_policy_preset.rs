#![cfg(all(feature = "mojo-runtime", prodex_mojo_required))]

use prodex_mojo_core::runtime::{
    RuntimeProxyPresetClass, RuntimeTuningProxyPresetPlan, RuntimeTuningProxyPresetValues,
    runtime_tuning_proxy_preset_plan,
};

#[test]
fn required_mojo_preserves_each_preset_capacity_profile() {
    for (preset, worker_count, active_limit, websocket_limit) in [
        (RuntimeProxyPresetClass::Low, Some(4), Some(48), Some(8)),
        (RuntimeProxyPresetClass::Default, None, None, None),
        (
            RuntimeProxyPresetClass::ManyTerminals,
            Some(12),
            Some(160),
            Some(32),
        ),
        (
            RuntimeProxyPresetClass::Aggressive,
            Some(24),
            Some(384),
            Some(96),
        ),
    ] {
        let plan = runtime_tuning_proxy_preset_plan(
            Some(preset),
            None,
            RuntimeTuningProxyPresetValues::default(),
        )
        .expect("required Mojo preset plan should accept valid settings");
        assert_eq!(plan.effective_preset, Some(preset));
        assert_eq!(plan.values.worker_count, worker_count);
        assert_eq!(plan.values.active_request_limit, active_limit);
        assert_eq!(plan.values.websocket_active_limit, websocket_limit);
    }
}

#[test]
fn required_mojo_round_trips_every_optional_field_override() {
    let overrides = RuntimeTuningProxyPresetValues {
        worker_count: Some(101),
        long_lived_worker_count: Some(102),
        probe_refresh_worker_count: Some(103),
        async_worker_count: Some(104),
        long_lived_queue_capacity: Some(105),
        active_request_limit: Some(106),
        profile_inflight_soft_limit: Some(107),
        profile_inflight_hard_limit: Some(108),
        responses_active_limit: Some(109),
        compact_active_limit: Some(110),
        websocket_active_limit: Some(111),
        standard_active_limit: Some(112),
        http_connect_timeout_ms: Some(113),
        stream_idle_timeout_ms: Some(114),
        compact_request_timeout_ms: Some(115),
        sse_lookahead_timeout_ms: Some(116),
        prefetch_backpressure_retry_ms: Some(117),
        prefetch_backpressure_timeout_ms: Some(118),
        prefetch_max_buffered_bytes: Some(119),
        websocket_connect_timeout_ms: Some(120),
        websocket_happy_eyeballs_delay_ms: Some(121),
        websocket_precommit_progress_timeout_ms: Some(122),
        websocket_connect_worker_count: Some(123),
        websocket_connect_queue_capacity: Some(124),
        websocket_connect_overflow_capacity: Some(125),
        websocket_dns_worker_count: Some(126),
        websocket_dns_queue_capacity: Some(127),
        websocket_dns_overflow_capacity: Some(128),
        broker_ready_timeout_ms: Some(129),
        broker_health_connect_timeout_ms: Some(130),
        broker_health_read_timeout_ms: Some(131),
        websocket_previous_response_reuse_stale_ms: Some(132),
        admission_wait_budget_ms: Some(133),
        pressure_admission_wait_budget_ms: Some(134),
        long_lived_queue_wait_budget_ms: Some(135),
        pressure_long_lived_queue_wait_budget_ms: Some(136),
        sync_probe_pressure_pause_ms: Some(137),
        responses_critical_floor_percent: Some(-138),
        startup_sync_probe_warm_limit: Some(139),
    };
    let plan = runtime_tuning_proxy_preset_plan(None, None, overrides)
        .expect("required Mojo preset plan should preserve explicit fields");

    assert_eq!(plan.effective_preset, None);
    assert_eq!(plan.values, overrides);
}

#[test]
fn required_mojo_resolves_preset_precedence_and_all_overrides() {
    let plan = runtime_tuning_proxy_preset_plan(
        Some(RuntimeProxyPresetClass::Low),
        Some(RuntimeProxyPresetClass::ManyTerminals),
        RuntimeTuningProxyPresetValues {
            worker_count: Some(7),
            http_connect_timeout_ms: Some(0),
            responses_critical_floor_percent: Some(-5),
            ..RuntimeTuningProxyPresetValues::default()
        },
    )
    .expect("required Mojo preset plan should accept valid settings");

    assert_eq!(
        plan,
        RuntimeTuningProxyPresetPlan {
            effective_preset: Some(RuntimeProxyPresetClass::ManyTerminals),
            values: RuntimeTuningProxyPresetValues {
                worker_count: Some(7),
                long_lived_worker_count: Some(32),
                probe_refresh_worker_count: Some(4),
                async_worker_count: Some(4),
                long_lived_queue_capacity: Some(512),
                active_request_limit: Some(160),
                profile_inflight_soft_limit: Some(4),
                profile_inflight_hard_limit: Some(8),
                responses_active_limit: Some(120),
                compact_active_limit: Some(8),
                websocket_active_limit: Some(32),
                standard_active_limit: Some(8),
                http_connect_timeout_ms: Some(0),
                websocket_connect_worker_count: Some(12),
                websocket_connect_queue_capacity: Some(96),
                websocket_connect_overflow_capacity: Some(384),
                websocket_dns_worker_count: Some(6),
                websocket_dns_queue_capacity: Some(48),
                websocket_dns_overflow_capacity: Some(96),
                responses_critical_floor_percent: Some(-5),
                startup_sync_probe_warm_limit: Some(2),
                ..RuntimeTuningProxyPresetValues::default()
            },
        }
    );
}
