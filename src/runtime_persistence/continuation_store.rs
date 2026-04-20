use super::*;

pub(crate) fn runtime_continuation_store_from_app_state(
    state: &AppState,
) -> RuntimeContinuationStore {
    RuntimeContinuationStore {
        response_profile_bindings: runtime_external_response_profile_bindings(
            &state.response_profile_bindings,
        ),
        session_profile_bindings: state.session_profile_bindings.clone(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: runtime_external_session_id_bindings(&state.session_profile_bindings),
        statuses: RuntimeContinuationStatuses::default(),
    }
}

pub(crate) fn compact_runtime_continuation_store(
    mut continuations: RuntimeContinuationStore,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> RuntimeContinuationStore {
    let now = Local::now().timestamp();
    prune_profile_bindings_for_housekeeping_without_retention(
        &mut continuations.response_profile_bindings,
        profiles,
    );
    prune_profile_bindings_for_housekeeping_without_retention(
        &mut continuations.session_profile_bindings,
        profiles,
    );
    prune_profile_bindings_for_housekeeping_without_retention(
        &mut continuations.turn_state_bindings,
        profiles,
    );
    prune_profile_bindings_for_housekeeping_without_retention(
        &mut continuations.session_id_bindings,
        profiles,
    );
    continuations
        .response_profile_bindings
        .retain(|key, binding| {
            runtime_continuation_binding_should_retain(
                binding,
                continuations.statuses.response.get(key),
                now,
            )
        });
    let response_turn_state_keys = continuations
        .response_profile_bindings
        .keys()
        .filter(|key| runtime_is_response_turn_state_lineage_key(key))
        .cloned()
        .collect::<Vec<_>>();
    for key in response_turn_state_keys {
        let Some((response_id, _)) = runtime_response_turn_state_lineage_parts(&key) else {
            continuations.response_profile_bindings.remove(&key);
            continue;
        };
        if continuations
            .response_profile_bindings
            .get(response_id)
            .is_none_or(|binding| !profiles.contains_key(&binding.profile_name))
        {
            continuations.response_profile_bindings.remove(&key);
        }
    }
    continuations.turn_state_bindings.retain(|key, binding| {
        runtime_continuation_binding_should_retain(
            binding,
            continuations.statuses.turn_state.get(key),
            now,
        )
    });
    continuations
        .session_profile_bindings
        .retain(|key, binding| {
            runtime_continuation_binding_should_retain(
                binding,
                continuations.statuses.session_id.get(key),
                now,
            )
        });
    continuations.session_id_bindings.retain(|key, binding| {
        runtime_continuation_binding_should_retain(
            binding,
            continuations.statuses.session_id.get(key),
            now,
        )
    });
    prune_runtime_continuation_response_bindings(
        &mut continuations.response_profile_bindings,
        &continuations.statuses.response,
        RESPONSE_PROFILE_BINDING_LIMIT,
    );
    prune_profile_bindings(
        &mut continuations.turn_state_bindings,
        TURN_STATE_PROFILE_BINDING_LIMIT,
    );
    prune_profile_bindings(
        &mut continuations.session_profile_bindings,
        SESSION_ID_PROFILE_BINDING_LIMIT,
    );
    prune_profile_bindings(
        &mut continuations.session_id_bindings,
        SESSION_ID_PROFILE_BINDING_LIMIT,
    );
    let statuses = std::mem::take(&mut continuations.statuses);
    continuations.statuses = compact_runtime_continuation_statuses(statuses, &continuations);
    continuations
}

pub(crate) fn merge_runtime_continuation_store(
    existing: &RuntimeContinuationStore,
    incoming: &RuntimeContinuationStore,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> RuntimeContinuationStore {
    compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings: merge_profile_bindings(
                &existing.response_profile_bindings,
                &incoming.response_profile_bindings,
                profiles,
            ),
            session_profile_bindings: merge_profile_bindings(
                &existing.session_profile_bindings,
                &incoming.session_profile_bindings,
                profiles,
            ),
            turn_state_bindings: merge_profile_bindings(
                &existing.turn_state_bindings,
                &incoming.turn_state_bindings,
                profiles,
            ),
            session_id_bindings: merge_profile_bindings(
                &existing.session_id_bindings,
                &incoming.session_id_bindings,
                profiles,
            ),
            statuses: merge_runtime_continuation_statuses(
                &existing.statuses,
                &incoming.statuses,
                &merge_profile_bindings(
                    &existing.response_profile_bindings,
                    &incoming.response_profile_bindings,
                    profiles,
                ),
                &merge_profile_bindings(
                    &existing.turn_state_bindings,
                    &incoming.turn_state_bindings,
                    profiles,
                ),
                &merge_profile_bindings(
                    &existing.session_id_bindings,
                    &incoming.session_id_bindings,
                    profiles,
                ),
            ),
        },
        profiles,
    )
}

pub(crate) fn load_runtime_continuations_with_recovery(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<RecoveredLoad<RuntimeContinuationStore>> {
    let path = runtime_continuations_file_path(paths);
    if !path.exists() && !runtime_continuations_last_good_file_path(paths).exists() {
        return Ok(RecoveredLoad {
            value: RuntimeContinuationStore::default(),
            recovered_from_backup: false,
        });
    }
    let loaded = read_versioned_json_file_with_backup::<RuntimeContinuationStore>(
        &path,
        &runtime_continuations_last_good_file_path(paths),
    )?;
    remember_runtime_sidecar_generation(&path, loaded.generation);
    Ok(RecoveredLoad {
        value: compact_runtime_continuation_store(loaded.value, profiles),
        recovered_from_backup: loaded.recovered_from_backup,
    })
}

pub(crate) fn save_runtime_continuations_for_profiles(
    paths: &AppPaths,
    continuations: &RuntimeContinuationStore,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<()> {
    if runtime_take_fault_injection("PRODEX_RUNTIME_FAULT_CONTINUATIONS_SAVE_ERROR_ONCE") {
        bail!("injected runtime continuations save failure");
    }
    let path = runtime_continuations_file_path(paths);
    let compacted = compact_runtime_continuation_store(continuations.clone(), profiles);
    save_versioned_json_file_with_fence(
        &path,
        &runtime_continuations_last_good_file_path(paths),
        &compacted,
    )?;
    Ok(())
}

pub(crate) fn load_runtime_continuation_journal_with_recovery(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<RecoveredLoad<RuntimeContinuationJournal>> {
    let path = runtime_continuation_journal_file_path(paths);
    if !path.exists() && !runtime_continuation_journal_last_good_file_path(paths).exists() {
        return Ok(RecoveredLoad {
            value: RuntimeContinuationJournal::default(),
            recovered_from_backup: false,
        });
    }
    let loaded = read_versioned_json_file_with_backup::<RuntimeContinuationJournal>(
        &path,
        &runtime_continuation_journal_last_good_file_path(paths),
    )?;
    remember_runtime_sidecar_generation(&path, loaded.generation);
    Ok(RecoveredLoad {
        value: RuntimeContinuationJournal {
            saved_at: loaded.value.saved_at,
            continuations: compact_runtime_continuation_store(loaded.value.continuations, profiles),
        },
        recovered_from_backup: loaded.recovered_from_backup,
    })
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn save_runtime_continuation_journal(
    paths: &AppPaths,
    continuations: &RuntimeContinuationStore,
    saved_at: i64,
) -> Result<()> {
    let profiles = AppState::load(paths)
        .map(|state| state.profiles)
        .unwrap_or_default();
    save_runtime_continuation_journal_for_profiles(paths, continuations, &profiles, saved_at)
}

pub(crate) fn save_runtime_continuation_journal_for_profiles(
    paths: &AppPaths,
    continuations: &RuntimeContinuationStore,
    profiles: &BTreeMap<String, ProfileEntry>,
    saved_at: i64,
) -> Result<()> {
    let incoming = compact_runtime_continuation_store(continuations.clone(), profiles);
    for attempt in 0..=RUNTIME_SIDECAR_STALE_SAVE_RETRY_LIMIT {
        let existing = load_runtime_continuation_journal_with_recovery(paths, profiles)?;
        let journal = RuntimeContinuationJournal {
            saved_at: saved_at.max(existing.value.saved_at),
            continuations: merge_runtime_continuation_store(
                &existing.value.continuations,
                &incoming,
                profiles,
            ),
        };
        match save_versioned_json_file_with_fence(
            &runtime_continuation_journal_file_path(paths),
            &runtime_continuation_journal_last_good_file_path(paths),
            &journal,
        ) {
            Ok(()) => return Ok(()),
            Err(err)
                if runtime_sidecar_generation_error_is_stale(&err)
                    && attempt < RUNTIME_SIDECAR_STALE_SAVE_RETRY_LIMIT =>
            {
                continue;
            }
            Err(err) => return Err(err),
        }
    }
    Ok(())
}
