use std::cmp::Reverse;

use crate::{
    RuntimeRouteKind, RuntimeSelectionQuotaPressureBand, RuntimeSelectionQuotaSource,
    RuntimeSelectionQuotaWindowStatus, runtime_quota_precommit_floor_percent_for_route,
    runtime_quota_window_precommit_guard,
};

pub type RuntimeProxyQuotaPressureSortKey = (
    RuntimeSelectionQuotaPressureBand,
    i64,
    i64,
    i64,
    Reverse<i64>,
    Reverse<i64>,
    Reverse<i64>,
    i64,
    i64,
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProxyQuotaWindowObservation {
    pub remaining_percent: i64,
    pub reset_at: i64,
    pub pressure_score: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProxyQuotaWindowSummary {
    pub status: RuntimeSelectionQuotaWindowStatus,
    pub remaining_percent: i64,
    pub reset_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProxyQuotaSummary {
    pub five_hour: RuntimeProxyQuotaWindowSummary,
    pub weekly: RuntimeProxyQuotaWindowSummary,
    pub route_band: RuntimeSelectionQuotaPressureBand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProxyUsageSnapshot {
    pub checked_at: i64,
    pub five_hour_status: RuntimeSelectionQuotaWindowStatus,
    pub five_hour_remaining_percent: i64,
    pub five_hour_reset_at: i64,
    pub weekly_status: RuntimeSelectionQuotaWindowStatus,
    pub weekly_remaining_percent: i64,
    pub weekly_reset_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProxyQuotaScore {
    pub total_pressure: i64,
    pub weekly_pressure: i64,
    pub five_hour_pressure: i64,
    pub reserve_floor: i64,
    pub weekly_remaining: i64,
    pub five_hour_remaining: i64,
    pub weekly_reset_at: i64,
    pub five_hour_reset_at: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimePrecommitQuotaBlockReason {
    ExhaustedBeforeSend,
    CriticalFloorBeforeSend,
    WindowsUnavailableAfterReprobe,
}

impl RuntimePrecommitQuotaBlockReason {
    pub fn as_str(self) -> &'static str {
        match self {
            RuntimePrecommitQuotaBlockReason::ExhaustedBeforeSend => "quota_exhausted_before_send",
            RuntimePrecommitQuotaBlockReason::CriticalFloorBeforeSend => {
                "quota_critical_floor_before_send"
            }
            RuntimePrecommitQuotaBlockReason::WindowsUnavailableAfterReprobe => {
                "quota_windows_unavailable_after_reprobe"
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeProxyPrecommitQuotaGateInitialDecision {
    Continue,
    RefreshRequired,
    Block {
        reason: RuntimePrecommitQuotaBlockReason,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeProxyPrecommitQuotaGateFinalDecision {
    Proceed,
    Block {
        reason: RuntimePrecommitQuotaBlockReason,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeProxyPrecommitQuotaGateInitialInput {
    pub summary: RuntimeProxyQuotaSummary,
    pub source: Option<RuntimeSelectionQuotaSource>,
    pub route_kind: RuntimeRouteKind,
    pub has_continuation_context: bool,
    pub responses_critical_floor_percent: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeProxyPrecommitQuotaGateFinalInput {
    pub summary: RuntimeProxyQuotaSummary,
    pub source: Option<RuntimeSelectionQuotaSource>,
    pub route_kind: RuntimeRouteKind,
    pub has_alternative_quota_profile: bool,
    pub responses_critical_floor_percent: i64,
}

pub fn runtime_proxy_quota_pressure_band_reason(
    band: RuntimeSelectionQuotaPressureBand,
) -> &'static str {
    match band {
        RuntimeSelectionQuotaPressureBand::Healthy => "quota_healthy",
        RuntimeSelectionQuotaPressureBand::Thin => "quota_thin",
        RuntimeSelectionQuotaPressureBand::Critical => "quota_critical",
        RuntimeSelectionQuotaPressureBand::Exhausted => "quota_exhausted",
        RuntimeSelectionQuotaPressureBand::Unknown => "quota_unknown",
    }
}

pub fn runtime_proxy_quota_window_status_reason(
    status: RuntimeSelectionQuotaWindowStatus,
) -> &'static str {
    match status {
        RuntimeSelectionQuotaWindowStatus::Ready => "ready",
        RuntimeSelectionQuotaWindowStatus::Thin => "thin",
        RuntimeSelectionQuotaWindowStatus::Critical => "critical",
        RuntimeSelectionQuotaWindowStatus::Exhausted => "exhausted",
        RuntimeSelectionQuotaWindowStatus::Unknown => "unknown",
    }
}

pub fn runtime_selection_quota_source_label(source: RuntimeSelectionQuotaSource) -> &'static str {
    match source {
        RuntimeSelectionQuotaSource::LiveProbe => "probe_cache",
        RuntimeSelectionQuotaSource::PersistedSnapshot => "persisted_snapshot",
    }
}

pub fn runtime_proxy_quota_window_summary(
    observation: Option<RuntimeProxyQuotaWindowObservation>,
) -> RuntimeProxyQuotaWindowSummary {
    let Some(window) = observation else {
        return RuntimeProxyQuotaWindowSummary {
            status: RuntimeSelectionQuotaWindowStatus::Unknown,
            remaining_percent: 0,
            reset_at: i64::MAX,
        };
    };
    let status = if window.remaining_percent == 0 {
        RuntimeSelectionQuotaWindowStatus::Exhausted
    } else if window.remaining_percent <= 5 {
        RuntimeSelectionQuotaWindowStatus::Critical
    } else if window.remaining_percent <= 15 {
        RuntimeSelectionQuotaWindowStatus::Thin
    } else {
        RuntimeSelectionQuotaWindowStatus::Ready
    };
    RuntimeProxyQuotaWindowSummary {
        status,
        remaining_percent: window.remaining_percent,
        reset_at: window.reset_at,
    }
}

pub fn runtime_proxy_quota_summary_for_route(
    five_hour: Option<RuntimeProxyQuotaWindowObservation>,
    weekly: Option<RuntimeProxyQuotaWindowObservation>,
    route_kind: RuntimeRouteKind,
) -> RuntimeProxyQuotaSummary {
    RuntimeProxyQuotaSummary {
        five_hour: runtime_proxy_quota_window_summary(five_hour),
        weekly: runtime_proxy_quota_window_summary(weekly),
        route_band: runtime_proxy_quota_pressure_band_for_route(five_hour, weekly, route_kind),
    }
}

pub fn runtime_proxy_quota_summary_blocking_reset_at(
    summary: RuntimeProxyQuotaSummary,
    route_kind: RuntimeRouteKind,
    responses_critical_floor_percent: i64,
) -> Option<i64> {
    let floor_percent = runtime_quota_precommit_floor_percent_for_route(
        route_kind,
        responses_critical_floor_percent,
    );
    [summary.five_hour, summary.weekly]
        .into_iter()
        .filter(|window| runtime_proxy_quota_window_precommit_guard(*window, floor_percent))
        .map(|window| window.reset_at)
        .filter(|reset_at| *reset_at != i64::MAX)
        .max()
}

pub fn runtime_proxy_usage_snapshot_from_observations_at(
    five_hour: Option<RuntimeProxyQuotaWindowObservation>,
    weekly: Option<RuntimeProxyQuotaWindowObservation>,
    checked_at: i64,
) -> RuntimeProxyUsageSnapshot {
    let five_hour = runtime_proxy_quota_window_summary(five_hour);
    let weekly = runtime_proxy_quota_window_summary(weekly);
    RuntimeProxyUsageSnapshot {
        checked_at,
        five_hour_status: five_hour.status,
        five_hour_remaining_percent: five_hour.remaining_percent,
        five_hour_reset_at: five_hour.reset_at,
        weekly_status: weekly.status,
        weekly_remaining_percent: weekly.remaining_percent,
        weekly_reset_at: weekly.reset_at,
    }
}

pub fn runtime_proxy_quota_summary_from_usage_snapshot_at(
    snapshot: RuntimeProxyUsageSnapshot,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> RuntimeProxyQuotaSummary {
    let five_hour = runtime_proxy_quota_window_summary_from_usage_snapshot_at(
        snapshot.five_hour_status,
        snapshot.five_hour_remaining_percent,
        snapshot.five_hour_reset_at,
        now,
    );
    let weekly = runtime_proxy_quota_window_summary_from_usage_snapshot_at(
        snapshot.weekly_status,
        snapshot.weekly_remaining_percent,
        snapshot.weekly_reset_at,
        now,
    );
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
        |band, status| band.max(runtime_proxy_quota_pressure_band_from_window_status(status)),
    );
    RuntimeProxyQuotaSummary {
        five_hour,
        weekly,
        route_band,
    }
}

pub fn runtime_proxy_quota_window_summary_from_usage_snapshot_at(
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

pub fn runtime_proxy_usage_snapshot_hold_active(
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

pub fn runtime_proxy_usage_snapshot_hold_expired(
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

pub fn runtime_proxy_usage_snapshot_is_usable(
    snapshot: RuntimeProxyUsageSnapshot,
    now: i64,
    stale_grace_seconds: i64,
) -> bool {
    if runtime_proxy_usage_snapshot_hold_active(snapshot, now) {
        return true;
    }
    if runtime_proxy_usage_snapshot_hold_expired(snapshot, now) {
        return false;
    }
    now.saturating_sub(snapshot.checked_at) <= stale_grace_seconds
}

pub fn runtime_proxy_quota_pressure_sort_key_for_route(
    five_hour: Option<RuntimeProxyQuotaWindowObservation>,
    weekly: Option<RuntimeProxyQuotaWindowObservation>,
    route_kind: RuntimeRouteKind,
) -> RuntimeProxyQuotaPressureSortKey {
    let score = runtime_proxy_quota_score_for_route(five_hour, weekly, route_kind);
    (
        runtime_proxy_quota_pressure_band_for_route(five_hour, weekly, route_kind),
        score.total_pressure,
        score.weekly_pressure,
        score.five_hour_pressure,
        Reverse(score.reserve_floor),
        Reverse(score.weekly_remaining),
        Reverse(score.five_hour_remaining),
        score.weekly_reset_at,
        score.five_hour_reset_at,
    )
}

pub fn runtime_proxy_quota_pressure_sort_key_for_route_from_summary(
    summary: RuntimeProxyQuotaSummary,
) -> RuntimeProxyQuotaPressureSortKey {
    (
        summary.route_band,
        runtime_proxy_quota_pressure_band_rank(summary.route_band),
        runtime_proxy_quota_window_status_rank(summary.weekly.status),
        runtime_proxy_quota_window_status_rank(summary.five_hour.status),
        Reverse(
            summary
                .weekly
                .remaining_percent
                .min(summary.five_hour.remaining_percent),
        ),
        Reverse(summary.weekly.remaining_percent),
        Reverse(summary.five_hour.remaining_percent),
        summary.weekly.reset_at,
        summary.five_hour.reset_at,
    )
}

pub fn runtime_proxy_quota_score_for_route(
    five_hour: Option<RuntimeProxyQuotaWindowObservation>,
    weekly: Option<RuntimeProxyQuotaWindowObservation>,
    route_kind: RuntimeRouteKind,
) -> RuntimeProxyQuotaScore {
    let weekly_pressure = weekly.map_or(i64::MAX, |window| window.pressure_score);
    let five_hour_pressure = five_hour.map_or(i64::MAX, |window| window.pressure_score);
    let weekly_remaining = weekly.map_or(0, |window| window.remaining_percent);
    let five_hour_remaining = five_hour.map_or(0, |window| window.remaining_percent);
    let weekly_weight = match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => 10,
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => 8,
    };
    let reserve_bias =
        match runtime_proxy_quota_pressure_band_for_route(five_hour, weekly, route_kind) {
            RuntimeSelectionQuotaPressureBand::Healthy => 0,
            RuntimeSelectionQuotaPressureBand::Thin => 250_000,
            RuntimeSelectionQuotaPressureBand::Critical => 1_000_000,
            RuntimeSelectionQuotaPressureBand::Exhausted
            | RuntimeSelectionQuotaPressureBand::Unknown => i64::MAX / 4,
        };

    RuntimeProxyQuotaScore {
        total_pressure: reserve_bias
            .saturating_add(weekly_pressure.saturating_mul(weekly_weight))
            .saturating_add(five_hour_pressure),
        weekly_pressure,
        five_hour_pressure,
        reserve_floor: weekly_remaining.min(five_hour_remaining),
        weekly_remaining,
        five_hour_remaining,
        weekly_reset_at: weekly.map_or(i64::MAX, |window| window.reset_at),
        five_hour_reset_at: five_hour.map_or(i64::MAX, |window| window.reset_at),
    }
}

pub fn runtime_proxy_quota_pressure_band_for_route(
    five_hour: Option<RuntimeProxyQuotaWindowObservation>,
    weekly: Option<RuntimeProxyQuotaWindowObservation>,
    route_kind: RuntimeRouteKind,
) -> RuntimeSelectionQuotaPressureBand {
    let Some(weekly) = weekly else {
        return RuntimeSelectionQuotaPressureBand::Unknown;
    };
    let Some(five_hour) = five_hour else {
        return RuntimeSelectionQuotaPressureBand::Unknown;
    };

    let weekly_remaining = weekly.remaining_percent;
    let five_hour_remaining = five_hour.remaining_percent;
    if weekly_remaining == 0 || five_hour_remaining == 0 {
        return RuntimeSelectionQuotaPressureBand::Exhausted;
    }

    let (thin_weekly, thin_five_hour, critical_weekly, critical_five_hour) = match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => (20, 10, 10, 5),
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => (10, 5, 5, 3),
    };

    if weekly_remaining <= critical_weekly || five_hour_remaining <= critical_five_hour {
        RuntimeSelectionQuotaPressureBand::Critical
    } else if weekly_remaining <= thin_weekly || five_hour_remaining <= thin_five_hour {
        RuntimeSelectionQuotaPressureBand::Thin
    } else {
        RuntimeSelectionQuotaPressureBand::Healthy
    }
}

pub fn runtime_proxy_quota_summary_requires_precommit_live_probe(
    summary: RuntimeProxyQuotaSummary,
    source: Option<RuntimeSelectionQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> bool {
    matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) && !matches!(source, Some(RuntimeSelectionQuotaSource::LiveProbe))
        && (matches!(
            summary.five_hour.status,
            RuntimeSelectionQuotaWindowStatus::Critical
        ) || matches!(
            summary.weekly.status,
            RuntimeSelectionQuotaWindowStatus::Critical
        ) || matches!(
            summary.five_hour.status,
            RuntimeSelectionQuotaWindowStatus::Unknown
        ) || matches!(
            summary.weekly.status,
            RuntimeSelectionQuotaWindowStatus::Unknown
        ))
}

pub fn runtime_proxy_quota_summary_requires_live_source_after_probe(
    summary: RuntimeProxyQuotaSummary,
    source: Option<RuntimeSelectionQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> bool {
    matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) && !matches!(source, Some(RuntimeSelectionQuotaSource::LiveProbe))
        && (matches!(
            summary.five_hour.status,
            RuntimeSelectionQuotaWindowStatus::Unknown
        ) || matches!(
            summary.weekly.status,
            RuntimeSelectionQuotaWindowStatus::Unknown
        ))
}

pub fn runtime_proxy_precommit_quota_block_reason(
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

pub fn runtime_proxy_precommit_quota_gate_initial_decision(
    input: RuntimeProxyPrecommitQuotaGateInitialInput,
) -> RuntimeProxyPrecommitQuotaGateInitialDecision {
    if input.has_continuation_context
        && matches!(
            input.source,
            Some(RuntimeSelectionQuotaSource::PersistedSnapshot)
        )
        && let Some(reason) = runtime_proxy_precommit_quota_block_reason(
            input.summary,
            input.route_kind,
            input.responses_critical_floor_percent,
        )
    {
        return RuntimeProxyPrecommitQuotaGateInitialDecision::Block { reason };
    }

    if runtime_proxy_quota_summary_requires_precommit_live_probe(
        input.summary,
        input.source,
        input.route_kind,
    ) {
        return RuntimeProxyPrecommitQuotaGateInitialDecision::RefreshRequired;
    }

    RuntimeProxyPrecommitQuotaGateInitialDecision::Continue
}

pub fn runtime_proxy_precommit_quota_gate_final_decision(
    input: RuntimeProxyPrecommitQuotaGateFinalInput,
) -> RuntimeProxyPrecommitQuotaGateFinalDecision {
    if runtime_proxy_quota_summary_requires_live_source_after_probe(
        input.summary,
        input.source,
        input.route_kind,
    ) && input.has_alternative_quota_profile
    {
        return RuntimeProxyPrecommitQuotaGateFinalDecision::Block {
            reason: RuntimePrecommitQuotaBlockReason::WindowsUnavailableAfterReprobe,
        };
    }

    if let Some(reason) = runtime_proxy_precommit_quota_block_reason(
        input.summary,
        input.route_kind,
        input.responses_critical_floor_percent,
    ) {
        return RuntimeProxyPrecommitQuotaGateFinalDecision::Block { reason };
    }

    RuntimeProxyPrecommitQuotaGateFinalDecision::Proceed
}

fn runtime_proxy_quota_window_precommit_guard(
    window: RuntimeProxyQuotaWindowSummary,
    floor_percent: i64,
) -> bool {
    runtime_quota_window_precommit_guard(
        crate::RuntimeSelectionQuotaWindowSummary {
            status: window.status,
            remaining_percent: window.remaining_percent,
        },
        floor_percent,
    )
}

fn runtime_proxy_quota_pressure_band_from_window_status(
    status: RuntimeSelectionQuotaWindowStatus,
) -> RuntimeSelectionQuotaPressureBand {
    match status {
        RuntimeSelectionQuotaWindowStatus::Ready => RuntimeSelectionQuotaPressureBand::Healthy,
        RuntimeSelectionQuotaWindowStatus::Thin => RuntimeSelectionQuotaPressureBand::Thin,
        RuntimeSelectionQuotaWindowStatus::Critical => RuntimeSelectionQuotaPressureBand::Critical,
        RuntimeSelectionQuotaWindowStatus::Exhausted => {
            RuntimeSelectionQuotaPressureBand::Exhausted
        }
        RuntimeSelectionQuotaWindowStatus::Unknown => RuntimeSelectionQuotaPressureBand::Unknown,
    }
}

fn runtime_proxy_quota_pressure_band_rank(band: RuntimeSelectionQuotaPressureBand) -> i64 {
    match band {
        RuntimeSelectionQuotaPressureBand::Healthy => 0,
        RuntimeSelectionQuotaPressureBand::Thin => 1,
        RuntimeSelectionQuotaPressureBand::Critical => 2,
        RuntimeSelectionQuotaPressureBand::Exhausted => 3,
        RuntimeSelectionQuotaPressureBand::Unknown => 4,
    }
}

fn runtime_proxy_quota_window_status_rank(status: RuntimeSelectionQuotaWindowStatus) -> i64 {
    match status {
        RuntimeSelectionQuotaWindowStatus::Ready => 0,
        RuntimeSelectionQuotaWindowStatus::Thin => 1,
        RuntimeSelectionQuotaWindowStatus::Critical => 2,
        RuntimeSelectionQuotaWindowStatus::Exhausted => 3,
        RuntimeSelectionQuotaWindowStatus::Unknown => 4,
    }
}

#[cfg(test)]
#[path = "../tests/src/quota.rs"]
mod tests;
