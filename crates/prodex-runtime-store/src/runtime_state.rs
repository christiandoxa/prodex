use crate::{
    RuntimeContinuationCompactionPolicy, merge_runtime_continuation_store,
    runtime_external_response_profile_bindings,
};
use prodex_runtime_state::{RuntimeContinuationStore, RuntimeProfileUsageSnapshot};
use prodex_state::{AppState, AppStateCompactionPolicy, ProfileEntry, ResponseProfileBinding};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct RuntimeMergedStateAndContinuations {
    pub state: AppState,
    pub continuations: RuntimeContinuationStore<ResponseProfileBinding>,
}

pub fn merge_runtime_state_snapshot_for_save(
    existing: AppState,
    snapshot: &AppState,
    now: i64,
    policy: AppStateCompactionPolicy,
) -> AppState {
    prodex_state::merge_runtime_state_snapshot_with_policy(existing, snapshot, now, policy)
}

pub fn merge_runtime_state_and_continuations_for_save(
    existing_state: AppState,
    state_snapshot: &AppState,
    existing_continuations: &RuntimeContinuationStore<ResponseProfileBinding>,
    continuation_snapshot: &RuntimeContinuationStore<ResponseProfileBinding>,
    now: i64,
    state_policy: AppStateCompactionPolicy,
    continuation_policy: RuntimeContinuationCompactionPolicy,
) -> RuntimeMergedStateAndContinuations {
    let mut state =
        merge_runtime_state_snapshot_for_save(existing_state, state_snapshot, now, state_policy);
    let continuations = merge_runtime_continuation_store(
        existing_continuations,
        continuation_snapshot,
        &state.profiles,
        now,
        continuation_policy,
    );
    state.response_profile_bindings =
        runtime_external_response_profile_bindings(&continuations.response_profile_bindings);
    state.session_profile_bindings = continuations.session_profile_bindings.clone();
    RuntimeMergedStateAndContinuations {
        state,
        continuations,
    }
}

pub fn compact_runtime_usage_snapshots<W>(
    mut snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot<W>>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> BTreeMap<String, RuntimeProfileUsageSnapshot<W>> {
    let oldest_allowed = now.saturating_sub(crate::RUNTIME_USAGE_SNAPSHOT_RETENTION_SECONDS);
    snapshots.retain(|profile_name, snapshot| {
        profiles.contains_key(profile_name) && snapshot.checked_at >= oldest_allowed
    });
    snapshots
}

pub fn merge_runtime_usage_snapshots<W: Clone>(
    existing: &BTreeMap<String, RuntimeProfileUsageSnapshot<W>>,
    incoming: &BTreeMap<String, RuntimeProfileUsageSnapshot<W>>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> BTreeMap<String, RuntimeProfileUsageSnapshot<W>> {
    let mut merged = existing.clone();
    for (profile_name, snapshot) in incoming {
        let should_replace = merged
            .get(profile_name)
            .is_none_or(|current| current.checked_at <= snapshot.checked_at);
        if should_replace {
            merged.insert(profile_name.clone(), snapshot.clone());
        }
    }
    compact_runtime_usage_snapshots(merged, profiles, now)
}
