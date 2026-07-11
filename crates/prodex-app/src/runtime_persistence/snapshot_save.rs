use super::*;

struct PreparedRuntimeStateSnapshotSave {
    state: AppState,
    continuations: RuntimeContinuationStore,
    json: String,
    profile_scores: BTreeMap<String, RuntimeProfileHealth>,
    usage_snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot>,
    backoffs: RuntimeProfileBackoffs,
}

struct PreparedRuntimeStateSelectedSnapshotSave {
    profiles: BTreeMap<String, ProfileEntry>,
    continuations: Option<RuntimeContinuationStore>,
    json: Option<String>,
    profile_scores: Option<BTreeMap<String, RuntimeProfileHealth>>,
    usage_snapshots: Option<BTreeMap<String, RuntimeProfileUsageSnapshot>>,
    backoffs: Option<RuntimeProfileBackoffs>,
}

pub(crate) struct RuntimeStateSnapshotSaveInput<'a> {
    pub(crate) paths: &'a AppPaths,
    pub(crate) snapshot: &'a AppState,
    pub(crate) continuations: &'a RuntimeContinuationStore,
    pub(crate) profile_scores: &'a BTreeMap<String, RuntimeProfileHealth>,
    pub(crate) usage_snapshots: &'a BTreeMap<String, RuntimeProfileUsageSnapshot>,
    pub(crate) backoffs: &'a RuntimeProfileBackoffs,
    pub(crate) revision: u64,
    pub(crate) latest_revision: &'a AtomicU64,
}

pub(crate) fn save_runtime_state_snapshot_if_latest(
    input: RuntimeStateSnapshotSaveInput<'_>,
) -> Result<bool> {
    for attempt in 0..=RUNTIME_SIDECAR_STALE_SAVE_RETRY_LIMIT {
        if !prodex_runtime_state::runtime_state_snapshot_is_latest_revision(
            input.latest_revision,
            input.revision,
        ) {
            return Ok(false);
        }

        let _lock = acquire_state_file_lock(input.paths)?;

        if !prodex_runtime_state::runtime_state_snapshot_is_latest_revision(
            input.latest_revision,
            input.revision,
        ) {
            return Ok(false);
        }

        let prepared = prepare_runtime_state_snapshot_save(
            input.paths,
            input.snapshot,
            input.continuations,
            input.profile_scores,
            input.usage_snapshots,
            input.backoffs,
        )?;

        if !prodex_runtime_state::runtime_state_snapshot_is_latest_revision(
            input.latest_revision,
            input.revision,
        ) {
            return Ok(false);
        }

        match persist_runtime_state_snapshot_save(input.paths, &prepared) {
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

pub(crate) fn save_runtime_state_selected_snapshot_if_latest(
    snapshot: &RuntimeStateSaveSelectedSnapshot,
    revision: u64,
    latest_revision: &AtomicU64,
) -> Result<bool> {
    for attempt in 0..=RUNTIME_SIDECAR_STALE_SAVE_RETRY_LIMIT {
        if !prodex_runtime_state::runtime_state_snapshot_is_latest_revision(
            latest_revision,
            revision,
        ) {
            return Ok(false);
        }

        let _lock = acquire_state_file_lock(&snapshot.paths)?;

        if !prodex_runtime_state::runtime_state_snapshot_is_latest_revision(
            latest_revision,
            revision,
        ) {
            return Ok(false);
        }

        let prepared = prepare_runtime_state_selected_snapshot_save(snapshot)?;

        if !prodex_runtime_state::runtime_state_snapshot_is_latest_revision(
            latest_revision,
            revision,
        ) {
            return Ok(false);
        }

        match persist_runtime_state_selected_snapshot_save(&snapshot.paths, &prepared) {
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

fn prepare_runtime_state_selected_snapshot_save(
    snapshot: &RuntimeStateSaveSelectedSnapshot,
) -> Result<PreparedRuntimeStateSelectedSnapshotSave> {
    let now = Local::now().timestamp();
    let state_policy = app_state_compaction_policy();
    let continuation_policy = runtime_continuation_compaction_policy();
    let prepare_plan = prodex_runtime_store::runtime_state_selected_snapshot_prepare_plan(snapshot);
    let existing_state = if prepare_plan.load.needs_existing_state {
        Some(AppState::load(&snapshot.paths)?)
    } else {
        None
    };
    let profiles_for_continuation_load =
        prodex_runtime_store::runtime_state_selected_snapshot_profiles_for_merge(
            snapshot,
            existing_state.as_ref(),
            now,
            state_policy,
        )
        .context("invalid runtime state selected snapshot merge plan")?;
    let existing_continuations = if prepare_plan.load.needs_existing_continuations {
        Some(
            load_runtime_continuations_with_recovery(
                &snapshot.paths,
                &profiles_for_continuation_load,
            )?
            .value,
        )
    } else {
        None
    };
    let merged = prodex_runtime_store::merge_runtime_state_selected_snapshot_sections(
        snapshot,
        existing_state.as_ref(),
        existing_continuations.as_ref(),
        now,
        state_policy,
        continuation_policy,
    )
    .context("invalid runtime state selected snapshot merge plan")?;

    let json = if prepare_plan.writes_state {
        Some(serialize_runtime_state_snapshot_for_save(
            merged
                .state
                .as_ref()
                .context("selected snapshot state section missing after merge")?,
        )?)
    } else {
        None
    };
    let profile_scores = if prepare_plan.writes_profile_scores {
        Some(merge_runtime_profile_scores_for_save(
            &snapshot.paths,
            &merged.profiles,
            snapshot
                .profile_scores
                .as_ref()
                .context("selected snapshot profile scores section missing")?,
        )?)
    } else {
        None
    };
    let usage_snapshots = if prepare_plan.writes_usage_snapshots {
        Some(merge_runtime_usage_snapshots_for_save(
            &snapshot.paths,
            &merged.profiles,
            snapshot
                .usage_snapshots
                .as_ref()
                .context("selected snapshot usage snapshots section missing")?,
        )?)
    } else {
        None
    };
    let backoffs = if prepare_plan.writes_backoffs {
        Some(merge_runtime_backoffs_for_save(
            &snapshot.paths,
            &merged.profiles,
            snapshot
                .backoffs
                .as_ref()
                .context("selected snapshot backoffs section missing")?,
        )?)
    } else {
        None
    };

    Ok(PreparedRuntimeStateSelectedSnapshotSave {
        profiles: merged.profiles,
        continuations: merged.continuations,
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
    let now = Local::now().timestamp();
    let state_for_profiles = prodex_runtime_store::merge_runtime_state_snapshot_for_save(
        existing.clone(),
        snapshot,
        now,
        app_state_compaction_policy(),
    );
    let existing_continuations =
        load_runtime_continuations_with_recovery(paths, &state_for_profiles.profiles)?;
    let merged = prodex_runtime_store::merge_runtime_state_and_continuations_for_save(
        existing,
        snapshot,
        &existing_continuations.value,
        continuations,
        now,
        app_state_compaction_policy(),
        runtime_continuation_compaction_policy(),
    );
    Ok((merged.state, merged.continuations))
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

fn persist_runtime_state_selected_snapshot_save(
    paths: &AppPaths,
    prepared: &PreparedRuntimeStateSelectedSnapshotSave,
) -> Result<()> {
    if let Some(continuations) = prepared.continuations.as_ref() {
        save_runtime_continuations_for_profiles(paths, continuations, &prepared.profiles)?;
    }
    if let Some(json) = prepared.json.as_ref() {
        write_state_json_atomic(paths, json)?;
    }
    if let Some(profile_scores) = prepared.profile_scores.as_ref() {
        save_runtime_profile_scores_for_profiles(paths, profile_scores, &prepared.profiles)?;
    }
    if let Some(usage_snapshots) = prepared.usage_snapshots.as_ref() {
        save_runtime_usage_snapshots_for_profiles(paths, usage_snapshots, &prepared.profiles)?;
    }
    if let Some(backoffs) = prepared.backoffs.as_ref() {
        save_runtime_profile_backoffs_for_profiles(paths, backoffs, &prepared.profiles)?;
    }
    Ok(())
}
