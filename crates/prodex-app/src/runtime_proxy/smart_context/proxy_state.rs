use super::{
    RuntimeRotationProxyShared, RuntimeSmartContextArtifactStore, RuntimeSmartContextProxyState,
    RuntimeSmartContextRewriteSafetyRecord, RuntimeSmartContextScopeConfig,
    RuntimeSmartContextTokenCalibrationObservation, RuntimeTokenUsage,
    SMART_CONTEXT_REWRITE_SAFETY_HISTORY_LIMIT, SMART_CONTEXT_REWRITE_SAFETY_TTL_SECS,
    SMART_CONTEXT_TOKEN_CALIBRATION_HISTORY_LIMIT, SMART_CONTEXT_TOKEN_USAGE_HISTORY_LIMIT,
    runtime_proxy_log, runtime_proxy_log_field, runtime_proxy_structured_log_message,
    runtime_smart_context_load_token_calibration_for_artifact_path,
    runtime_smart_context_token_calibration_path, runtime_smart_context_token_calibration_snapshot,
    schedule_runtime_smart_context_token_calibration_save,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::{Duration, SystemTime};

const SMART_CONTEXT_SCOPE_RETENTION: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const SMART_CONTEXT_GLOBAL_DISK_CAP_BYTES: u64 = 64 * 1024 * 1024;

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
    let Ok(runtime) = shared.runtime.lock() else {
        shared
            .smart_context_engine
            .enabled
            .store(false, Ordering::Relaxed);
        return;
    };
    let tenant = runtime.paths.root.to_string_lossy().into_owned();
    let provider = runtime.upstream_base_url.clone();
    let default_profile = runtime.current_profile.clone();
    let mut profiles = runtime
        .state
        .profiles
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    profiles.insert(default_profile.clone());
    drop(runtime);
    let workspace = std::env::current_dir()
        .and_then(std::fs::canonicalize)
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();

    if let Some(scopes_root) = artifact_path
        .as_deref()
        .filter(|path| {
            path.file_name().and_then(|name| name.to_str())
                == Some("runtime-smart-context-artifacts.json")
        })
        .and_then(Path::parent)
        .map(|parent| parent.join("smart-context").join("scopes"))
    {
        if let Ok(removed) = runtime_smart_context_prune_scope_stores(
            &scopes_root,
            SystemTime::now(),
            SMART_CONTEXT_SCOPE_RETENTION,
            SMART_CONTEXT_GLOBAL_DISK_CAP_BYTES,
        ) && removed > 0
        {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "smart_context_retention_pruned",
                    [runtime_proxy_log_field("stores", removed.to_string())],
                ),
            );
        }
    }

    if let Some(legacy_path) = artifact_path.as_deref().filter(|path| {
        path.file_name().and_then(|name| name.to_str())
            == Some("runtime-smart-context-artifacts.json")
            && path.exists()
    }) {
        let quarantined =
            runtime_smart_context_quarantine_artifact_path(legacy_path, "legacy-unscoped").is_ok();
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "smart_context_legacy_store_migration",
                [runtime_proxy_log_field(
                    "quarantined",
                    quarantined.to_string(),
                )],
            ),
        );
    }

    let mut profile_scopes = BTreeMap::new();
    let mut states = BTreeMap::new();
    for profile in profiles {
        let scope = runtime_proxy_crate::ContextScopeId::new(
            &tenant, &profile, &provider, &workspace, None,
        );
        let scoped_path = runtime_smart_context_artifact_path_for_scope(
            artifact_path.as_deref(),
            &scope,
            profile == default_profile,
        );
        states.insert(
            scope.clone(),
            runtime_smart_context_proxy_state_from_path(
                enabled,
                model_context_window_tokens,
                scoped_path,
                &scope,
            ),
        );
        profile_scopes.insert(profile, scope);
    }
    let Some(default_scope) = profile_scopes.get(&default_profile).cloned() else {
        return;
    };
    for (scope, state) in &states {
        if let Some(reason) = state.degraded_reason.as_deref() {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "smart_context_state_degraded",
                    [
                        runtime_proxy_log_field("scope", scope.to_string()),
                        runtime_proxy_log_field("reason", reason),
                    ],
                ),
            );
        }
    }
    if let Ok(mut current) = shared.smart_context_engine.states.write() {
        *current = states;
    }
    if let Ok(mut config) = shared.smart_context_engine.scope_config.write() {
        *config = Some(RuntimeSmartContextScopeConfig {
            default_scope,
            profile_scopes,
        });
    }
    shared
        .smart_context_engine
        .enabled
        .store(enabled, Ordering::Relaxed);
}

fn runtime_smart_context_artifact_path_for_scope(
    configured: Option<&Path>,
    scope: &runtime_proxy_crate::ContextScopeId,
    default_scope: bool,
) -> Option<PathBuf> {
    let configured = configured?;
    if configured.file_name().and_then(|name| name.to_str())
        != Some("runtime-smart-context-artifacts.json")
        && default_scope
    {
        return Some(configured.to_path_buf());
    }
    let parent = configured.parent()?;
    Some(
        parent
            .join("smart-context")
            .join("scopes")
            .join(scope.path_component())
            .join("artifacts.json"),
    )
}

fn runtime_smart_context_proxy_state_from_path(
    enabled: bool,
    model_context_window_tokens: Option<u64>,
    artifact_path: Option<PathBuf>,
    scope: &runtime_proxy_crate::ContextScopeId,
) -> RuntimeSmartContextProxyState {
    let (artifacts, degraded_reason) = match artifact_path.as_deref().filter(|_| enabled) {
        Some(path) => match RuntimeSmartContextArtifactStore::load_scoped_from_path(path, scope) {
            Ok(store) => (store, None),
            Err(_) => {
                let _ = runtime_smart_context_quarantine_artifact_path(path, "corrupt");
                (
                    RuntimeSmartContextArtifactStore::default(),
                    Some("artifact_store_corrupt".to_string()),
                )
            }
        },
        None => (RuntimeSmartContextArtifactStore::default(), None),
    };
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
    RuntimeSmartContextProxyState {
        generation: 0,
        enabled: enabled && degraded_reason.is_none(),
        degraded_reason,
        model_context_window_tokens,
        artifacts: std::sync::Arc::new(artifacts),
        artifact_path,
        last_token_usage: token_usage_history.last().copied(),
        token_usage_history,
        token_calibration_history,
        rewrite_telemetry_history: Vec::new(),
        rewrite_safety_history,
    }
}

fn runtime_smart_context_quarantine_artifact_path(
    path: &Path,
    reason: &str,
) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let timestamp = runtime_smart_context_unix_secs_now();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("artifacts.json");
    let quarantine = path.with_file_name(format!(
        "{file_name}.{reason}.{timestamp}.{}",
        std::process::id()
    ));
    std::fs::rename(path, quarantine)
}

pub(crate) fn runtime_smart_context_prune_scope_stores(
    scopes_root: &Path,
    now: SystemTime,
    retention: Duration,
    disk_cap_bytes: u64,
) -> std::io::Result<usize> {
    let entries = match std::fs::read_dir(scopes_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };
    let mut stores = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path().join("artifacts.json");
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if !metadata.file_type().is_file() {
            continue;
        }
        stores.push((
            path,
            metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            metadata.len(),
        ));
    }
    stores.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));
    let mut total_bytes = stores.iter().map(|entry| entry.2).sum::<u64>();
    let mut removed = 0usize;
    for (path, modified, bytes) in stores {
        let expired = now.duration_since(modified).unwrap_or_default() > retention;
        if !expired && total_bytes <= disk_cap_bytes {
            continue;
        }
        runtime_smart_context_remove_scope_store(&path)?;
        total_bytes = total_bytes.saturating_sub(bytes);
        removed = removed.saturating_add(1);
    }
    Ok(removed)
}

fn runtime_smart_context_remove_scope_store(path: &Path) -> std::io::Result<()> {
    for candidate in [
        path.to_path_buf(),
        runtime_smart_context_token_calibration_path(path),
        crate::runtime_store::json_lock_file_path(path),
    ] {
        match std::fs::remove_file(candidate) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir(parent);
    }
    Ok(())
}

pub(super) fn runtime_smart_context_proxy_state_snapshot(
    shared: &RuntimeRotationProxyShared,
) -> Option<(u64, RuntimeSmartContextProxyState)> {
    let scope = runtime_smart_context_scope_id(shared, None)?;
    runtime_smart_context_proxy_state_snapshot_for_scope(shared, &scope)
}

pub(super) fn runtime_smart_context_scope_id(
    shared: &RuntimeRotationProxyShared,
    profile_name: Option<&str>,
) -> Option<runtime_proxy_crate::ContextScopeId> {
    let config = shared.smart_context_engine.scope_config.read().ok()?;
    let config = config.as_ref()?;
    profile_name
        .and_then(|profile| config.profile_scopes.get(profile.trim()))
        .cloned()
        .or_else(|| Some(config.default_scope.clone()))
}

pub(super) fn runtime_smart_context_proxy_state_snapshot_for_scope(
    shared: &RuntimeRotationProxyShared,
    scope: &runtime_proxy_crate::ContextScopeId,
) -> Option<(u64, RuntimeSmartContextProxyState)> {
    let states = shared.smart_context_engine.states.read().ok()?;
    let state = states.get(scope)?;
    state.enabled.then(|| (state.generation, state.clone()))
}

pub(super) fn commit_runtime_smart_context_proxy_state_for_scope(
    shared: &RuntimeRotationProxyShared,
    scope: &runtime_proxy_crate::ContextScopeId,
    expected_generation: u64,
    mut planned: RuntimeSmartContextProxyState,
) -> bool {
    let Ok(mut states) = shared.smart_context_engine.states.write() else {
        return false;
    };
    let Some(current) = states.get_mut(scope) else {
        return false;
    };
    if !current.enabled || current.generation != expected_generation {
        return false;
    }
    planned.generation = expected_generation.saturating_add(1);
    *current = planned;
    true
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
    let Some(scope) = runtime_smart_context_scope_id(
        shared,
        bucket_key
            .as_ref()
            .and_then(|bucket| bucket.profile.as_deref()),
    ) else {
        return;
    };
    let Ok(mut states) = shared.smart_context_engine.states.write() else {
        return;
    };
    let mut save_job = None;
    if let Some(state) = states.get_mut(&scope)
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
    drop(states);
    if let Some((path, snapshot)) = save_job {
        schedule_runtime_smart_context_token_calibration_save(
            shared,
            path,
            snapshot,
            "smart_context_token_calibration",
        );
    }
}
