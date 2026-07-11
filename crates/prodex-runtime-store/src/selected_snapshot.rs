use super::*;

pub fn runtime_state_save_selected_snapshot_from_parts<P, C, H, U, B>(
    paths: &P,
    state: &AppState,
    continuations: &C,
    profile_scores: &BTreeMap<String, H>,
    usage_snapshots: &BTreeMap<String, U>,
    backoffs: &B,
    sections: RuntimeStateSaveSections,
) -> RuntimeStateSaveSelectedSnapshot<P, AppState, ProfileEntry, C, H, U, B>
where
    P: Clone,
    C: Clone,
    H: Clone,
    U: Clone,
    B: Clone,
{
    let state_snapshot = match sections.state {
        RuntimeStateSaveStateSection::None => None,
        RuntimeStateSaveStateSection::Core => Some(AppState {
            active_profile: state.active_profile.clone(),
            profiles: state.profiles.clone(),
            last_run_selected_at: state.last_run_selected_at.clone(),
            response_profile_bindings: BTreeMap::new(),
            session_profile_bindings: BTreeMap::new(),
        }),
        RuntimeStateSaveStateSection::Full => Some(state.clone()),
    };
    let profiles = state_snapshot.is_none().then(|| state.profiles.clone());
    RuntimeStateSaveSelectedSnapshot {
        paths: paths.clone(),
        state: state_snapshot,
        profiles,
        continuations: sections.continuations.then(|| continuations.clone()),
        profile_scores: sections.profile_scores.then(|| profile_scores.clone()),
        usage_snapshots: sections.usage_snapshots.then(|| usage_snapshots.clone()),
        backoffs: sections.backoffs.then(|| backoffs.clone()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeStateSelectedSnapshotLoadPlan {
    pub needs_existing_state: bool,
    pub needs_existing_continuations: bool,
}

pub fn runtime_state_selected_snapshot_load_plan<P, S, E, C, H, U, B>(
    snapshot: &RuntimeStateSaveSelectedSnapshot<P, S, E, C, H, U, B>,
) -> RuntimeStateSelectedSnapshotLoadPlan {
    RuntimeStateSelectedSnapshotLoadPlan {
        needs_existing_state: snapshot.state.is_some() || snapshot.profiles.is_none(),
        needs_existing_continuations: snapshot.continuations.is_some(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeStateSelectedSnapshotPreparePlan {
    pub load: RuntimeStateSelectedSnapshotLoadPlan,
    pub writes_state: bool,
    pub writes_continuations: bool,
    pub writes_profile_scores: bool,
    pub writes_usage_snapshots: bool,
    pub writes_backoffs: bool,
}

pub fn runtime_state_selected_snapshot_prepare_plan<P, S, E, C, H, U, B>(
    snapshot: &RuntimeStateSaveSelectedSnapshot<P, S, E, C, H, U, B>,
) -> RuntimeStateSelectedSnapshotPreparePlan {
    RuntimeStateSelectedSnapshotPreparePlan {
        load: runtime_state_selected_snapshot_load_plan(snapshot),
        writes_state: snapshot.state.is_some(),
        writes_continuations: snapshot.continuations.is_some(),
        writes_profile_scores: snapshot.profile_scores.is_some(),
        writes_usage_snapshots: snapshot.usage_snapshots.is_some(),
        writes_backoffs: snapshot.backoffs.is_some(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStateSelectedSnapshotMergedSections {
    pub profiles: BTreeMap<String, ProfileEntry>,
    pub state: Option<AppState>,
    pub continuations: Option<RuntimeContinuationStore<ResponseProfileBinding>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeStateSelectedSnapshotMergeError {
    MissingExistingState,
    MissingProfiles,
    MissingExistingContinuations,
}

impl std::fmt::Display for RuntimeStateSelectedSnapshotMergeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingExistingState => formatter.write_str("missing existing state"),
            Self::MissingProfiles => formatter.write_str("missing profiles"),
            Self::MissingExistingContinuations => {
                formatter.write_str("missing existing continuations")
            }
        }
    }
}

impl std::error::Error for RuntimeStateSelectedSnapshotMergeError {}

pub fn runtime_state_selected_snapshot_profiles_for_merge<P, H, U, B>(
    snapshot: &RuntimeStateSaveSelectedSnapshot<
        P,
        AppState,
        ProfileEntry,
        RuntimeContinuationStore<ResponseProfileBinding>,
        H,
        U,
        B,
    >,
    existing_state: Option<&AppState>,
    now: i64,
    state_policy: AppStateCompactionPolicy,
) -> Result<BTreeMap<String, ProfileEntry>, RuntimeStateSelectedSnapshotMergeError> {
    if let Some(state_snapshot) = snapshot.state.as_ref() {
        let existing_state =
            existing_state.ok_or(RuntimeStateSelectedSnapshotMergeError::MissingExistingState)?;
        return Ok(merge_runtime_state_snapshot_for_save(
            existing_state.clone(),
            state_snapshot,
            now,
            state_policy,
        )
        .profiles);
    }

    snapshot
        .profiles
        .clone()
        .or_else(|| existing_state.map(|state| state.profiles.clone()))
        .ok_or(RuntimeStateSelectedSnapshotMergeError::MissingProfiles)
}

pub fn merge_runtime_state_selected_snapshot_sections<P, H, U, B>(
    snapshot: &RuntimeStateSaveSelectedSnapshot<
        P,
        AppState,
        ProfileEntry,
        RuntimeContinuationStore<ResponseProfileBinding>,
        H,
        U,
        B,
    >,
    existing_state: Option<&AppState>,
    existing_continuations: Option<&RuntimeContinuationStore<ResponseProfileBinding>>,
    now: i64,
    state_policy: AppStateCompactionPolicy,
    continuation_policy: RuntimeContinuationCompactionPolicy,
) -> Result<RuntimeStateSelectedSnapshotMergedSections, RuntimeStateSelectedSnapshotMergeError> {
    let mut state = None;
    let mut continuations = None;
    let profiles = if let Some(state_snapshot) = snapshot.state.as_ref() {
        let existing_state =
            existing_state.ok_or(RuntimeStateSelectedSnapshotMergeError::MissingExistingState)?;
        if let Some(continuation_snapshot) = snapshot.continuations.as_ref() {
            let existing_continuations = existing_continuations
                .ok_or(RuntimeStateSelectedSnapshotMergeError::MissingExistingContinuations)?;
            let merged = merge_runtime_state_and_continuations_for_save(
                existing_state.clone(),
                state_snapshot,
                existing_continuations,
                continuation_snapshot,
                now,
                state_policy,
                continuation_policy,
            );
            let profiles = merged.state.profiles.clone();
            state = Some(merged.state);
            continuations = Some(merged.continuations);
            profiles
        } else {
            let merged_state = merge_runtime_state_snapshot_for_save(
                existing_state.clone(),
                state_snapshot,
                now,
                state_policy,
            );
            let profiles = merged_state.profiles.clone();
            state = Some(merged_state);
            profiles
        }
    } else {
        let profiles = snapshot
            .profiles
            .clone()
            .or_else(|| existing_state.map(|state| state.profiles.clone()))
            .ok_or(RuntimeStateSelectedSnapshotMergeError::MissingProfiles)?;
        if let Some(continuation_snapshot) = snapshot.continuations.as_ref() {
            let existing_continuations = existing_continuations
                .ok_or(RuntimeStateSelectedSnapshotMergeError::MissingExistingContinuations)?;
            continuations = Some(merge_runtime_continuation_store(
                existing_continuations,
                continuation_snapshot,
                &profiles,
                now,
                continuation_policy,
            ));
        }
        profiles
    };

    Ok(RuntimeStateSelectedSnapshotMergedSections {
        profiles,
        state,
        continuations,
    })
}
