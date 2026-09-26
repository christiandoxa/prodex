use crate::{
    RuntimeProxyQuotaSummary, RuntimeProxyQuotaWindowSummary, RuntimeProxyUsageSnapshot,
    RuntimeRouteKind, RuntimeSelectionQuotaSource, RuntimeSelectionQuotaWindowStatus,
    RuntimeTokenUsage,
};

#[path = "quota/mojo.rs"]
mod common_quota;
pub(crate) use common_quota::{
    pressure_band_for_route, quota_score_batch, runtime_response_candidate_plan_batch,
};
use common_quota::{quota_band_from_tag, quota_source_tag, quota_status_tag, route_kind_tag};

pub(crate) struct RuntimeQuotaSnapshotDecision {
    pub summary: RuntimeProxyQuotaSummary,
    pub hold_active: bool,
    pub hold_expired: bool,
    pub usable: bool,
}

fn quota_status_from_tag(
    status: i64,
) -> Result<RuntimeSelectionQuotaWindowStatus, prodex_mojo_core::MojoError> {
    match status {
        0 => Ok(RuntimeSelectionQuotaWindowStatus::Ready),
        1 => Ok(RuntimeSelectionQuotaWindowStatus::Thin),
        2 => Ok(RuntimeSelectionQuotaWindowStatus::Critical),
        3 => Ok(RuntimeSelectionQuotaWindowStatus::Exhausted),
        4 => Ok(RuntimeSelectionQuotaWindowStatus::Unknown),
        _ => Err(prodex_mojo_core::MojoError::InvalidOutput),
    }
}

pub(crate) fn quota_snapshot_plan(
    snapshot: RuntimeProxyUsageSnapshot,
    route_kind: RuntimeRouteKind,
    now: i64,
    stale_grace_seconds: i64,
) -> Result<RuntimeQuotaSnapshotDecision, prodex_mojo_core::MojoError> {
    let plan = prodex_mojo_core::runtime::quota_snapshot_plan(
        prodex_mojo_core::runtime::QuotaSnapshotPlanInput {
            five_hour_status: quota_status_tag(snapshot.five_hour_status),
            five_hour_remaining: snapshot.five_hour_remaining_percent,
            five_hour_reset_at: snapshot.five_hour_reset_at,
            weekly_status: quota_status_tag(snapshot.weekly_status),
            weekly_remaining: snapshot.weekly_remaining_percent,
            weekly_reset_at: snapshot.weekly_reset_at,
            route_kind: route_kind_tag(route_kind),
            checked_at: snapshot.checked_at,
            now,
            stale_grace_seconds,
        },
    )?;
    Ok(RuntimeQuotaSnapshotDecision {
        summary: RuntimeProxyQuotaSummary {
            five_hour: RuntimeProxyQuotaWindowSummary {
                status: quota_status_from_tag(plan.five_hour_status)?,
                remaining_percent: plan.five_hour_remaining,
                reset_at: plan.five_hour_reset_at,
            },
            weekly: RuntimeProxyQuotaWindowSummary {
                status: quota_status_from_tag(plan.weekly_status)?,
                remaining_percent: plan.weekly_remaining,
                reset_at: plan.weekly_reset_at,
            },
            route_band: quota_band_from_tag(plan.route_band)?,
        },
        hold_active: plan.hold_active,
        hold_expired: plan.hold_expired,
        usable: plan.usable,
    })
}

pub(crate) fn quota_gate_plan(
    summary: RuntimeProxyQuotaSummary,
    source: Option<RuntimeSelectionQuotaSource>,
    route_kind: RuntimeRouteKind,
    has_continuation_context: bool,
    has_alternative_quota_profile: bool,
) -> Result<prodex_mojo_core::runtime::QuotaGatePlan, prodex_mojo_core::MojoError> {
    prodex_mojo_core::runtime::quota_gate_plan(prodex_mojo_core::runtime::QuotaGatePlanInput {
        five_hour_status: quota_status_tag(summary.five_hour.status),
        five_hour_reset_at: summary.five_hour.reset_at,
        weekly_status: quota_status_tag(summary.weekly.status),
        weekly_reset_at: summary.weekly.reset_at,
        route_kind: route_kind_tag(route_kind),
        source: quota_source_tag(source),
        has_continuation_context,
        has_alternative_quota_profile,
    })
}

pub(crate) fn window_status(
    remaining_percent: i64,
) -> Result<RuntimeSelectionQuotaWindowStatus, prodex_mojo_core::MojoError> {
    let status = prodex_mojo_core::quota::window_status(remaining_percent, true);
    if status == 4 {
        return Err(prodex_mojo_core::MojoError::InvalidOutput);
    }
    quota_status_from_tag(status)
}

pub(crate) fn smart_context_pressure_snapshot(
    model_context_window_tokens: Option<u64>,
    reserved_output_tokens: u64,
    effective_input_tokens: u64,
    effective_input_source: i64,
    unknown_token_window: bool,
    zero_context_window: bool,
    reserved_output_consumes_window: bool,
) -> Result<prodex_mojo_core::runtime::SmartContextPressureSnapshot, prodex_mojo_core::MojoError> {
    prodex_mojo_core::runtime::smart_context_pressure_snapshot(
        model_context_window_tokens,
        reserved_output_tokens,
        effective_input_tokens,
        effective_input_source,
        unknown_token_window,
        zero_context_window,
        reserved_output_consumes_window,
    )
}

pub(crate) fn smart_context_token_usage_summary(
    usages: &[RuntimeTokenUsage],
) -> Result<prodex_mojo_core::runtime::SmartContextTokenUsageSummary, prodex_mojo_core::MojoError> {
    let inputs = usages
        .iter()
        .map(
            |usage| prodex_mojo_core::runtime::SmartContextTokenUsageInput {
                input_tokens: usage.input_tokens,
                cached_input_tokens: usage.cached_input_tokens,
                output_tokens: usage.output_tokens,
                reasoning_tokens: usage.reasoning_tokens,
            },
        )
        .collect::<Vec<_>>();
    prodex_mojo_core::runtime::smart_context_token_usage_summary_batch(&inputs)
}
