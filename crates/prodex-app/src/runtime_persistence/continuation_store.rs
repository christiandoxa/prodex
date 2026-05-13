use super::{
    AppState, AppStateIoExt, ProfileEntry, RUNTIME_SIDECAR_STALE_SAVE_RETRY_LIMIT, RecoveredLoad,
    RuntimeContinuationJournal, RuntimeContinuationStore, read_versioned_json_file_with_backup,
    remember_runtime_sidecar_generation, runtime_continuation_compaction_policy,
    runtime_continuation_journal_file_path, runtime_continuation_journal_last_good_file_path,
    runtime_continuations_file_path, runtime_continuations_last_good_file_path,
    runtime_sidecar_generation_error_is_stale, runtime_take_fault_injection,
    save_versioned_json_file_with_fence,
};
use anyhow::{Result, bail};
use chrono::Local;
use prodex_core::AppPaths;
use std::collections::BTreeMap;

pub(crate) fn runtime_continuation_store_from_app_state(
    state: &AppState,
) -> RuntimeContinuationStore {
    prodex_runtime_store::runtime_continuation_store_from_app_state(state)
}

pub(crate) fn compact_runtime_continuation_store(
    continuations: RuntimeContinuationStore,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> RuntimeContinuationStore {
    prodex_runtime_store::compact_runtime_continuation_store(
        continuations,
        profiles,
        Local::now().timestamp(),
        runtime_continuation_compaction_policy(),
    )
}

pub(crate) fn merge_runtime_continuation_store(
    existing: &RuntimeContinuationStore,
    incoming: &RuntimeContinuationStore,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> RuntimeContinuationStore {
    prodex_runtime_store::merge_runtime_continuation_store(
        existing,
        incoming,
        profiles,
        Local::now().timestamp(),
        runtime_continuation_compaction_policy(),
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
