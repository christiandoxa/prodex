use super::*;

fn window(remaining_percent: i64) -> RuntimeProxyQuotaWindowObservation {
    RuntimeProxyQuotaWindowObservation {
        remaining_percent,
        reset_at: 3_600,
        pressure_score: 3_600_000 / remaining_percent.max(1),
    }
}

#[test]
fn route_pressure_uses_route_specific_reserve_thresholds() {
    assert_eq!(
        runtime_proxy_quota_pressure_band_for_route(
            Some(window(9)),
            Some(window(19)),
            RuntimeRouteKind::Responses,
        ),
        RuntimeSelectionQuotaPressureBand::Thin
    );
    assert_eq!(
        runtime_proxy_quota_pressure_band_for_route(
            Some(window(9)),
            Some(window(19)),
            RuntimeRouteKind::Compact,
        ),
        RuntimeSelectionQuotaPressureBand::Healthy
    );
}

#[test]
fn snapshot_hold_preserves_active_exhaustion_until_reset() {
    let snapshot = RuntimeProxyUsageSnapshot {
        checked_at: 100,
        five_hour_status: RuntimeSelectionQuotaWindowStatus::Exhausted,
        five_hour_remaining_percent: 0,
        five_hour_reset_at: 200,
        weekly_status: RuntimeSelectionQuotaWindowStatus::Ready,
        weekly_remaining_percent: 90,
        weekly_reset_at: i64::MAX,
    };

    assert!(runtime_proxy_usage_snapshot_hold_active(snapshot, 150));
    assert!(!runtime_proxy_usage_snapshot_hold_active(snapshot, 250));
    assert!(runtime_proxy_usage_snapshot_hold_expired(snapshot, 250));
}

#[test]
fn precommit_block_reason_keeps_response_floor_only_for_main_lanes() {
    let summary = RuntimeProxyQuotaSummary {
        five_hour: RuntimeProxyQuotaWindowSummary {
            status: RuntimeSelectionQuotaWindowStatus::Critical,
            remaining_percent: 2,
            reset_at: 200,
        },
        weekly: RuntimeProxyQuotaWindowSummary {
            status: RuntimeSelectionQuotaWindowStatus::Ready,
            remaining_percent: 80,
            reset_at: i64::MAX,
        },
        route_band: RuntimeSelectionQuotaPressureBand::Critical,
    };

    assert_eq!(
        runtime_proxy_precommit_quota_block_reason(summary, RuntimeRouteKind::Responses, 2,),
        Some(RuntimePrecommitQuotaBlockReason::CriticalFloorBeforeSend)
    );
    assert_eq!(
        runtime_proxy_precommit_quota_block_reason(summary, RuntimeRouteKind::Compact, 2),
        None
    );
}

#[test]
fn precommit_block_reason_allows_weekly_critical_when_five_hour_is_ready() {
    let summary = RuntimeProxyQuotaSummary {
        five_hour: RuntimeProxyQuotaWindowSummary {
            status: RuntimeSelectionQuotaWindowStatus::Ready,
            remaining_percent: 85,
            reset_at: 200,
        },
        weekly: RuntimeProxyQuotaWindowSummary {
            status: RuntimeSelectionQuotaWindowStatus::Critical,
            remaining_percent: 1,
            reset_at: 300,
        },
        route_band: RuntimeSelectionQuotaPressureBand::Critical,
    };

    assert_eq!(
        runtime_proxy_precommit_quota_block_reason(summary, RuntimeRouteKind::Responses, 2,),
        None
    );
}

#[test]
fn precommit_block_reason_allows_weekly_exhausted_when_five_hour_is_ready() {
    let summary = RuntimeProxyQuotaSummary {
        five_hour: RuntimeProxyQuotaWindowSummary {
            status: RuntimeSelectionQuotaWindowStatus::Ready,
            remaining_percent: 80,
            reset_at: i64::MAX,
        },
        weekly: RuntimeProxyQuotaWindowSummary {
            status: RuntimeSelectionQuotaWindowStatus::Exhausted,
            remaining_percent: 0,
            reset_at: 300,
        },
        route_band: RuntimeSelectionQuotaPressureBand::Exhausted,
    };

    assert_eq!(
        runtime_proxy_precommit_quota_block_reason(summary, RuntimeRouteKind::Responses, 2,),
        None
    );
}

#[test]
fn precommit_quota_gate_blocks_five_hour_exhausted_continuation_snapshot_before_reprobe() {
    let summary = RuntimeProxyQuotaSummary {
        five_hour: RuntimeProxyQuotaWindowSummary {
            status: RuntimeSelectionQuotaWindowStatus::Exhausted,
            remaining_percent: 0,
            reset_at: 300,
        },
        weekly: RuntimeProxyQuotaWindowSummary {
            status: RuntimeSelectionQuotaWindowStatus::Ready,
            remaining_percent: 80,
            reset_at: i64::MAX,
        },
        route_band: RuntimeSelectionQuotaPressureBand::Exhausted,
    };

    assert_eq!(
        runtime_proxy_precommit_quota_gate_initial_decision(
            RuntimeProxyPrecommitQuotaGateInitialInput {
                summary,
                source: Some(RuntimeSelectionQuotaSource::PersistedSnapshot),
                route_kind: RuntimeRouteKind::Websocket,
                has_continuation_context: true,
                responses_critical_floor_percent: 2,
            },
        ),
        RuntimeProxyPrecommitQuotaGateInitialDecision::Block {
            reason: RuntimePrecommitQuotaBlockReason::ExhaustedBeforeSend,
        }
    );
}

#[test]
fn precommit_quota_gate_requests_refresh_for_unknown_main_lane_quota() {
    let summary = RuntimeProxyQuotaSummary {
        five_hour: RuntimeProxyQuotaWindowSummary {
            status: RuntimeSelectionQuotaWindowStatus::Unknown,
            remaining_percent: 0,
            reset_at: i64::MAX,
        },
        weekly: RuntimeProxyQuotaWindowSummary {
            status: RuntimeSelectionQuotaWindowStatus::Ready,
            remaining_percent: 80,
            reset_at: i64::MAX,
        },
        route_band: RuntimeSelectionQuotaPressureBand::Unknown,
    };

    assert_eq!(
        runtime_proxy_precommit_quota_gate_initial_decision(
            RuntimeProxyPrecommitQuotaGateInitialInput {
                summary,
                source: None,
                route_kind: RuntimeRouteKind::Responses,
                has_continuation_context: false,
                responses_critical_floor_percent: 2,
            },
        ),
        RuntimeProxyPrecommitQuotaGateInitialDecision::RefreshRequired
    );
}

#[test]
fn precommit_quota_gate_final_blocks_unknown_quota_only_when_pool_fallback_exists() {
    let summary = RuntimeProxyQuotaSummary {
        five_hour: RuntimeProxyQuotaWindowSummary {
            status: RuntimeSelectionQuotaWindowStatus::Unknown,
            remaining_percent: 0,
            reset_at: i64::MAX,
        },
        weekly: RuntimeProxyQuotaWindowSummary {
            status: RuntimeSelectionQuotaWindowStatus::Ready,
            remaining_percent: 80,
            reset_at: i64::MAX,
        },
        route_band: RuntimeSelectionQuotaPressureBand::Unknown,
    };

    assert_eq!(
        runtime_proxy_precommit_quota_gate_final_decision(
            RuntimeProxyPrecommitQuotaGateFinalInput {
                summary,
                source: None,
                route_kind: RuntimeRouteKind::Responses,
                has_alternative_quota_profile: true,
                responses_critical_floor_percent: 2,
            },
        ),
        RuntimeProxyPrecommitQuotaGateFinalDecision::Block {
            reason: RuntimePrecommitQuotaBlockReason::WindowsUnavailableAfterReprobe,
        }
    );
    assert_eq!(
        runtime_proxy_precommit_quota_gate_final_decision(
            RuntimeProxyPrecommitQuotaGateFinalInput {
                summary,
                source: None,
                route_kind: RuntimeRouteKind::Responses,
                has_alternative_quota_profile: false,
                responses_critical_floor_percent: 2,
            },
        ),
        RuntimeProxyPrecommitQuotaGateFinalDecision::Proceed
    );
}
