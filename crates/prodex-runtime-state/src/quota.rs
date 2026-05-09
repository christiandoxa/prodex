use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    let value = Option::<T>::deserialize(deserializer)?;
    Ok(value.unwrap_or_default())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeQuotaWindowStatus {
    Ready,
    Thin,
    Critical,
    Exhausted,
    Unknown,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeProfileBackoffs {
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub retry_backoff_until: BTreeMap<String, i64>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub transport_backoff_until: BTreeMap<String, i64>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub route_circuit_open_until: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeProfileUsageSnapshot<W = RuntimeQuotaWindowStatus> {
    pub checked_at: i64,
    pub five_hour_status: W,
    pub five_hour_remaining_percent: i64,
    pub five_hour_reset_at: i64,
    pub weekly_status: W,
    pub weekly_remaining_percent: i64,
    pub weekly_reset_at: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeProfileHealth {
    pub score: u32,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProbeCacheFreshness {
    Fresh,
    StaleUsable,
    Expired,
}

pub fn runtime_timestamp_touch_should_persist(
    timestamp: i64,
    now: i64,
    persist_interval_seconds: i64,
) -> bool {
    // Timestamps are persisted with second precision. Require strictly more
    // than interval so boundary crossings do not persist almost a second early.
    now.saturating_sub(timestamp) > persist_interval_seconds
}

pub fn runtime_probe_cache_freshness(
    checked_at: i64,
    now: i64,
    fresh_seconds: i64,
    stale_grace_seconds: i64,
) -> RuntimeProbeCacheFreshness {
    let age = now.saturating_sub(checked_at);
    if age <= fresh_seconds {
        RuntimeProbeCacheFreshness::Fresh
    } else if age <= stale_grace_seconds {
        RuntimeProbeCacheFreshness::StaleUsable
    } else {
        RuntimeProbeCacheFreshness::Expired
    }
}

pub fn runtime_profile_usage_snapshot_hold_active<W, F>(
    snapshot: &RuntimeProfileUsageSnapshot<W>,
    now: i64,
    is_exhausted: F,
) -> bool
where
    W: Copy,
    F: Fn(W) -> bool + Copy,
{
    [
        (snapshot.five_hour_status, snapshot.five_hour_reset_at),
        (snapshot.weekly_status, snapshot.weekly_reset_at),
    ]
    .into_iter()
    .any(|(status, reset_at)| is_exhausted(status) && reset_at != i64::MAX && reset_at > now)
}

pub fn runtime_profile_usage_snapshot_hold_expired<W, F>(
    snapshot: &RuntimeProfileUsageSnapshot<W>,
    now: i64,
    is_exhausted: F,
) -> bool
where
    W: Copy,
    F: Fn(W) -> bool + Copy,
{
    [
        (snapshot.five_hour_status, snapshot.five_hour_reset_at),
        (snapshot.weekly_status, snapshot.weekly_reset_at),
    ]
    .into_iter()
    .any(|(status, reset_at)| is_exhausted(status) && reset_at != i64::MAX && reset_at <= now)
}

pub fn runtime_profile_usage_snapshot_is_usable<W, F>(
    snapshot: &RuntimeProfileUsageSnapshot<W>,
    now: i64,
    stale_grace_seconds: i64,
    is_exhausted: F,
) -> bool
where
    W: Copy,
    F: Fn(W) -> bool + Copy,
{
    if runtime_profile_usage_snapshot_hold_active(snapshot, now, is_exhausted) {
        return true;
    }
    if runtime_profile_usage_snapshot_hold_expired(snapshot, now, is_exhausted) {
        return false;
    }
    now.saturating_sub(snapshot.checked_at) <= stale_grace_seconds
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStartupProbeRefreshCandidate<N> {
    pub profile_name: N,
    pub probe_fresh: bool,
    pub snapshot_usable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeStartupProbeRefreshPlan {
    pub now: i64,
    pub probe_fresh_seconds: i64,
    pub stale_grace_seconds: i64,
    pub warm_limit: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeStartupProbeRefreshInput<'a, N, W> {
    pub profile_name: N,
    pub probe_checked_at: Option<i64>,
    pub usage_snapshot: Option<&'a RuntimeProfileUsageSnapshot<W>>,
}

pub fn runtime_profiles_needing_startup_probe_refresh<N, I>(
    candidates: I,
    warm_limit: usize,
) -> Vec<N>
where
    I: IntoIterator<Item = RuntimeStartupProbeRefreshCandidate<N>>,
{
    candidates
        .into_iter()
        .filter_map(|candidate| {
            (!candidate.probe_fresh && !candidate.snapshot_usable).then_some(candidate.profile_name)
        })
        .take(warm_limit)
        .collect()
}

pub fn runtime_profiles_needing_startup_probe_refresh_from_snapshots<'a, N, W, I, F>(
    inputs: I,
    plan: RuntimeStartupProbeRefreshPlan,
    is_exhausted: F,
) -> Vec<N>
where
    W: Copy + 'a,
    I: IntoIterator<Item = RuntimeStartupProbeRefreshInput<'a, N, W>>,
    F: Fn(W) -> bool + Copy,
{
    runtime_profiles_needing_startup_probe_refresh(
        inputs.into_iter().map(|input| {
            let probe_fresh = input.probe_checked_at.is_some_and(|checked_at| {
                runtime_probe_cache_freshness(
                    checked_at,
                    plan.now,
                    plan.probe_fresh_seconds,
                    plan.stale_grace_seconds,
                ) == RuntimeProbeCacheFreshness::Fresh
            });
            let snapshot_usable = input.usage_snapshot.is_some_and(|snapshot| {
                runtime_profile_usage_snapshot_is_usable(
                    snapshot,
                    plan.now,
                    plan.stale_grace_seconds,
                    is_exhausted,
                )
            });
            RuntimeStartupProbeRefreshCandidate {
                profile_name: input.profile_name,
                probe_fresh,
                snapshot_usable,
            }
        }),
        plan.warm_limit,
    )
}

pub fn runtime_profile_usage_snapshot_materially_matches<W: PartialEq>(
    previous: &RuntimeProfileUsageSnapshot<W>,
    next: &RuntimeProfileUsageSnapshot<W>,
) -> bool {
    previous.five_hour_status == next.five_hour_status
        && previous.five_hour_remaining_percent == next.five_hour_remaining_percent
        && previous.five_hour_reset_at == next.five_hour_reset_at
        && previous.weekly_status == next.weekly_status
        && previous.weekly_remaining_percent == next.weekly_remaining_percent
        && previous.weekly_reset_at == next.weekly_reset_at
}

pub fn runtime_profile_usage_snapshot_should_persist<W: PartialEq>(
    previous: Option<&RuntimeProfileUsageSnapshot<W>>,
    next: &RuntimeProfileUsageSnapshot<W>,
    now: i64,
    touch_persist_interval_seconds: i64,
) -> bool {
    let Some(previous) = previous else {
        return true;
    };

    !runtime_profile_usage_snapshot_materially_matches(previous, next)
        || runtime_timestamp_touch_should_persist(
            previous.checked_at,
            now,
            touch_persist_interval_seconds,
        )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProbeUsageSnapshotApplyPlan {
    pub snapshot_should_persist: bool,
    pub blocking_reset_at: Option<i64>,
    pub retry_backoff_until: Option<i64>,
    pub retry_backoff_changed: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeProbeUsageSnapshotApplyInput<'a, W> {
    pub previous_snapshot: Option<&'a RuntimeProfileUsageSnapshot<W>>,
    pub previous_retry_backoff_until: Option<i64>,
    pub next_snapshot: &'a RuntimeProfileUsageSnapshot<W>,
    pub quota_blocked: bool,
    pub blocking_reset_at: Option<i64>,
    pub now: i64,
    pub quota_quarantine_fallback_seconds: i64,
    pub touch_persist_interval_seconds: i64,
}

pub fn runtime_probe_usage_snapshot_apply_plan<W: PartialEq>(
    input: RuntimeProbeUsageSnapshotApplyInput<'_, W>,
) -> RuntimeProbeUsageSnapshotApplyPlan {
    let snapshot_should_persist = runtime_profile_usage_snapshot_should_persist(
        input.previous_snapshot,
        input.next_snapshot,
        input.now,
        input.touch_persist_interval_seconds,
    );
    let blocking_reset_at = input
        .blocking_reset_at
        .filter(|reset_at| *reset_at > input.now);
    let quarantine_until = input.quota_blocked.then(|| {
        blocking_reset_at.unwrap_or_else(|| {
            input
                .now
                .saturating_add(input.quota_quarantine_fallback_seconds)
        })
    });
    let retry_backoff_until = quarantine_until.map(|until| {
        input
            .previous_retry_backoff_until
            .unwrap_or(until)
            .max(until)
    });
    let retry_backoff_changed =
        retry_backoff_until.is_some_and(|until| Some(until) != input.previous_retry_backoff_until);

    RuntimeProbeUsageSnapshotApplyPlan {
        snapshot_should_persist,
        blocking_reset_at,
        retry_backoff_until,
        retry_backoff_changed,
    }
}
