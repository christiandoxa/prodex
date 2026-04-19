use super::*;

struct PreparedRuntimeStateSnapshotSave {
    state: AppState,
    continuations: RuntimeContinuationStore,
    json: String,
    profile_scores: BTreeMap<String, RuntimeProfileHealth>,
    usage_snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot>,
    backoffs: RuntimeProfileBackoffs,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn save_runtime_state_snapshot_if_latest(
    paths: &AppPaths,
    snapshot: &AppState,
    continuations: &RuntimeContinuationStore,
    profile_scores: &BTreeMap<String, RuntimeProfileHealth>,
    usage_snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    backoffs: &RuntimeProfileBackoffs,
    revision: u64,
    latest_revision: &AtomicU64,
) -> Result<bool> {
    for attempt in 0..=RUNTIME_SIDECAR_STALE_SAVE_RETRY_LIMIT {
        if !runtime_state_snapshot_is_latest_revision(latest_revision, revision) {
            return Ok(false);
        }

        let _lock = acquire_state_file_lock(paths)?;

        if !runtime_state_snapshot_is_latest_revision(latest_revision, revision) {
            return Ok(false);
        }

        let prepared = prepare_runtime_state_snapshot_save(
            paths,
            snapshot,
            continuations,
            profile_scores,
            usage_snapshots,
            backoffs,
        )?;

        if !runtime_state_snapshot_is_latest_revision(latest_revision, revision) {
            return Ok(false);
        }

        match persist_runtime_state_snapshot_save(paths, &prepared) {
            Ok(()) => return Ok(true),
            Err(err)
                if runtime_sidecar_generation_error_is_stale(&err)
                    && attempt < RUNTIME_SIDECAR_STALE_SAVE_RETRY_LIMIT =>
            {
                continue;
            }
            Err(err) => return Err(err),
        }
    }
    Ok(false)
}

fn runtime_state_snapshot_is_latest_revision(latest_revision: &AtomicU64, revision: u64) -> bool {
    latest_revision.load(Ordering::SeqCst) == revision
}

fn prepare_runtime_state_snapshot_save(
    paths: &AppPaths,
    snapshot: &AppState,
    continuations: &RuntimeContinuationStore,
    profile_scores: &BTreeMap<String, RuntimeProfileHealth>,
    usage_snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    backoffs: &RuntimeProfileBackoffs,
) -> Result<PreparedRuntimeStateSnapshotSave> {
    let (state, continuations) =
        merge_runtime_state_and_continuations_for_save(paths, snapshot, continuations)?;
    let json = serialize_runtime_state_snapshot_for_save(&state)?;
    let profile_scores =
        merge_runtime_profile_scores_for_save(paths, &state.profiles, profile_scores)?;
    let usage_snapshots =
        merge_runtime_usage_snapshots_for_save(paths, &state.profiles, usage_snapshots)?;
    let backoffs = merge_runtime_backoffs_for_save(paths, &state.profiles, backoffs)?;

    Ok(PreparedRuntimeStateSnapshotSave {
        state,
        continuations,
        json,
        profile_scores,
        usage_snapshots,
        backoffs,
    })
}

fn merge_runtime_state_and_continuations_for_save(
    paths: &AppPaths,
    snapshot: &AppState,
    continuations: &RuntimeContinuationStore,
) -> Result<(AppState, RuntimeContinuationStore)> {
    let existing = AppState::load(paths)?;
    let mut state = merge_runtime_state_snapshot(existing, snapshot);
    let existing_continuations = load_runtime_continuations_with_recovery(paths, &state.profiles)?;
    let continuations = merge_runtime_continuation_store(
        &existing_continuations.value,
        continuations,
        &state.profiles,
    );
    state.response_profile_bindings =
        runtime_external_response_profile_bindings(&continuations.response_profile_bindings);
    state.session_profile_bindings = continuations.session_profile_bindings.clone();
    Ok((state, continuations))
}

fn serialize_runtime_state_snapshot_for_save(state: &AppState) -> Result<String> {
    serde_json::to_string_pretty(state).context("failed to serialize prodex state")
}

fn merge_runtime_profile_scores_for_save(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
    desired_scores: &BTreeMap<String, RuntimeProfileHealth>,
) -> Result<BTreeMap<String, RuntimeProfileHealth>> {
    let existing_scores = load_runtime_profile_scores(paths, profiles)?;
    Ok(merge_runtime_profile_scores(
        &existing_scores,
        desired_scores,
        profiles,
    ))
}

fn merge_runtime_usage_snapshots_for_save(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
    desired_snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
) -> Result<BTreeMap<String, RuntimeProfileUsageSnapshot>> {
    let existing_usage_snapshots = load_runtime_usage_snapshots(paths, profiles)?;
    Ok(merge_runtime_usage_snapshots(
        &existing_usage_snapshots,
        desired_snapshots,
        profiles,
    ))
}

fn merge_runtime_backoffs_for_save(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
    desired_backoffs: &RuntimeProfileBackoffs,
) -> Result<RuntimeProfileBackoffs> {
    let existing_backoffs = load_runtime_profile_backoffs(paths, profiles)?;
    Ok(merge_runtime_profile_backoffs(
        &existing_backoffs,
        desired_backoffs,
        profiles,
        Local::now().timestamp(),
    ))
}

fn persist_runtime_state_snapshot_save(
    paths: &AppPaths,
    prepared: &PreparedRuntimeStateSnapshotSave,
) -> Result<()> {
    // Continuations are restored as the stronger source of truth on startup,
    // so persist them before the state snapshot to reduce crash windows where
    // a newer state file could be overwritten by an older continuation sidecar.
    save_runtime_continuations_for_profiles(
        paths,
        &prepared.continuations,
        &prepared.state.profiles,
    )?;
    write_state_json_atomic(paths, &prepared.json)?;
    save_runtime_profile_scores_for_profiles(
        paths,
        &prepared.profile_scores,
        &prepared.state.profiles,
    )?;
    save_runtime_usage_snapshots_for_profiles(
        paths,
        &prepared.usage_snapshots,
        &prepared.state.profiles,
    )?;
    save_runtime_profile_backoffs_for_profiles(paths, &prepared.backoffs, &prepared.state.profiles)
}
