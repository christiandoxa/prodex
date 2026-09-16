#[cfg(any(not(feature = "mojo"), test))]
mod rust_compat {
    use super::super::*;

    pub(crate) fn summary_from_usage_snapshot_at(
        snapshot: RuntimeProxyUsageSnapshot,
        route_kind: RuntimeRouteKind,
        now: i64,
    ) -> RuntimeProxyQuotaSummary {
        let mut five_hour = window_summary_from_usage_snapshot_at(
            snapshot.five_hour_status,
            snapshot.five_hour_remaining_percent,
            snapshot.five_hour_reset_at,
            now,
        );
        let mut weekly = window_summary_from_usage_snapshot_at(
            snapshot.weekly_status,
            snapshot.weekly_remaining_percent,
            snapshot.weekly_reset_at,
            now,
        );
        let neutral = RuntimeProxyQuotaWindowSummary {
            status: RuntimeSelectionQuotaWindowStatus::Ready,
            remaining_percent: 100,
            reset_at: i64::MAX,
        };
        match (five_hour.status, weekly.status) {
            (RuntimeSelectionQuotaWindowStatus::Unknown, status)
                if status != RuntimeSelectionQuotaWindowStatus::Unknown =>
            {
                five_hour = neutral;
            }
            (status, RuntimeSelectionQuotaWindowStatus::Unknown)
                if status != RuntimeSelectionQuotaWindowStatus::Unknown =>
            {
                weekly = neutral;
            }
            _ => {}
        }
        let route_band = [
            five_hour.status,
            weekly.status,
            match route_kind {
                RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => weekly.status,
                RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => five_hour.status,
            },
        ]
        .into_iter()
        .fold(
            RuntimeSelectionQuotaPressureBand::Healthy,
            |band, status| {
                band.max(match status {
                    RuntimeSelectionQuotaWindowStatus::Ready => {
                        RuntimeSelectionQuotaPressureBand::Healthy
                    }
                    RuntimeSelectionQuotaWindowStatus::Thin => {
                        RuntimeSelectionQuotaPressureBand::Thin
                    }
                    RuntimeSelectionQuotaWindowStatus::Critical => {
                        RuntimeSelectionQuotaPressureBand::Critical
                    }
                    RuntimeSelectionQuotaWindowStatus::Exhausted => {
                        RuntimeSelectionQuotaPressureBand::Exhausted
                    }
                    RuntimeSelectionQuotaWindowStatus::Unknown => {
                        RuntimeSelectionQuotaPressureBand::Unknown
                    }
                })
            },
        );
        RuntimeProxyQuotaSummary {
            five_hour,
            weekly,
            route_band,
        }
    }

    pub(crate) fn window_summary_from_usage_snapshot_at(
        status: RuntimeSelectionQuotaWindowStatus,
        remaining_percent: i64,
        reset_at: i64,
        now: i64,
    ) -> RuntimeProxyQuotaWindowSummary {
        if reset_at != i64::MAX && reset_at <= now {
            return RuntimeProxyQuotaWindowSummary {
                status: RuntimeSelectionQuotaWindowStatus::Ready,
                remaining_percent: 100,
                reset_at,
            };
        }
        RuntimeProxyQuotaWindowSummary {
            status,
            remaining_percent,
            reset_at,
        }
    }

    pub(crate) fn usage_snapshot_hold_active(
        snapshot: RuntimeProxyUsageSnapshot,
        now: i64,
    ) -> bool {
        [
            (snapshot.five_hour_status, snapshot.five_hour_reset_at),
            (snapshot.weekly_status, snapshot.weekly_reset_at),
        ]
        .into_iter()
        .any(|(status, reset_at)| {
            matches!(status, RuntimeSelectionQuotaWindowStatus::Exhausted)
                && reset_at != i64::MAX
                && reset_at > now
        })
    }

    pub(crate) fn usage_snapshot_hold_expired(
        snapshot: RuntimeProxyUsageSnapshot,
        now: i64,
    ) -> bool {
        [
            (snapshot.five_hour_status, snapshot.five_hour_reset_at),
            (snapshot.weekly_status, snapshot.weekly_reset_at),
        ]
        .into_iter()
        .any(|(status, reset_at)| {
            matches!(status, RuntimeSelectionQuotaWindowStatus::Exhausted)
                && reset_at != i64::MAX
                && reset_at <= now
        })
    }

    pub(crate) fn usage_snapshot_is_usable(
        snapshot: RuntimeProxyUsageSnapshot,
        now: i64,
        stale_grace_seconds: i64,
    ) -> bool {
        if usage_snapshot_hold_active(snapshot, now) {
            return true;
        }
        if usage_snapshot_hold_expired(snapshot, now) {
            return false;
        }
        now.saturating_sub(snapshot.checked_at) <= stale_grace_seconds
    }

    pub(crate) fn summary_requires_precommit_live_probe(
        summary: RuntimeProxyQuotaSummary,
        source: Option<RuntimeSelectionQuotaSource>,
        route_kind: RuntimeRouteKind,
    ) -> bool {
        matches!(
            route_kind,
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
        ) && !matches!(source, Some(RuntimeSelectionQuotaSource::LiveProbe))
            && matches!(
                (summary.five_hour.status, summary.weekly.status),
                (
                    RuntimeSelectionQuotaWindowStatus::Critical
                        | RuntimeSelectionQuotaWindowStatus::Unknown,
                    _
                ) | (
                    _,
                    RuntimeSelectionQuotaWindowStatus::Critical
                        | RuntimeSelectionQuotaWindowStatus::Unknown
                )
            )
    }

    pub(crate) fn summary_requires_live_source_after_probe(
        summary: RuntimeProxyQuotaSummary,
        source: Option<RuntimeSelectionQuotaSource>,
        route_kind: RuntimeRouteKind,
    ) -> bool {
        matches!(
            route_kind,
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
        ) && !matches!(source, Some(RuntimeSelectionQuotaSource::LiveProbe))
            && matches!(
                (summary.five_hour.status, summary.weekly.status),
                (RuntimeSelectionQuotaWindowStatus::Unknown, _)
                    | (_, RuntimeSelectionQuotaWindowStatus::Unknown)
            )
    }

    pub(crate) fn precommit_quota_block_reason(
        summary: RuntimeProxyQuotaSummary,
        route_kind: RuntimeRouteKind,
        responses_critical_floor_percent: i64,
    ) -> Option<RuntimePrecommitQuotaBlockReason> {
        let floor_percent = runtime_quota_precommit_floor_percent_for_route(
            route_kind,
            responses_critical_floor_percent,
        );
        if matches!(
            summary.five_hour.status,
            RuntimeSelectionQuotaWindowStatus::Exhausted
        ) {
            return Some(RuntimePrecommitQuotaBlockReason::ExhaustedBeforeSend);
        }
        if matches!(
            route_kind,
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
        ) && runtime_proxy_quota_window_precommit_guard(summary.five_hour, floor_percent)
        {
            return Some(RuntimePrecommitQuotaBlockReason::CriticalFloorBeforeSend);
        }
        None
    }

    pub(crate) fn precommit_quota_gate_initial_decision(
        input: RuntimeProxyPrecommitQuotaGateInitialInput,
    ) -> RuntimeProxyPrecommitQuotaGateInitialDecision {
        if input.has_continuation_context
            && matches!(
                input.source,
                Some(RuntimeSelectionQuotaSource::PersistedSnapshot)
            )
            && let Some(reason) = precommit_quota_block_reason(
                input.summary,
                input.route_kind,
                input.responses_critical_floor_percent,
            )
        {
            return RuntimeProxyPrecommitQuotaGateInitialDecision::Block { reason };
        }
        if summary_requires_precommit_live_probe(input.summary, input.source, input.route_kind) {
            return RuntimeProxyPrecommitQuotaGateInitialDecision::RefreshRequired;
        }
        RuntimeProxyPrecommitQuotaGateInitialDecision::Continue
    }

    pub(crate) fn precommit_quota_gate_final_decision(
        input: RuntimeProxyPrecommitQuotaGateFinalInput,
    ) -> RuntimeProxyPrecommitQuotaGateFinalDecision {
        if matches!(
            input.route_kind,
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
        ) && input.has_alternative_quota_profile
            && matches!(
                input.summary.weekly.status,
                RuntimeSelectionQuotaWindowStatus::Exhausted
            )
        {
            return RuntimeProxyPrecommitQuotaGateFinalDecision::Block {
                reason: RuntimePrecommitQuotaBlockReason::ExhaustedBeforeSend,
            };
        }
        if summary_requires_live_source_after_probe(input.summary, input.source, input.route_kind)
            && input.has_alternative_quota_profile
        {
            return RuntimeProxyPrecommitQuotaGateFinalDecision::Block {
                reason: RuntimePrecommitQuotaBlockReason::WindowsUnavailableAfterReprobe,
            };
        }
        match precommit_quota_block_reason(
            input.summary,
            input.route_kind,
            input.responses_critical_floor_percent,
        ) {
            Some(reason) => RuntimeProxyPrecommitQuotaGateFinalDecision::Block { reason },
            None => RuntimeProxyPrecommitQuotaGateFinalDecision::Proceed,
        }
    }
}

#[cfg(any(not(feature = "mojo"), test))]
pub(crate) use rust_compat::*;
