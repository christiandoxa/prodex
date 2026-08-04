use super::*;

pub fn prune_runtime_continuation_status_map(
    statuses: &mut BTreeMap<String, RuntimeContinuationBindingStatus>,
    bindings: &BTreeMap<String, ResponseProfileBinding>,
    max_entries: usize,
    policy: RuntimeContinuationCompactionPolicy,
) {
    if statuses.len() <= max_entries {
        return;
    }

    let excess = statuses.len() - max_entries;
    let mut coldest = statuses
        .iter()
        .map(|(key, status)| {
            (
                key.clone(),
                runtime_continuation_status_retention_sort_key(key, status, bindings, policy),
            )
        })
        .collect::<Vec<_>>();
    coldest.sort_by_key(|(_, retention)| *retention);

    for (key, _) in coldest.into_iter().take(excess) {
        statuses.remove(&key);
    }
}

pub fn runtime_continuation_binding_should_retain(
    binding: &ResponseProfileBinding,
    status: Option<&RuntimeContinuationBindingStatus>,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> bool {
    if is_hard_binding_conflict_profile(&binding.profile_name) {
        return true;
    }
    match status {
        Some(status) if runtime_continuation_dead_status_shadowed_by_binding(binding, status) => {
            true
        }
        Some(status) if runtime_continuation_status_is_terminal(status, policy) => false,
        Some(status) => runtime_continuation_status_should_retain_with_binding(status, now, policy),
        None => binding.bound_at <= now,
    }
}

pub fn runtime_continuation_binding_retention_sort_key(
    binding: &ResponseProfileBinding,
    status: Option<&RuntimeContinuationBindingStatus>,
    policy: RuntimeContinuationCompactionPolicy,
) -> (u8, u32, u32, u32, u8, i64, i64, i64, i64) {
    if is_hard_binding_conflict_profile(&binding.profile_name) {
        return (
            u8::MAX,
            u32::MAX,
            u32::MAX,
            u32::MAX,
            u8::MAX,
            i64::MAX,
            i64::MAX,
            i64::MAX,
            binding.bound_at,
        );
    }
    let evidence = status
        .map(|status| runtime_continuation_status_evidence_sort_key(status, policy))
        .unwrap_or((0, 0, 0, 0, 0, i64::MIN, i64::MIN, i64::MIN));
    (
        evidence.0,
        evidence.1,
        evidence.2,
        evidence.3,
        evidence.4,
        evidence.5,
        evidence.6,
        evidence.7,
        binding.bound_at,
    )
}

pub fn prune_runtime_continuation_response_bindings(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
    statuses: &BTreeMap<String, RuntimeContinuationBindingStatus>,
    max_entries: usize,
    policy: RuntimeContinuationCompactionPolicy,
) {
    if bindings.len() <= max_entries {
        return;
    }

    let excess = bindings.len() - max_entries;
    let mut coldest = bindings
        .iter()
        .map(|(response_id, binding)| {
            (
                response_id.clone(),
                runtime_continuation_binding_retention_sort_key(
                    binding,
                    statuses.get(response_id),
                    policy,
                ),
            )
        })
        .collect::<Vec<_>>();
    coldest.sort_by_key(|(_, retention)| *retention);

    for (response_id, _) in coldest.into_iter().take(excess) {
        bindings.remove(&response_id);
    }
}

pub fn compact_runtime_continuation_store(
    mut continuations: RuntimeContinuationStore<ResponseProfileBinding>,
    _profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> RuntimeContinuationStore<ResponseProfileBinding> {
    mark_unbounded_profile_bindings_as_conflict(&mut continuations.response_profile_bindings);
    mark_unbounded_profile_bindings_as_conflict(&mut continuations.session_profile_bindings);
    mark_unbounded_profile_bindings_as_conflict(&mut continuations.turn_state_bindings);
    mark_unbounded_profile_bindings_as_conflict(&mut continuations.session_id_bindings);
    continuations
        .response_profile_bindings
        .retain(|key, binding| {
            prodex_runtime_state::runtime_lineage_key_is_bounded(key)
                && runtime_continuation_binding_should_retain(
                    binding,
                    continuations.statuses.response.get(key),
                    now,
                    policy,
                )
        });
    let response_turn_state_keys = continuations
        .response_profile_bindings
        .keys()
        .filter(|key| runtime_is_response_turn_state_lineage_key(key))
        .cloned()
        .collect::<Vec<_>>();
    for key in response_turn_state_keys {
        if continuations
            .response_profile_bindings
            .get(&key)
            .is_some_and(|binding| is_hard_binding_conflict_profile(&binding.profile_name))
        {
            continue;
        }
        let Some((response_id, _)) = runtime_response_turn_state_lineage_parts(&key) else {
            continuations.response_profile_bindings.remove(&key);
            continue;
        };
        if continuations
            .response_profile_bindings
            .get(response_id)
            .is_none_or(|binding| binding.profile_name.is_empty())
        {
            continuations.response_profile_bindings.remove(&key);
        }
    }
    continuations.turn_state_bindings.retain(|key, binding| {
        prodex_runtime_state::runtime_lineage_key_is_bounded(key)
            && runtime_continuation_binding_should_retain(
                binding,
                continuations.statuses.turn_state.get(key),
                now,
                policy,
            )
    });
    continuations
        .session_profile_bindings
        .retain(|key, binding| {
            prodex_runtime_state::runtime_lineage_key_is_bounded(key)
                && runtime_continuation_binding_should_retain(
                    binding,
                    continuations.statuses.session_id.get(key),
                    now,
                    policy,
                )
        });
    continuations.session_id_bindings.retain(|key, binding| {
        prodex_runtime_state::runtime_lineage_key_is_bounded(key)
            && runtime_continuation_binding_should_retain(
                binding,
                continuations.statuses.session_id.get(key),
                now,
                policy,
            )
    });
    prune_runtime_continuation_response_bindings(
        &mut continuations.response_profile_bindings,
        &continuations.statuses.response,
        policy.response_binding_limit,
        policy,
    );
    prune_profile_bindings(
        &mut continuations.turn_state_bindings,
        policy.turn_state_binding_limit,
    );
    prune_profile_bindings(
        &mut continuations.session_profile_bindings,
        policy.session_id_binding_limit,
    );
    prune_profile_bindings(
        &mut continuations.session_id_bindings,
        policy.session_id_binding_limit,
    );
    continuations
        .statuses
        .response
        .retain(|key, _| prodex_runtime_state::runtime_lineage_key_is_bounded(key));
    continuations
        .statuses
        .turn_state
        .retain(|key, _| prodex_runtime_state::runtime_lineage_key_is_bounded(key));
    continuations
        .statuses
        .session_id
        .retain(|key, _| prodex_runtime_state::runtime_lineage_key_is_bounded(key));
    let statuses = std::mem::take(&mut continuations.statuses);
    continuations.statuses =
        compact_runtime_continuation_statuses(statuses, &continuations, now, policy);
    continuations
}

pub fn merge_runtime_continuation_store(
    existing: &RuntimeContinuationStore<ResponseProfileBinding>,
    incoming: &RuntimeContinuationStore<ResponseProfileBinding>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> RuntimeContinuationStore<ResponseProfileBinding> {
    let response_profile_bindings = merge_runtime_hard_profile_bindings(
        &existing.response_profile_bindings,
        &incoming.response_profile_bindings,
    );
    let turn_state_bindings = merge_runtime_hard_profile_bindings(
        &existing.turn_state_bindings,
        &incoming.turn_state_bindings,
    );
    let session_id_bindings = merge_runtime_hard_profile_bindings(
        &existing.session_id_bindings,
        &incoming.session_id_bindings,
    );
    let statuses = merge_runtime_continuation_statuses(
        &existing.statuses,
        &incoming.statuses,
        &response_profile_bindings,
        &turn_state_bindings,
        &session_id_bindings,
        now,
        policy,
    );
    compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings,
            session_profile_bindings: merge_runtime_hard_profile_bindings(
                &existing.session_profile_bindings,
                &incoming.session_profile_bindings,
            ),
            turn_state_bindings: turn_state_bindings.clone(),
            session_id_bindings: session_id_bindings.clone(),
            statuses,
        },
        profiles,
        now,
        policy,
    )
}

fn mark_unbounded_profile_bindings_as_conflict(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
) {
    for binding in bindings.values_mut() {
        if !is_hard_binding_conflict_profile(&binding.profile_name)
            && !prodex_runtime_state::runtime_identity_component_is_valid(&binding.profile_name)
        {
            binding.profile_name = prodex_state::HARD_BINDING_CONFLICT_PROFILE.to_string();
            binding.binding_identity = None;
        }
    }
}

pub fn merge_runtime_hard_profile_bindings(
    existing: &BTreeMap<String, ResponseProfileBinding>,
    incoming: &BTreeMap<String, ResponseProfileBinding>,
) -> BTreeMap<String, ResponseProfileBinding> {
    let keys = existing
        .keys()
        .chain(incoming.keys())
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    keys.into_iter()
        .filter_map(|key| {
            let binding = match (existing.get(&key), incoming.get(&key)) {
                (Some(left), Some(right)) => {
                    prodex_state::merge_response_profile_binding(left, right)
                }
                (Some(binding), None) | (None, Some(binding)) => binding.clone(),
                (None, None) => return None,
            };
            prodex_runtime_state::runtime_lineage_key_is_bounded(&key).then_some((key, binding))
        })
        .collect()
}
