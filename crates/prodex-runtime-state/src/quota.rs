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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub updated_at: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RuntimeProfileUsageSnapshot<W = RuntimeQuotaWindowStatus> {
    pub checked_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_type: Option<String>,
    pub five_hour_status: W,
    pub five_hour_remaining_percent: i64,
    pub five_hour_reset_at: i64,
    pub weekly_status: W,
    pub weekly_remaining_percent: i64,
    pub weekly_reset_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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
    prodex_mojo_core::runtime_state_quota::timestamp_touch_should_persist(
        timestamp,
        now,
        persist_interval_seconds,
    )
    .expect("Mojo runtime-state timestamp persistence policy returned invalid output")
}

pub fn runtime_probe_cache_freshness(
    checked_at: i64,
    now: i64,
    fresh_seconds: i64,
    stale_grace_seconds: i64,
) -> RuntimeProbeCacheFreshness {
    use prodex_mojo_core::runtime_state_quota::ProbeCacheFreshness as MojoFreshness;
    match prodex_mojo_core::runtime_state_quota::probe_cache_freshness(
        checked_at,
        now,
        fresh_seconds,
        stale_grace_seconds,
    )
    .expect("Mojo runtime-state probe freshness policy returned invalid output")
    {
        MojoFreshness::Fresh => RuntimeProbeCacheFreshness::Fresh,
        MojoFreshness::StaleUsable => RuntimeProbeCacheFreshness::StaleUsable,
        MojoFreshness::Expired => RuntimeProbeCacheFreshness::Expired,
    }
}

fn runtime_profile_usage_snapshot_usability<W, F>(
    snapshot: &RuntimeProfileUsageSnapshot<W>,
    now: i64,
    stale_grace_seconds: i64,
    is_exhausted: F,
) -> prodex_mojo_core::runtime_state_quota::SnapshotUsabilityPlan
where
    W: Copy,
    F: Fn(W) -> bool + Copy,
{
    prodex_mojo_core::runtime_state_quota::snapshot_usability(
        is_exhausted(snapshot.five_hour_status),
        snapshot.five_hour_reset_at,
        is_exhausted(snapshot.weekly_status),
        snapshot.weekly_reset_at,
        snapshot.checked_at,
        now,
        stale_grace_seconds,
    )
    .expect("Mojo runtime-state quota snapshot usability policy returned invalid output")
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
    runtime_profile_usage_snapshot_usability(snapshot, now, 0, is_exhausted).hold_active
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
    runtime_profile_usage_snapshot_usability(snapshot, now, 0, is_exhausted).hold_expired
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
    runtime_profile_usage_snapshot_usability(snapshot, now, stale_grace_seconds, is_exhausted)
        .usable
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
        && previous.plan_type == next.plan_type
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
    let previous_snapshot_present = input.previous_snapshot.is_some();
    let snapshots_materially_match = input.previous_snapshot.is_some_and(|previous| {
        runtime_profile_usage_snapshot_materially_matches(previous, input.next_snapshot)
    });
    let previous_checked_at = input
        .previous_snapshot
        .map_or(0, |previous| previous.checked_at);
    let plan = prodex_mojo_core::runtime_state_quota::probe_usage_snapshot_apply_plan(
        previous_snapshot_present,
        snapshots_materially_match,
        previous_checked_at,
        input.previous_retry_backoff_until,
        input.blocking_reset_at,
        input.quota_blocked,
        input.now,
        input.quota_quarantine_fallback_seconds,
        input.touch_persist_interval_seconds,
    )
    .expect("Mojo runtime-state probe-apply policy returned invalid output");

    RuntimeProbeUsageSnapshotApplyPlan {
        snapshot_should_persist: plan.snapshot_should_persist,
        blocking_reset_at: plan.blocking_reset_at,
        retry_backoff_until: plan.retry_backoff_until,
        retry_backoff_changed: plan.retry_backoff_changed,
    }
}
