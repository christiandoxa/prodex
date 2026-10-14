use super::*;

/// Merge explicit session-release barriers before merging owner maps. This
/// prevents an old snapshot from converting a legitimate rotation into a
/// permanent hard-owner conflict. Other binding kinds still use strict merge.
pub(super) fn merge_session_release_epochs(
    left: &RuntimeContinuationStatuses,
    right: &RuntimeContinuationStatuses,
) -> BTreeMap<String, i64> {
    let mut epochs = BTreeMap::new();
    for statuses in [left, right] {
        for (key, epoch) in &statuses.session_release_epochs {
            add_epoch(&mut epochs, key, *epoch);
        }
        // Legacy stores recorded release only in transient lifecycle status.
        // Import that proof before a later verified status can replace it.
        for (key, status) in &statuses.session_id {
            if status.state == RuntimeContinuationBindingLifecycle::Dead
                && let Some(at) = status.last_not_found_at.or(status.last_touched_at)
            {
                add_epoch(&mut epochs, key, at);
            }
        }
    }
    epochs
}

fn add_epoch(epochs: &mut BTreeMap<String, i64>, key: &str, at: i64) {
    if at < 0 || !prodex_runtime_state::runtime_lineage_key_is_bounded(key) {
        return;
    }
    let epoch = prodex_mojo_core::runtime_lineage::merge_session_release_epoch(
        epochs.get(key).copied(),
        Some(at),
    )
    .expect("Mojo session release fence union failed")
    .expect("present release epoch");
    epochs.insert(key.to_owned(), epoch);
}

pub(super) fn merge_session_bindings(
    left: &BTreeMap<String, ResponseProfileBinding>,
    right: &BTreeMap<String, ResponseProfileBinding>,
    epochs: &BTreeMap<String, i64>,
) -> BTreeMap<String, ResponseProfileBinding> {
    let filter = |bindings: &BTreeMap<String, ResponseProfileBinding>| {
        bindings
            .iter()
            .filter(|(key, binding)| session_binding_is_live(binding, epochs.get(*key).copied()))
            .map(|(key, binding)| (key.clone(), binding.clone()))
            .collect()
    };
    merge_runtime_hard_profile_bindings(&filter(left), &filter(right))
}

fn session_binding_is_live(binding: &ResponseProfileBinding, release: Option<i64>) -> bool {
    prodex_mojo_core::runtime_lineage::session_binding_survives_release(binding.bound_at, release)
        .expect("Mojo session release fence validation failed")
}

pub(super) fn apply_session_release_fences(
    store: &mut RuntimeContinuationStore<ResponseProfileBinding>,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) {
    let mut epochs =
        merge_session_release_epochs(&store.statuses, &RuntimeContinuationStatuses::default());
    for bindings in [
        &mut store.session_id_bindings,
        &mut store.session_profile_bindings,
    ] {
        bindings.retain(|key, binding| session_binding_is_live(binding, epochs.get(key).copied()));
    }
    // Retain barriers for every live session; retain retired sessions a week to
    // fence stale shared-state writers, and bound inactive barrier storage.
    let live = |key: &str| {
        store.session_id_bindings.contains_key(key)
            || store.session_profile_bindings.contains_key(key)
    };
    epochs.retain(|key, at| live(key) || now.saturating_sub(*at) <= 7 * 24 * 60 * 60);
    let limit = policy.session_id_binding_limit.saturating_mul(4).max(16);
    if epochs.len() > limit {
        let mut retired = epochs
            .iter()
            .filter(|(key, _)| !live(key))
            .map(|(key, at)| (*at, key.clone()))
            .collect::<Vec<_>>();
        retired.sort();
        let excess = epochs.len() - limit;
        for (_, key) in retired.into_iter().take(excess) {
            epochs.remove(&key);
        }
    }
    store.statuses.session_release_epochs = epochs;
}
