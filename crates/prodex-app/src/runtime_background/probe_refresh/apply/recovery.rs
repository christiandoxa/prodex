use super::*;

// Only actual successful HTTP receipts from the probe worker enter this path.
// Persisted snapshots and ordinary cache writes carry no auth evidence.
pub(super) fn recover_confirmed_probe_backoffs(
    runtime: &mut RuntimeRotationState,
    profile: &str,
    next: &RuntimeProfileUsageSnapshot,
    observation: Option<RuntimeProbeAuthObservation<'_>>,
    now: i64,
    messages: &mut Vec<String>,
) -> Option<bool> {
    let Some(observation) = observation else {
        return Some(false);
    };
    let previous = runtime.profile_usage_snapshots.get(profile);
    let auth_key = crate::runtime_profile_auth_failure_key(profile);
    let retry_key = prodex_runtime_store::runtime_profile_retry_backoff_update_key(profile);
    let summary = runtime_quota_summary_from_usage_snapshot(next, RuntimeRouteKind::Responses);
    let plan = prodex_mojo_core::runtime::profile_probe_recovery_plan(
        prodex_mojo_core::runtime::ProfileProbeRecoveryInput {
            // A malformed/empty 2xx payload is not a conclusive quota receipt.
            authenticated_success: summary.route_band != RuntimeQuotaPressureBand::Unknown,
            same_credentials: runtime
                .profile_usage_auth
                .get(profile)
                .is_some_and(|entry| entry.auth == *observation.auth),
            probe_started_at: observation.started_at,
            now,
            auth_failure: runtime
                .profile_health
                .get(&auth_key)
                .map(|entry| (entry.score, entry.updated_at)),
            quota_ready: summary.route_band != RuntimeQuotaPressureBand::Unknown
                && runtime_quota_precommit_guard_reason(summary, RuntimeRouteKind::Responses)
                    .is_none(),
            previous_checked_at: previous.map(|entry| entry.checked_at),
            previous_five_hour_exhausted: previous
                .is_some_and(|entry| entry.five_hour_status == RuntimeQuotaWindowStatus::Exhausted),
            previous_five_hour_reset_at: previous.map_or(0, |entry| entry.five_hour_reset_at),
            previous_weekly_exhausted: previous
                .is_some_and(|entry| entry.weekly_status == RuntimeQuotaWindowStatus::Exhausted),
            previous_weekly_reset_at: previous.map_or(0, |entry| entry.weekly_reset_at),
            retry_until: runtime.profile_retry_backoff_until.get(profile).copied(),
            retry_updated_at_ms: runtime.profile_backoff_updated_at.get(&retry_key).copied(),
        },
    )
    .expect("Mojo authenticated probe recovery policy returned invalid output");
    if !plan.accept_observation {
        messages.push(format!("profile_probe_refresh_stale profile={profile} reason=newer_observation_or_invalid_payload"));
        return None;
    }
    let mut changed = false;
    if plan.clear_auth_failure {
        changed = prodex_runtime_store::clear_runtime_profile_score(
            &mut runtime.profile_health,
            &auth_key,
            now,
        );
        if changed {
            messages.push(format!(
                "profile_auth_backoff_cleared profile={profile} reason=authenticated_probe"
            ));
        }
    }
    if plan.clear_quota_backoff {
        runtime.profile_retry_backoff_until.remove(profile);
        mark_runtime_profile_retry_backoff_update(runtime, profile);
        messages.push(format!(
            "profile_quota_recovered profile={profile} reason=authenticated_probe"
        ));
        changed = true;
    }
    Some(changed)
}
