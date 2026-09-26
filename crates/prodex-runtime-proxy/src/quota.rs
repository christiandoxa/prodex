use std::cmp::Reverse;

use crate::{
    RuntimeRouteKind, RuntimeSelectionQuotaPressureBand, RuntimeSelectionQuotaSource,
    RuntimeSelectionQuotaWindowStatus, runtime_quota_precommit_floor_percent_for_route,
    runtime_quota_window_precommit_guard,
};

#[cfg(feature = "mojo")]
#[path = "mojo.rs"]
pub(crate) mod mojo;

#[cfg(not(feature = "mojo"))]
#[path = "quota/mojo.rs"]
pub(crate) mod mojo;

#[cfg(any(not(feature = "mojo"), test))]
#[path = "quota/rust_oracles.rs"]
mod rust_oracles;

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
    pub pressure_band: RuntimeSelectionQuotaPressureBand,
    pub total_pressure: i64,
    pub weekly_pressure: i64,
    pub five_hour_pressure: i64,
    pub reserve_floor: i64,
    pub weekly_remaining: i64,
    pub five_hour_remaining: i64,
    pub weekly_reset_at: i64,
    pub five_hour_reset_at: i64,
}

pub type RuntimeProxyQuotaObservationPair = (
    Option<RuntimeProxyQuotaWindowObservation>,
    Option<RuntimeProxyQuotaWindowObservation>,
);

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
    #[cfg(feature = "mojo")]
    let status = mojo::window_status(window.remaining_percent)
        .expect("Mojo quota window status returned an invalid tag");
    #[cfg(not(feature = "mojo"))]
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
    let (five_hour_summary, weekly_summary) =
        runtime_proxy_quota_window_summaries(five_hour, weekly);
    RuntimeProxyQuotaSummary {
        five_hour: five_hour_summary,
        weekly: weekly_summary,
        route_band: runtime_proxy_quota_pressure_band_for_route(five_hour, weekly, route_kind),
    }
}

fn runtime_proxy_quota_window_summaries(
    five_hour: Option<RuntimeProxyQuotaWindowObservation>,
    weekly: Option<RuntimeProxyQuotaWindowObservation>,
) -> (
    RuntimeProxyQuotaWindowSummary,
    RuntimeProxyQuotaWindowSummary,
) {
    let neutral = RuntimeProxyQuotaWindowSummary {
        status: RuntimeSelectionQuotaWindowStatus::Ready,
        remaining_percent: 100,
        reset_at: i64::MAX,
    };
    match (five_hour, weekly) {
        (None, Some(weekly)) => (neutral, runtime_proxy_quota_window_summary(Some(weekly))),
        (Some(five_hour), None) => (runtime_proxy_quota_window_summary(Some(five_hour)), neutral),
        _ => (
            runtime_proxy_quota_window_summary(five_hour),
            runtime_proxy_quota_window_summary(weekly),
        ),
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

#[cfg(test)]
pub(crate) fn runtime_proxy_usage_snapshot_from_observations_at(
    five_hour: Option<RuntimeProxyQuotaWindowObservation>,
    weekly: Option<RuntimeProxyQuotaWindowObservation>,
    checked_at: i64,
) -> RuntimeProxyUsageSnapshot {
    let (five_hour, weekly) = runtime_proxy_quota_window_summaries(five_hour, weekly);
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
    #[cfg(feature = "mojo")]
    {
        mojo::quota_snapshot_plan(snapshot, route_kind, now, 0)
            .expect("Mojo quota snapshot planning returned an invalid result")
            .summary
    }

    #[cfg(not(feature = "mojo"))]
    {
        rust_oracles::summary_from_usage_snapshot_at(snapshot, route_kind, now)
    }
}

pub fn runtime_proxy_quota_window_summary_from_usage_snapshot_at(
    status: RuntimeSelectionQuotaWindowStatus,
    remaining_percent: i64,
    reset_at: i64,
    now: i64,
) -> RuntimeProxyQuotaWindowSummary {
    #[cfg(feature = "mojo")]
    {
        let snapshot = RuntimeProxyUsageSnapshot {
            checked_at: now,
            five_hour_status: status,
            five_hour_remaining_percent: remaining_percent,
            five_hour_reset_at: reset_at,
            weekly_status: status,
            weekly_remaining_percent: remaining_percent,
            weekly_reset_at: reset_at,
        };
        mojo::quota_snapshot_plan(snapshot, RuntimeRouteKind::Standard, now, 0)
            .expect("Mojo quota snapshot-window planning returned an invalid result")
            .summary
            .five_hour
    }

    #[cfg(not(feature = "mojo"))]
    {
        rust_oracles::window_summary_from_usage_snapshot_at(
            status,
            remaining_percent,
            reset_at,
            now,
        )
    }
}

pub fn runtime_proxy_usage_snapshot_hold_active(
    snapshot: RuntimeProxyUsageSnapshot,
    now: i64,
) -> bool {
    #[cfg(feature = "mojo")]
    {
        mojo::quota_snapshot_plan(snapshot, RuntimeRouteKind::Standard, now, 0)
            .expect("Mojo quota hold planning returned an invalid result")
            .hold_active
    }

    #[cfg(not(feature = "mojo"))]
    {
        rust_oracles::usage_snapshot_hold_active(snapshot, now)
    }
}

pub fn runtime_proxy_usage_snapshot_hold_expired(
    snapshot: RuntimeProxyUsageSnapshot,
    now: i64,
) -> bool {
    #[cfg(feature = "mojo")]
    {
        mojo::quota_snapshot_plan(snapshot, RuntimeRouteKind::Standard, now, 0)
            .expect("Mojo quota hold planning returned an invalid result")
            .hold_expired
    }

    #[cfg(not(feature = "mojo"))]
    {
        rust_oracles::usage_snapshot_hold_expired(snapshot, now)
    }
}

pub fn runtime_proxy_usage_snapshot_is_usable(
    snapshot: RuntimeProxyUsageSnapshot,
    now: i64,
    stale_grace_seconds: i64,
) -> bool {
    #[cfg(feature = "mojo")]
    {
        mojo::quota_snapshot_plan(
            snapshot,
            RuntimeRouteKind::Standard,
            now,
            stale_grace_seconds,
        )
        .expect("Mojo quota snapshot usability planning returned an invalid result")
        .usable
    }

    #[cfg(not(feature = "mojo"))]
    {
        rust_oracles::usage_snapshot_is_usable(snapshot, now, stale_grace_seconds)
    }
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
    runtime_proxy_quota_scores_for_route_batch(&[(five_hour, weekly)], route_kind)
        .into_iter()
        .next()
        .expect("Mojo runtime quota score batch returned no score")
}

pub fn runtime_proxy_quota_scores_for_route_batch(
    observations: &[RuntimeProxyQuotaObservationPair],
    route_kind: RuntimeRouteKind,
) -> Vec<RuntimeProxyQuotaScore> {
    let mut scores = Vec::with_capacity(observations.len());
    for chunk in observations.chunks(prodex_mojo_core::runtime::RUNTIME_QUOTA_SCORE_MAX_COUNT) {
        scores.extend(
            mojo::quota_score_batch(chunk, route_kind)
                .expect("Mojo runtime quota score batch returned invalid output"),
        );
    }
    assert_eq!(
        scores.len(),
        observations.len(),
        "Mojo runtime quota score batch returned the wrong count"
    );
    scores
}

pub fn runtime_proxy_quota_pressure_band_for_route(
    five_hour: Option<RuntimeProxyQuotaWindowObservation>,
    weekly: Option<RuntimeProxyQuotaWindowObservation>,
    route_kind: RuntimeRouteKind,
) -> RuntimeSelectionQuotaPressureBand {
    mojo::pressure_band_for_route(five_hour, weekly, route_kind)
        .expect("Mojo runtime quota pressure band returned an invalid result")
}

pub fn runtime_proxy_quota_summary_requires_precommit_live_probe(
    summary: RuntimeProxyQuotaSummary,
    source: Option<RuntimeSelectionQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> bool {
    #[cfg(feature = "mojo")]
    {
        mojo::quota_gate_plan(summary, source, route_kind, false, false)
            .expect("Mojo quota gate planning returned an invalid result")
            .requires_precommit_live_probe
    }

    #[cfg(not(feature = "mojo"))]
    {
        rust_oracles::summary_requires_precommit_live_probe(summary, source, route_kind)
    }
}

pub fn runtime_proxy_quota_summary_requires_live_source_after_probe(
    summary: RuntimeProxyQuotaSummary,
    source: Option<RuntimeSelectionQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> bool {
    #[cfg(feature = "mojo")]
    {
        mojo::quota_gate_plan(summary, source, route_kind, false, false)
            .expect("Mojo quota post-probe planning returned an invalid result")
            .requires_live_source_after_probe
    }

    #[cfg(not(feature = "mojo"))]
    {
        rust_oracles::summary_requires_live_source_after_probe(summary, source, route_kind)
    }
}

pub fn runtime_proxy_precommit_quota_block_reason(
    summary: RuntimeProxyQuotaSummary,
    route_kind: RuntimeRouteKind,
    _responses_critical_floor_percent: i64,
) -> Option<RuntimePrecommitQuotaBlockReason> {
    #[cfg(feature = "mojo")]
    {
        runtime_proxy_precommit_quota_block_reason_from_tag(
            mojo::quota_gate_plan(summary, None, route_kind, false, false)
                .expect("Mojo quota block planning returned an invalid result")
                .block_reason,
        )
    }

    #[cfg(not(feature = "mojo"))]
    {
        rust_oracles::precommit_quota_block_reason(
            summary,
            route_kind,
            _responses_critical_floor_percent,
        )
    }
}

#[cfg(feature = "mojo")]
fn runtime_proxy_precommit_quota_block_reason_from_tag(
    reason: i64,
) -> Option<RuntimePrecommitQuotaBlockReason> {
    match reason {
        0 => None,
        1 => Some(RuntimePrecommitQuotaBlockReason::ExhaustedBeforeSend),
        2 => Some(RuntimePrecommitQuotaBlockReason::CriticalFloorBeforeSend),
        3 => Some(RuntimePrecommitQuotaBlockReason::WindowsUnavailableAfterReprobe),
        _ => unreachable!("validated Mojo quota block reason"),
    }
}

pub fn runtime_proxy_precommit_quota_gate_initial_decision(
    input: RuntimeProxyPrecommitQuotaGateInitialInput,
) -> RuntimeProxyPrecommitQuotaGateInitialDecision {
    #[cfg(feature = "mojo")]
    {
        let plan = mojo::quota_gate_plan(
            input.summary,
            input.source,
            input.route_kind,
            input.has_continuation_context,
            false,
        )
        .expect("Mojo initial quota gate planning returned an invalid result");
        match plan.initial_decision {
            0 => RuntimeProxyPrecommitQuotaGateInitialDecision::Continue,
            1 => RuntimeProxyPrecommitQuotaGateInitialDecision::RefreshRequired,
            2 => RuntimeProxyPrecommitQuotaGateInitialDecision::Block {
                reason: runtime_proxy_precommit_quota_block_reason_from_tag(plan.initial_reason)
                    .expect("Mojo blocked initial quota plan omitted reason"),
            },
            _ => unreachable!("validated Mojo initial quota decision"),
        }
    }

    #[cfg(not(feature = "mojo"))]
    rust_oracles::precommit_quota_gate_initial_decision(input)
}

pub fn runtime_proxy_precommit_quota_gate_final_decision(
    input: RuntimeProxyPrecommitQuotaGateFinalInput,
) -> RuntimeProxyPrecommitQuotaGateFinalDecision {
    #[cfg(feature = "mojo")]
    {
        let plan = mojo::quota_gate_plan(
            input.summary,
            input.source,
            input.route_kind,
            false,
            input.has_alternative_quota_profile,
        )
        .expect("Mojo final quota gate planning returned an invalid result");
        if plan.final_blocked {
            RuntimeProxyPrecommitQuotaGateFinalDecision::Block {
                reason: runtime_proxy_precommit_quota_block_reason_from_tag(plan.final_reason)
                    .expect("Mojo blocked final quota plan omitted reason"),
            }
        } else {
            RuntimeProxyPrecommitQuotaGateFinalDecision::Proceed
        }
    }

    #[cfg(not(feature = "mojo"))]
    rust_oracles::precommit_quota_gate_final_decision(input)
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

#[cfg(test)]
#[path = "../tests/src/quota_mojo.rs"]
mod mojo_tests;
