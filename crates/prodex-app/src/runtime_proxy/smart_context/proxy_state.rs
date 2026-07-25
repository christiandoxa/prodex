use super::{
    RuntimeRotationProxyShared, RuntimeSmartContextArtifactStore, RuntimeSmartContextEngine,
    RuntimeSmartContextProxyState, RuntimeSmartContextRepoStateFacts,
    RuntimeSmartContextRewriteSafetyRecord, RuntimeSmartContextTokenCalibrationObservation,
    RuntimeTokenUsage, SMART_CONTEXT_REWRITE_SAFETY_HISTORY_LIMIT,
    SMART_CONTEXT_REWRITE_SAFETY_TTL_SECS, SMART_CONTEXT_TOKEN_CALIBRATION_HISTORY_LIMIT,
    SMART_CONTEXT_TOKEN_USAGE_HISTORY_LIMIT,
    runtime_smart_context_artifact_alias_state_from_persisted,
    runtime_smart_context_load_token_calibration_for_artifact_path,
    runtime_smart_context_static_section_fingerprint_state_from_persisted,
    runtime_smart_context_token_calibration_path, runtime_smart_context_token_calibration_snapshot,
    schedule_runtime_smart_context_token_calibration_save,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub(crate) fn runtime_smart_context_unix_secs_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_rewrite_safety_record_fresh(
    record: RuntimeSmartContextRewriteSafetyRecord,
    now: u64,
) -> bool {
    record.observed_at_unix_secs == 0
        || now.saturating_sub(record.observed_at_unix_secs) <= SMART_CONTEXT_REWRITE_SAFETY_TTL_SECS
}

pub(crate) fn register_runtime_smart_context_proxy_state(
    shared: &RuntimeRotationProxyShared,
    enabled: bool,
    model_context_window_tokens: Option<u64>,
    artifact_path: Option<PathBuf>,
) {
    let artifacts = artifact_path
        .as_deref()
        .filter(|_| enabled)
        .map(RuntimeSmartContextArtifactStore::load_from_path)
        .unwrap_or_default();
    let durable_artifact_ids = artifacts.artifact_ids();
    let calibration = artifact_path
        .as_deref()
        .filter(|_| enabled)
        .map(runtime_smart_context_load_token_calibration_for_artifact_path)
        .unwrap_or_default();
    let token_usage_history = calibration
        .token_usage_history
        .into_iter()
        .map(RuntimeTokenUsage::from)
        .rev()
        .take(SMART_CONTEXT_TOKEN_USAGE_HISTORY_LIMIT)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    let token_calibration_history = calibration
        .token_calibration_history
        .into_iter()
        .map(RuntimeSmartContextTokenCalibrationObservation::from)
        .rev()
        .take(SMART_CONTEXT_TOKEN_CALIBRATION_HISTORY_LIMIT)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    let now = runtime_smart_context_unix_secs_now();
    let rewrite_safety_history = calibration
        .rewrite_safety_history
        .into_iter()
        .map(RuntimeSmartContextRewriteSafetyRecord::from)
        .filter(|record| runtime_smart_context_rewrite_safety_record_fresh(*record, now))
        .rev()
        .take(SMART_CONTEXT_REWRITE_SAFETY_HISTORY_LIMIT)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    let (artifact_aliases, next_artifact_alias_index) =
        runtime_smart_context_artifact_alias_state_from_persisted(calibration.artifact_aliases);
    let static_section_fingerprints =
        runtime_smart_context_static_section_fingerprint_state_from_persisted(
            calibration.static_section_fingerprints,
        );
    let state = RuntimeSmartContextProxyState {
        generation: 0,
        enabled,
        model_context_window_tokens,
        artifacts,
        durable_artifact_ids,
        artifact_path,
        last_token_usage: token_usage_history.last().copied(),
        token_usage_history,
        token_calibration_history,
        rewrite_telemetry_history: Vec::new(),
        rewrite_safety_history,
        last_static_context_fingerprints: Vec::new(),
        last_static_context_prompt_cache_hash: None,
        last_artifact_manifest_ids: BTreeSet::new(),
        last_artifact_manifest_emitted_at: None,
        artifact_aliases,
        next_artifact_alias_index,
        static_section_fingerprints,
        repo_state_facts: RuntimeSmartContextRepoStateFacts::default(),
    };
    if let Ok(mut current) = shared.smart_context_engine.state.lock() {
        *current = Some(state);
    }
}

pub(super) fn runtime_smart_context_proxy_state_snapshot(
    shared: &RuntimeRotationProxyShared,
) -> Option<(u64, RuntimeSmartContextProxyState)> {
    let state = shared.smart_context_engine.state.lock().ok()?;
    let state = state.as_ref()?;
    state.enabled.then(|| (state.generation, state.clone()))
}

pub(super) fn commit_runtime_smart_context_proxy_state(
    shared: &RuntimeRotationProxyShared,
    expected_generation: u64,
    mut planned: RuntimeSmartContextProxyState,
) -> bool {
    let Ok(mut state) = shared.smart_context_engine.state.lock() else {
        return false;
    };
    let Some(current) = state.as_mut() else {
        return false;
    };
    if !current.enabled || current.generation != expected_generation {
        return false;
    }
    planned.generation = expected_generation.saturating_add(1);
    *current = planned;
    true
}

pub(crate) fn mark_runtime_smart_context_artifacts_durable(
    engine: &RuntimeSmartContextEngine,
    artifact_path: &Path,
    submitted: &RuntimeSmartContextArtifactStore,
    persisted: &RuntimeSmartContextArtifactStore,
) {
    let Ok(mut current) = engine.state.lock() else {
        return;
    };
    let Some(state) = current.as_mut() else {
        return;
    };
    if state.artifact_path.as_deref() != Some(artifact_path) {
        return;
    }
    let ids = state
        .artifacts
        .apply_persisted_artifact_ordering(submitted, persisted);
    if ids.is_empty() {
        return;
    }
    state.durable_artifact_ids.extend(ids);
    state.generation = state.generation.saturating_add(1);
}

#[cfg(test)]
pub(crate) fn observe_runtime_smart_context_token_usage(
    shared: &RuntimeRotationProxyShared,
    usage: RuntimeTokenUsage,
) {
    observe_runtime_smart_context_token_usage_for_bucket(shared, usage, None);
}

pub(crate) fn observe_runtime_smart_context_token_usage_for_bucket(
    shared: &RuntimeRotationProxyShared,
    usage: RuntimeTokenUsage,
    bucket_key: Option<runtime_proxy_crate::SmartContextTokenCalibrationBucketKey>,
) {
    let Ok(mut current) = shared.smart_context_engine.state.lock() else {
        return;
    };
    let mut save_job = None;
    if let Some(state) = current.as_mut()
        && state.enabled
    {
        state.generation = state.generation.saturating_add(1);
        state.last_token_usage = Some(usage);
        state.token_usage_history.push(usage);
        if state.token_usage_history.len() > SMART_CONTEXT_TOKEN_USAGE_HISTORY_LIMIT {
            let overflow = state
                .token_usage_history
                .len()
                .saturating_sub(SMART_CONTEXT_TOKEN_USAGE_HISTORY_LIMIT);
            state.token_usage_history.drain(0..overflow);
        }
        if let Some(bucket_key) = bucket_key.clone() {
            state
                .token_calibration_history
                .push(RuntimeSmartContextTokenCalibrationObservation { bucket_key, usage });
            if state.token_calibration_history.len() > SMART_CONTEXT_TOKEN_CALIBRATION_HISTORY_LIMIT
            {
                let overflow = state
                    .token_calibration_history
                    .len()
                    .saturating_sub(SMART_CONTEXT_TOKEN_CALIBRATION_HISTORY_LIMIT);
                state.token_calibration_history.drain(0..overflow);
            }
        }
        save_job = state.artifact_path.as_deref().map(|artifact_path| {
            (
                runtime_smart_context_token_calibration_path(artifact_path),
                runtime_smart_context_token_calibration_snapshot(state),
            )
        });
    }
    drop(current);
    if let Some((path, snapshot)) = save_job {
        schedule_runtime_smart_context_token_calibration_save(
            shared,
            path,
            snapshot,
            "smart_context_token_calibration",
        );
    }
}
