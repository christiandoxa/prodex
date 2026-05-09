use super::*;
mod waitable;
pub(crate) use waitable::*;

mod affinity;
mod catalog;
mod current;
mod dispatch;
mod next;
mod policy;
mod previous_response;

use self::policy::{
    RuntimeAffinitySelectionKind, RuntimeSoftAffinityPolicyInput, runtime_soft_affinity_allowed,
    runtime_soft_affinity_rejection_reason,
};
#[allow(unused_imports)]
pub(crate) use self::policy::{
    RuntimeCandidateAffinity, RuntimeNoRotateAffinity,
    RuntimePreviousResponseNotFoundFallbackPolicy, RuntimePreviousResponseNotFoundFallbackRequest,
    RuntimePreviousResponseStaleContinuationPolicy, RuntimeQuotaBlockedAffinityReleasePolicy,
    RuntimeQuotaBlockedAffinityReleaseRequest, RuntimeResponseCandidateSelection,
    RuntimeWebsocketReuseWatchdogPreviousResponseFallback, runtime_candidate_has_hard_affinity,
    runtime_candidate_no_rotate_affinity, runtime_previous_response_not_found_fallback_policy,
    runtime_quota_blocked_affinity_is_releasable, runtime_quota_blocked_affinity_release_policy,
    runtime_quota_blocked_previous_response_fresh_fallback_allowed,
    runtime_quota_precommit_guard_reason,
    runtime_websocket_previous_response_not_found_requires_stale_continuation,
    runtime_websocket_previous_response_reuse_is_nonreplayable,
    runtime_websocket_previous_response_reuse_is_stale,
    runtime_websocket_reuse_watchdog_previous_response_fresh_fallback_allowed,
};

use self::affinity::{RuntimeAffinitySelectionDecision, runtime_affinity_selection_decision};
pub(crate) use self::affinity::{
    runtime_previous_response_affinity_is_bound, runtime_previous_response_affinity_is_trusted,
};
pub(crate) use self::catalog::*;
pub(crate) use self::current::runtime_proxy_optimistic_current_candidate_for_route;
use self::current::runtime_proxy_optimistic_current_candidate_for_route_with_selection;
pub(crate) use self::dispatch::*;
pub(crate) use self::next::next_runtime_response_candidate_for_route;
use self::next::next_runtime_response_candidate_for_route_with_prompt_cache_key;
pub(crate) use self::previous_response::next_runtime_previous_response_candidate;

fn runtime_selection_quota_source_label(source: Option<RuntimeQuotaSource>) -> &'static str {
    source.map(runtime_quota_source_label).unwrap_or("unknown")
}

fn runtime_selection_log_fields_with_quota<'a>(
    fields: impl IntoIterator<Item = RuntimeProxyLogField<'a>>,
    summary: RuntimeQuotaSummary,
) -> Vec<RuntimeProxyLogField<'a>> {
    let mut fields = fields.into_iter().collect::<Vec<_>>();
    fields.extend([
        runtime_proxy_log_field(
            "quota_band",
            runtime_quota_pressure_band_reason(summary.route_band),
        ),
        runtime_proxy_log_field(
            "five_hour_status",
            runtime_quota_window_status_reason(summary.five_hour.status),
        ),
        runtime_proxy_log_field(
            "five_hour_remaining",
            summary.five_hour.remaining_percent.to_string(),
        ),
        runtime_proxy_log_field("five_hour_reset_at", summary.five_hour.reset_at.to_string()),
        runtime_proxy_log_field(
            "weekly_status",
            runtime_quota_window_status_reason(summary.weekly.status),
        ),
        runtime_proxy_log_field(
            "weekly_remaining",
            summary.weekly.remaining_percent.to_string(),
        ),
        runtime_proxy_log_field("weekly_reset_at", summary.weekly.reset_at.to_string()),
    ]);
    fields
}

pub(crate) fn runtime_proxy_sync_probe_pressure_pause(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
) {
    if !runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind) {
        return;
    }
    let pause_ms = runtime_proxy_sync_probe_pressure_pause_ms();
    let observed_revision = runtime_probe_refresh_revision();
    let started_at = Instant::now();
    let wait_outcome = runtime_probe_refresh_wait_outcome_since(
        Duration::from_millis(pause_ms),
        observed_revision,
    );
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "runtime_proxy_sync_probe_pressure_pause",
            [
                runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                runtime_proxy_log_field("pause_ms", pause_ms.to_string()),
                runtime_proxy_log_field("waited_ms", started_at.elapsed().as_millis().to_string()),
                runtime_proxy_log_field(
                    "outcome",
                    runtime_profile_wait_outcome_label(wait_outcome),
                ),
            ],
        ),
    );
}
