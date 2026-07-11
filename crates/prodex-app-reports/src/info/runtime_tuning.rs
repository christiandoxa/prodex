use super::*;

pub fn format_runtime_tuning_workers(snapshot: &RuntimeTuningSnapshot) -> String {
    terminal_ui::format_runtime_tuning_workers_display(terminal_ui::RuntimeTuningWorkersDisplay {
        worker_count: snapshot.worker_count,
        long_lived_worker_count: snapshot.long_lived_worker_count,
        async_worker_count: snapshot.async_worker_count,
        probe_refresh_worker_count: snapshot.probe_refresh_worker_count,
        active_request_limit: snapshot.active_request_limit,
        long_lived_queue_capacity: snapshot.long_lived_queue_capacity,
        lane_responses: snapshot.lane_limits.responses,
        lane_compact: snapshot.lane_limits.compact,
        lane_websocket: snapshot.lane_limits.websocket,
        lane_standard: snapshot.lane_limits.standard,
        websocket_connect_worker_count: snapshot.websocket_connect_worker_count,
        websocket_connect_queue_capacity: snapshot.websocket_connect_queue_capacity,
        websocket_connect_overflow_capacity: snapshot.websocket_connect_overflow_capacity,
        websocket_dns_worker_count: snapshot.websocket_dns_worker_count,
        websocket_dns_queue_capacity: snapshot.websocket_dns_queue_capacity,
        websocket_dns_overflow_capacity: snapshot.websocket_dns_overflow_capacity,
    })
}

pub fn format_runtime_tuning_budgets(snapshot: &RuntimeTuningSnapshot) -> String {
    terminal_ui::format_runtime_tuning_budgets_display(terminal_ui::RuntimeTuningBudgetsDisplay {
        precommit_attempt_limit: snapshot.precommit_attempt_limit,
        precommit_budget_ms: snapshot.precommit_budget_ms,
        pressure_precommit_attempt_limit: snapshot.pressure_precommit_attempt_limit,
        pressure_precommit_budget_ms: snapshot.pressure_precommit_budget_ms,
        continuation_precommit_attempt_limit: snapshot.continuation_precommit_attempt_limit,
        continuation_precommit_budget_ms: snapshot.continuation_precommit_budget_ms,
        admission_wait_budget_ms: snapshot.admission_wait_budget_ms,
        pressure_admission_wait_budget_ms: snapshot.pressure_admission_wait_budget_ms,
        long_lived_queue_wait_budget_ms: snapshot.long_lived_queue_wait_budget_ms,
        pressure_long_lived_queue_wait_budget_ms: snapshot.pressure_long_lived_queue_wait_budget_ms,
    })
}

pub fn format_runtime_tuning_transport(snapshot: &RuntimeTuningSnapshot) -> String {
    terminal_ui::format_runtime_tuning_transport_display(
        terminal_ui::RuntimeTuningTransportDisplay {
            http_connect_timeout_ms: snapshot.http_connect_timeout_ms,
            stream_idle_timeout_ms: snapshot.stream_idle_timeout_ms,
            sse_lookahead_timeout_ms: snapshot.sse_lookahead_timeout_ms,
            websocket_connect_timeout_ms: snapshot.websocket_connect_timeout_ms,
            websocket_precommit_progress_timeout_ms: snapshot
                .websocket_precommit_progress_timeout_ms,
            websocket_happy_eyeballs_delay_ms: snapshot.websocket_happy_eyeballs_delay_ms,
            websocket_previous_response_reuse_stale_ms: snapshot
                .websocket_previous_response_reuse_stale_ms,
            profile_inflight_soft_limit: snapshot.profile_inflight_soft_limit,
            profile_inflight_hard_limit: snapshot.profile_inflight_hard_limit,
        },
    )
}
