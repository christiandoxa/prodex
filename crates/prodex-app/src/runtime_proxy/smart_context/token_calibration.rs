use super::*;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextTokenCalibrationObservation {
    pub(super) bucket_key: runtime_proxy_crate::SmartContextTokenCalibrationBucketKey,
    pub(super) usage: RuntimeTokenUsage,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct RuntimeSmartContextPersistedTokenCalibration {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    pub(super) token_usage_history: Vec<RuntimeSmartContextPersistedTokenUsage>,
    #[serde(default)]
    pub(super) token_calibration_history:
        Vec<RuntimeSmartContextPersistedTokenCalibrationObservation>,
    #[serde(default)]
    pub(super) rewrite_safety_history: Vec<RuntimeSmartContextPersistedRewriteSafetyObservation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) artifact_aliases: Vec<RuntimeSmartContextPersistedArtifactAlias>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) static_section_fingerprints:
        Vec<RuntimeSmartContextPersistedStaticSectionFingerprint>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct RuntimeSmartContextPersistedTokenUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    cached_input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    reasoning_tokens: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct RuntimeSmartContextPersistedTokenCalibrationObservation {
    #[serde(default)]
    bucket_key: RuntimeSmartContextPersistedTokenCalibrationBucketKey,
    #[serde(default)]
    usage: RuntimeSmartContextPersistedTokenUsage,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct RuntimeSmartContextPersistedRewriteSafetyObservation {
    #[serde(default)]
    safe: bool,
    #[serde(default)]
    saved_tokens: u64,
    #[serde(default)]
    observed_at_unix_secs: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct RuntimeSmartContextPersistedArtifactAlias {
    #[serde(default)]
    pub(super) id: String,
    #[serde(default)]
    pub(super) alias: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct RuntimeSmartContextPersistedStaticSectionFingerprint {
    #[serde(default)]
    item_id: String,
    #[serde(default)]
    heading: String,
    #[serde(default)]
    ordinal: usize,
    #[serde(default)]
    content_hash: String,
    #[serde(default)]
    byte_len: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct RuntimeSmartContextPersistedTokenCalibrationBucketKey {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    route: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    transport: Option<String>,
}

#[derive(Debug, Clone)]
struct RuntimeSmartContextTokenCalibrationSaveJob {
    path: PathBuf,
    snapshot: RuntimeSmartContextPersistedTokenCalibration,
    log_path: PathBuf,
    reason: String,
    queued_at: Instant,
    ready_at: Instant,
}

struct RuntimeSmartContextTokenCalibrationSaveQueue {
    pending: Mutex<BTreeMap<PathBuf, RuntimeSmartContextTokenCalibrationSaveJob>>,
    wake: Condvar,
}

static RUNTIME_SMART_CONTEXT_TOKEN_CALIBRATION_SAVE_QUEUE: OnceLock<
    Arc<RuntimeSmartContextTokenCalibrationSaveQueue>,
> = OnceLock::new();

impl From<RuntimeSmartContextPersistedTokenUsage> for RuntimeTokenUsage {
    fn from(value: RuntimeSmartContextPersistedTokenUsage) -> Self {
        Self {
            input_tokens: value.input_tokens,
            cached_input_tokens: value.cached_input_tokens,
            output_tokens: value.output_tokens,
            reasoning_tokens: value.reasoning_tokens,
        }
    }
}

impl From<RuntimeTokenUsage> for RuntimeSmartContextPersistedTokenUsage {
    fn from(value: RuntimeTokenUsage) -> Self {
        Self {
            input_tokens: value.input_tokens,
            cached_input_tokens: value.cached_input_tokens,
            output_tokens: value.output_tokens,
            reasoning_tokens: value.reasoning_tokens,
        }
    }
}

impl From<RuntimeSmartContextPersistedTokenCalibrationBucketKey>
    for runtime_proxy_crate::SmartContextTokenCalibrationBucketKey
{
    fn from(value: RuntimeSmartContextPersistedTokenCalibrationBucketKey) -> Self {
        Self {
            route: value.route,
            model: value.model,
            profile: value.profile,
            transport: value.transport,
        }
    }
}

impl From<runtime_proxy_crate::SmartContextTokenCalibrationBucketKey>
    for RuntimeSmartContextPersistedTokenCalibrationBucketKey
{
    fn from(value: runtime_proxy_crate::SmartContextTokenCalibrationBucketKey) -> Self {
        Self {
            route: value.route,
            model: value.model,
            profile: value.profile,
            transport: value.transport,
        }
    }
}

impl From<RuntimeSmartContextPersistedTokenCalibrationObservation>
    for RuntimeSmartContextTokenCalibrationObservation
{
    fn from(value: RuntimeSmartContextPersistedTokenCalibrationObservation) -> Self {
        Self {
            bucket_key: value.bucket_key.into(),
            usage: value.usage.into(),
        }
    }
}

impl From<&RuntimeSmartContextTokenCalibrationObservation>
    for RuntimeSmartContextPersistedTokenCalibrationObservation
{
    fn from(value: &RuntimeSmartContextTokenCalibrationObservation) -> Self {
        Self {
            bucket_key: value.bucket_key.clone().into(),
            usage: value.usage.into(),
        }
    }
}

impl From<RuntimeSmartContextPersistedRewriteSafetyObservation>
    for RuntimeSmartContextRewriteSafetyRecord
{
    fn from(value: RuntimeSmartContextPersistedRewriteSafetyObservation) -> Self {
        Self {
            observation: RuntimeSmartContextRewriteSafetyObservation {
                safe: value.safe,
                saved_tokens: value.saved_tokens,
            },
            observed_at_unix_secs: value.observed_at_unix_secs,
        }
    }
}

impl From<RuntimeSmartContextRewriteSafetyRecord>
    for RuntimeSmartContextPersistedRewriteSafetyObservation
{
    fn from(value: RuntimeSmartContextRewriteSafetyRecord) -> Self {
        Self {
            safe: value.observation.safe,
            saved_tokens: value.observation.saved_tokens,
            observed_at_unix_secs: value.observed_at_unix_secs,
        }
    }
}

impl From<RuntimeSmartContextStaticSectionFingerprint>
    for RuntimeSmartContextPersistedStaticSectionFingerprint
{
    fn from(value: RuntimeSmartContextStaticSectionFingerprint) -> Self {
        Self {
            item_id: value.item_id,
            heading: value.heading,
            ordinal: value.ordinal,
            content_hash: value.content_hash,
            byte_len: value.byte_len,
        }
    }
}

impl From<RuntimeSmartContextPersistedStaticSectionFingerprint>
    for RuntimeSmartContextStaticSectionFingerprint
{
    fn from(value: RuntimeSmartContextPersistedStaticSectionFingerprint) -> Self {
        Self {
            item_id: value.item_id,
            heading: value.heading,
            ordinal: value.ordinal,
            content_hash: value.content_hash,
            byte_len: value.byte_len,
        }
    }
}

impl prodex_runtime_state::RuntimeScheduledSaveJob for RuntimeSmartContextTokenCalibrationSaveJob {
    fn ready_at(&self) -> Instant {
        self.ready_at
    }
}

pub(super) fn runtime_smart_context_token_calibration_path(artifact_path: &Path) -> PathBuf {
    let mut path = artifact_path.to_path_buf();
    let file_name = artifact_path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| format!("{value}.token-calibration.json"))
        .unwrap_or_else(|| "smart-context-token-calibration.json".to_string());
    path.set_file_name(file_name);
    path
}

pub(super) fn runtime_smart_context_load_token_calibration_for_artifact_path(
    artifact_path: &Path,
) -> RuntimeSmartContextPersistedTokenCalibration {
    let path = runtime_smart_context_token_calibration_path(artifact_path);
    runtime_smart_context_load_token_calibration_path(&path)
}

pub(super) fn runtime_smart_context_token_calibration_snapshot(
    state: &RuntimeSmartContextProxyState,
) -> RuntimeSmartContextPersistedTokenCalibration {
    let now = runtime_smart_context_unix_secs_now();
    RuntimeSmartContextPersistedTokenCalibration {
        version: SMART_CONTEXT_TOKEN_CALIBRATION_PERSISTENCE_VERSION,
        token_usage_history: state
            .token_usage_history
            .iter()
            .copied()
            .map(RuntimeSmartContextPersistedTokenUsage::from)
            .collect(),
        token_calibration_history: state
            .token_calibration_history
            .iter()
            .map(RuntimeSmartContextPersistedTokenCalibrationObservation::from)
            .collect(),
        rewrite_safety_history: state
            .rewrite_safety_history
            .iter()
            .copied()
            .filter(|record| runtime_smart_context_rewrite_safety_record_fresh(*record, now))
            .map(RuntimeSmartContextPersistedRewriteSafetyObservation::from)
            .collect(),
        artifact_aliases: runtime_smart_context_persisted_artifact_aliases(state),
        static_section_fingerprints: state
            .static_section_fingerprints
            .values()
            .take(SMART_CONTEXT_PERSISTED_STATIC_SECTION_LIMIT)
            .cloned()
            .map(RuntimeSmartContextPersistedStaticSectionFingerprint::from)
            .collect(),
    }
}

pub(super) fn schedule_runtime_smart_context_token_calibration_save(
    shared: &RuntimeRotationProxyShared,
    path: PathBuf,
    snapshot: RuntimeSmartContextPersistedTokenCalibration,
    reason: &str,
) {
    if !runtime_proxy_persistence_enabled(shared) {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "smart_context_token_calibration_save_suppressed",
                [
                    runtime_proxy_log_field("role", "follower"),
                    runtime_proxy_log_field("reason", reason),
                    runtime_proxy_log_field("path", path.display().to_string()),
                ],
            ),
        );
        return;
    }

    if cfg!(test) {
        match runtime_smart_context_save_token_calibration_snapshot(&path, &snapshot) {
            Ok(()) => runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "smart_context_token_calibration_save_ok",
                    [
                        runtime_proxy_log_field("reason", reason),
                        runtime_proxy_log_field("lag_ms", "0"),
                        runtime_proxy_log_field(
                            "samples",
                            snapshot.token_calibration_history.len().to_string(),
                        ),
                    ],
                ),
            ),
            Err(err) => runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "smart_context_token_calibration_save_error",
                    [
                        runtime_proxy_log_field("reason", reason),
                        runtime_proxy_log_field("lag_ms", "0"),
                        runtime_proxy_log_field("stage", "write"),
                        runtime_proxy_log_field("error", format!("{err:#}")),
                    ],
                ),
            ),
        }
        return;
    }

    let queue = runtime_smart_context_token_calibration_save_queue();
    let queued_at = Instant::now();
    let ready_at = queued_at + Duration::from_millis(SMART_CONTEXT_TOKEN_CALIBRATION_SAVE_DELAY_MS);
    let mut pending = queue
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    pending.insert(
        path.clone(),
        RuntimeSmartContextTokenCalibrationSaveJob {
            path,
            snapshot,
            log_path: shared.log_path.clone(),
            reason: reason.to_string(),
            queued_at,
            ready_at,
        },
    );
    let backlog = pending.len();
    drop(pending);
    queue.wake.notify_one();

    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "smart_context_token_calibration_save_queued",
            [
                runtime_proxy_log_field("reason", reason),
                runtime_proxy_log_field("backlog", backlog.to_string()),
                runtime_proxy_log_field(
                    "ready_in_ms",
                    SMART_CONTEXT_TOKEN_CALIBRATION_SAVE_DELAY_MS.to_string(),
                ),
            ],
        ),
    );
}

fn runtime_smart_context_token_calibration_save_queue()
-> Arc<RuntimeSmartContextTokenCalibrationSaveQueue> {
    Arc::clone(
        RUNTIME_SMART_CONTEXT_TOKEN_CALIBRATION_SAVE_QUEUE.get_or_init(|| {
            let queue = Arc::new(RuntimeSmartContextTokenCalibrationSaveQueue {
                pending: Mutex::new(BTreeMap::new()),
                wake: Condvar::new(),
            });
            let worker_queue = Arc::clone(&queue);
            thread::spawn(move || {
                runtime_smart_context_token_calibration_save_worker_loop(worker_queue)
            });
            queue
        }),
    )
}

fn runtime_smart_context_token_calibration_save_worker_loop(
    queue: Arc<RuntimeSmartContextTokenCalibrationSaveQueue>,
) {
    loop {
        let job = runtime_smart_context_next_token_calibration_save_job(&queue);
        let RuntimeSmartContextTokenCalibrationSaveJob {
            path,
            snapshot,
            log_path,
            reason,
            queued_at,
            ready_at: _,
        } = job;
        match runtime_smart_context_save_token_calibration_snapshot(&path, &snapshot) {
            Ok(()) => runtime_proxy_log_to_path(
                &log_path,
                &runtime_proxy_structured_log_message(
                    "smart_context_token_calibration_save_ok",
                    [
                        runtime_proxy_log_field("reason", reason.as_str()),
                        runtime_proxy_log_field(
                            "lag_ms",
                            queued_at.elapsed().as_millis().to_string(),
                        ),
                        runtime_proxy_log_field(
                            "samples",
                            snapshot.token_calibration_history.len().to_string(),
                        ),
                    ],
                ),
            ),
            Err(err) => runtime_proxy_log_to_path(
                &log_path,
                &runtime_proxy_structured_log_message(
                    "smart_context_token_calibration_save_error",
                    [
                        runtime_proxy_log_field("reason", reason.as_str()),
                        runtime_proxy_log_field(
                            "lag_ms",
                            queued_at.elapsed().as_millis().to_string(),
                        ),
                        runtime_proxy_log_field("stage", "write"),
                        runtime_proxy_log_field("error", format!("{err:#}")),
                    ],
                ),
            ),
        }
        runtime_allocator_trim_best_effort();
    }
}

fn runtime_smart_context_next_token_calibration_save_job(
    queue: &RuntimeSmartContextTokenCalibrationSaveQueue,
) -> RuntimeSmartContextTokenCalibrationSaveJob {
    let mut pending = queue
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    loop {
        if let Some(path) = pending
            .iter()
            .filter(|(_, job)| job.ready_at <= Instant::now())
            .map(|(path, _)| path.clone())
            .next()
        {
            if let Some(job) = pending.remove(&path) {
                return job;
            }
            continue;
        }
        let wait = pending
            .values()
            .map(|job| job.ready_at.saturating_duration_since(Instant::now()))
            .min()
            .unwrap_or_else(|| Duration::from_secs(60));
        let (next_pending, _) = queue
            .wake
            .wait_timeout(pending, wait)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        pending = next_pending;
    }
}

fn runtime_smart_context_save_token_calibration_snapshot(
    path: &Path,
    snapshot: &RuntimeSmartContextPersistedTokenCalibration,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let _lock = crate::runtime_store::acquire_json_file_lock(path)?;
    let existing = runtime_smart_context_load_token_calibration_path(path);
    let merged =
        runtime_smart_context_merge_persisted_token_calibration(existing, snapshot.clone());
    let bytes = serde_json::to_vec(&merged).context("failed to encode token calibration")?;
    fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

fn runtime_smart_context_load_token_calibration_path(
    path: &Path,
) -> RuntimeSmartContextPersistedTokenCalibration {
    let Ok(bytes) = fs::read(path) else {
        return RuntimeSmartContextPersistedTokenCalibration::default();
    };
    let Ok(calibration) =
        serde_json::from_slice::<RuntimeSmartContextPersistedTokenCalibration>(&bytes)
    else {
        return RuntimeSmartContextPersistedTokenCalibration::default();
    };
    if calibration.version == SMART_CONTEXT_TOKEN_CALIBRATION_PERSISTENCE_VERSION {
        calibration
    } else {
        RuntimeSmartContextPersistedTokenCalibration::default()
    }
}

fn runtime_smart_context_merge_persisted_token_calibration(
    mut existing: RuntimeSmartContextPersistedTokenCalibration,
    incoming: RuntimeSmartContextPersistedTokenCalibration,
) -> RuntimeSmartContextPersistedTokenCalibration {
    existing.version = SMART_CONTEXT_TOKEN_CALIBRATION_PERSISTENCE_VERSION;
    runtime_smart_context_extend_unique_tail(
        &mut existing.token_usage_history,
        incoming.token_usage_history,
    );
    runtime_smart_context_truncate_vec_tail(
        &mut existing.token_usage_history,
        SMART_CONTEXT_TOKEN_USAGE_HISTORY_LIMIT,
    );

    runtime_smart_context_extend_unique_tail(
        &mut existing.token_calibration_history,
        incoming.token_calibration_history,
    );
    runtime_smart_context_truncate_vec_tail(
        &mut existing.token_calibration_history,
        SMART_CONTEXT_TOKEN_CALIBRATION_HISTORY_LIMIT,
    );

    let now = runtime_smart_context_unix_secs_now();
    existing
        .rewrite_safety_history
        .extend(incoming.rewrite_safety_history);
    existing.rewrite_safety_history.retain(|record| {
        runtime_smart_context_rewrite_safety_record_fresh(
            RuntimeSmartContextRewriteSafetyRecord::from(*record),
            now,
        )
    });
    existing
        .rewrite_safety_history
        .sort_by_key(|record| record.observed_at_unix_secs);
    runtime_smart_context_truncate_vec_tail(
        &mut existing.rewrite_safety_history,
        SMART_CONTEXT_REWRITE_SAFETY_HISTORY_LIMIT,
    );

    existing.artifact_aliases = runtime_smart_context_merge_persisted_artifact_aliases(
        existing.artifact_aliases,
        incoming.artifact_aliases,
    );
    existing.static_section_fingerprints =
        runtime_smart_context_merge_persisted_static_section_fingerprints(
            existing.static_section_fingerprints,
            incoming.static_section_fingerprints,
        );
    existing
}

fn runtime_smart_context_truncate_vec_tail<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit {
        let overflow = items.len().saturating_sub(limit);
        items.drain(0..overflow);
    }
}

fn runtime_smart_context_extend_unique_tail<T: PartialEq>(items: &mut Vec<T>, incoming: Vec<T>) {
    for item in incoming {
        if !items.contains(&item) {
            items.push(item);
        }
    }
}
