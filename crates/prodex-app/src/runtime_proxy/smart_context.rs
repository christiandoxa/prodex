use super::*;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::path::{Path, PathBuf};

const SMART_CONTEXT_DUPLICATE_TEXT_MIN_BYTES: usize = 1024;
const SMART_CONTEXT_FALLBACK_CONTEXT_WINDOW_TOKENS: u64 = 32_000;
const SMART_CONTEXT_RESERVED_OUTPUT_TOKENS: u64 = 4_096;
const SMART_CONTEXT_MODEL_SCAN_MAX_BYTES: usize = 4 * 1024;
const SMART_CONTEXT_MODEL_NAME_MAX_BYTES: usize = 128;
const SMART_CONTEXT_TOKEN_USAGE_HISTORY_LIMIT: usize = 8;
const SMART_CONTEXT_TOKEN_CALIBRATION_HISTORY_LIMIT: usize = 16;
const SMART_CONTEXT_TOKEN_CALIBRATION_PERSISTENCE_VERSION: u32 = 1;
const SMART_CONTEXT_TOKEN_CALIBRATION_SAVE_DELAY_MS: u64 = 250;
const SMART_CONTEXT_REWRITE_TELEMETRY_HISTORY_LIMIT: usize = 16;
const SMART_CONTEXT_REWRITE_SAFETY_HISTORY_LIMIT: usize = 4;
const SMART_CONTEXT_REWRITE_SAFETY_TTL_SECS: u64 = 6 * 60 * 60;
const SMART_CONTEXT_SURGICAL_CRITICAL_MAX_RANGES: usize = 12;
const SMART_CONTEXT_STATIC_CONTEXT_CHUNK_MIN_BYTES: usize = 512;
const SMART_CONTEXT_TOOL_PREVIEW_ESTIMATED_LINE_BYTES: usize = 256;
const SMART_CONTEXT_TOOL_PREVIEW_MAX_LINE_CHARS: usize = 220;
const SMART_CONTEXT_TOOL_PROGRESSIVE_SUMMARY_MAX_BYTES: usize = 8 * 1024;
const SMART_CONTEXT_ARTIFACT_MANIFEST_MAX_ENTRIES: usize = 12;
const SMART_CONTEXT_ARTIFACT_MANIFEST_MAX_CHARS: usize = 1_600;
const SMART_CONTEXT_ARTIFACT_MANIFEST_COOLDOWN_MS: u64 = 30_000;
const SMART_CONTEXT_TOOL_ARGS_INLINE_MIN_BYTES: usize = 2 * 1024;
const SMART_CONTEXT_SEMANTIC_REHYDRATE_GLOBAL_MAX_RANGES: usize = 12;
const SMART_CONTEXT_SEMANTIC_REHYDRATE_NARROW_MAX_RANGES: usize = 4;
const SMART_CONTEXT_LABEL_SUMMARY: &str = "sum:";
const SMART_CONTEXT_LABEL_CRITICAL_EXACT: &str = "crit exact:";
const SMART_CONTEXT_LABEL_CRITICAL_EXACT_LEGACY: &str = "critical exact ranges:";
const SMART_CONTEXT_LABEL_SEMANTIC_EXACT: &str = "sem exact:";
const SMART_CONTEXT_LABEL_DUPLICATE_CHUNKS: &str = "dup exact chunks:";
const SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX: &str = "psc:";
const SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX: &str = "psc static ";
const SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX_LEGACY: &str =
    "prodex static context unchanged ";
const SMART_CONTEXT_STATIC_CONTEXT_DUP_MARKER_PREFIX: &str = "psc static dup ";
const SMART_CONTEXT_STATIC_CONTEXT_CHUNK_DUP_MARKER_PREFIX: &str = "psc static chunk dup ";
const SMART_CONTEXT_STATIC_CONTEXT_SECTION_DUP_MARKER_PREFIX: &str = "psc static section dup ";
const SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX: &str = "psc a ";
const SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX_LEGACY: &str = "psc aliases ";
const SMART_CONTEXT_PATH_ALIAS_LEGEND_PREFIX: &str = "psc p ";
const SMART_CONTEXT_PATH_ALIAS_LEGEND_PREFIX_LEGACY: &str = "psc path aliases ";
const RUNTIME_SMART_CONTEXT_STATIC_PROMPT_FIELDS: [&str; 3] =
    ["instructions", "system", "developer"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeSmartContextTransport {
    Http,
    Websocket,
}

impl RuntimeSmartContextTransport {
    fn label(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Websocket => "websocket",
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct RuntimeSmartContextTransformStats {
    artifacts_stored: usize,
    tool_outputs_condensed: usize,
    tool_call_args_condensed: usize,
    duplicate_texts: usize,
    cross_turn_duplicate_texts: usize,
    repeat_tool_output_refs: usize,
    blob_outputs_condensed: usize,
    rehydrated_refs: usize,
    static_context_deltas: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct RuntimeSmartContextTransformOutcome {
    stats: RuntimeSmartContextTransformStats,
    deferred_rehydrate_refs: Vec<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct RuntimeSmartContextStaticContextObservation {
    seen_before: bool,
    changed: bool,
    item_count: usize,
    delta_count: usize,
    prompt_cache_hash: Option<String>,
    changed_item_ids: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeSmartContextRewriteSafetyObservation {
    safe: bool,
    saved_tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeSmartContextRewriteSafetyRecord {
    observation: RuntimeSmartContextRewriteSafetyObservation,
    observed_at_unix_secs: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct RuntimeSmartContextTokenCalibrationObservation {
    bucket_key: runtime_proxy_crate::SmartContextTokenCalibrationBucketKey,
    usage: RuntimeTokenUsage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct RuntimeSmartContextLineRange {
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RuntimeSmartContextArtifactReference {
    id: String,
    marker: String,
    line_range: Option<RuntimeSmartContextLineRange>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct RuntimeSmartContextSelectiveRehydrateTerms {
    file_paths: BTreeSet<String>,
    error_codes: BTreeSet<String>,
    test_symbols: BTreeSet<String>,
    diff_hunks: Vec<RuntimeSmartContextSelectiveDiffHunkTerm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeSmartContextSelectiveDiffHunkTerm {
    path: Option<String>,
    old_start: Option<usize>,
    new_start: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct RuntimeSmartContextIntentSignals {
    intent_terms: Vec<String>,
    semantic_terms: RuntimeSmartContextSelectiveRehydrateTerms,
    artifact_refs: Vec<RuntimeSmartContextArtifactReference>,
    command_kind_hints: BTreeSet<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct RuntimeSmartContextToolCallMetadata {
    command: Option<String>,
    exit_code: Option<i32>,
    kind_hint: Option<prodex_context::CommandOutputKind>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct RuntimeSmartContextToolOutputCompactionMetadata {
    kind_hint: Option<prodex_context::CommandOutputKind>,
    command: Option<String>,
    exit_code: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeSmartContextDuplicateChunkSummaryPlan {
    text: String,
    content_hash: String,
    byte_len: usize,
    occurrence_count: usize,
    refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeSmartContextExactAppendixRange {
    reference: String,
    body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeSmartContextSeenExactAppendixBody {
    body: String,
    refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeSmartContextStaticChunkSeen {
    source_id: String,
    body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeSmartContextArtifactAlias {
    id: String,
    alias: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeSmartContextRewriteTelemetryRecord {
    body_bytes_before: usize,
    body_bytes_after: usize,
    estimated_tokens_before: u64,
    estimated_tokens_after: u64,
    rewrite_kind: String,
    status: String,
    fallback_reason: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct RuntimeSmartContextArtifactIndexes<'a> {
    line_index: Option<&'a RuntimeSmartContextArtifactLineIndex>,
    chunk_index: Option<&'a RuntimeSmartContextArtifactChunkIndex>,
}

#[derive(Debug, Default)]
struct RuntimeSmartContextProxyState {
    enabled: bool,
    model_context_window_tokens: Option<u64>,
    artifacts: RuntimeSmartContextArtifactStore,
    artifact_path: Option<PathBuf>,
    last_token_usage: Option<RuntimeTokenUsage>,
    token_usage_history: Vec<RuntimeTokenUsage>,
    token_calibration_history: Vec<RuntimeSmartContextTokenCalibrationObservation>,
    rewrite_telemetry_history: Vec<RuntimeSmartContextRewriteTelemetryRecord>,
    rewrite_safety_history: Vec<RuntimeSmartContextRewriteSafetyRecord>,
    last_static_context_fingerprints: Vec<runtime_proxy_crate::SmartContextFingerprint>,
    last_static_context_prompt_cache_hash: Option<String>,
    last_artifact_manifest_ids: BTreeSet<String>,
    last_artifact_manifest_emitted_at: Option<Instant>,
    artifact_aliases: BTreeMap<String, String>,
    next_artifact_alias_index: usize,
}

static RUNTIME_SMART_CONTEXT_PROXY_STATES: OnceLock<
    Mutex<BTreeMap<PathBuf, RuntimeSmartContextProxyState>>,
> = OnceLock::new();

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RuntimeSmartContextPersistedTokenCalibration {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    token_usage_history: Vec<RuntimeSmartContextPersistedTokenUsage>,
    #[serde(default)]
    token_calibration_history: Vec<RuntimeSmartContextPersistedTokenCalibrationObservation>,
    #[serde(default)]
    rewrite_safety_history: Vec<RuntimeSmartContextPersistedRewriteSafetyObservation>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
struct RuntimeSmartContextPersistedTokenUsage {
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
struct RuntimeSmartContextPersistedTokenCalibrationObservation {
    #[serde(default)]
    bucket_key: RuntimeSmartContextPersistedTokenCalibrationBucketKey,
    #[serde(default)]
    usage: RuntimeSmartContextPersistedTokenUsage,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
struct RuntimeSmartContextPersistedRewriteSafetyObservation {
    #[serde(default)]
    safe: bool,
    #[serde(default)]
    saved_tokens: u64,
    #[serde(default)]
    observed_at_unix_secs: u64,
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

impl prodex_runtime_state::RuntimeScheduledSaveJob for RuntimeSmartContextTokenCalibrationSaveJob {
    fn ready_at(&self) -> Instant {
        self.ready_at
    }
}

fn runtime_smart_context_token_calibration_path(artifact_path: &Path) -> PathBuf {
    let mut path = artifact_path.to_path_buf();
    let file_name = artifact_path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| format!("{value}.token-calibration.json"))
        .unwrap_or_else(|| "smart-context-token-calibration.json".to_string());
    path.set_file_name(file_name);
    path
}

fn runtime_smart_context_load_token_calibration_for_artifact_path(
    artifact_path: &Path,
) -> RuntimeSmartContextPersistedTokenCalibration {
    let path = runtime_smart_context_token_calibration_path(artifact_path);
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

fn runtime_smart_context_token_calibration_snapshot(
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
    }
}

fn runtime_smart_context_unix_secs_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn runtime_smart_context_rewrite_safety_record_fresh(
    record: RuntimeSmartContextRewriteSafetyRecord,
    now: u64,
) -> bool {
    record.observed_at_unix_secs == 0
        || now.saturating_sub(record.observed_at_unix_secs) <= SMART_CONTEXT_REWRITE_SAFETY_TTL_SECS
}

fn schedule_runtime_smart_context_token_calibration_save(
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
            return pending.remove(&path).expect("ready job should exist");
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
    let bytes = serde_json::to_vec(snapshot).context("failed to encode token calibration")?;
    fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

pub(crate) fn register_runtime_smart_context_proxy_state(
    log_path: &Path,
    enabled: bool,
    model_context_window_tokens: Option<u64>,
    artifact_path: Option<PathBuf>,
) {
    let states = RUNTIME_SMART_CONTEXT_PROXY_STATES.get_or_init(|| Mutex::new(BTreeMap::new()));
    let Ok(mut states) = states.lock() else {
        return;
    };
    let artifacts = artifact_path
        .as_deref()
        .filter(|_| enabled)
        .map(RuntimeSmartContextArtifactStore::load_from_path)
        .unwrap_or_default();
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
    states.insert(
        log_path.to_path_buf(),
        RuntimeSmartContextProxyState {
            enabled,
            model_context_window_tokens,
            artifacts,
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
            artifact_aliases: BTreeMap::new(),
            next_artifact_alias_index: 0,
        },
    );
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
    let Some(states) = RUNTIME_SMART_CONTEXT_PROXY_STATES.get() else {
        return;
    };
    let Ok(mut states) = states.lock() else {
        return;
    };
    let mut save_job = None;
    if let Some(state) = states.get_mut(&shared.log_path)
        && state.enabled
    {
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

pub(crate) fn runtime_smart_context_effective_prompt_cache_key(
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    allow_internal_derivation: bool,
) -> Option<String> {
    if let Some(prompt_cache_key) = runtime_request_prompt_cache_key(request) {
        return Some(prompt_cache_key);
    }
    if !allow_internal_derivation || !runtime_smart_context_enabled(shared) {
        return None;
    }
    runtime_smart_context_static_prompt_cache_key_from_body(&request.body)
}

pub(super) fn runtime_smart_context_effective_websocket_prompt_cache_key(
    request_text: &str,
    explicit_prompt_cache_key: Option<&str>,
    shared: &RuntimeRotationProxyShared,
    allow_internal_derivation: bool,
) -> Option<String> {
    if let Some(prompt_cache_key) = explicit_prompt_cache_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(prompt_cache_key.to_string());
    }
    if !allow_internal_derivation || !runtime_smart_context_enabled(shared) {
        return None;
    }
    runtime_smart_context_static_prompt_cache_key_from_body(request_text.as_bytes())
}

fn runtime_smart_context_static_prompt_cache_key_from_body(body: &[u8]) -> Option<String> {
    if body.is_empty() {
        return None;
    }
    let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    if let Some(prompt_cache_hash) =
        runtime_smart_context_static_context_delta_prompt_cache_hash(&value)
    {
        return Some(prompt_cache_hash);
    }
    let cache = runtime_proxy_crate::smart_context_static_context_prompt_cache_fingerprint(
        runtime_smart_context_static_context_items(&value),
    );
    (!cache.items.is_empty()).then_some(cache.content_hash)
}

fn observe_runtime_smart_context_rewrite_safety(
    shared: &RuntimeRotationProxyShared,
    observation: RuntimeSmartContextRewriteSafetyObservation,
) {
    let Some(states) = RUNTIME_SMART_CONTEXT_PROXY_STATES.get() else {
        return;
    };
    let Ok(mut states) = states.lock() else {
        return;
    };
    let mut save_job = None;
    if let Some(state) = states.get_mut(&shared.log_path)
        && state.enabled
    {
        let now = runtime_smart_context_unix_secs_now();
        state
            .rewrite_safety_history
            .retain(|record| runtime_smart_context_rewrite_safety_record_fresh(*record, now));
        state
            .rewrite_safety_history
            .push(RuntimeSmartContextRewriteSafetyRecord {
                observation,
                observed_at_unix_secs: now,
            });
        if state.rewrite_safety_history.len() > SMART_CONTEXT_REWRITE_SAFETY_HISTORY_LIMIT {
            let overflow = state
                .rewrite_safety_history
                .len()
                .saturating_sub(SMART_CONTEXT_REWRITE_SAFETY_HISTORY_LIMIT);
            state.rewrite_safety_history.drain(0..overflow);
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
            "smart_context_rewrite_safety",
        );
    }
}

pub(crate) fn prepare_runtime_smart_context_http_body<'a>(
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
) -> Cow<'a, [u8]> {
    prepare_runtime_smart_context_http_body_for_profile(
        request_id, request, shared, route_kind, None,
    )
}

pub(crate) fn prepare_runtime_smart_context_http_body_for_profile<'a>(
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    profile_name: Option<&str>,
) -> Cow<'a, [u8]> {
    if !runtime_smart_context_enabled(shared) {
        return Cow::Borrowed(&request.body);
    }

    prepare_runtime_smart_context_body(
        request_id,
        request,
        shared,
        route_kind,
        RuntimeSmartContextTransport::Http,
        profile_name,
    )
}

pub(super) fn prepare_runtime_smart_context_websocket_text<'a>(
    request_id: u64,
    request_text: &'a str,
    handshake_request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Cow<'a, str> {
    if !runtime_smart_context_enabled(shared) {
        return Cow::Borrowed(request_text);
    }

    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: handshake_request.path_and_query.clone(),
        headers: handshake_request.headers.clone(),
        body: request_text.as_bytes().to_vec(),
    };
    match prepare_runtime_smart_context_body(
        request_id,
        &request,
        shared,
        RuntimeRouteKind::Websocket,
        RuntimeSmartContextTransport::Websocket,
        Some(profile_name),
    ) {
        Cow::Borrowed(_) => Cow::Borrowed(request_text),
        Cow::Owned(body) => String::from_utf8(body)
            .map(Cow::Owned)
            .unwrap_or(Cow::Borrowed(request_text)),
    }
}

fn prepare_runtime_smart_context_body<'a>(
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    transport: RuntimeSmartContextTransport,
    profile_name: Option<&str>,
) -> Cow<'a, [u8]> {
    let budget = runtime_smart_context_budget(
        shared,
        &request.body,
        route_kind,
        transport,
        profile_name,
        runtime_proxy_crate::SmartContextExactnessGuard {
            decision: runtime_proxy_crate::SmartContextExactnessDecision::Allow,
            reasons: Vec::new(),
        },
        Vec::new(),
        false,
    );
    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(&request.body) else {
        runtime_smart_context_log(
            request_id,
            shared,
            route_kind,
            transport,
            "invalid_json",
            "pass_through",
            "-",
            request.body.len(),
            request.body.len(),
            RuntimeSmartContextTransformStats::default(),
            &budget,
            "pass_through",
        );
        return Cow::Borrowed(&request.body);
    };

    let missing_rehydrate_refs = runtime_smart_context_missing_artifact_refs(&value, shared);
    let exactness = runtime_proxy_crate::smart_context_exactness_guard(
        runtime_proxy_crate::SmartContextExactnessInput {
            exact_mode: runtime_smart_context_exact_header(request),
            previous_response_id: runtime_request_previous_response_id(request),
            turn_state: runtime_request_turn_state(request),
            session_id: runtime_request_session_id(request),
            tool_output_without_artifact: false,
            missing_rehydrate_refs: missing_rehydrate_refs.clone(),
        },
    );
    let static_observation = runtime_smart_context_observe_static_context(shared, &value);
    let budget = runtime_smart_context_budget(
        shared,
        &request.body,
        route_kind,
        transport,
        profile_name,
        exactness.clone(),
        missing_rehydrate_refs.clone(),
        false,
    );
    let tier = budget.tier;
    let intent_signals = runtime_smart_context_collect_intent_signals(&value);

    if exactness.decision == runtime_proxy_crate::SmartContextExactnessDecision::RequireExact {
        if let Some(body) = runtime_smart_context_minified_json_body(&value, &request.body) {
            runtime_smart_context_log(
                request_id,
                shared,
                route_kind,
                transport,
                runtime_smart_context_tier_label(tier),
                "require_exact",
                &runtime_smart_context_reason_labels(&exactness.reasons),
                request.body.len(),
                body.len(),
                RuntimeSmartContextTransformStats::default(),
                &budget,
                "ok_minified",
            );
            return Cow::Owned(body);
        }
        runtime_smart_context_log(
            request_id,
            shared,
            route_kind,
            transport,
            runtime_smart_context_tier_label(tier),
            "require_exact",
            &runtime_smart_context_reason_labels(&exactness.reasons),
            request.body.len(),
            request.body.len(),
            RuntimeSmartContextTransformStats::default(),
            &budget,
            "pass_through_exact",
        );
        return Cow::Borrowed(&request.body);
    }

    let Some(mut outcome) = with_runtime_smart_context_proxy_state(shared, |state| {
        let mut outcome = RuntimeSmartContextTransformOutcome::default();
        {
            let store = &mut state.artifacts;
            let rehydrate_plan = runtime_smart_context_auto_rehydrate_plan(
                &value,
                store,
                budget.available_tokens,
                tier,
            );
            outcome.deferred_rehydrate_refs =
                runtime_smart_context_deferred_rehydrate_refs(&rehydrate_plan);
            runtime_smart_context_rehydrate_value_with_plan(
                &mut value,
                store,
                &rehydrate_plan,
                &mut outcome.stats,
            );
            runtime_smart_context_selective_rehydrate_semantic_ranges(
                &mut value,
                store,
                &exactness,
                &intent_signals.semantic_terms,
                &mut outcome.stats,
            );
            if budget.policy.mode != runtime_proxy_crate::SmartContextBudgetMode::ExactPassThrough {
                runtime_smart_context_condense_tool_outputs(
                    &mut value,
                    store,
                    request_id,
                    tier,
                    budget.policy.max_inline_tool_output_bytes,
                    &intent_signals,
                    &mut outcome.stats,
                );
                runtime_smart_context_condense_historical_tool_call_arguments(
                    &mut value,
                    store,
                    request_id,
                    tier,
                    budget.policy.max_inline_tool_output_bytes,
                    &mut outcome.stats,
                );
                runtime_smart_context_dedupe_input_text(
                    &mut value,
                    store,
                    &exactness,
                    &mut outcome.stats,
                );
            }
        }
        if budget.policy.mode != runtime_proxy_crate::SmartContextBudgetMode::ExactPassThrough {
            runtime_smart_context_append_artifact_manifest_delta_if_useful(
                &mut value,
                state,
                &outcome.stats,
                &intent_signals,
            );
        }
        outcome
    }) else {
        if let Some(body) = runtime_smart_context_minified_json_body(&value, &request.body) {
            runtime_smart_context_log(
                request_id,
                shared,
                route_kind,
                transport,
                runtime_smart_context_tier_label(tier),
                "artifact_store_unavailable",
                "-",
                request.body.len(),
                body.len(),
                RuntimeSmartContextTransformStats::default(),
                &budget,
                "ok_minified",
            );
            return Cow::Owned(body);
        }
        runtime_smart_context_log(
            request_id,
            shared,
            route_kind,
            transport,
            runtime_smart_context_tier_label(tier),
            "artifact_store_unavailable",
            "-",
            request.body.len(),
            request.body.len(),
            RuntimeSmartContextTransformStats::default(),
            &budget,
            "pass_through",
        );
        return Cow::Borrowed(&request.body);
    };
    runtime_smart_context_apply_static_context_section_dedupe(
        &mut value,
        &exactness,
        &mut outcome.stats,
    );
    runtime_smart_context_apply_static_context_cross_field_dedupe(
        &mut value,
        &exactness,
        &mut outcome.stats,
    );
    runtime_smart_context_apply_static_context_chunk_dedupe(
        &mut value,
        &exactness,
        &mut outcome.stats,
    );
    runtime_smart_context_apply_static_context_delta(
        &mut value,
        &static_observation,
        &exactness,
        &mut outcome.stats,
    );
    if outcome.stats != RuntimeSmartContextTransformStats::default() {
        with_runtime_smart_context_proxy_state(shared, |state| {
            runtime_smart_context_apply_artifact_aliases_to_generated_texts_with_state(
                &mut value, state,
            );
        });
        runtime_smart_context_apply_path_aliases_to_generated_texts(&mut value);
    }
    let stats = outcome.stats.clone();
    if stats.artifacts_stored > 0 {
        persist_runtime_smart_context_artifacts(shared);
    }

    if stats == RuntimeSmartContextTransformStats::default() {
        if let Some(body) = runtime_smart_context_minified_json_body(&value, &request.body) {
            runtime_smart_context_log(
                request_id,
                shared,
                route_kind,
                transport,
                runtime_smart_context_tier_label(tier),
                "minified",
                "-",
                request.body.len(),
                body.len(),
                stats,
                &budget,
                "ok_minified",
            );
            return Cow::Owned(body);
        }
        runtime_smart_context_log(
            request_id,
            shared,
            route_kind,
            transport,
            runtime_smart_context_tier_label(tier),
            "pass_through",
            "-",
            request.body.len(),
            request.body.len(),
            stats,
            &budget,
            "noop",
        );
        return Cow::Borrowed(&request.body);
    }

    let Ok(body) = serde_json::to_vec(&value) else {
        return Cow::Borrowed(&request.body);
    };
    let self_check =
        runtime_smart_context_rewrite_self_check(request.body.len(), body.len(), &stats);
    let mut unresolved_rehydrate_refs = missing_rehydrate_refs;
    unresolved_rehydrate_refs.extend(outcome.deferred_rehydrate_refs);
    let regression_check = runtime_smart_context_regression_self_check(
        &request.body,
        &body,
        exactness.clone(),
        unresolved_rehydrate_refs.clone(),
    );
    let critical_signal_check =
        runtime_smart_context_critical_signal_self_check(&request.body, &body);
    if critical_signal_check.has_loss()
        && let Some((repaired_body, repaired_stats)) =
            runtime_smart_context_try_surgical_rehydrate_critical_ranges(
                &value,
                shared,
                &request.body,
                &exactness,
                &unresolved_rehydrate_refs,
                &stats,
            )
    {
        observe_runtime_smart_context_rewrite_safety(
            shared,
            RuntimeSmartContextRewriteSafetyObservation {
                safe: true,
                saved_tokens: runtime_smart_context_saved_tokens(
                    request.body.len(),
                    repaired_body.len(),
                ),
            },
        );
        runtime_smart_context_log(
            request_id,
            shared,
            route_kind,
            transport,
            runtime_smart_context_tier_label(tier),
            "rewritten",
            "surgical_rehydrate",
            request.body.len(),
            repaired_body.len(),
            repaired_stats,
            &budget,
            "ok_surgical_rehydrate",
        );
        return Cow::Owned(repaired_body);
    }
    if let Some(fallback_reason) = runtime_smart_context_fallback_exact_reason(
        &regression_check,
        critical_signal_check,
        &stats,
    ) {
        observe_runtime_smart_context_rewrite_safety(
            shared,
            RuntimeSmartContextRewriteSafetyObservation {
                safe: false,
                saved_tokens: 0,
            },
        );
        if let Some(body) = runtime_smart_context_minified_json_body_from_original(&request.body) {
            runtime_smart_context_log(
                request_id,
                shared,
                route_kind,
                transport,
                runtime_smart_context_tier_label(tier),
                "self_check_passthrough",
                "-",
                request.body.len(),
                body.len(),
                stats,
                &budget,
                fallback_reason,
            );
            return Cow::Owned(body);
        }
        runtime_smart_context_log(
            request_id,
            shared,
            route_kind,
            transport,
            runtime_smart_context_tier_label(tier),
            "self_check_passthrough",
            "-",
            request.body.len(),
            request.body.len(),
            stats,
            &budget,
            fallback_reason,
        );
        return Cow::Borrowed(&request.body);
    }
    observe_runtime_smart_context_rewrite_safety(
        shared,
        RuntimeSmartContextRewriteSafetyObservation {
            safe: true,
            saved_tokens: regression_check.saved_tokens,
        },
    );
    runtime_smart_context_log(
        request_id,
        shared,
        route_kind,
        transport,
        runtime_smart_context_tier_label(tier),
        "rewritten",
        "-",
        request.body.len(),
        body.len(),
        stats,
        &budget,
        self_check,
    );
    Cow::Owned(body)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeSmartContextBudget {
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    policy: runtime_proxy_crate::SmartContextAdaptiveBudgetPolicy,
    model_context_window_tokens: u64,
    model_context_window_source: &'static str,
    available_tokens: usize,
    observed_context_tokens: Option<usize>,
    token_usage_source: &'static str,
}

fn runtime_smart_context_budget(
    shared: &RuntimeRotationProxyShared,
    body: &[u8],
    route_kind: RuntimeRouteKind,
    transport: RuntimeSmartContextTransport,
    profile_name: Option<&str>,
    exactness_guard: runtime_proxy_crate::SmartContextExactnessGuard,
    missing_rehydrate_refs: Vec<String>,
    static_context_changed: bool,
) -> RuntimeSmartContextBudget {
    let model_name = runtime_smart_context_model_name_from_body(body);
    let bucket_key = runtime_smart_context_token_calibration_bucket_key_with_model(
        route_kind,
        transport,
        profile_name,
        model_name.as_deref(),
    );
    let (
        global_history,
        bucket_history,
        calibration_samples,
        configured_context_window_tokens,
        recent_rewrite_safety,
    ) = runtime_smart_context_budget_inputs(shared, &bucket_key);
    let history = if bucket_history.is_empty() {
        global_history
    } else {
        bucket_history
    };
    let model_context_window_tokens =
        configured_context_window_tokens.unwrap_or(SMART_CONTEXT_FALLBACK_CONTEXT_WINDOW_TOKENS);
    let observed_context_tokens_u64 = history
        .last()
        .and_then(|usage| runtime_proxy_crate::smart_context_observed_usage_context_tokens(*usage));
    let observed_context_tokens =
        observed_context_tokens_u64.and_then(|tokens| usize::try_from(tokens).ok());
    let current_input_tokens = observed_context_tokens_u64.unwrap_or(0);
    let accounting = runtime_proxy_crate::smart_context_observed_token_accounting_with_calibration(
        runtime_proxy_crate::SmartContextObservedTokenAccountingCalibrationInput {
            accounting: runtime_proxy_crate::SmartContextObservedTokenAccountingInput {
                model_context_window_tokens: Some(model_context_window_tokens),
                reserved_output_tokens: SMART_CONTEXT_RESERVED_OUTPUT_TOKENS,
                current_input_tokens,
                current_request_body_bytes: body.len(),
                current_request_estimated_tokens: Some(
                    runtime_proxy_crate::smart_context_estimate_tokens_from_body(body),
                ),
                observed_usage: history,
            },
            calibration_bucket_key: Some(bucket_key),
            calibration_samples,
        },
    );
    let policy = runtime_proxy_crate::smart_context_adaptive_budget_policy(
        runtime_proxy_crate::SmartContextAdaptiveBudgetPolicyInput {
            exactness_guard,
            accounting,
            recent_rewrite_safety,
            static_context_changed,
            missing_rehydrate_refs,
        },
    );
    let available_tokens = policy
        .max_rehydrate_tokens
        .min(usize::MAX as u64)
        .try_into()
        .unwrap_or(usize::MAX);
    RuntimeSmartContextBudget {
        tier: policy.tier,
        policy,
        model_context_window_tokens,
        model_context_window_source: if configured_context_window_tokens.is_some() {
            "launch_config"
        } else {
            "fallback"
        },
        available_tokens,
        observed_context_tokens,
        token_usage_source: if observed_context_tokens.is_some() {
            "runtime_usage"
        } else {
            "estimated_body"
        },
    }
}

fn runtime_smart_context_enabled(shared: &RuntimeRotationProxyShared) -> bool {
    let Some(states) = RUNTIME_SMART_CONTEXT_PROXY_STATES.get() else {
        return false;
    };
    let Ok(states) = states.lock() else {
        return false;
    };
    states
        .get(&shared.log_path)
        .is_some_and(|state| state.enabled)
}

#[cfg(test)]
fn runtime_smart_context_token_calibration_bucket_key(
    route_kind: RuntimeRouteKind,
    transport: RuntimeSmartContextTransport,
    profile_name: Option<&str>,
) -> runtime_proxy_crate::SmartContextTokenCalibrationBucketKey {
    runtime_smart_context_token_calibration_bucket_key_with_model(
        route_kind,
        transport,
        profile_name,
        None,
    )
}

fn runtime_smart_context_token_calibration_bucket_key_with_model(
    route_kind: RuntimeRouteKind,
    transport: RuntimeSmartContextTransport,
    profile_name: Option<&str>,
    model_name: Option<&str>,
) -> runtime_proxy_crate::SmartContextTokenCalibrationBucketKey {
    runtime_proxy_crate::SmartContextTokenCalibrationBucketKey {
        route: Some(runtime_route_kind_label(route_kind).to_string()),
        model: runtime_smart_context_normalized_model_name(model_name),
        profile: profile_name
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        transport: Some(transport.label().to_string()),
    }
}

pub(crate) fn runtime_smart_context_model_name_from_body(body: &[u8]) -> Option<String> {
    if body.is_empty() {
        return None;
    }
    if body.len() <= SMART_CONTEXT_MODEL_SCAN_MAX_BYTES
        && let Ok(value) = serde_json::from_slice::<serde_json::Value>(body)
    {
        return runtime_smart_context_model_name_from_value(&value);
    }
    let scan_len = body.len().min(SMART_CONTEXT_MODEL_SCAN_MAX_BYTES);
    let scan = std::str::from_utf8(&body[..scan_len]).ok()?;
    runtime_smart_context_model_name_from_json_prefix(scan)
}

fn runtime_smart_context_model_name_from_value(value: &serde_json::Value) -> Option<String> {
    runtime_smart_context_normalized_model_name(value.get("model")?.as_str())
}

fn runtime_smart_context_model_name_from_json_prefix(text: &str) -> Option<String> {
    let (_, after_key) = text.split_once("\"model\"")?;
    let after_colon = after_key.trim_start().strip_prefix(':')?.trim_start();
    let mut chars = after_colon.strip_prefix('"')?.chars();
    let mut model = String::new();
    let mut escaped = false;
    for ch in chars.by_ref() {
        if escaped {
            model.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return runtime_smart_context_normalized_model_name(Some(&model));
        } else if ch.is_control() {
            return None;
        } else {
            model.push(ch);
        }
        if model.len() > SMART_CONTEXT_MODEL_NAME_MAX_BYTES {
            return None;
        }
    }
    None
}

pub(crate) fn runtime_smart_context_normalized_model_name(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty()
        || value.len() > SMART_CONTEXT_MODEL_NAME_MAX_BYTES
        || value.chars().any(char::is_control)
    {
        return None;
    }
    Some(value.to_string())
}

fn runtime_smart_context_budget_inputs(
    shared: &RuntimeRotationProxyShared,
    bucket_key: &runtime_proxy_crate::SmartContextTokenCalibrationBucketKey,
) -> (
    Vec<RuntimeTokenUsage>,
    Vec<RuntimeTokenUsage>,
    Vec<runtime_proxy_crate::SmartContextTokenCalibrationSample>,
    Option<u64>,
    runtime_proxy_crate::SmartContextRecentRewriteSafety,
) {
    let Some(states) = RUNTIME_SMART_CONTEXT_PROXY_STATES.get() else {
        return (Vec::new(), Vec::new(), Vec::new(), None, Default::default());
    };
    let Ok(states) = states.lock() else {
        return (Vec::new(), Vec::new(), Vec::new(), None, Default::default());
    };
    states
        .get(&shared.log_path)
        .map(|state| {
            let calibration_samples = state
                .token_calibration_history
                .iter()
                .map(
                    |sample| runtime_proxy_crate::SmartContextTokenCalibrationSample {
                        bucket_key: Some(sample.bucket_key.clone()),
                        usage: sample.usage,
                    },
                )
                .collect::<Vec<_>>();
            let bucket_history = state
                .token_calibration_history
                .iter()
                .filter(|sample| &sample.bucket_key == bucket_key)
                .map(|sample| sample.usage)
                .collect::<Vec<_>>();
            (
                state.token_usage_history.clone(),
                bucket_history,
                calibration_samples,
                state.model_context_window_tokens,
                runtime_smart_context_recent_rewrite_safety(&state.rewrite_safety_history),
            )
        })
        .unwrap_or_default()
}

fn runtime_smart_context_recent_rewrite_safety(
    history: &[RuntimeSmartContextRewriteSafetyRecord],
) -> runtime_proxy_crate::SmartContextRecentRewriteSafety {
    let mut safety = runtime_proxy_crate::SmartContextRecentRewriteSafety::default();
    let now = runtime_smart_context_unix_secs_now();
    for record in history
        .iter()
        .filter(|record| runtime_smart_context_rewrite_safety_record_fresh(**record, now))
    {
        let observation = record.observation;
        if observation.safe {
            safety.safe_rewrites = safety.safe_rewrites.saturating_add(1);
            safety.saved_tokens = safety.saved_tokens.saturating_add(observation.saved_tokens);
        } else {
            safety.fallback_rewrites = safety.fallback_rewrites.saturating_add(1);
        }
    }
    safety
}

fn runtime_smart_context_observe_static_context(
    shared: &RuntimeRotationProxyShared,
    value: &serde_json::Value,
) -> RuntimeSmartContextStaticContextObservation {
    let cache = runtime_proxy_crate::smart_context_static_context_prompt_cache_fingerprint(
        runtime_smart_context_static_context_items(value),
    );
    if cache.items.is_empty() {
        return RuntimeSmartContextStaticContextObservation::default();
    }

    let current = cache
        .items
        .iter()
        .map(|item| runtime_proxy_crate::SmartContextFingerprint {
            id: item.id.clone(),
            kind: runtime_proxy_crate::SmartContextFingerprintKind::StaticContext,
            content_hash: item.content_hash.clone(),
            byte_len: item.byte_len,
        })
        .collect::<Vec<_>>();

    let Some(states) = RUNTIME_SMART_CONTEXT_PROXY_STATES.get() else {
        return RuntimeSmartContextStaticContextObservation {
            seen_before: false,
            changed: false,
            item_count: current.len(),
            delta_count: 0,
            prompt_cache_hash: Some(cache.content_hash),
            changed_item_ids: BTreeSet::new(),
        };
    };
    let Ok(mut states) = states.lock() else {
        return RuntimeSmartContextStaticContextObservation {
            seen_before: false,
            changed: false,
            item_count: current.len(),
            delta_count: 0,
            prompt_cache_hash: Some(cache.content_hash),
            changed_item_ids: BTreeSet::new(),
        };
    };
    let Some(state) = states.get_mut(&shared.log_path) else {
        return RuntimeSmartContextStaticContextObservation {
            seen_before: false,
            changed: false,
            item_count: current.len(),
            delta_count: 0,
            prompt_cache_hash: Some(cache.content_hash),
            changed_item_ids: BTreeSet::new(),
        };
    };

    let seen_before = !state.last_static_context_fingerprints.is_empty();
    let delta = if seen_before {
        runtime_proxy_crate::smart_context_fingerprint_delta(
            state.last_static_context_fingerprints.clone(),
            current.clone(),
        )
    } else {
        Vec::new()
    };
    let changed = delta
        .iter()
        .any(runtime_smart_context_fingerprint_change_is_substantive);
    let changed_item_ids =
        runtime_smart_context_substantive_static_context_changed_item_ids(&delta);
    let observation = RuntimeSmartContextStaticContextObservation {
        seen_before,
        changed,
        item_count: current.len(),
        delta_count: delta.len(),
        prompt_cache_hash: Some(cache.content_hash.clone()),
        changed_item_ids,
    };
    state.last_static_context_fingerprints = current;
    state.last_static_context_prompt_cache_hash = Some(cache.content_hash);
    state.artifacts.set_static_context_fingerprints(
        observation.prompt_cache_hash.clone(),
        state.last_static_context_fingerprints.clone(),
    );
    let save_job = state
        .artifact_path
        .clone()
        .map(|path| (path, state.artifacts.clone()));
    drop(states);
    if let Some((path, store)) = save_job {
        schedule_runtime_smart_context_artifact_save(
            shared,
            path,
            store,
            "smart_context_static_fingerprints",
        );
    }
    observation
}

fn runtime_smart_context_fingerprint_change_is_substantive(
    change: &runtime_proxy_crate::SmartContextFingerprintChange,
) -> bool {
    !matches!(
        change,
        runtime_proxy_crate::SmartContextFingerprintChange::Unchanged { .. }
    )
}

fn runtime_smart_context_substantive_static_context_changed_item_ids(
    changes: &[runtime_proxy_crate::SmartContextFingerprintChange],
) -> BTreeSet<String> {
    changes
        .iter()
        .filter_map(|change| match change {
            runtime_proxy_crate::SmartContextFingerprintChange::Added { fingerprint }
            | runtime_proxy_crate::SmartContextFingerprintChange::Changed {
                after: fingerprint,
                ..
            } => Some(fingerprint.id.clone()),
            runtime_proxy_crate::SmartContextFingerprintChange::Removed { .. }
            | runtime_proxy_crate::SmartContextFingerprintChange::Unchanged { .. } => None,
        })
        .collect()
}

fn runtime_smart_context_apply_static_context_delta(
    value: &mut serde_json::Value,
    observation: &RuntimeSmartContextStaticContextObservation,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    if exactness_guard.decision != runtime_proxy_crate::SmartContextExactnessDecision::Allow
        || !observation.seen_before
        || observation.item_count == 0
    {
        return;
    }
    let Some(prompt_cache_hash) = observation.prompt_cache_hash.as_deref() else {
        return;
    };
    let marker = runtime_smart_context_static_context_delta_marker(prompt_cache_hash);
    stats.static_context_deltas = stats.static_context_deltas.saturating_add(
        runtime_smart_context_replace_static_context_texts(value, observation, &marker),
    );
}

fn runtime_smart_context_apply_static_context_section_dedupe(
    value: &mut serde_json::Value,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    if exactness_guard.decision != runtime_proxy_crate::SmartContextExactnessDecision::Allow {
        return;
    }
    let items = runtime_smart_context_static_context_items(value);
    if items.is_empty() {
        return;
    }
    let mut first_by_heading_hash = BTreeMap::<(String, String, usize), String>::new();
    let mut replacements = BTreeMap::<String, String>::new();
    for item in items {
        let Some(next_text) = runtime_smart_context_static_context_section_deduped_text(
            &item.id,
            &item.text,
            &mut first_by_heading_hash,
        ) else {
            continue;
        };
        replacements.insert(item.id, next_text);
    }
    if replacements.is_empty() {
        return;
    }
    stats.static_context_deltas = stats.static_context_deltas.saturating_add(
        runtime_smart_context_replace_static_context_item_texts(value, &replacements),
    );
}

fn runtime_smart_context_static_context_section_deduped_text(
    item_id: &str,
    text: &str,
    first_by_heading_hash: &mut BTreeMap<(String, String, usize), String>,
) -> Option<String> {
    let sections = runtime_smart_context_static_context_heading_sections(text);
    if sections.len() < 2 {
        return None;
    }
    let mut candidate = String::new();
    let mut cursor = 0usize;
    let mut changed = false;
    for section in sections {
        if section.start < cursor || section.end > text.len() {
            continue;
        }
        candidate.push_str(&text[cursor..section.start]);
        let body = &text[section.start..section.end];
        let body_key = body.trim();
        let content_hash = runtime_proxy_crate::smart_context_hash_text(body_key);
        let key = (
            section.heading.to_ascii_lowercase(),
            content_hash.clone(),
            body_key.len(),
        );
        let marker = if let Some(first_id) = first_by_heading_hash.get(&key) {
            runtime_smart_context_static_context_section_dup_marker(
                first_id,
                &content_hash,
                &section.heading,
            )
        } else {
            first_by_heading_hash.insert(key, format!("{item_id}:{}", section.ordinal));
            String::new()
        };
        if !marker.is_empty()
            && marker.len() < body.len()
            && prodex_context::critical_signal_self_check(
                text,
                &format!("{candidate}{marker}{}", &text[section.end..]),
            )
            .passed()
        {
            candidate.push_str(&marker);
            changed = true;
        } else {
            candidate.push_str(body);
        }
        cursor = section.end;
    }
    candidate.push_str(&text[cursor..]);
    (changed && candidate.len() < text.len()).then_some(candidate)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeSmartContextStaticHeadingSection {
    heading: String,
    start: usize,
    end: usize,
    ordinal: usize,
}

fn runtime_smart_context_static_context_heading_sections(
    text: &str,
) -> Vec<RuntimeSmartContextStaticHeadingSection> {
    let mut headings = Vec::<(String, usize)>::new();
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        let line_without_newline = line.trim_end_matches('\n').trim_end_matches('\r');
        if let Some(heading) = runtime_smart_context_static_context_heading(line_without_newline) {
            headings.push((heading, offset));
        }
        offset = offset.saturating_add(line.len());
    }
    if !text.ends_with('\n')
        && let Some(last_line) = text.rsplit('\n').next()
        && let Some(heading) = runtime_smart_context_static_context_heading(last_line)
    {
        let start = text.len().saturating_sub(last_line.len());
        if !headings
            .iter()
            .any(|(_, existing_start)| *existing_start == start)
        {
            headings.push((heading, start));
        }
    }
    let mut sections = Vec::new();
    for (index, (heading, start)) in headings.iter().enumerate() {
        let end = headings
            .get(index + 1)
            .map(|(_, next_start)| *next_start)
            .unwrap_or(text.len());
        if end.saturating_sub(*start) < SMART_CONTEXT_STATIC_CONTEXT_CHUNK_MIN_BYTES {
            continue;
        }
        let body = text[*start..end].trim();
        if body.starts_with(SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX)
            || body.starts_with(SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX_LEGACY)
            || body.starts_with(SMART_CONTEXT_STATIC_CONTEXT_DUP_MARKER_PREFIX)
            || body.starts_with(SMART_CONTEXT_STATIC_CONTEXT_CHUNK_DUP_MARKER_PREFIX)
            || body.starts_with(SMART_CONTEXT_STATIC_CONTEXT_SECTION_DUP_MARKER_PREFIX)
        {
            continue;
        }
        sections.push(RuntimeSmartContextStaticHeadingSection {
            heading: heading.clone(),
            start: *start,
            end,
            ordinal: index,
        });
    }
    sections
}

fn runtime_smart_context_static_context_heading(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('#') {
        return None;
    }
    let level = trimmed.chars().take_while(|ch| *ch == '#').count();
    if level == 0 || level > 6 || !trimmed.chars().nth(level).is_some_and(char::is_whitespace) {
        return None;
    }
    Some(trimmed.to_string())
}

fn runtime_smart_context_apply_static_context_cross_field_dedupe(
    value: &mut serde_json::Value,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    if exactness_guard.decision != runtime_proxy_crate::SmartContextExactnessDecision::Allow {
        return;
    }
    let items = runtime_smart_context_static_context_items(value);
    if items.len() < 2 {
        return;
    }
    let mut first_by_hash = BTreeMap::<String, String>::new();
    let mut duplicate_ids = BTreeMap::<String, String>::new();
    for item in items {
        if item.text.len() < SMART_CONTEXT_DUPLICATE_TEXT_MIN_BYTES {
            continue;
        }
        let content_hash = runtime_proxy_crate::smart_context_hash_text(&item.text);
        if let Some(first_id) = first_by_hash.get(&content_hash) {
            duplicate_ids.insert(
                item.id.clone(),
                runtime_smart_context_static_context_dup_marker(first_id, &content_hash),
            );
        } else {
            first_by_hash.insert(content_hash, item.id);
        }
    }
    if duplicate_ids.is_empty() {
        return;
    }
    stats.static_context_deltas = stats.static_context_deltas.saturating_add(
        runtime_smart_context_replace_static_context_duplicate_texts(value, &duplicate_ids),
    );
}

fn runtime_smart_context_apply_static_context_chunk_dedupe(
    value: &mut serde_json::Value,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    if exactness_guard.decision != runtime_proxy_crate::SmartContextExactnessDecision::Allow {
        return;
    }
    let items = runtime_smart_context_static_context_items(value);
    if items.len() < 2 {
        return;
    }
    let mut seen_chunks =
        BTreeMap::<(String, usize), Vec<RuntimeSmartContextStaticChunkSeen>>::new();
    let mut replacements = BTreeMap::<String, String>::new();
    for item in items {
        let next_text = runtime_smart_context_static_context_chunk_deduped_text(
            &item.id,
            &item.text,
            &seen_chunks,
        );
        let source_text = next_text.as_deref().unwrap_or(&item.text);
        runtime_smart_context_record_static_context_chunks(&item.id, source_text, &mut seen_chunks);
        if let Some(next_text) = next_text {
            replacements.insert(item.id, next_text);
        }
    }
    if replacements.is_empty() {
        return;
    }
    stats.static_context_deltas = stats.static_context_deltas.saturating_add(
        runtime_smart_context_replace_static_context_item_texts(value, &replacements),
    );
}

fn runtime_smart_context_static_context_chunk_deduped_text(
    item_id: &str,
    text: &str,
    seen_chunks: &BTreeMap<(String, usize), Vec<RuntimeSmartContextStaticChunkSeen>>,
) -> Option<String> {
    let mut candidate = text.to_string();
    let mut changed = false;
    for chunk in runtime_smart_context_static_context_dedupe_chunks(text) {
        let content_hash = runtime_proxy_crate::smart_context_hash_text(chunk);
        let Some(seen) = seen_chunks.get(&(content_hash.clone(), chunk.len())) else {
            continue;
        };
        let Some(first_seen) = seen
            .iter()
            .find(|seen| seen.body == chunk && seen.source_id != item_id)
        else {
            continue;
        };
        let marker = runtime_smart_context_static_context_chunk_dup_marker(
            &first_seen.source_id,
            &content_hash,
        );
        if marker.len() >= chunk.len() || !candidate.contains(chunk) {
            continue;
        }
        let next = candidate.replacen(chunk, &marker, 1);
        if next.len() < candidate.len()
            && prodex_context::critical_signal_self_check(text, &next).passed()
        {
            candidate = next;
            changed = true;
        }
    }
    changed.then_some(candidate)
}

fn runtime_smart_context_static_context_dedupe_chunks(text: &str) -> Vec<&str> {
    text.split("\n\n")
        .map(str::trim)
        .filter(|chunk| chunk.len() >= SMART_CONTEXT_STATIC_CONTEXT_CHUNK_MIN_BYTES)
        .filter(|chunk| {
            !chunk.starts_with(SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX)
                && !chunk.starts_with(SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX_LEGACY)
                && !chunk.starts_with(SMART_CONTEXT_STATIC_CONTEXT_DUP_MARKER_PREFIX)
                && !chunk.starts_with(SMART_CONTEXT_STATIC_CONTEXT_CHUNK_DUP_MARKER_PREFIX)
        })
        .collect()
}

fn runtime_smart_context_record_static_context_chunks(
    item_id: &str,
    text: &str,
    seen_chunks: &mut BTreeMap<(String, usize), Vec<RuntimeSmartContextStaticChunkSeen>>,
) {
    for chunk in runtime_smart_context_static_context_dedupe_chunks(text) {
        let content_hash = runtime_proxy_crate::smart_context_hash_text(chunk);
        let entry = seen_chunks.entry((content_hash, chunk.len())).or_default();
        if entry
            .iter()
            .any(|seen| seen.source_id == item_id && seen.body == chunk)
        {
            continue;
        }
        entry.push(RuntimeSmartContextStaticChunkSeen {
            source_id: item_id.to_string(),
            body: chunk.to_string(),
        });
    }
}

fn runtime_smart_context_static_context_dup_marker(source_id: &str, content_hash: &str) -> String {
    format!("{SMART_CONTEXT_STATIC_CONTEXT_DUP_MARKER_PREFIX}{source_id} {content_hash}")
}

fn runtime_smart_context_static_context_chunk_dup_marker(
    source_id: &str,
    content_hash: &str,
) -> String {
    format!("{SMART_CONTEXT_STATIC_CONTEXT_CHUNK_DUP_MARKER_PREFIX}{source_id} {content_hash}")
}

fn runtime_smart_context_static_context_section_dup_marker(
    source_id: &str,
    content_hash: &str,
    heading: &str,
) -> String {
    format!(
        "{heading}\n{SMART_CONTEXT_STATIC_CONTEXT_SECTION_DUP_MARKER_PREFIX}{source_id} {content_hash}"
    )
}

fn runtime_smart_context_static_context_delta_marker(prompt_cache_hash: &str) -> String {
    format!("{SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX}{prompt_cache_hash}")
}

fn runtime_smart_context_static_context_delta_marker_hash(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    trimmed
        .strip_prefix(SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX)
        .or_else(|| trimmed.strip_prefix(SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX_LEGACY))
        .filter(|hash| hash.starts_with("scpc:") && !hash.chars().any(char::is_whitespace))
}

fn runtime_smart_context_static_context_delta_prompt_cache_hash(
    value: &serde_json::Value,
) -> Option<String> {
    let items = runtime_smart_context_static_context_items(value);
    let mut hashes = Vec::new();
    for item in items {
        hashes
            .push(runtime_smart_context_static_context_delta_marker_hash(&item.text)?.to_string());
    }
    let first = hashes.first()?;
    hashes
        .iter()
        .all(|hash| hash == first)
        .then(|| first.to_string())
}

fn runtime_smart_context_replace_static_context_texts(
    value: &mut serde_json::Value,
    observation: &RuntimeSmartContextStaticContextObservation,
    marker: &str,
) -> usize {
    let Some(object) = value.as_object_mut() else {
        return 0;
    };
    let mut replaced = 0usize;
    for key in RUNTIME_SMART_CONTEXT_STATIC_PROMPT_FIELDS {
        if !runtime_smart_context_static_context_item_delta_allowed(key, observation) {
            continue;
        }
        if let Some(text) = object.get(key).and_then(serde_json::Value::as_str)
            && !text.trim().is_empty()
        {
            object.insert(
                key.to_string(),
                serde_json::Value::String(marker.to_string()),
            );
            replaced = replaced.saturating_add(1);
        }
    }

    let Some(input) = object
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return replaced;
    };
    for (index, item) in input.iter_mut().enumerate() {
        if runtime_smart_context_replace_static_message_text(index, item, observation, marker) {
            replaced = replaced.saturating_add(1);
        }
    }
    replaced
}

fn runtime_smart_context_replace_static_context_duplicate_texts(
    value: &mut serde_json::Value,
    duplicate_ids: &BTreeMap<String, String>,
) -> usize {
    let Some(object) = value.as_object_mut() else {
        return 0;
    };
    let mut replaced = 0usize;
    for key in RUNTIME_SMART_CONTEXT_STATIC_PROMPT_FIELDS {
        if let Some(marker) = duplicate_ids.get(key)
            && runtime_smart_context_replace_top_level_static_field(object, key, marker)
        {
            replaced = replaced.saturating_add(1);
        }
    }

    let Some(input) = object
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return replaced;
    };
    for (index, item) in input.iter_mut().enumerate() {
        let role = item
            .as_object()
            .and_then(|object| object.get("role"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let id = format!("input[{index}].{role}");
        if let Some(marker) = duplicate_ids.get(&id)
            && runtime_smart_context_replace_static_message_text_with_marker(item, marker)
        {
            replaced = replaced.saturating_add(1);
        }
    }
    replaced
}

fn runtime_smart_context_replace_static_context_item_texts(
    value: &mut serde_json::Value,
    replacements: &BTreeMap<String, String>,
) -> usize {
    let Some(object) = value.as_object_mut() else {
        return 0;
    };
    let mut replaced = 0usize;
    for key in RUNTIME_SMART_CONTEXT_STATIC_PROMPT_FIELDS {
        if let Some(next_text) = replacements.get(key) {
            object.insert(
                key.to_string(),
                serde_json::Value::String(next_text.to_string()),
            );
            replaced = replaced.saturating_add(1);
        }
    }

    let Some(input) = object
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return replaced;
    };
    for (index, item) in input.iter_mut().enumerate() {
        let role = item
            .as_object()
            .and_then(|object| object.get("role"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let id = format!("input[{index}].{role}");
        if let Some(next_text) = replacements.get(&id)
            && runtime_smart_context_replace_static_message_text_with_marker(item, next_text)
        {
            replaced = replaced.saturating_add(1);
        }
    }
    replaced
}

fn runtime_smart_context_replace_top_level_static_field(
    object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    marker: &str,
) -> bool {
    if object
        .get(key)
        .and_then(serde_json::Value::as_str)
        .is_some_and(|text| !text.trim().is_empty())
    {
        object.insert(
            key.to_string(),
            serde_json::Value::String(marker.to_string()),
        );
        true
    } else {
        false
    }
}

fn runtime_smart_context_replace_static_message_text(
    index: usize,
    value: &mut serde_json::Value,
    observation: &RuntimeSmartContextStaticContextObservation,
    marker: &str,
) -> bool {
    if !runtime_smart_context_value_is_static_context_item(value) {
        return false;
    }
    let Some(object) = value.as_object_mut() else {
        return false;
    };
    let role = object
        .get("role")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let id = format!("input[{index}].{role}");
    if !runtime_smart_context_static_context_item_delta_allowed(&id, observation) {
        return false;
    }
    if let Some(text) = object.get("content").and_then(serde_json::Value::as_str)
        && !text.trim().is_empty()
    {
        return runtime_smart_context_replace_static_message_text_with_marker(value, marker);
    }
    if let Some(text) = object.get("input_text").and_then(serde_json::Value::as_str)
        && !text.trim().is_empty()
    {
        return runtime_smart_context_replace_static_message_text_with_marker(value, marker);
    }
    if object.get("content").is_some() {
        return runtime_smart_context_replace_static_message_text_with_marker(value, marker);
    }
    false
}

fn runtime_smart_context_replace_static_message_text_with_marker(
    value: &mut serde_json::Value,
    marker: &str,
) -> bool {
    let Some(object) = value.as_object_mut() else {
        return false;
    };
    if object.get("content").is_some() {
        object.insert(
            "content".to_string(),
            serde_json::Value::String(marker.to_string()),
        );
        return true;
    }
    if object.get("input_text").is_some() {
        object.insert(
            "input_text".to_string(),
            serde_json::Value::String(marker.to_string()),
        );
        return true;
    }
    false
}

fn runtime_smart_context_static_context_item_delta_allowed(
    id: &str,
    observation: &RuntimeSmartContextStaticContextObservation,
) -> bool {
    !observation.changed || !observation.changed_item_ids.contains(id)
}

fn runtime_smart_context_static_context_items(
    value: &serde_json::Value,
) -> Vec<runtime_proxy_crate::SmartContextStaticContextItem> {
    let mut items = Vec::new();
    for key in RUNTIME_SMART_CONTEXT_STATIC_PROMPT_FIELDS {
        if let Some(text) = value.get(key).and_then(serde_json::Value::as_str)
            && !text.trim().is_empty()
        {
            items.push(runtime_proxy_crate::SmartContextStaticContextItem {
                id: key.to_string(),
                text: text.to_string(),
            });
        }
    }

    let Some(input) = value.get("input").and_then(serde_json::Value::as_array) else {
        return items;
    };
    for (index, item) in input.iter().enumerate() {
        let Some(object) = item.as_object() else {
            continue;
        };
        let role = object
            .get("role")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if !runtime_smart_context_static_role_is_prompt_prefix(role) {
            continue;
        }
        if let Some(text) = runtime_smart_context_static_message_text(item)
            && !text.trim().is_empty()
        {
            items.push(runtime_proxy_crate::SmartContextStaticContextItem {
                id: format!("input[{index}].{role}"),
                text,
            });
        }
    }
    items.sort_by(|left, right| left.id.cmp(&right.id));
    items
}

fn runtime_smart_context_static_prompt_field_key(key: &str) -> bool {
    RUNTIME_SMART_CONTEXT_STATIC_PROMPT_FIELDS.contains(&key)
}

fn runtime_smart_context_static_role_is_prompt_prefix(role: &str) -> bool {
    matches!(role, "system" | "developer")
}

fn runtime_smart_context_value_is_static_context_item(value: &serde_json::Value) -> bool {
    value
        .as_object()
        .and_then(|object| object.get("role"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(runtime_smart_context_static_role_is_prompt_prefix)
}

fn runtime_smart_context_static_message_text(value: &serde_json::Value) -> Option<String> {
    let object = value.as_object()?;
    if let Some(text) = object.get("content").and_then(serde_json::Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(text) = object.get("input_text").and_then(serde_json::Value::as_str) {
        return Some(text.to_string());
    }

    let content = object.get("content")?;
    let mut parts = Vec::new();
    runtime_smart_context_collect_static_text_parts(content, &mut parts);
    (!parts.is_empty()).then(|| parts.join("\n"))
}

fn runtime_smart_context_collect_static_text_parts(
    value: &serde_json::Value,
    parts: &mut Vec<String>,
) {
    match value {
        serde_json::Value::String(text) if !text.trim().is_empty() => parts.push(text.clone()),
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_collect_static_text_parts(item, parts);
            }
        }
        serde_json::Value::Object(object) => {
            for key in ["text", "input_text", "content"] {
                if let Some(item) = object.get(key) {
                    runtime_smart_context_collect_static_text_parts(item, parts);
                }
            }
        }
        _ => {}
    }
}

fn runtime_smart_context_collect_intent_signals(
    value: &serde_json::Value,
) -> RuntimeSmartContextIntentSignals {
    let mut signals = RuntimeSmartContextIntentSignals {
        artifact_refs: runtime_smart_context_collect_rehydratable_artifact_refs(value),
        ..RuntimeSmartContextIntentSignals::default()
    };

    let artifact_ref_ids = signals
        .artifact_refs
        .iter()
        .map(|reference| reference.id.clone())
        .collect::<Vec<_>>();
    for id in artifact_ref_ids {
        signals.add_intent_text(&id);
    }

    runtime_smart_context_collect_user_intent_text(value, &mut signals);
    runtime_smart_context_collect_tool_intent_metadata(value, &mut signals);
    signals
}

impl RuntimeSmartContextIntentSignals {
    fn add_intent_text(&mut self, text: &str) {
        for term in prodex_context::extract_intent_terms_from_prompt(text) {
            if self.intent_terms.iter().any(|existing| existing == &term) {
                continue;
            }
            self.add_semantic_term(&term);
            self.intent_terms.push(term);
            if self.intent_terms.len() >= prodex_context::MAX_EXTRACTED_INTENT_TERMS {
                break;
            }
        }
    }

    fn add_command_kind_hint(&mut self, hint: prodex_context::CommandOutputKind) {
        self.command_kind_hints
            .insert(runtime_smart_context_command_kind_hint_label(hint).to_string());
    }

    fn add_semantic_term(&mut self, term: &str) {
        if runtime_smart_context_intent_term_is_error_code(term) {
            self.semantic_terms.error_codes.insert(term.to_string());
        } else if runtime_smart_context_intent_term_is_path(term) {
            self.semantic_terms.file_paths.insert(term.to_string());
        } else if runtime_smart_context_intent_term_is_symbol(term) {
            self.semantic_terms.test_symbols.insert(term.to_string());
        }
    }
}

fn runtime_smart_context_collect_user_intent_text(
    value: &serde_json::Value,
    signals: &mut RuntimeSmartContextIntentSignals,
) {
    if let Some(text) = value.get("prompt").and_then(serde_json::Value::as_str) {
        signals.add_intent_text(text);
    }
    match value.get("input") {
        Some(serde_json::Value::String(text)) => signals.add_intent_text(text),
        Some(serde_json::Value::Array(items)) => {
            for item in items {
                runtime_smart_context_collect_user_intent_text_from_input_item(item, signals);
            }
        }
        _ => {}
    }
}

fn runtime_smart_context_collect_user_intent_text_from_input_item(
    item: &serde_json::Value,
    signals: &mut RuntimeSmartContextIntentSignals,
) {
    if runtime_smart_context_value_is_static_context_item(item) {
        return;
    }
    let Some(object) = item.as_object() else {
        return;
    };
    let item_type = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if item_type.ends_with("_call_output") || item_type.ends_with("_call") {
        return;
    }
    let role = object
        .get("role")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("user");
    if role != "user" {
        return;
    }
    for key in ["content", "input_text", "text"] {
        if let Some(child) = object.get(key) {
            runtime_smart_context_collect_intent_text_parts(child, signals);
        }
    }
}

fn runtime_smart_context_collect_tool_intent_metadata(
    value: &serde_json::Value,
    signals: &mut RuntimeSmartContextIntentSignals,
) {
    let Some(input) = value.get("input").and_then(serde_json::Value::as_array) else {
        return;
    };
    for item in input {
        let Some(object) = item.as_object() else {
            continue;
        };
        let item_type = object
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if item_type.ends_with("_call_output") {
            continue;
        }
        let metadata = runtime_smart_context_tool_item_metadata(object);
        if let Some(command) = metadata.command.as_deref() {
            signals.add_intent_text(command);
        }
        if let Some(kind_hint) = metadata.kind_hint {
            signals.add_command_kind_hint(kind_hint);
        }
    }
}

fn runtime_smart_context_collect_intent_text_parts(
    value: &serde_json::Value,
    signals: &mut RuntimeSmartContextIntentSignals,
) {
    match value {
        serde_json::Value::String(text) => signals.add_intent_text(text),
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_collect_intent_text_parts(item, signals);
            }
        }
        serde_json::Value::Object(object) => {
            for key in ["text", "input_text", "content"] {
                if let Some(item) = object.get(key) {
                    runtime_smart_context_collect_intent_text_parts(item, signals);
                }
            }
        }
        _ => {}
    }
}

fn runtime_smart_context_intent_term_is_error_code(term: &str) -> bool {
    let rest = term
        .strip_prefix('E')
        .or_else(|| term.strip_prefix("TS"))
        .or_else(|| term.strip_prefix('F'));
    rest.is_some_and(|value| value.len() >= 3 && value.chars().all(|ch| ch.is_ascii_digit()))
}

fn runtime_smart_context_intent_term_is_path(term: &str) -> bool {
    term.contains('/')
        || term.rsplit_once('.').is_some_and(|(_, extension)| {
            matches!(
                extension,
                "c" | "cc"
                    | "cpp"
                    | "css"
                    | "go"
                    | "h"
                    | "hpp"
                    | "html"
                    | "js"
                    | "jsx"
                    | "json"
                    | "md"
                    | "py"
                    | "rs"
                    | "toml"
                    | "ts"
                    | "tsx"
                    | "yaml"
                    | "yml"
            )
        })
}

fn runtime_smart_context_intent_term_is_symbol(term: &str) -> bool {
    term.contains("::") || term.contains('#')
}

fn runtime_smart_context_command_kind_hint_label(
    hint: prodex_context::CommandOutputKind,
) -> &'static str {
    match hint {
        prodex_context::CommandOutputKind::Auto => "auto",
        prodex_context::CommandOutputKind::GitStatus => "git-status",
        prodex_context::CommandOutputKind::GitDiff => "git-diff",
        prodex_context::CommandOutputKind::RustDiagnostics => "rust-diagnostics",
        prodex_context::CommandOutputKind::Diagnostics => "diagnostics",
        prodex_context::CommandOutputKind::GitLog => "git-log",
        prodex_context::CommandOutputKind::Search => "search",
        prodex_context::CommandOutputKind::FileList => "file-list",
        prodex_context::CommandOutputKind::LogStream => "log-stream",
        prodex_context::CommandOutputKind::NoisySuccess => "noisy-success",
        prodex_context::CommandOutputKind::Plain => "plain",
    }
}

fn with_runtime_smart_context_artifacts<R>(
    shared: &RuntimeRotationProxyShared,
    action: impl FnOnce(&mut RuntimeSmartContextArtifactStore) -> R,
) -> Option<R> {
    with_runtime_smart_context_proxy_state(shared, |state| action(&mut state.artifacts))
}

fn with_runtime_smart_context_proxy_state<R>(
    shared: &RuntimeRotationProxyShared,
    action: impl FnOnce(&mut RuntimeSmartContextProxyState) -> R,
) -> Option<R> {
    let states = RUNTIME_SMART_CONTEXT_PROXY_STATES.get()?;
    let mut states = states.lock().ok()?;
    let state = states.get_mut(&shared.log_path)?;
    state.enabled.then(|| action(state))
}

fn persist_runtime_smart_context_artifacts(shared: &RuntimeRotationProxyShared) {
    let Some(states) = RUNTIME_SMART_CONTEXT_PROXY_STATES.get() else {
        return;
    };
    let Ok(states) = states.lock() else {
        return;
    };
    let Some(state) = states.get(&shared.log_path) else {
        return;
    };
    if !state.enabled {
        return;
    }
    let Some(path) = state.artifact_path.clone() else {
        return;
    };
    let store = state.artifacts.clone();
    schedule_runtime_smart_context_artifact_save(shared, path, store, "smart_context_artifacts");
}

fn runtime_smart_context_exact_header(request: &RuntimeProxyRequest) -> bool {
    runtime_proxy_request_header_value(&request.headers, "x-prodex-smart-context")
        .is_some_and(|value| value.eq_ignore_ascii_case("exact"))
}

fn runtime_smart_context_missing_artifact_refs(
    value: &serde_json::Value,
    shared: &RuntimeRotationProxyShared,
) -> Vec<String> {
    let ref_ids = runtime_smart_context_collect_rehydratable_artifact_ref_ids(value);
    if ref_ids.is_empty() {
        return Vec::new();
    }
    with_runtime_smart_context_artifacts(shared, |store| {
        ref_ids
            .into_iter()
            .filter(|id| !store.contains(id))
            .collect::<Vec<_>>()
    })
    .unwrap_or_default()
}

fn runtime_smart_context_collect_rehydratable_artifact_ref_ids(
    value: &serde_json::Value,
) -> Vec<String> {
    runtime_smart_context_collect_rehydratable_artifact_refs(value)
        .into_iter()
        .map(|reference| reference.id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn runtime_smart_context_collect_rehydratable_artifact_refs(
    value: &serde_json::Value,
) -> Vec<RuntimeSmartContextArtifactReference> {
    let aliases = runtime_smart_context_collect_artifact_aliases(value);
    let mut refs = BTreeSet::<RuntimeSmartContextArtifactReference>::new();
    runtime_smart_context_collect_rehydratable_artifact_refs_from_value(value, &aliases, &mut refs);
    refs.into_iter().collect()
}

fn runtime_smart_context_collect_artifact_refs(
    value: &serde_json::Value,
) -> Vec<RuntimeSmartContextArtifactReference> {
    let aliases = runtime_smart_context_collect_artifact_aliases(value);
    let mut refs = BTreeSet::<RuntimeSmartContextArtifactReference>::new();
    runtime_smart_context_collect_artifact_refs_from_value(value, &aliases, &mut refs);
    refs.into_iter().collect()
}

fn runtime_smart_context_collect_rehydratable_artifact_refs_from_value(
    value: &serde_json::Value,
    aliases: &BTreeMap<String, String>,
    refs: &mut BTreeSet<RuntimeSmartContextArtifactReference>,
) {
    if runtime_smart_context_value_is_static_context_item(value) {
        return;
    }
    match value {
        serde_json::Value::Object(object) => {
            for (key, item) in object {
                if runtime_smart_context_static_prompt_field_key(key) {
                    continue;
                }
                runtime_smart_context_collect_rehydratable_artifact_refs_from_value(
                    item, aliases, refs,
                );
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_collect_rehydratable_artifact_refs_from_value(
                    item, aliases, refs,
                );
            }
        }
        _ => runtime_smart_context_collect_artifact_refs_from_value(value, aliases, refs),
    }
}

fn runtime_smart_context_collect_artifact_refs_from_value(
    value: &serde_json::Value,
    aliases: &BTreeMap<String, String>,
    refs: &mut BTreeSet<RuntimeSmartContextArtifactReference>,
) {
    match value {
        serde_json::Value::String(text)
            if text.contains("prodex-artifact:")
                || text.contains(SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX)
                || text.contains('@')
                || text.contains("prodex smart context artifact")
                || text.contains("prodex-sc ") =>
        {
            for reference in runtime_smart_context_artifact_ref_occurrences_from_text(text, aliases)
            {
                refs.insert(reference);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_collect_artifact_refs_from_value(item, aliases, refs);
            }
        }
        serde_json::Value::Object(object) => {
            for item in object.values() {
                runtime_smart_context_collect_artifact_refs_from_value(item, aliases, refs);
            }
        }
        _ => {}
    }
}

fn runtime_smart_context_collect_artifact_aliases(
    value: &serde_json::Value,
) -> BTreeMap<String, String> {
    let mut aliases = BTreeMap::new();
    runtime_smart_context_collect_artifact_aliases_from_value(value, &mut aliases);
    aliases
}

fn runtime_smart_context_collect_artifact_aliases_from_value(
    value: &serde_json::Value,
    aliases: &mut BTreeMap<String, String>,
) {
    match value {
        serde_json::Value::String(text) if text.contains('@') && text.contains('=') => {
            for token in runtime_smart_context_artifact_ref_tokens(text) {
                if let Some((alias, id)) = runtime_smart_context_parse_artifact_alias(token) {
                    aliases.entry(alias).or_insert(id);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_collect_artifact_aliases_from_value(item, aliases);
            }
        }
        serde_json::Value::Object(object) => {
            for item in object.values() {
                runtime_smart_context_collect_artifact_aliases_from_value(item, aliases);
            }
        }
        _ => {}
    }
}

fn runtime_smart_context_artifact_ref_occurrences_from_text(
    text: &str,
    aliases: &BTreeMap<String, String>,
) -> Vec<RuntimeSmartContextArtifactReference> {
    runtime_smart_context_artifact_ref_tokens(text)
        .into_iter()
        .filter_map(|token| {
            runtime_smart_context_parse_artifact_reference_with_aliases(token, aliases)
        })
        .collect()
}

fn runtime_smart_context_artifact_ref_tokens(text: &str) -> Vec<&str> {
    text.split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ')' | ']' | '}'))
        .collect()
}

fn runtime_smart_context_parse_artifact_alias(token: &str) -> Option<(String, String)> {
    let token = runtime_smart_context_trim_artifact_ref_token(token);
    let (alias, reference) = token.split_once('=')?;
    if !runtime_smart_context_artifact_alias_valid(alias) {
        return None;
    }
    let reference = runtime_smart_context_parse_non_alias_artifact_reference(reference)?;
    Some((alias.to_string(), reference.id))
}

fn runtime_smart_context_parse_artifact_reference_with_aliases(
    token: &str,
    aliases: &BTreeMap<String, String>,
) -> Option<RuntimeSmartContextArtifactReference> {
    let token = runtime_smart_context_trim_artifact_ref_token(token);
    if token.starts_with('@') && token.contains('=') {
        return None;
    }
    if let Some(reference) = runtime_smart_context_parse_alias_artifact_reference(token, aliases) {
        return Some(reference);
    }
    runtime_smart_context_parse_non_alias_artifact_reference(token)
}

fn runtime_smart_context_parse_alias_artifact_reference(
    token: &str,
    aliases: &BTreeMap<String, String>,
) -> Option<RuntimeSmartContextArtifactReference> {
    let (alias, suffix) = runtime_smart_context_split_artifact_alias_ref(token)?;
    let id = aliases.get(alias)?;
    Some(RuntimeSmartContextArtifactReference {
        id: id.clone(),
        marker: token.to_string(),
        line_range: runtime_smart_context_parse_line_range(suffix),
    })
}

fn runtime_smart_context_split_artifact_alias_ref(token: &str) -> Option<(&str, &str)> {
    let rest = token.strip_prefix('@')?;
    let digit_len = rest
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .map(char::len_utf8)
        .sum::<usize>();
    if digit_len == 0 {
        return None;
    }
    let alias_end = 1 + digit_len;
    Some((&token[..alias_end], &token[alias_end..]))
}

fn runtime_smart_context_artifact_alias_valid(alias: &str) -> bool {
    alias
        .strip_prefix('@')
        .is_some_and(|digits| !digits.is_empty() && digits.chars().all(|ch| ch.is_ascii_digit()))
}

fn runtime_smart_context_trim_artifact_ref_token(token: &str) -> &str {
    token.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\''
                | '`'
                | ':'
                | ';'
                | '.'
                | '!'
                | '?'
                | '('
                | '['
                | '{'
                | '<'
                | ')'
                | ']'
                | '}'
                | '>'
        )
    })
}

fn runtime_smart_context_parse_non_alias_artifact_reference(
    token: &str,
) -> Option<RuntimeSmartContextArtifactReference> {
    let token = runtime_smart_context_trim_artifact_ref_token(token);
    let raw = if let Some(raw) = token.strip_prefix("prodex-artifact:") {
        Cow::Borrowed(raw)
    } else if let Some(raw) = token.strip_prefix(SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX) {
        if raw.starts_with("sc:") {
            Cow::Borrowed(raw)
        } else {
            Cow::Owned(format!("sc:{raw}"))
        }
    } else {
        Cow::Borrowed(token)
    };
    let raw = raw.as_ref();
    if !raw.starts_with("sc:") {
        return None;
    }

    let mut id_end = 3usize;
    for (offset, ch) in raw[3..].char_indices() {
        if ch.is_ascii_hexdigit() {
            id_end = 3 + offset + ch.len_utf8();
        } else {
            break;
        }
    }
    if id_end == 3 {
        return None;
    }

    Some(RuntimeSmartContextArtifactReference {
        id: raw[..id_end].to_string(),
        marker: token.to_string(),
        line_range: runtime_smart_context_parse_line_range(&raw[id_end..]),
    })
}

fn runtime_smart_context_parse_line_range(suffix: &str) -> Option<RuntimeSmartContextLineRange> {
    let suffix = suffix
        .strip_prefix('#')
        .or_else(|| suffix.strip_prefix(':'))
        .or_else(|| suffix.strip_prefix('?'))?;
    let suffix = suffix.strip_prefix("lines=").unwrap_or(suffix);
    let suffix = suffix
        .strip_prefix('L')
        .or_else(|| suffix.strip_prefix('l'))
        .unwrap_or(suffix);
    let (start, end) = suffix.split_once('-').unwrap_or((suffix, suffix));
    let start = runtime_smart_context_parse_line_number(start)?;
    let end = runtime_smart_context_parse_line_number(end)?;
    (start > 0 && end >= start).then_some(RuntimeSmartContextLineRange { start, end })
}

fn runtime_smart_context_parse_line_number(value: &str) -> Option<usize> {
    value
        .strip_prefix('L')
        .or_else(|| value.strip_prefix('l'))
        .unwrap_or(value)
        .parse::<usize>()
        .ok()
}

fn runtime_smart_context_auto_rehydrate_plan(
    value: &serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    token_budget: usize,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
) -> runtime_proxy_crate::SmartContextRehydratePlan {
    let mut refs_by_id = BTreeMap::<String, usize>::new();
    let mut available_ids = BTreeSet::<String>::new();
    for reference in runtime_smart_context_collect_rehydratable_artifact_refs(value) {
        if store.contains(&reference.id) {
            available_ids.insert(reference.id.clone());
        }
        let token_cost =
            runtime_smart_context_rehydrate_ref_token_cost(&reference, store).unwrap_or(0);
        refs_by_id
            .entry(reference.id)
            .and_modify(|cost| *cost = cost.saturating_add(token_cost))
            .or_insert(token_cost);
    }

    runtime_proxy_crate::smart_context_auto_rehydrate_plan(
        refs_by_id.into_iter().map(|(id, token_cost)| {
            runtime_proxy_crate::SmartContextRehydrateRef {
                id,
                token_cost,
                required: true,
            }
        }),
        available_ids,
        token_budget,
        tier,
    )
}

fn runtime_smart_context_rehydrate_ref_token_cost(
    reference: &RuntimeSmartContextArtifactReference,
    store: &RuntimeSmartContextArtifactStore,
) -> Option<usize> {
    let artifact_text = store.get_text(&reference.id)?;
    let rehydrated =
        runtime_smart_context_rehydrated_artifact_text(&artifact_text, reference.line_range)?;
    runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(rehydrated.len())
        .try_into()
        .ok()
}

fn runtime_smart_context_deferred_rehydrate_refs(
    plan: &runtime_proxy_crate::SmartContextRehydratePlan,
) -> Vec<String> {
    plan.actions
        .iter()
        .filter_map(|action| match action {
            runtime_proxy_crate::SmartContextRehydrateAction::Defer { id, .. } => Some(id.clone()),
            runtime_proxy_crate::SmartContextRehydrateAction::Rehydrate { .. } => None,
        })
        .collect()
}

#[cfg(test)]
fn runtime_smart_context_rehydrate_value(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let plan = runtime_smart_context_auto_rehydrate_plan(
        value,
        store,
        usize::MAX,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Exact,
    );
    runtime_smart_context_rehydrate_value_with_plan(value, store, &plan, stats);
}

fn runtime_smart_context_rehydrate_value_with_plan(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    plan: &runtime_proxy_crate::SmartContextRehydratePlan,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let aliases = runtime_smart_context_collect_artifact_aliases(value);
    let rehydrate_ids = plan
        .actions
        .iter()
        .filter_map(|action| match action {
            runtime_proxy_crate::SmartContextRehydrateAction::Rehydrate { id, .. } => {
                Some(id.clone())
            }
            runtime_proxy_crate::SmartContextRehydrateAction::Defer { .. } => None,
        })
        .collect::<BTreeSet<_>>();
    runtime_smart_context_rehydrate_value_for_ids(value, store, &rehydrate_ids, &aliases, stats);
}

fn runtime_smart_context_rehydrate_value_for_ids(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    rehydrate_ids: &BTreeSet<String>,
    aliases: &BTreeMap<String, String>,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    if runtime_smart_context_value_is_static_context_item(value) {
        return;
    }
    match value {
        serde_json::Value::String(text) => {
            let mut next = text.clone();
            for reference in runtime_smart_context_artifact_ref_occurrences_from_text(text, aliases)
            {
                if rehydrate_ids.contains(&reference.id)
                    && let Some(artifact_text) = store.get_text(&reference.id)
                    && let Some(rehydrated_text) = runtime_smart_context_rehydrated_artifact_text(
                        &artifact_text,
                        reference.line_range,
                    )
                {
                    let legacy_marker = format!("prodex-artifact:{}", reference.id);
                    let short_marker = runtime_smart_context_artifact_ref(&reference.id);
                    if reference.line_range.is_none()
                        && runtime_smart_context_text_is_artifact_marker_summary(
                            &next,
                            &reference.id,
                        )
                    {
                        next = rehydrated_text;
                        stats.rehydrated_refs += 1;
                        break;
                    } else if next.contains(&reference.marker) {
                        next = next.replace(&reference.marker, &rehydrated_text);
                        stats.rehydrated_refs += 1;
                    } else if next.contains(&legacy_marker) {
                        next = next.replace(&legacy_marker, &rehydrated_text);
                        stats.rehydrated_refs += 1;
                    } else if next.contains(&short_marker) {
                        next = next.replace(&short_marker, &rehydrated_text);
                        stats.rehydrated_refs += 1;
                    } else if next.trim() == reference.id {
                        next = rehydrated_text;
                        stats.rehydrated_refs += 1;
                    }
                }
            }
            *text = next;
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_rehydrate_value_for_ids(
                    item,
                    store,
                    rehydrate_ids,
                    aliases,
                    stats,
                );
            }
        }
        serde_json::Value::Object(object) => {
            for (key, item) in object {
                if runtime_smart_context_static_prompt_field_key(key) {
                    continue;
                }
                runtime_smart_context_rehydrate_value_for_ids(
                    item,
                    store,
                    rehydrate_ids,
                    aliases,
                    stats,
                );
            }
        }
        _ => {}
    }
}

fn runtime_smart_context_text_is_artifact_marker_summary(text: &str, id: &str) -> bool {
    let Some(first_line) = text.trim_start().lines().next() else {
        return false;
    };
    let legacy = (first_line.starts_with("prodex-sc artifact ")
        || first_line.starts_with("prodex-sc repeat "))
        && first_line.contains(&format!("prodex-artifact:{id}"));
    let short = (first_line.starts_with("psc art ")
        || first_line.starts_with("psc repeat ")
        || first_line.starts_with("prodex-sc artifact ")
        || first_line.starts_with("prodex-sc repeat "))
        && first_line.contains(&runtime_smart_context_artifact_ref(id));
    legacy || short
}

fn runtime_smart_context_rehydrated_artifact_text(
    artifact_text: &str,
    line_range: Option<RuntimeSmartContextLineRange>,
) -> Option<String> {
    let Some(line_range) = line_range else {
        return Some(artifact_text.to_string());
    };
    let lines = artifact_text.lines().collect::<Vec<_>>();
    if line_range.start > lines.len() {
        return None;
    }
    let end = line_range.end.min(lines.len());
    Some(lines[line_range.start - 1..end].join("\n"))
}

fn runtime_smart_context_selective_rehydrate_semantic_ranges(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    if exactness_guard.decision != runtime_proxy_crate::SmartContextExactnessDecision::Allow
        || runtime_smart_context_selective_rehydrate_terms_empty(terms)
    {
        return 0;
    }
    runtime_smart_context_selective_rehydrate_semantic_ranges_inner(value, store, terms, stats)
}

fn runtime_smart_context_selective_rehydrate_semantic_ranges_inner(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    if runtime_smart_context_value_is_static_context_item(value) {
        return 0;
    }
    match value {
        serde_json::Value::String(text) => {
            runtime_smart_context_selective_rehydrate_semantic_ranges_in_text(
                text, store, terms, stats,
            )
        }
        serde_json::Value::Array(items) => items
            .iter_mut()
            .map(|item| {
                runtime_smart_context_selective_rehydrate_semantic_ranges_inner(
                    item, store, terms, stats,
                )
            })
            .sum(),
        serde_json::Value::Object(object) => {
            let mut count = 0usize;
            for (key, item) in object {
                if runtime_smart_context_static_prompt_field_key(key) {
                    continue;
                }
                count = count.saturating_add(
                    runtime_smart_context_selective_rehydrate_semantic_ranges_inner(
                        item, store, terms, stats,
                    ),
                );
            }
            count
        }
        _ => 0,
    }
}

fn runtime_smart_context_selective_rehydrate_semantic_ranges_in_text(
    text: &mut String,
    store: &RuntimeSmartContextArtifactStore,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    let ids = runtime_smart_context_collect_artifact_refs(&serde_json::Value::String(text.clone()))
        .into_iter()
        .map(|reference| reference.id)
        .collect::<BTreeSet<_>>();
    if ids.is_empty() {
        return 0;
    }

    let mut next = text.clone();
    let mut rehydrated_ranges = 0usize;
    for id in ids {
        let Some(line_index) = store.line_index(&id) else {
            continue;
        };
        let Some((appendix, range_count)) =
            runtime_smart_context_matching_semantic_range_appendix(&id, line_index, &next, terms)
        else {
            continue;
        };
        if !next.ends_with('\n') {
            next.push('\n');
        }
        next.push('\n');
        next.push_str(&appendix);
        rehydrated_ranges = rehydrated_ranges.saturating_add(range_count);
    }

    if rehydrated_ranges > 0 {
        *text = next;
        stats.rehydrated_refs = stats.rehydrated_refs.saturating_add(rehydrated_ranges);
    }
    rehydrated_ranges
}

fn runtime_smart_context_matching_semantic_range_appendix(
    artifact_id: &str,
    line_index: &RuntimeSmartContextArtifactLineIndex,
    current_text: &str,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> Option<(String, usize)> {
    let ranges = runtime_smart_context_matching_semantic_ranges(
        artifact_id,
        line_index,
        current_text,
        terms,
    );
    if ranges.is_empty() {
        return None;
    }

    runtime_smart_context_render_exact_appendix(
        SMART_CONTEXT_LABEL_SEMANTIC_EXACT,
        ranges
            .iter()
            .map(|range| RuntimeSmartContextExactAppendixRange {
                reference: runtime_smart_context_artifact_line_ref(
                    artifact_id,
                    range.start,
                    range.end,
                ),
                body: range.text.clone(),
            })
            .collect(),
    )
}

fn runtime_smart_context_matching_semantic_ranges<'a>(
    artifact_id: &str,
    line_index: &'a RuntimeSmartContextArtifactLineIndex,
    current_text: &str,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> Vec<&'a RuntimeSmartContextArtifactSemanticLineRange> {
    let mut ranges = Vec::new();
    let mut seen = BTreeSet::<(usize, usize, String)>::new();
    for range in line_index
        .file_location_ranges
        .iter()
        .chain(line_index.diff_hunk_ranges.iter())
        .chain(line_index.test_failure_ranges.iter())
        .chain(line_index.error_ranges.iter())
    {
        if !runtime_smart_context_artifact_semantic_range_valid(range)
            || !runtime_smart_context_semantic_range_matches_terms(range, terms)
            || current_text.contains(&runtime_smart_context_artifact_line_ref(
                artifact_id,
                range.start,
                range.end,
            ))
            || current_text.contains(&format!(
                "prodex-artifact:{artifact_id}#L{}-L{}",
                range.start, range.end
            ))
            || current_text.contains(&range.text)
        {
            continue;
        }
        if seen.insert((range.start, range.end, range.content_hash.clone())) {
            ranges.push(range);
        }
    }
    ranges.sort_by_key(|range| (range.start, range.end));
    ranges.truncate(runtime_smart_context_semantic_rehydrate_range_cap(terms));
    ranges
}

fn runtime_smart_context_semantic_rehydrate_range_cap(
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> usize {
    if runtime_smart_context_selective_rehydrate_terms_narrow(terms) {
        SMART_CONTEXT_SEMANTIC_REHYDRATE_NARROW_MAX_RANGES
    } else {
        SMART_CONTEXT_SEMANTIC_REHYDRATE_GLOBAL_MAX_RANGES
    }
}

fn runtime_smart_context_selective_rehydrate_terms_narrow(
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> bool {
    let term_count = terms
        .file_paths
        .len()
        .saturating_add(terms.error_codes.len())
        .saturating_add(terms.test_symbols.len())
        .saturating_add(terms.diff_hunks.len());
    term_count == 1
}

fn runtime_smart_context_selective_rehydrate_terms_empty(
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> bool {
    terms.file_paths.is_empty()
        && terms.error_codes.is_empty()
        && terms.test_symbols.is_empty()
        && terms.diff_hunks.is_empty()
}

fn runtime_smart_context_semantic_range_matches_terms(
    range: &RuntimeSmartContextArtifactSemanticLineRange,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> bool {
    range
        .path
        .as_deref()
        .is_some_and(|path| terms.file_paths.contains(path))
        || range
            .code
            .as_deref()
            .is_some_and(|code| terms.error_codes.contains(code))
        || range
            .symbol
            .as_deref()
            .is_some_and(|symbol| terms.test_symbols.contains(symbol))
        || terms
            .diff_hunks
            .iter()
            .any(|term| runtime_smart_context_diff_hunk_range_matches_term(range, term))
}

fn runtime_smart_context_diff_hunk_range_matches_term(
    range: &RuntimeSmartContextArtifactSemanticLineRange,
    term: &RuntimeSmartContextSelectiveDiffHunkTerm,
) -> bool {
    range.label.as_deref() == Some("diff_hunk")
        && term
            .path
            .as_deref()
            .map(|path| range.path.as_deref() == Some(path))
            .unwrap_or(true)
        && term
            .old_start
            .map(|old_start| range.old_start == Some(old_start))
            .unwrap_or(true)
        && term
            .new_start
            .map(|new_start| range.new_start == Some(new_start))
            .unwrap_or(true)
}

fn runtime_smart_context_artifact_semantic_range_valid(
    range: &RuntimeSmartContextArtifactSemanticLineRange,
) -> bool {
    range.start > 0
        && range.end >= range.start
        && range.byte_len == range.text.len()
        && range.content_hash == runtime_proxy_crate::smart_context_hash_text(&range.text)
}

fn runtime_smart_context_try_surgical_rehydrate_critical_ranges(
    value: &serde_json::Value,
    shared: &RuntimeRotationProxyShared,
    request_body: &[u8],
    exactness: &runtime_proxy_crate::SmartContextExactnessGuard,
    unresolved_rehydrate_refs: &[String],
    stats: &RuntimeSmartContextTransformStats,
) -> Option<(Vec<u8>, RuntimeSmartContextTransformStats)> {
    with_runtime_smart_context_artifacts(shared, |store| {
        let mut repaired_value = value.clone();
        let mut repaired_stats = stats.clone();
        if runtime_smart_context_rehydrate_lost_critical_ranges(
            &mut repaired_value,
            store,
            &mut repaired_stats,
        ) == 0
        {
            return None;
        }

        let repaired_body = serde_json::to_vec(&repaired_value).ok()?;
        let regression_check = runtime_smart_context_regression_self_check(
            request_body,
            &repaired_body,
            exactness.clone(),
            unresolved_rehydrate_refs.to_vec(),
        );
        let critical_signal_check =
            runtime_smart_context_critical_signal_self_check(request_body, &repaired_body);
        runtime_smart_context_fallback_exact_reason(
            &regression_check,
            critical_signal_check,
            &repaired_stats,
        )
        .is_none()
        .then_some((repaired_body, repaired_stats))
    })
    .flatten()
}

fn runtime_smart_context_rehydrate_lost_critical_ranges(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    if runtime_smart_context_value_is_static_context_item(value) {
        return 0;
    }
    match value {
        serde_json::Value::String(text) => {
            runtime_smart_context_rehydrate_lost_critical_ranges_in_text(text, store, stats)
        }
        serde_json::Value::Array(items) => items
            .iter_mut()
            .map(|item| runtime_smart_context_rehydrate_lost_critical_ranges(item, store, stats))
            .sum(),
        serde_json::Value::Object(object) => {
            let mut count = 0usize;
            for (key, item) in object {
                if runtime_smart_context_static_prompt_field_key(key) {
                    continue;
                }
                count = count.saturating_add(runtime_smart_context_rehydrate_lost_critical_ranges(
                    item, store, stats,
                ));
            }
            count
        }
        _ => 0,
    }
}

fn runtime_smart_context_rehydrate_lost_critical_ranges_in_text(
    text: &mut String,
    store: &RuntimeSmartContextArtifactStore,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    let ids = runtime_smart_context_collect_artifact_refs(&serde_json::Value::String(text.clone()))
        .into_iter()
        .map(|reference| reference.id)
        .collect::<BTreeSet<_>>();
    if ids.is_empty() {
        return 0;
    }

    let mut next = text.clone();
    let mut rehydrated_ranges = 0usize;
    for id in ids {
        let Some(artifact_text) = store.get_text(&id) else {
            continue;
        };
        let artifact_line_index = store.line_index(&id);
        let Some((appendix, range_count)) = runtime_smart_context_missing_critical_range_appendix(
            &id,
            &artifact_text,
            artifact_line_index,
            &next,
        ) else {
            continue;
        };
        if !next.ends_with('\n') {
            next.push('\n');
        }
        next.push('\n');
        next.push_str(&appendix);
        rehydrated_ranges = rehydrated_ranges.saturating_add(range_count);
    }

    if rehydrated_ranges > 0 {
        *text = next;
        stats.rehydrated_refs = stats.rehydrated_refs.saturating_add(rehydrated_ranges);
    }
    rehydrated_ranges
}

enum RuntimeSmartContextIndexedCriticalAppendix {
    Found(String, usize),
    NoLoss,
    Unusable,
}

fn runtime_smart_context_missing_critical_range_appendix(
    artifact_id: &str,
    artifact_text: &str,
    artifact_line_index: Option<&RuntimeSmartContextArtifactLineIndex>,
    current_text: &str,
) -> Option<(String, usize)> {
    if let Some(line_index) = artifact_line_index {
        match runtime_smart_context_missing_indexed_critical_range_appendix(
            artifact_id,
            line_index,
            current_text,
        ) {
            RuntimeSmartContextIndexedCriticalAppendix::Found(appendix, range_count) => {
                return Some((appendix, range_count));
            }
            RuntimeSmartContextIndexedCriticalAppendix::NoLoss => return None,
            RuntimeSmartContextIndexedCriticalAppendix::Unusable => {}
        }
    }

    let ranges = prodex_context::critical_signal_lost_line_ranges_with_options(
        artifact_text,
        current_text,
        prodex_context::CriticalSignalLineRangeOptions {
            context_lines: 1,
            max_ranges: SMART_CONTEXT_SURGICAL_CRITICAL_MAX_RANGES,
            max_range_lines: 6,
        },
    );
    if ranges.is_empty() {
        return None;
    }

    let mut exact_ranges = Vec::new();
    for range in ranges {
        let exact = runtime_smart_context_rehydrated_artifact_text(
            artifact_text,
            Some(RuntimeSmartContextLineRange {
                start: range.start,
                end: range.end,
            }),
        )?;
        if exact.trim().is_empty() {
            continue;
        }
        exact_ranges.push(RuntimeSmartContextExactAppendixRange {
            reference: runtime_smart_context_artifact_line_ref(artifact_id, range.start, range.end),
            body: exact,
        });
    }

    runtime_smart_context_render_exact_appendix(SMART_CONTEXT_LABEL_CRITICAL_EXACT, exact_ranges)
}

fn runtime_smart_context_missing_indexed_critical_range_appendix(
    artifact_id: &str,
    line_index: &RuntimeSmartContextArtifactLineIndex,
    current_text: &str,
) -> RuntimeSmartContextIndexedCriticalAppendix {
    if line_index.critical_ranges.is_empty() {
        return if line_index.complete {
            RuntimeSmartContextIndexedCriticalAppendix::NoLoss
        } else {
            RuntimeSmartContextIndexedCriticalAppendix::Unusable
        };
    }

    let mut synthetic = String::new();
    let mut synthetic_line_to_range = Vec::new();
    for (range_index, range) in line_index.critical_ranges.iter().enumerate() {
        if !runtime_smart_context_artifact_line_index_range_valid(range) {
            return RuntimeSmartContextIndexedCriticalAppendix::Unusable;
        }
        for line in range.text.lines() {
            if !synthetic.is_empty() {
                synthetic.push('\n');
            }
            synthetic.push_str(line);
            synthetic_line_to_range.push(range_index);
        }
    }

    if synthetic_line_to_range.is_empty() {
        return if line_index.complete {
            RuntimeSmartContextIndexedCriticalAppendix::NoLoss
        } else {
            RuntimeSmartContextIndexedCriticalAppendix::Unusable
        };
    }

    let lost_synthetic_ranges = prodex_context::critical_signal_lost_line_ranges_with_options(
        &synthetic,
        current_text,
        prodex_context::CriticalSignalLineRangeOptions {
            context_lines: 0,
            max_ranges: SMART_CONTEXT_SURGICAL_CRITICAL_MAX_RANGES,
            max_range_lines: 1,
        },
    );
    if lost_synthetic_ranges.is_empty() {
        return if line_index.complete {
            RuntimeSmartContextIndexedCriticalAppendix::NoLoss
        } else {
            RuntimeSmartContextIndexedCriticalAppendix::Unusable
        };
    }

    let mut selected = BTreeSet::<usize>::new();
    for lost_range in lost_synthetic_ranges {
        for synthetic_line in lost_range.start..=lost_range.end {
            let Some(range_index) = synthetic_line_to_range.get(synthetic_line - 1) else {
                return RuntimeSmartContextIndexedCriticalAppendix::Unusable;
            };
            selected.insert(*range_index);
            if selected.len() >= SMART_CONTEXT_SURGICAL_CRITICAL_MAX_RANGES {
                break;
            }
        }
        if selected.len() >= SMART_CONTEXT_SURGICAL_CRITICAL_MAX_RANGES {
            break;
        }
    }

    let mut exact_ranges = Vec::new();
    for range_index in &selected {
        let range = &line_index.critical_ranges[*range_index];
        if range.text.trim().is_empty() {
            continue;
        }
        exact_ranges.push(RuntimeSmartContextExactAppendixRange {
            reference: runtime_smart_context_artifact_line_ref(artifact_id, range.start, range.end),
            body: range.text.clone(),
        });
    }

    let Some((appendix, range_count)) = runtime_smart_context_render_exact_appendix(
        SMART_CONTEXT_LABEL_CRITICAL_EXACT,
        exact_ranges,
    ) else {
        return if line_index.complete {
            RuntimeSmartContextIndexedCriticalAppendix::NoLoss
        } else {
            RuntimeSmartContextIndexedCriticalAppendix::Unusable
        };
    };

    RuntimeSmartContextIndexedCriticalAppendix::Found(appendix, range_count)
}

fn runtime_smart_context_artifact_line_index_range_valid(
    range: &RuntimeSmartContextArtifactLineRange,
) -> bool {
    range.start > 0
        && range.end >= range.start
        && range.byte_len == range.text.len()
        && range.content_hash == runtime_proxy_crate::smart_context_hash_text(&range.text)
}

fn runtime_smart_context_condense_tool_outputs(
    value: &mut serde_json::Value,
    store: &mut RuntimeSmartContextArtifactStore,
    request_id: u64,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    inline_limit: usize,
    intent_signals: &RuntimeSmartContextIntentSignals,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let Some(input) = value
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    let tool_call_metadata = runtime_smart_context_tool_call_metadata_by_call_id(input);
    for item in input {
        let Some(object) = item.as_object_mut() else {
            continue;
        };
        let item_type = object
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if !item_type.ends_with("_call_output") {
            continue;
        }
        let linked_metadata = runtime_smart_context_tool_call_id(object)
            .and_then(|call_id| tool_call_metadata.get(call_id));
        let compaction_metadata =
            runtime_smart_context_tool_output_compaction_metadata(object, linked_metadata);
        for field in ["output", "content"] {
            let Some(output) = object
                .get(field)
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
            else {
                continue;
            };
            if output.len() <= inline_limit.max(256) {
                continue;
            }
            let existing_artifact = store.artifact_ref_for_exact_text(&output);
            let Some(artifact) = existing_artifact
                .clone()
                .or_else(|| store.insert_text(request_id, &output))
            else {
                continue;
            };
            let replacement = if existing_artifact.is_some() {
                stats.repeat_tool_output_refs += 1;
                runtime_smart_context_artifact_reference_summary(&artifact)
            } else {
                let compacted = runtime_smart_context_progressive_tool_output_summary(
                    &artifact,
                    &output,
                    RuntimeSmartContextArtifactIndexes {
                        line_index: store.line_index(&artifact.id),
                        chunk_index: store.chunk_index(&artifact.id),
                    },
                    tier,
                    inline_limit,
                    &compaction_metadata,
                    &intent_signals.intent_terms,
                );
                runtime_smart_context_artifact_summary(&artifact, &compacted)
            };
            if replacement.len().saturating_mul(100) >= output.len().saturating_mul(90) {
                continue;
            }
            object.insert(field.to_string(), serde_json::Value::String(replacement));
            if existing_artifact.is_none() {
                stats.artifacts_stored += 1;
            }
            stats.tool_outputs_condensed += 1;
            if runtime_smart_context_likely_blob_or_noise(&output) {
                stats.blob_outputs_condensed += 1;
            }
            break;
        }
    }
}

fn runtime_smart_context_condense_historical_tool_call_arguments(
    value: &mut serde_json::Value,
    store: &mut RuntimeSmartContextArtifactStore,
    request_id: u64,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    inline_limit: usize,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let Some(input) = value
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    let completed_call_ids = input
        .iter()
        .filter_map(|item| {
            let object = item.as_object()?;
            let item_type = object
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            item_type
                .ends_with("_call_output")
                .then(|| runtime_smart_context_tool_call_id(object))
                .flatten()
                .map(str::to_string)
        })
        .collect::<BTreeSet<_>>();
    if completed_call_ids.is_empty() {
        return;
    }

    for item in input {
        let Some(object) = item.as_object_mut() else {
            continue;
        };
        let Some(call_id) = runtime_smart_context_tool_call_id(object) else {
            continue;
        };
        if !completed_call_ids.contains(call_id) {
            continue;
        }
        let item_type = object
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if item_type.ends_with("_call_output") {
            continue;
        }
        for field in ["arguments", "input", "payload"] {
            let Some(arguments) = object.get(field) else {
                continue;
            };
            let Some(arguments_text) = runtime_smart_context_tool_argument_text(arguments) else {
                continue;
            };
            if arguments_text.len()
                <= inline_limit
                    .min(SMART_CONTEXT_TOOL_ARGS_INLINE_MIN_BYTES)
                    .max(256)
            {
                continue;
            }
            let existing_artifact = store.artifact_ref_for_exact_text(&arguments_text);
            let Some(artifact) = existing_artifact
                .clone()
                .or_else(|| store.insert_text(request_id, &arguments_text))
            else {
                continue;
            };
            let replacement =
                runtime_smart_context_tool_argument_summary(&artifact, &arguments_text, tier);
            if replacement.len().saturating_mul(100) >= arguments_text.len().saturating_mul(75) {
                continue;
            }
            object.insert(field.to_string(), serde_json::Value::String(replacement));
            if existing_artifact.is_none() {
                stats.artifacts_stored = stats.artifacts_stored.saturating_add(1);
            }
            stats.tool_call_args_condensed = stats.tool_call_args_condensed.saturating_add(1);
            break;
        }
    }
}

fn runtime_smart_context_tool_argument_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) if !text.trim().is_empty() => Some(text.clone()),
        serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
            serde_json::to_string(value).ok()
        }
        _ => None,
    }
}

fn runtime_smart_context_tool_argument_summary(
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
    text: &str,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
) -> String {
    let reference = runtime_smart_context_artifact_ref(&artifact.id);
    let preview = text
        .chars()
        .take(runtime_smart_context_tool_args_preview_max_chars(tier))
        .collect::<String>();
    format!(
        "psc args {reference} b={}; p: {}",
        artifact.byte_len,
        preview.trim()
    )
}

fn runtime_smart_context_tool_args_preview_max_chars(
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
) -> usize {
    match tier {
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal => 160,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Condensed => 240,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Large => 360,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Exact => 480,
    }
}

fn runtime_smart_context_progressive_tool_output_summary(
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
    text: &str,
    indexes: RuntimeSmartContextArtifactIndexes<'_>,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    preview_byte_limit: usize,
    metadata: &RuntimeSmartContextToolOutputCompactionMetadata,
    intent_terms: &[String],
) -> String {
    let compacted = runtime_smart_context_compact_successful_tool_output(
        text,
        tier,
        preview_byte_limit,
        metadata,
    )
    .unwrap_or_else(|| {
        runtime_smart_context_compact_tool_output_preserving_critical(
            text,
            tier,
            preview_byte_limit,
            metadata.kind_hint,
            intent_terms,
        )
    });
    let summary = runtime_smart_context_progressive_summary_excerpt(&compacted);
    let summary = runtime_smart_context_dedupe_progressive_summary_chunks(
        &artifact.id,
        text,
        &summary,
        indexes.chunk_index,
    );
    let mut sections = Vec::new();
    if !summary.trim().is_empty() {
        sections.push(format!(
            "{SMART_CONTEXT_LABEL_SUMMARY}\n{}",
            summary.trim_end()
        ));
    }
    if let Some(critical_ranges) = runtime_smart_context_progressive_critical_exact_ranges(
        &artifact.id,
        text,
        indexes.line_index,
        &summary,
    ) {
        sections.push(critical_ranges);
    }
    if sections.is_empty() {
        sections.push(format!(
            "{SMART_CONTEXT_LABEL_SUMMARY}\nlarge tool output omitted; use artifact ref or line refs to rehydrate"
        ));
    }
    sections.join("\n\n")
}

fn runtime_smart_context_progressive_summary_excerpt(text: &str) -> String {
    if text.len() <= SMART_CONTEXT_TOOL_PROGRESSIVE_SUMMARY_MAX_BYTES {
        return text.to_string();
    }

    let mut summary = String::new();
    for line in text.lines() {
        let next_len = summary
            .len()
            .saturating_add((!summary.is_empty()) as usize)
            .saturating_add(line.len());
        if next_len > SMART_CONTEXT_TOOL_PROGRESSIVE_SUMMARY_MAX_BYTES {
            break;
        }
        if !summary.is_empty() {
            summary.push('\n');
        }
        summary.push_str(line);
    }
    if summary.is_empty() {
        summary.extend(
            text.chars()
                .take(SMART_CONTEXT_TOOL_PROGRESSIVE_SUMMARY_MAX_BYTES),
        );
    }
    summary.push_str("\n[... summary truncated; use artifact ref for full output ...]");
    summary
}

fn runtime_smart_context_dedupe_progressive_summary_chunks(
    artifact_id: &str,
    original: &str,
    summary: &str,
    chunk_index: Option<&RuntimeSmartContextArtifactChunkIndex>,
) -> String {
    let Some(chunk_index) = chunk_index.filter(|index| index.complete) else {
        return summary.to_string();
    };
    if chunk_index.duplicate_chunks.is_empty() {
        return summary.to_string();
    }
    let lines = original.lines().collect::<Vec<_>>();
    let plans = chunk_index
        .duplicate_chunks
        .iter()
        .filter_map(|duplicate| {
            runtime_smart_context_duplicate_chunk_summary_plan(artifact_id, &lines, duplicate)
        })
        .collect::<Vec<_>>();
    if plans.is_empty() {
        return summary.to_string();
    }

    let mut candidate = summary.to_string();
    let mut entries = Vec::new();
    for plan in plans {
        let matches = candidate.match_indices(&plan.text).count();
        if matches < 2 {
            continue;
        }
        let marker = format!(
            "[psc dup chunk h={} b={} refs below]",
            plan.content_hash, plan.byte_len
        );
        let entry = format!(
            "- h={} b={} x={} refs={}",
            plan.content_hash,
            plan.byte_len,
            plan.occurrence_count,
            plan.refs.join(",")
        );
        let removed_bytes = plan.text.len().saturating_mul(matches.saturating_sub(1));
        let added_bytes = marker
            .len()
            .saturating_mul(matches.saturating_sub(1))
            .saturating_add(entry.len())
            .saturating_add(1);
        if added_bytes >= removed_bytes {
            continue;
        }
        candidate = runtime_smart_context_replace_repeated_exact_text_after_first(
            &candidate, &plan.text, &marker,
        );
        entries.push(entry);
    }

    if entries.is_empty() {
        return summary.to_string();
    }
    candidate.push_str("\n\n");
    candidate.push_str(SMART_CONTEXT_LABEL_DUPLICATE_CHUNKS);
    candidate.push('\n');
    candidate.push_str(&entries.join("\n"));
    if candidate.len() < summary.len() {
        candidate
    } else {
        summary.to_string()
    }
}

fn runtime_smart_context_duplicate_chunk_summary_plan(
    artifact_id: &str,
    lines: &[&str],
    duplicate: &RuntimeSmartContextArtifactDuplicateChunkFingerprint,
) -> Option<RuntimeSmartContextDuplicateChunkSummaryPlan> {
    if duplicate.occurrence_count < 2
        || duplicate.occurrence_count != duplicate.occurrences.len()
        || duplicate.byte_len == 0
    {
        return None;
    }
    let mut refs = Vec::new();
    let mut ranges = BTreeSet::<(usize, usize)>::new();
    let mut exact_text: Option<String> = None;
    for occurrence in &duplicate.occurrences {
        if occurrence.start == 0
            || occurrence.end < occurrence.start
            || !ranges.insert((occurrence.start, occurrence.end))
        {
            return None;
        }
        let text = runtime_smart_context_line_excerpt(lines, occurrence.start, occurrence.end)?;
        if text.len() != duplicate.byte_len
            || runtime_proxy_crate::smart_context_hash_text(&text) != duplicate.content_hash
        {
            return None;
        }
        if let Some(existing) = exact_text.as_ref() {
            if existing != &text {
                return None;
            }
        } else {
            exact_text = Some(text);
        }
        refs.push(runtime_smart_context_artifact_line_ref(
            artifact_id,
            occurrence.start,
            occurrence.end,
        ));
    }
    Some(RuntimeSmartContextDuplicateChunkSummaryPlan {
        text: exact_text?,
        content_hash: duplicate.content_hash.clone(),
        byte_len: duplicate.byte_len,
        occurrence_count: duplicate.occurrence_count,
        refs,
    })
}

fn runtime_smart_context_line_excerpt(lines: &[&str], start: usize, end: usize) -> Option<String> {
    if start == 0 || start > lines.len() || end < start {
        return None;
    }
    let end = end.min(lines.len());
    Some(lines[start - 1..end].join("\n"))
}

fn runtime_smart_context_render_exact_appendix(
    label: &str,
    ranges: Vec<RuntimeSmartContextExactAppendixRange>,
) -> Option<(String, usize)> {
    let range_count = ranges
        .iter()
        .filter(|range| !range.body.trim().is_empty())
        .count();
    if range_count == 0 {
        return None;
    }
    let mut rendered = vec![label.to_string()];
    let mut seen =
        BTreeMap::<(String, usize), Vec<RuntimeSmartContextSeenExactAppendixBody>>::new();
    for range in runtime_smart_context_merge_exact_appendix_ranges(ranges) {
        if range.body.trim().is_empty() {
            continue;
        }
        rendered.push(runtime_smart_context_render_exact_appendix_range(
            &range, &mut seen,
        ));
    }
    Some((rendered.join("\n"), range_count))
}

fn runtime_smart_context_merge_exact_appendix_ranges(
    ranges: Vec<RuntimeSmartContextExactAppendixRange>,
) -> Vec<RuntimeSmartContextExactAppendixRange> {
    let mut merged = Vec::<RuntimeSmartContextExactAppendixRange>::new();
    for range in ranges {
        let Some((range_base, range_lines)) =
            runtime_smart_context_parse_artifact_line_ref(&range.reference)
        else {
            merged.push(range);
            continue;
        };
        let Some(last) = merged.last_mut() else {
            merged.push(range);
            continue;
        };
        let Some((last_base, last_lines)) =
            runtime_smart_context_parse_artifact_line_ref(&last.reference)
        else {
            merged.push(range);
            continue;
        };
        if last_base != range_base
            || range_lines.start > last_lines.end.saturating_add(1)
            || range_lines.end < last_lines.start
        {
            merged.push(range);
            continue;
        }

        let overlap = if range_lines.start <= last_lines.end {
            last_lines
                .end
                .saturating_sub(range_lines.start)
                .saturating_add(1)
        } else {
            0
        };
        let next_line_count = range.body.lines().count();
        if overlap >= next_line_count && range_lines.end > last_lines.end {
            merged.push(range);
            continue;
        }
        let next_lines = range.body.lines().collect::<Vec<_>>();
        if overlap < next_lines.len() {
            if !last.body.is_empty() {
                last.body.push('\n');
            }
            last.body.push_str(&next_lines[overlap..].join("\n"));
        }
        let end = last_lines.end.max(range_lines.end);
        last.reference = format!("{last_base}#L{}-L{end}", last_lines.start);
    }
    merged
}

fn runtime_smart_context_parse_artifact_line_ref(
    reference: &str,
) -> Option<(String, RuntimeSmartContextLineRange)> {
    let (base, range) = reference.rsplit_once("#L")?;
    let (start, end) = range.split_once("-L")?;
    Some((
        base.to_string(),
        RuntimeSmartContextLineRange {
            start: start.parse().ok()?,
            end: end.parse().ok()?,
        },
    ))
}

fn runtime_smart_context_render_exact_appendix_range(
    range: &RuntimeSmartContextExactAppendixRange,
    seen: &mut BTreeMap<(String, usize), Vec<RuntimeSmartContextSeenExactAppendixBody>>,
) -> String {
    let content_hash = runtime_proxy_crate::smart_context_hash_text(&range.body);
    let byte_len = range.body.len();
    let key = (content_hash.clone(), byte_len);
    if let Some(entries) = seen.get_mut(&key)
        && let Some(existing) = entries.iter_mut().find(|entry| entry.body == range.body)
    {
        let marker = format!(
            "[psc exact dup h={content_hash} b={byte_len} refs={}]",
            existing.refs.join(",")
        );
        let candidate = format!("{}\n{marker}", range.reference);
        let original = format!("{}\n{}", range.reference, range.body);
        existing.refs.push(range.reference.clone());
        if candidate.len() < original.len() {
            return candidate;
        }
        return original;
    }
    seen.entry(key)
        .or_default()
        .push(RuntimeSmartContextSeenExactAppendixBody {
            body: range.body.clone(),
            refs: vec![range.reference.clone()],
        });
    format!("{}\n{}", range.reference, range.body)
}

fn runtime_smart_context_replace_repeated_exact_text_after_first(
    text: &str,
    needle: &str,
    replacement: &str,
) -> String {
    if needle.is_empty() {
        return text.to_string();
    }
    let mut rendered = String::with_capacity(text.len());
    let mut cursor = 0usize;
    let mut seen = 0usize;
    for (offset, _) in text.match_indices(needle) {
        rendered.push_str(&text[cursor..offset]);
        seen = seen.saturating_add(1);
        if seen == 1 {
            rendered.push_str(needle);
        } else {
            rendered.push_str(replacement);
        }
        cursor = offset.saturating_add(needle.len());
    }
    rendered.push_str(&text[cursor..]);
    rendered
}

fn runtime_smart_context_labeled_section_body<'a>(
    text: &'a str,
    labels: &[&str],
) -> Option<&'a str> {
    for label in labels {
        let section_needle = format!("\n\n{label}\n");
        if let Some((_, body)) = text.split_once(&section_needle) {
            return Some(body);
        }
        let prefix_needle = format!("{label}\n");
        if let Some(body) = text.strip_prefix(&prefix_needle) {
            return Some(body);
        }
    }
    None
}

fn runtime_smart_context_progressive_critical_exact_ranges(
    artifact_id: &str,
    original: &str,
    line_index: Option<&RuntimeSmartContextArtifactLineIndex>,
    summary: &str,
) -> Option<String> {
    let mut exact_ranges = Vec::new();
    if let Some(line_index) = line_index {
        for range in line_index
            .critical_ranges
            .iter()
            .take(SMART_CONTEXT_SURGICAL_CRITICAL_MAX_RANGES)
        {
            if !runtime_smart_context_artifact_line_index_range_valid(range)
                || range.text.trim().is_empty()
            {
                continue;
            }
            exact_ranges.push(RuntimeSmartContextExactAppendixRange {
                reference: runtime_smart_context_artifact_line_ref(
                    artifact_id,
                    range.start,
                    range.end,
                ),
                body: range.text.clone(),
            });
        }
    }

    if exact_ranges.is_empty() {
        let compacted = summary.to_string();
        let repaired = runtime_smart_context_append_missing_critical_ranges(
            original,
            compacted,
            SMART_CONTEXT_SURGICAL_CRITICAL_MAX_RANGES,
        );
        if let Some(exact_ranges) = runtime_smart_context_labeled_section_body(
            &repaired,
            &[
                SMART_CONTEXT_LABEL_CRITICAL_EXACT,
                SMART_CONTEXT_LABEL_CRITICAL_EXACT_LEGACY,
            ],
        ) {
            return Some(format!(
                "{SMART_CONTEXT_LABEL_CRITICAL_EXACT}\n{}",
                exact_ranges.trim_end()
            ));
        }
    }

    runtime_smart_context_render_exact_appendix(SMART_CONTEXT_LABEL_CRITICAL_EXACT, exact_ranges)
        .map(|(appendix, _)| appendix)
}

fn runtime_smart_context_compact_successful_tool_output(
    text: &str,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    preview_byte_limit: usize,
    metadata: &RuntimeSmartContextToolOutputCompactionMetadata,
) -> Option<String> {
    let max_lines =
        runtime_smart_context_tool_preview_max_lines(tier, preview_byte_limit).unwrap_or(40);
    let report = prodex_context::compact_successful_command_output_with_options(
        text,
        &prodex_context::CommandSuccessOutputCompactOptions {
            command: metadata.command.clone(),
            exit_code: metadata.exit_code,
            min_lines_to_compact: max_lines.saturating_mul(2).max(20),
            max_touched_files: max_lines.clamp(8, 48),
            max_key_lines: (max_lines / 2).clamp(4, 24),
            max_line_chars: SMART_CONTEXT_TOOL_PREVIEW_MAX_LINE_CHARS,
        },
    );
    if report.compacted && !report.failure_suspected && report.output.len() < text.len() {
        Some(report.output)
    } else {
        None
    }
}

fn runtime_smart_context_compact_tool_output_preserving_critical(
    text: &str,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    preview_byte_limit: usize,
    command_kind_hint: Option<prodex_context::CommandOutputKind>,
    intent_terms: &[String],
) -> String {
    let compacted = runtime_smart_context_compact_tool_output(
        text,
        tier,
        preview_byte_limit,
        command_kind_hint,
        intent_terms,
    );
    runtime_smart_context_append_missing_critical_ranges(
        text,
        compacted,
        SMART_CONTEXT_SURGICAL_CRITICAL_MAX_RANGES,
    )
}

fn runtime_smart_context_compact_tool_output(
    text: &str,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    preview_byte_limit: usize,
    command_kind_hint: Option<prodex_context::CommandOutputKind>,
    intent_terms: &[String],
) -> String {
    let Some(max_lines) = runtime_smart_context_tool_preview_max_lines(tier, preview_byte_limit)
    else {
        return text.to_string();
    };
    let options = prodex_context::CommandOutputCompactOptions {
        max_lines,
        head_lines: max_lines.saturating_mul(2) / 3,
        tail_lines: max_lines / 3,
        max_line_chars: SMART_CONTEXT_TOOL_PREVIEW_MAX_LINE_CHARS,
        ..prodex_context::CommandOutputCompactOptions::default()
    };
    prodex_context::compact_command_output_with_intent_options(
        text,
        &prodex_context::CommandOutputIntentCompactOptions::new(options, intent_terms.to_vec())
            .with_kind_hint(command_kind_hint),
    )
    .output
}

fn runtime_smart_context_tool_preview_max_lines(
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    preview_byte_limit: usize,
) -> Option<usize> {
    if tier == runtime_proxy_crate::SmartContextTokenBudgetTier::Exact
        && preview_byte_limit == usize::MAX
    {
        return None;
    }

    let budget_lines = preview_byte_limit / SMART_CONTEXT_TOOL_PREVIEW_ESTIMATED_LINE_BYTES;
    let (floor, cap) = match tier {
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal => (8, 24),
        runtime_proxy_crate::SmartContextTokenBudgetTier::Condensed => (24, 80),
        runtime_proxy_crate::SmartContextTokenBudgetTier::Large => (80, 240),
        runtime_proxy_crate::SmartContextTokenBudgetTier::Exact => (120, 360),
    };
    Some(budget_lines.max(floor).min(cap))
}

fn runtime_smart_context_tool_call_metadata_by_call_id(
    input: &[serde_json::Value],
) -> BTreeMap<String, RuntimeSmartContextToolCallMetadata> {
    let mut metadata_by_call_id = BTreeMap::new();
    for item in input {
        let Some(object) = item.as_object() else {
            continue;
        };
        let Some(call_id) = runtime_smart_context_tool_call_id(object) else {
            continue;
        };
        let item_type = object
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if item_type.ends_with("_call_output") {
            continue;
        }
        let metadata = runtime_smart_context_tool_item_metadata(object);
        if metadata.command.is_some()
            || metadata.exit_code.is_some()
            || metadata.kind_hint.is_some()
        {
            metadata_by_call_id.insert(call_id.to_string(), metadata);
        }
    }
    metadata_by_call_id
}

fn runtime_smart_context_tool_output_compaction_metadata(
    object: &serde_json::Map<String, serde_json::Value>,
    linked_metadata: Option<&RuntimeSmartContextToolCallMetadata>,
) -> RuntimeSmartContextToolOutputCompactionMetadata {
    let local_metadata = runtime_smart_context_tool_item_metadata(object);
    let command = linked_metadata
        .and_then(|metadata| metadata.command.clone())
        .or(local_metadata.command);
    let exit_code = local_metadata
        .exit_code
        .or_else(|| linked_metadata.and_then(|metadata| metadata.exit_code));
    let kind_hint = local_metadata
        .kind_hint
        .or_else(|| linked_metadata.and_then(|metadata| metadata.kind_hint))
        .or_else(|| {
            command
                .as_deref()
                .and_then(prodex_context::command_output_kind_hint_for_command)
        });
    RuntimeSmartContextToolOutputCompactionMetadata {
        kind_hint,
        command,
        exit_code,
    }
}

const SMART_CONTEXT_TOOL_COMMAND_HINT_MAX_CHARS: usize = 512;
const SMART_CONTEXT_TOOL_METADATA_SCAN_MAX_DEPTH: usize = 8;

fn runtime_smart_context_tool_item_metadata(
    object: &serde_json::Map<String, serde_json::Value>,
) -> RuntimeSmartContextToolCallMetadata {
    let command = runtime_smart_context_tool_command_hint(object);
    let exit_code = runtime_smart_context_tool_exit_code_hint(object);
    let kind_hint = runtime_smart_context_tool_kind_hint(object, command.as_deref());
    RuntimeSmartContextToolCallMetadata {
        command,
        exit_code,
        kind_hint,
    }
}

fn runtime_smart_context_tool_command_hint(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<String> {
    runtime_smart_context_tool_command_hint_from_object(object, 0)
}

fn runtime_smart_context_tool_command_hint_from_object(
    object: &serde_json::Map<String, serde_json::Value>,
    depth: usize,
) -> Option<String> {
    if depth > SMART_CONTEXT_TOOL_METADATA_SCAN_MAX_DEPTH {
        return None;
    }
    for key in runtime_smart_context_tool_command_keys() {
        if let Some(command) = object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .and_then(runtime_smart_context_bounded_tool_command)
        {
            return Some(command);
        }
    }
    if let Some(command) = object.get("arguments").and_then(|value| {
        runtime_smart_context_tool_command_hint_from_value(value, depth + 1, true)
    }) {
        return Some(command);
    }
    for (field, child) in object {
        if field == "arguments" || runtime_smart_context_tool_output_payload_field(field) {
            continue;
        }
        if let Some(command) =
            runtime_smart_context_tool_command_hint_from_value(child, depth + 1, false)
        {
            return Some(command);
        }
    }
    None
}

fn runtime_smart_context_tool_command_hint_from_value(
    value: &serde_json::Value,
    depth: usize,
    allow_plain_command: bool,
) -> Option<String> {
    if depth > SMART_CONTEXT_TOOL_METADATA_SCAN_MAX_DEPTH {
        return None;
    }
    match value {
        serde_json::Value::String(text) => {
            if let Some(command) = runtime_smart_context_bounded_tool_command(text)
                && allow_plain_command
                && !text.trim_start().starts_with('{')
            {
                return Some(command);
            }
            serde_json::from_str::<serde_json::Value>(text)
                .ok()
                .and_then(|value| {
                    runtime_smart_context_tool_command_hint_from_value(
                        &value,
                        depth + 1,
                        allow_plain_command,
                    )
                })
        }
        serde_json::Value::Object(object) => {
            runtime_smart_context_tool_command_hint_from_object(object, depth + 1)
        }
        serde_json::Value::Array(items) => items.iter().find_map(|item| {
            runtime_smart_context_tool_command_hint_from_value(item, depth + 1, allow_plain_command)
        }),
        _ => None,
    }
}

fn runtime_smart_context_tool_command_keys() -> [&'static str; 4] {
    ["cmd", "command", "shell_command", "shellCommand"]
}

fn runtime_smart_context_bounded_tool_command(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(
        trimmed
            .chars()
            .take(SMART_CONTEXT_TOOL_COMMAND_HINT_MAX_CHARS)
            .collect(),
    )
}

fn runtime_smart_context_tool_exit_code_hint(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<i32> {
    runtime_smart_context_tool_exit_code_hint_from_object(object, 0)
}

fn runtime_smart_context_tool_exit_code_hint_from_object(
    object: &serde_json::Map<String, serde_json::Value>,
    depth: usize,
) -> Option<i32> {
    if depth > SMART_CONTEXT_TOOL_METADATA_SCAN_MAX_DEPTH {
        return None;
    }
    for key in runtime_smart_context_tool_exit_code_keys() {
        if let Some(exit_code) = object
            .get(key)
            .and_then(runtime_smart_context_i32_from_json_value)
        {
            return Some(exit_code);
        }
    }
    if let Some(exit_code) = object.get("arguments").and_then(|value| {
        runtime_smart_context_tool_exit_code_hint_from_value(value, depth + 1, true)
    }) {
        return Some(exit_code);
    }
    for (field, child) in object {
        if field == "arguments" || runtime_smart_context_tool_output_payload_field(field) {
            continue;
        }
        if let Some(exit_code) =
            runtime_smart_context_tool_exit_code_hint_from_value(child, depth + 1, false)
        {
            return Some(exit_code);
        }
    }
    None
}

fn runtime_smart_context_tool_exit_code_hint_from_value(
    value: &serde_json::Value,
    depth: usize,
    allow_plain_exit_code: bool,
) -> Option<i32> {
    if depth > SMART_CONTEXT_TOOL_METADATA_SCAN_MAX_DEPTH {
        return None;
    }
    match value {
        serde_json::Value::String(text) => serde_json::from_str::<serde_json::Value>(text)
            .ok()
            .and_then(|value| {
                runtime_smart_context_tool_exit_code_hint_from_value(
                    &value,
                    depth + 1,
                    allow_plain_exit_code,
                )
            }),
        serde_json::Value::Object(object) => {
            runtime_smart_context_tool_exit_code_hint_from_object(object, depth + 1)
        }
        serde_json::Value::Array(items) => items.iter().find_map(|item| {
            runtime_smart_context_tool_exit_code_hint_from_value(
                item,
                depth + 1,
                allow_plain_exit_code,
            )
        }),
        _ => allow_plain_exit_code
            .then(|| runtime_smart_context_i32_from_json_value(value))
            .flatten(),
    }
}

fn runtime_smart_context_tool_exit_code_keys() -> [&'static str; 7] {
    [
        "exit_code",
        "exitCode",
        "exit_status",
        "exitStatus",
        "status_code",
        "statusCode",
        "status",
    ]
}

fn runtime_smart_context_tool_kind_hint(
    object: &serde_json::Map<String, serde_json::Value>,
    command: Option<&str>,
) -> Option<prodex_context::CommandOutputKind> {
    command
        .and_then(prodex_context::command_output_kind_hint_for_command)
        .or_else(|| runtime_smart_context_tool_kind_hint_from_object(object, 0))
}

fn runtime_smart_context_tool_kind_hint_from_object(
    object: &serde_json::Map<String, serde_json::Value>,
    depth: usize,
) -> Option<prodex_context::CommandOutputKind> {
    if depth > SMART_CONTEXT_TOOL_METADATA_SCAN_MAX_DEPTH {
        return None;
    }
    for key in runtime_smart_context_tool_kind_keys() {
        if let Some(kind) = object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .and_then(runtime_smart_context_tool_kind_hint_from_text)
        {
            return Some(kind);
        }
    }
    if let Some(kind) = object
        .get("arguments")
        .and_then(|value| runtime_smart_context_tool_kind_hint_from_value(value, depth + 1, true))
    {
        return Some(kind);
    }
    for (field, child) in object {
        if field == "arguments" || runtime_smart_context_tool_output_payload_field(field) {
            continue;
        }
        if let Some(kind) = runtime_smart_context_tool_kind_hint_from_value(child, depth + 1, false)
        {
            return Some(kind);
        }
    }
    None
}

fn runtime_smart_context_tool_kind_hint_from_value(
    value: &serde_json::Value,
    depth: usize,
    allow_plain_kind: bool,
) -> Option<prodex_context::CommandOutputKind> {
    if depth > SMART_CONTEXT_TOOL_METADATA_SCAN_MAX_DEPTH {
        return None;
    }
    match value {
        serde_json::Value::String(text) => serde_json::from_str::<serde_json::Value>(text)
            .ok()
            .and_then(|value| {
                runtime_smart_context_tool_kind_hint_from_value(&value, depth + 1, allow_plain_kind)
            })
            .or_else(|| {
                allow_plain_kind
                    .then(|| runtime_smart_context_tool_kind_hint_from_text(text))
                    .flatten()
            }),
        serde_json::Value::Object(object) => {
            runtime_smart_context_tool_kind_hint_from_object(object, depth + 1)
        }
        serde_json::Value::Array(items) => items.iter().find_map(|item| {
            runtime_smart_context_tool_kind_hint_from_value(item, depth + 1, allow_plain_kind)
        }),
        _ => None,
    }
}

fn runtime_smart_context_tool_kind_hint_from_text(
    text: &str,
) -> Option<prodex_context::CommandOutputKind> {
    if text.len() > SMART_CONTEXT_TOOL_COMMAND_HINT_MAX_CHARS {
        return None;
    }
    let normalized = text.trim().to_ascii_lowercase().replace('_', "-");
    match normalized.as_str() {
        "git-status" | "status" => Some(prodex_context::CommandOutputKind::GitStatus),
        "git-diff" | "diff" | "git-show" => Some(prodex_context::CommandOutputKind::GitDiff),
        "rust-diagnostics" | "cargo-test" | "cargo-check" | "cargo-clippy" | "rust" => {
            Some(prodex_context::CommandOutputKind::RustDiagnostics)
        }
        "diagnostics" | "test" | "typecheck" | "type-check" => {
            Some(prodex_context::CommandOutputKind::Diagnostics)
        }
        "git-log" => Some(prodex_context::CommandOutputKind::GitLog),
        "search" | "rg" | "grep" => Some(prodex_context::CommandOutputKind::Search),
        "file-list" | "find" | "tree" => Some(prodex_context::CommandOutputKind::FileList),
        "log-stream" => Some(prodex_context::CommandOutputKind::LogStream),
        "noisy-success" => Some(prodex_context::CommandOutputKind::NoisySuccess),
        "plain" => Some(prodex_context::CommandOutputKind::Plain),
        _ => prodex_context::infer_command_output_kind_from_metadata(&normalized),
    }
}

fn runtime_smart_context_tool_kind_keys() -> [&'static str; 8] {
    [
        "kind",
        "output_kind",
        "outputKind",
        "command_kind",
        "commandKind",
        "detected_kind",
        "detectedKind",
        "name",
    ]
}

fn runtime_smart_context_i32_from_json_value(value: &serde_json::Value) -> Option<i32> {
    match value {
        serde_json::Value::Number(number) => {
            number.as_i64().and_then(|value| value.try_into().ok())
        }
        serde_json::Value::String(text) => text.trim().parse::<i32>().ok(),
        _ => None,
    }
}

fn runtime_smart_context_tool_call_id(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<&str> {
    ["call_id", "tool_call_id", "id"]
        .into_iter()
        .find_map(|key| object.get(key).and_then(serde_json::Value::as_str))
        .filter(|value| !value.trim().is_empty())
}

fn runtime_smart_context_tool_output_payload_field(key: &str) -> bool {
    matches!(key, "output" | "content")
}

fn runtime_smart_context_artifact_summary(
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
    compacted: &str,
) -> String {
    let marker = runtime_smart_context_artifact_marker_line("artifact", artifact);
    if compacted.is_empty() {
        return marker;
    }
    format!("{marker}\n{compacted}")
}

fn runtime_smart_context_artifact_reference_summary(
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
) -> String {
    runtime_smart_context_artifact_ref(&artifact.id)
}

fn runtime_smart_context_artifact_marker_line(
    kind: &str,
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
) -> String {
    let reference = runtime_smart_context_artifact_ref(&artifact.id);
    let kind = match kind {
        "artifact" => "art",
        other => other,
    };
    format!(
        "psc {kind} {reference} b={}; ref {reference}[#Lx-Ly]",
        artifact.byte_len
    )
}

fn runtime_smart_context_artifact_line_ref(id: &str, start: usize, end: usize) -> String {
    format!("{}#L{start}-L{end}", runtime_smart_context_artifact_ref(id))
}

fn runtime_smart_context_artifact_ref(id: &str) -> String {
    let short_id = id.strip_prefix("sc:").unwrap_or(id);
    format!("{SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX}{short_id}")
}

#[cfg(test)]
fn runtime_smart_context_apply_artifact_aliases_to_generated_texts(
    value: &mut serde_json::Value,
) -> bool {
    let counts = runtime_smart_context_generated_artifact_ref_counts(value);
    let aliases = runtime_smart_context_artifact_alias_plan(counts, None);
    if aliases.is_empty() {
        return false;
    }
    let replacement_count =
        runtime_smart_context_replace_generated_artifact_refs_with_aliases(value, &aliases);
    if replacement_count == 0 {
        return false;
    }
    runtime_smart_context_insert_artifact_alias_legend(value, &aliases)
}

fn runtime_smart_context_apply_artifact_aliases_to_generated_texts_with_state(
    value: &mut serde_json::Value,
    state: &mut RuntimeSmartContextProxyState,
) -> bool {
    let counts = runtime_smart_context_generated_artifact_ref_counts(value);
    let aliases = runtime_smart_context_artifact_alias_plan(counts, Some(state));
    if aliases.is_empty() {
        return false;
    }
    let replacement_count =
        runtime_smart_context_replace_generated_artifact_refs_with_aliases(value, &aliases);
    if replacement_count == 0 {
        return false;
    }
    runtime_smart_context_insert_artifact_alias_legend(value, &aliases)
}

fn runtime_smart_context_generated_artifact_ref_counts(
    value: &serde_json::Value,
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    runtime_smart_context_generated_artifact_ref_counts_from_value(value, &mut counts);
    counts
}

fn runtime_smart_context_generated_artifact_ref_counts_from_value(
    value: &serde_json::Value,
    counts: &mut BTreeMap<String, usize>,
) {
    match value {
        serde_json::Value::String(text)
            if runtime_smart_context_text_is_generated_summary_or_manifest(text) =>
        {
            let aliases = BTreeMap::new();
            let ids = runtime_smart_context_artifact_ref_occurrences_from_text(text, &aliases)
                .into_iter()
                .map(|reference| reference.id)
                .collect::<BTreeSet<_>>();
            for id in ids {
                let reference = runtime_smart_context_artifact_ref(&id);
                let count = text.matches(&reference).count();
                if count > 0 {
                    *counts.entry(id).or_default() += count;
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_generated_artifact_ref_counts_from_value(item, counts);
            }
        }
        serde_json::Value::Object(object) => {
            for item in object.values() {
                runtime_smart_context_generated_artifact_ref_counts_from_value(item, counts);
            }
        }
        _ => {}
    }
}

fn runtime_smart_context_artifact_alias_plan(
    counts: BTreeMap<String, usize>,
    mut state: Option<&mut RuntimeSmartContextProxyState>,
) -> Vec<RuntimeSmartContextArtifactAlias> {
    let mut candidates = counts
        .into_iter()
        .filter_map(|(id, count)| {
            (count > 1).then(|| {
                let reference = runtime_smart_context_artifact_ref(&id);
                let potential = reference.len().saturating_mul(count);
                (id, count, reference, potential)
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| right.3.cmp(&left.3).then_with(|| left.0.cmp(&right.0)));

    let mut aliases = Vec::new();
    for (id, count, reference, _) in candidates {
        let alias = if let Some(state) = state.as_deref_mut() {
            runtime_smart_context_stable_artifact_alias(state, &id)
        } else {
            format!("@{}", aliases.len())
        };
        if reference.len() <= alias.len() {
            continue;
        }
        let replacement_savings = reference
            .len()
            .saturating_sub(alias.len())
            .saturating_mul(count);
        let definition_len = alias
            .len()
            .saturating_add(1)
            .saturating_add(reference.len());
        let legend_cost = if aliases.is_empty() {
            SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX
                .len()
                .saturating_add(definition_len)
                .saturating_add(1)
        } else {
            definition_len.saturating_add(1)
        };
        if replacement_savings <= legend_cost {
            continue;
        }
        aliases.push(RuntimeSmartContextArtifactAlias { id, alias });
    }
    aliases
}

fn runtime_smart_context_stable_artifact_alias(
    state: &mut RuntimeSmartContextProxyState,
    id: &str,
) -> String {
    if let Some(alias) = state.artifact_aliases.get(id) {
        return alias.clone();
    }
    loop {
        let alias = format!("@{}", state.next_artifact_alias_index);
        state.next_artifact_alias_index = state.next_artifact_alias_index.saturating_add(1);
        if state
            .artifact_aliases
            .values()
            .all(|existing| existing != &alias)
        {
            state.artifact_aliases.insert(id.to_string(), alias.clone());
            return alias;
        }
    }
}

fn runtime_smart_context_replace_generated_artifact_refs_with_aliases(
    value: &mut serde_json::Value,
    aliases: &[RuntimeSmartContextArtifactAlias],
) -> usize {
    match value {
        serde_json::Value::String(text)
            if runtime_smart_context_text_is_generated_summary_or_manifest(text) =>
        {
            let mut next_lines = Vec::new();
            let mut replacements = 0usize;
            for line in text.lines() {
                if runtime_smart_context_generated_control_line(line) {
                    next_lines.push(line.to_string());
                    continue;
                }
                let mut next_line = line.to_string();
                for alias in aliases {
                    let reference = runtime_smart_context_artifact_ref(&alias.id);
                    let count = next_line.matches(&reference).count();
                    if count == 0 {
                        continue;
                    }
                    next_line = next_line.replace(&reference, &alias.alias);
                    replacements = replacements.saturating_add(count);
                }
                next_lines.push(next_line);
            }
            if replacements > 0 {
                *text = next_lines.join("\n");
            }
            replacements
        }
        serde_json::Value::Array(items) => items
            .iter_mut()
            .map(|item| {
                runtime_smart_context_replace_generated_artifact_refs_with_aliases(item, aliases)
            })
            .sum(),
        serde_json::Value::Object(object) => object
            .values_mut()
            .map(|item| {
                runtime_smart_context_replace_generated_artifact_refs_with_aliases(item, aliases)
            })
            .sum(),
        _ => 0,
    }
}

fn runtime_smart_context_generated_control_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with(SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX)
        || trimmed.starts_with(SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX_LEGACY)
        || trimmed.starts_with(SMART_CONTEXT_PATH_ALIAS_LEGEND_PREFIX)
        || trimmed.starts_with(SMART_CONTEXT_PATH_ALIAS_LEGEND_PREFIX_LEGACY)
        || trimmed.starts_with("psc art ")
        || trimmed.starts_with("psc repeat ")
        || trimmed.starts_with("prodex-sc artifact ")
        || trimmed.starts_with("prodex-sc repeat ")
}

fn runtime_smart_context_insert_artifact_alias_legend(
    value: &mut serde_json::Value,
    aliases: &[RuntimeSmartContextArtifactAlias],
) -> bool {
    let legend = runtime_smart_context_artifact_alias_legend(aliases);
    runtime_smart_context_insert_artifact_alias_legend_in_value(value, &legend)
}

fn runtime_smart_context_artifact_alias_legend(
    aliases: &[RuntimeSmartContextArtifactAlias],
) -> String {
    let defs = aliases
        .iter()
        .map(|alias| {
            format!(
                "{}={}",
                alias.alias,
                runtime_smart_context_artifact_ref(&alias.id)
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("{SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX}{defs}")
}

fn runtime_smart_context_insert_artifact_alias_legend_in_value(
    value: &mut serde_json::Value,
    legend: &str,
) -> bool {
    match value {
        serde_json::Value::String(text)
            if runtime_smart_context_text_is_generated_summary_or_manifest(text) =>
        {
            *text = runtime_smart_context_insert_artifact_alias_legend_in_text(text, legend);
            true
        }
        serde_json::Value::Array(items) => items
            .iter_mut()
            .any(|item| runtime_smart_context_insert_artifact_alias_legend_in_value(item, legend)),
        serde_json::Value::Object(object) => object
            .values_mut()
            .any(|item| runtime_smart_context_insert_artifact_alias_legend_in_value(item, legend)),
        _ => false,
    }
}

fn runtime_smart_context_insert_artifact_alias_legend_in_text(text: &str, legend: &str) -> String {
    let duplicate_legend = if legend.starts_with(SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX) {
        text.contains(SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX)
            || text.contains(SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX_LEGACY)
    } else if legend.starts_with(SMART_CONTEXT_PATH_ALIAS_LEGEND_PREFIX) {
        text.contains(SMART_CONTEXT_PATH_ALIAS_LEGEND_PREFIX)
            || text.contains(SMART_CONTEXT_PATH_ALIAS_LEGEND_PREFIX_LEGACY)
    } else {
        text.contains(legend)
    };
    if duplicate_legend {
        return text.to_string();
    }
    if let Some((first, rest)) = text.split_once('\n') {
        format!("{first}\n{legend}\n{rest}")
    } else {
        format!("{text}\n{legend}")
    }
}

fn runtime_smart_context_apply_path_aliases_to_generated_texts(
    value: &mut serde_json::Value,
) -> bool {
    match value {
        serde_json::Value::String(text)
            if runtime_smart_context_text_is_generated_summary_or_manifest(text) =>
        {
            let Some(next) = runtime_smart_context_path_aliased_generated_text(text) else {
                return false;
            };
            *text = next;
            true
        }
        serde_json::Value::Array(items) => items
            .iter_mut()
            .any(runtime_smart_context_apply_path_aliases_to_generated_texts),
        serde_json::Value::Object(object) => object
            .values_mut()
            .any(runtime_smart_context_apply_path_aliases_to_generated_texts),
        _ => false,
    }
}

fn runtime_smart_context_path_aliased_generated_text(text: &str) -> Option<String> {
    let aliases = runtime_smart_context_path_aliases(text);
    if aliases.is_empty() {
        return None;
    }
    let mut candidate = text.to_string();
    for (alias, prefix) in &aliases {
        candidate = candidate.replace(prefix, alias);
    }
    if candidate == text || candidate.len() >= text.len() {
        return None;
    }
    let legend = aliases
        .iter()
        .map(|(alias, prefix)| format!("{alias}={prefix}"))
        .collect::<Vec<_>>()
        .join(" ");
    candidate = runtime_smart_context_insert_artifact_alias_legend_in_text(
        &candidate,
        &format!("{SMART_CONTEXT_PATH_ALIAS_LEGEND_PREFIX}{legend}"),
    );
    if candidate.len() < text.len()
        && prodex_context::critical_signal_self_check(text, &candidate).passed()
    {
        Some(candidate)
    } else {
        None
    }
}

fn runtime_smart_context_path_aliases(text: &str) -> Vec<(String, String)> {
    let mut counts = BTreeMap::<String, usize>::new();
    for token in text.split_whitespace() {
        let token = token.trim_matches(|ch: char| {
            ch.is_ascii_punctuation() && !matches!(ch, '/' | '_' | '-' | '.')
        });
        if let Some(prefix) = runtime_smart_context_repeated_path_prefix(token) {
            *counts.entry(prefix).or_default() += 1;
        }
    }
    let mut candidates = counts
        .into_iter()
        .filter(|(prefix, count)| *count >= 2 && prefix.len() > 8)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .0
            .len()
            .saturating_mul(right.1)
            .cmp(&left.0.len().saturating_mul(left.1))
    });
    candidates
        .into_iter()
        .take(4)
        .enumerate()
        .filter_map(|(index, (prefix, count))| {
            let alias = if index == 0 {
                "$R".to_string()
            } else {
                format!("$P{index}")
            };
            let saved = prefix
                .len()
                .saturating_sub(alias.len())
                .saturating_mul(count);
            let cost = alias.len().saturating_add(prefix.len()).saturating_add(2);
            (saved > cost).then_some((alias, prefix))
        })
        .collect()
}

fn runtime_smart_context_repeated_path_prefix(token: &str) -> Option<String> {
    if !token.starts_with('/') {
        return None;
    }
    for marker in [
        "/crates/",
        "/src/",
        "/tests/",
        "/test/",
        "/dist/",
        "/target/",
        "/apps/",
        "/packages/",
    ] {
        if let Some(index) = token.find(marker) {
            let prefix = &token[..index];
            if prefix.matches('/').count() >= 2 {
                return Some(prefix.to_string());
            }
        }
    }
    token
        .rsplit_once('/')
        .map(|(prefix, _)| prefix)
        .filter(|prefix| prefix.matches('/').count() >= 3)
        .map(str::to_string)
}

fn runtime_smart_context_text_is_generated_summary_or_manifest(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("prodex-sc artifact ")
        || trimmed.starts_with("prodex-sc repeat ")
        || trimmed.starts_with("psc art ")
        || trimmed.starts_with("psc repeat ")
        || trimmed.starts_with("psc m ")
        || trimmed.starts_with("psc manifest ")
}

fn runtime_smart_context_append_artifact_manifest_delta_if_useful(
    value: &mut serde_json::Value,
    state: &mut RuntimeSmartContextProxyState,
    stats: &RuntimeSmartContextTransformStats,
    intent_signals: &RuntimeSmartContextIntentSignals,
) -> bool {
    if !runtime_smart_context_artifact_manifest_useful(stats) {
        return false;
    }
    if !runtime_smart_context_artifact_manifest_delta_eligible(state, intent_signals) {
        return false;
    }
    if runtime_smart_context_explicit_artifact_refs_fully_resolved_without_manifest_request(
        state,
        intent_signals,
    ) {
        return false;
    }
    let mut entries = state
        .artifacts
        .artifact_manifest_entries(SMART_CONTEXT_ARTIFACT_MANIFEST_MAX_ENTRIES);
    runtime_smart_context_filter_manifest_entries(value, intent_signals, &mut entries);
    let ids = entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<BTreeSet<_>>();
    if ids.is_empty() || ids == state.last_artifact_manifest_ids {
        return false;
    }
    let detailed_manifest = runtime_smart_context_manifest_requested(intent_signals);
    let unchanged_count = ids.intersection(&state.last_artifact_manifest_ids).count();
    let manifest_entries = if detailed_manifest {
        entries
    } else {
        entries
            .into_iter()
            .filter(|entry| !state.last_artifact_manifest_ids.contains(&entry.id))
            .collect::<Vec<_>>()
    };
    let Some(manifest) = runtime_smart_context_artifact_manifest_delta_from_entries(
        manifest_entries,
        detailed_manifest,
        unchanged_count,
    ) else {
        return false;
    };
    if !runtime_smart_context_append_input_manifest(value, manifest) {
        return false;
    }
    state.last_artifact_manifest_ids = ids;
    state.last_artifact_manifest_emitted_at = Some(Instant::now());
    true
}

fn runtime_smart_context_explicit_artifact_refs_fully_resolved_without_manifest_request(
    state: &RuntimeSmartContextProxyState,
    intent_signals: &RuntimeSmartContextIntentSignals,
) -> bool {
    !runtime_smart_context_manifest_requested(intent_signals)
        && !intent_signals.artifact_refs.is_empty()
        && intent_signals
            .artifact_refs
            .iter()
            .all(|reference| state.artifacts.contains(&reference.id))
}

fn runtime_smart_context_artifact_manifest_delta_eligible(
    state: &RuntimeSmartContextProxyState,
    intent_signals: &RuntimeSmartContextIntentSignals,
) -> bool {
    if runtime_smart_context_manifest_requested(intent_signals) {
        return true;
    }
    if intent_signals.artifact_refs.is_empty()
        && runtime_smart_context_selective_rehydrate_terms_empty(&intent_signals.semantic_terms)
        && intent_signals.command_kind_hints.is_empty()
    {
        return false;
    }
    state
        .last_artifact_manifest_emitted_at
        .is_none_or(|emitted_at| {
            emitted_at.elapsed()
                >= Duration::from_millis(SMART_CONTEXT_ARTIFACT_MANIFEST_COOLDOWN_MS)
        })
}

#[cfg(test)]
fn runtime_smart_context_append_artifact_manifest_if_useful(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    stats: &RuntimeSmartContextTransformStats,
) -> bool {
    if !runtime_smart_context_artifact_manifest_useful(stats) {
        return false;
    }
    let Some(manifest) =
        runtime_smart_context_artifact_manifest_for_value(value, store, &Default::default())
    else {
        return false;
    };
    runtime_smart_context_append_input_manifest(value, manifest)
}

fn runtime_smart_context_artifact_manifest_useful(
    stats: &RuntimeSmartContextTransformStats,
) -> bool {
    stats.artifacts_stored > 0
        || stats.tool_outputs_condensed > 0
        || stats.cross_turn_duplicate_texts > 0
        || stats.repeat_tool_output_refs > 0
}

#[cfg(test)]
fn runtime_smart_context_artifact_manifest(
    store: &RuntimeSmartContextArtifactStore,
) -> Option<String> {
    let entries = store.artifact_manifest_entries(SMART_CONTEXT_ARTIFACT_MANIFEST_MAX_ENTRIES);
    runtime_smart_context_artifact_manifest_from_entries(entries, true)
}

#[cfg(test)]
fn runtime_smart_context_artifact_manifest_for_value(
    value: &serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    intent_signals: &RuntimeSmartContextIntentSignals,
) -> Option<String> {
    let mut entries = store.artifact_manifest_entries(SMART_CONTEXT_ARTIFACT_MANIFEST_MAX_ENTRIES);
    runtime_smart_context_filter_manifest_entries(value, intent_signals, &mut entries);
    runtime_smart_context_artifact_manifest_from_entries(
        entries,
        runtime_smart_context_manifest_requested(intent_signals),
    )
}

fn runtime_smart_context_filter_manifest_entries(
    value: &serde_json::Value,
    intent_signals: &RuntimeSmartContextIntentSignals,
    entries: &mut Vec<RuntimeSmartContextArtifactManifestEntry>,
) {
    if !runtime_smart_context_manifest_requested(intent_signals) {
        let visible_ids = runtime_smart_context_collect_artifact_refs(value)
            .into_iter()
            .map(|reference| reference.id)
            .collect::<BTreeSet<_>>();
        entries.retain(|entry| !visible_ids.contains(&entry.id));
    }
    runtime_smart_context_sort_manifest_entries_for_intent(entries, intent_signals);
}

fn runtime_smart_context_manifest_requested(
    intent_signals: &RuntimeSmartContextIntentSignals,
) -> bool {
    intent_signals.intent_terms.iter().any(|term| {
        matches!(
            term.as_str(),
            "artifact"
                | "artifacts"
                | "manifest"
                | "reference"
                | "references"
                | "refs"
                | "rehydrate"
        )
    })
}

fn runtime_smart_context_sort_manifest_entries_for_intent(
    entries: &mut Vec<RuntimeSmartContextArtifactManifestEntry>,
    intent_signals: &RuntimeSmartContextIntentSignals,
) {
    if runtime_smart_context_selective_rehydrate_terms_empty(&intent_signals.semantic_terms) {
        return;
    }
    let original = std::mem::take(entries);
    let (mut matching, rest): (Vec<_>, Vec<_>) = original.into_iter().partition(|entry| {
        runtime_smart_context_manifest_entry_matches_intent(entry, intent_signals)
    });
    matching.extend(rest);
    *entries = matching;
}

fn runtime_smart_context_manifest_entry_matches_intent(
    entry: &RuntimeSmartContextArtifactManifestEntry,
    intent_signals: &RuntimeSmartContextIntentSignals,
) -> bool {
    entry.file_location_range_count > 0 && !intent_signals.semantic_terms.file_paths.is_empty()
        || entry.error_range_count > 0 && !intent_signals.semantic_terms.error_codes.is_empty()
        || entry.test_failure_range_count > 0
            && !intent_signals.semantic_terms.test_symbols.is_empty()
        || entry.diff_hunk_range_count > 0 && !intent_signals.semantic_terms.diff_hunks.is_empty()
}

#[cfg(test)]
fn runtime_smart_context_artifact_manifest_from_entries(
    entries: Vec<RuntimeSmartContextArtifactManifestEntry>,
    detailed: bool,
) -> Option<String> {
    runtime_smart_context_artifact_manifest_delta_from_entries(entries, detailed, 0)
}

fn runtime_smart_context_artifact_manifest_delta_from_entries(
    entries: Vec<RuntimeSmartContextArtifactManifestEntry>,
    detailed: bool,
    unchanged_count: usize,
) -> Option<String> {
    if entries.is_empty() {
        if unchanged_count == 0 {
            return None;
        }
        return Some(format!("psc m refs-only unchanged={unchanged_count}"));
    }

    let mut header = "psc m refs-only".to_string();
    if unchanged_count > 0 {
        header.push_str(&format!(" unchanged={unchanged_count}"));
    }
    let mut lines = vec![header];
    let mut rendered_len = lines[0].len();
    for entry in entries {
        let semantic_count = entry
            .file_location_range_count
            .saturating_add(entry.diff_hunk_range_count)
            .saturating_add(entry.test_failure_range_count)
            .saturating_add(entry.error_range_count);
        let reference = runtime_smart_context_artifact_ref(&entry.id);
        let mut parts = vec![format!("- {reference}"), format!("b={}", entry.byte_len)];
        if detailed && entry.critical_range_count > 0 {
            parts.push(format!("cr={}", entry.critical_range_count));
        }
        if detailed && semantic_count > 0 {
            parts.push(format!("sr={semantic_count}"));
        }
        if detailed && entry.file_location_range_count > 0 {
            parts.push(format!("f={}", entry.file_location_range_count));
        }
        if detailed && entry.diff_hunk_range_count > 0 {
            parts.push(format!("d={}", entry.diff_hunk_range_count));
        }
        if detailed && entry.test_failure_range_count > 0 {
            parts.push(format!("t={}", entry.test_failure_range_count));
        }
        if detailed && entry.error_range_count > 0 {
            parts.push(format!("e={}", entry.error_range_count));
        }
        if detailed && let Some(kind) = entry.command_kind.as_deref() {
            parts.push(format!("k={kind}"));
        }
        let line = parts.join(" ");
        if rendered_len.saturating_add(line.len()).saturating_add(1)
            > SMART_CONTEXT_ARTIFACT_MANIFEST_MAX_CHARS
        {
            lines.push("[... psc manifest truncated ...]".to_string());
            break;
        }
        rendered_len = rendered_len.saturating_add(line.len()).saturating_add(1);
        lines.push(line);
    }
    Some(lines.join("\n"))
}

fn runtime_smart_context_append_input_manifest(
    value: &mut serde_json::Value,
    manifest: String,
) -> bool {
    let Some(input) = value
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return false;
    };
    input.push(serde_json::json!({
        "type": "message",
        "role": "user",
        "content": manifest,
    }));
    true
}

fn runtime_smart_context_append_missing_critical_ranges(
    original: &str,
    compacted: String,
    max_ranges: usize,
) -> String {
    if !prodex_context::critical_signal_self_check(original, &compacted).has_loss() {
        return compacted;
    }

    let ranges = prodex_context::critical_signal_lost_line_ranges_with_options(
        original,
        &compacted,
        prodex_context::CriticalSignalLineRangeOptions {
            context_lines: 1,
            max_ranges,
            max_range_lines: 6,
        },
    );
    if ranges.is_empty() {
        return compacted;
    }

    let lines = original.lines().collect::<Vec<_>>();
    let mut exact_ranges = Vec::new();
    for range in ranges {
        if range.start == 0 || range.start > lines.len() {
            continue;
        }
        let end = range.end.min(lines.len());
        exact_ranges.push(RuntimeSmartContextExactAppendixRange {
            reference: format!("L{}-L{}:", range.start, end),
            body: lines[range.start - 1..end].join("\n"),
        });
    }
    let Some((appendix, _)) = runtime_smart_context_render_exact_appendix(
        SMART_CONTEXT_LABEL_CRITICAL_EXACT,
        exact_ranges,
    ) else {
        return compacted;
    };

    format!("{compacted}\n\n{appendix}")
}

fn runtime_smart_context_dedupe_input_text(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let Some(input) = value
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    let mut seen = BTreeMap::<String, usize>::new();
    for (index, item) in input.iter_mut().enumerate() {
        if runtime_smart_context_value_is_static_context_item(item) {
            continue;
        }
        runtime_smart_context_dedupe_value_text(item, index, &mut seen, stats);
    }
    runtime_smart_context_replace_cross_turn_duplicate_refs(value, store, exactness_guard, stats);
}

fn runtime_smart_context_dedupe_value_text(
    value: &mut serde_json::Value,
    item_index: usize,
    seen: &mut BTreeMap<String, usize>,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    match value {
        serde_json::Value::String(text) => {
            if text.len() < SMART_CONTEXT_DUPLICATE_TEXT_MIN_BYTES {
                return;
            }
            let hash = runtime_proxy_crate::smart_context_hash_text(text);
            if let Some(first_index) = seen.get(&hash) {
                *text = format!("[psc dup input[{first_index}] h={hash}]");
                stats.duplicate_texts += 1;
            } else {
                seen.insert(hash.clone(), item_index);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_dedupe_value_text(item, item_index, seen, stats);
            }
        }
        serde_json::Value::Object(object) => {
            for item in object.values_mut() {
                runtime_smart_context_dedupe_value_text(item, item_index, seen, stats);
            }
        }
        _ => {}
    }
}

fn runtime_smart_context_replace_cross_turn_duplicate_refs(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let items = runtime_smart_context_collect_large_rehydratable_text_items(value);
    if items.is_empty() {
        return;
    }
    let artifacts = runtime_smart_context_available_artifacts_for_text_items(items.iter(), store);
    let plan = runtime_proxy_crate::smart_context_cross_turn_duplicate_ref_plan(
        items,
        artifacts,
        SMART_CONTEXT_DUPLICATE_TEXT_MIN_BYTES,
        exactness_guard,
    );
    let replacements = plan
        .actions
        .into_iter()
        .filter_map(|action| {
            let runtime_proxy_crate::SmartContextCrossTurnDuplicateRefAction::ReplaceWithArtifactRef {
                id,
                artifact,
                content_hash,
                byte_len,
            } = action
            else {
                return None;
            };
            Some((
                id,
                format!(
                    "[psc repeat {} h={} b={}]",
                    runtime_smart_context_artifact_ref(&artifact.id),
                    content_hash,
                    byte_len
                ),
            ))
        })
        .collect::<BTreeMap<_, _>>();
    if replacements.is_empty() {
        return;
    }
    stats.cross_turn_duplicate_texts +=
        runtime_smart_context_apply_text_replacements(value, &replacements);
}

fn runtime_smart_context_available_artifacts_for_text_items<'a>(
    items: impl IntoIterator<Item = &'a runtime_proxy_crate::SmartContextConversationItem>,
    store: &RuntimeSmartContextArtifactStore,
) -> Vec<runtime_proxy_crate::SmartContextArtifactRef> {
    items
        .into_iter()
        .filter_map(|item| {
            let content_hash = runtime_proxy_crate::smart_context_hash_text(&item.text);
            let artifact_text = store.get_text(&content_hash)?;
            (artifact_text == item.text).then_some(runtime_proxy_crate::SmartContextArtifactRef {
                id: content_hash.clone(),
                byte_len: item.text.len(),
                content_hash,
            })
        })
        .collect()
}

fn runtime_smart_context_collect_large_rehydratable_text_items(
    value: &serde_json::Value,
) -> Vec<runtime_proxy_crate::SmartContextConversationItem> {
    let mut items = Vec::new();
    let Some(object) = value.as_object() else {
        return items;
    };
    let mut keys = object.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    for key in keys {
        if runtime_smart_context_static_prompt_field_key(&key) {
            continue;
        }
        if let Some(item) = object.get(&key) {
            runtime_smart_context_collect_large_text_items_from_value(key, item, &mut items);
        }
    }
    items
}

fn runtime_smart_context_collect_large_text_items_from_value(
    id: String,
    value: &serde_json::Value,
    items: &mut Vec<runtime_proxy_crate::SmartContextConversationItem>,
) {
    if runtime_smart_context_value_is_static_context_item(value) {
        return;
    }
    match value {
        serde_json::Value::String(text)
            if text.len() >= SMART_CONTEXT_DUPLICATE_TEXT_MIN_BYTES
                && !text.contains("prodex-artifact:")
                && !text.contains(SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX)
                && !text.contains("prodex smart context artifact")
                && !text.contains("prodex-sc ") =>
        {
            items.push(runtime_proxy_crate::SmartContextConversationItem {
                id,
                text: text.clone(),
            });
        }
        serde_json::Value::Array(values) => {
            for (index, item) in values.iter().enumerate() {
                runtime_smart_context_collect_large_text_items_from_value(
                    format!("{id}[{index}]"),
                    item,
                    items,
                );
            }
        }
        serde_json::Value::Object(object) => {
            let mut keys = object.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                if let Some(item) = object.get(&key) {
                    if runtime_smart_context_static_prompt_field_key(&key) {
                        continue;
                    }
                    runtime_smart_context_collect_large_text_items_from_value(
                        format!("{id}.{key}"),
                        item,
                        items,
                    );
                }
            }
        }
        _ => {}
    }
}

fn runtime_smart_context_apply_text_replacements(
    value: &mut serde_json::Value,
    replacements: &BTreeMap<String, String>,
) -> usize {
    let Some(object) = value.as_object_mut() else {
        return 0;
    };
    let mut replaced = 0usize;
    let mut keys = object.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    for key in keys {
        if runtime_smart_context_static_prompt_field_key(&key) {
            continue;
        }
        if let Some(item) = object.get_mut(&key) {
            replaced +=
                runtime_smart_context_apply_text_replacements_to_value(item, key, replacements);
        }
    }
    replaced
}

fn runtime_smart_context_apply_text_replacements_to_value(
    value: &mut serde_json::Value,
    id: String,
    replacements: &BTreeMap<String, String>,
) -> usize {
    if runtime_smart_context_value_is_static_context_item(value) {
        return 0;
    }
    match value {
        serde_json::Value::String(text) => {
            if let Some(replacement) = replacements.get(&id) {
                *text = replacement.clone();
                1
            } else {
                0
            }
        }
        serde_json::Value::Array(values) => values
            .iter_mut()
            .enumerate()
            .map(|(index, item)| {
                runtime_smart_context_apply_text_replacements_to_value(
                    item,
                    format!("{id}[{index}]"),
                    replacements,
                )
            })
            .sum(),
        serde_json::Value::Object(object) => {
            let mut keys = object.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            keys.into_iter()
                .filter_map(|key| {
                    if runtime_smart_context_static_prompt_field_key(&key) {
                        return None;
                    }
                    object.get_mut(&key).map(|item| {
                        runtime_smart_context_apply_text_replacements_to_value(
                            item,
                            format!("{id}.{key}"),
                            replacements,
                        )
                    })
                })
                .sum()
        }
        _ => 0,
    }
}

fn runtime_smart_context_likely_blob_or_noise(text: &str) -> bool {
    prodex_context::is_context_blob_noise(text)
}

fn runtime_smart_context_critical_signal_self_check(
    before: &[u8],
    after: &[u8],
) -> prodex_context::CriticalSignalSelfCheck {
    let before = String::from_utf8_lossy(before);
    let after = String::from_utf8_lossy(after);
    prodex_context::critical_signal_self_check(&before, &after)
}

fn runtime_smart_context_regression_self_check(
    before: &[u8],
    after: &[u8],
    exactness_guard: runtime_proxy_crate::SmartContextExactnessGuard,
    missing_rehydrate_refs: Vec<String>,
) -> runtime_proxy_crate::SmartContextRegressionSelfCheck {
    let before_text = String::from_utf8_lossy(before);
    let after_text = String::from_utf8_lossy(after);
    runtime_proxy_crate::smart_context_regression_self_check(
        runtime_proxy_crate::SmartContextRegressionSelfCheckInput {
            exactness_guard,
            before_hash: runtime_proxy_crate::smart_context_hash_text(&before_text),
            after_hash: runtime_proxy_crate::smart_context_hash_text(&after_text),
            before_estimated_tokens:
                runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(before.len()),
            after_estimated_tokens:
                runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(after.len()),
            before_critical_signal_count: prodex_context::count_critical_signals(&before_text)
                .total(),
            after_critical_signal_count: prodex_context::count_critical_signals(&after_text)
                .total(),
            missing_rehydrate_refs,
        },
    )
}

fn runtime_smart_context_fallback_exact_reason(
    regression_check: &runtime_proxy_crate::SmartContextRegressionSelfCheck,
    critical_signal_check: prodex_context::CriticalSignalSelfCheck,
    stats: &RuntimeSmartContextTransformStats,
) -> Option<&'static str> {
    if critical_signal_check.has_loss() {
        return Some("critical_signal_loss");
    }
    if runtime_smart_context_rewrite_is_rehydrate_only(stats) {
        return None;
    }
    if regression_check.decision
        == runtime_proxy_crate::SmartContextRegressionSelfCheckDecision::FallbackExact
    {
        return Some(runtime_smart_context_regression_reason_label(
            &regression_check.reasons,
        ));
    }
    None
}

fn runtime_smart_context_rewrite_is_rehydrate_only(
    stats: &RuntimeSmartContextTransformStats,
) -> bool {
    stats.rehydrated_refs > 0
        && stats.tool_outputs_condensed == 0
        && stats.duplicate_texts == 0
        && stats.cross_turn_duplicate_texts == 0
        && stats.repeat_tool_output_refs == 0
        && stats.static_context_deltas == 0
}

fn runtime_smart_context_regression_reason_label(
    reasons: &[runtime_proxy_crate::SmartContextRegressionSelfCheckReason],
) -> &'static str {
    if reasons
        .iter()
        .any(|reason| matches!(reason, runtime_proxy_crate::SmartContextRegressionSelfCheckReason::CriticalSignalDropped))
    {
        "critical_signal_loss"
    } else if reasons.iter().any(|reason| {
        matches!(
            reason,
            runtime_proxy_crate::SmartContextRegressionSelfCheckReason::MissingRehydrateRefs
        )
    }) {
        "missing_rehydrate_refs"
    } else if reasons.iter().any(|reason| {
        matches!(
            reason,
            runtime_proxy_crate::SmartContextRegressionSelfCheckReason::ExactnessRequiredButPayloadChanged
        )
    }) {
        "exactness_required"
    } else if reasons.iter().any(|reason| {
        matches!(
            reason,
            runtime_proxy_crate::SmartContextRegressionSelfCheckReason::EmptyAfterPayload
        )
    }) {
        "empty_after_payload"
    } else {
        "token_budget_did_not_improve"
    }
}

#[allow(clippy::too_many_arguments)]
fn runtime_smart_context_log(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    transport: RuntimeSmartContextTransport,
    tier: &str,
    decision: &str,
    reasons: &str,
    body_bytes_before: usize,
    body_bytes_after: usize,
    stats: RuntimeSmartContextTransformStats,
    budget: &RuntimeSmartContextBudget,
    self_check: &'static str,
) {
    runtime_smart_context_record_rewrite_telemetry(
        shared,
        RuntimeSmartContextRewriteTelemetryRecord {
            body_bytes_before,
            body_bytes_after,
            estimated_tokens_before:
                runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(
                    body_bytes_before,
                ),
            estimated_tokens_after:
                runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(body_bytes_after),
            rewrite_kind: decision.to_string(),
            status: self_check.to_string(),
            fallback_reason: runtime_smart_context_telemetry_fallback_reason(decision, self_check)
                .map(str::to_string),
        },
    );
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "smart_context_autopilot",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", transport.label()),
                runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                runtime_proxy_log_field("tier", tier),
                runtime_proxy_log_field("decision", decision),
                runtime_proxy_log_field("reasons", reasons),
                runtime_proxy_log_field("token_usage_source", budget.token_usage_source),
                runtime_proxy_log_field(
                    "model_context_window_tokens",
                    budget.model_context_window_tokens.to_string(),
                ),
                runtime_proxy_log_field(
                    "model_context_window_source",
                    budget.model_context_window_source,
                ),
                runtime_proxy_log_field(
                    "observed_context_tokens",
                    budget
                        .observed_context_tokens
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                ),
                runtime_proxy_log_field("body_bytes_before", body_bytes_before.to_string()),
                runtime_proxy_log_field("body_bytes_after", body_bytes_after.to_string()),
                runtime_proxy_log_field(
                    "estimated_tokens_before",
                    runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(
                        body_bytes_before,
                    )
                    .to_string(),
                ),
                runtime_proxy_log_field(
                    "estimated_tokens_after",
                    runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(
                        body_bytes_after,
                    )
                    .to_string(),
                ),
                runtime_proxy_log_field(
                    "body_bytes_saved",
                    body_bytes_before
                        .saturating_sub(body_bytes_after)
                        .to_string(),
                ),
                runtime_proxy_log_field(
                    "rewrite_ratio_percent",
                    runtime_smart_context_rewrite_ratio_percent(
                        body_bytes_before,
                        body_bytes_after,
                    )
                    .to_string(),
                ),
                runtime_proxy_log_field("self_check", self_check),
                runtime_proxy_log_field("rewrite_kind", decision),
                runtime_proxy_log_field("rewrite_status", self_check),
                runtime_proxy_log_field(
                    "fallback_reason",
                    runtime_smart_context_telemetry_fallback_reason(decision, self_check)
                        .unwrap_or("-"),
                ),
                runtime_proxy_log_field("available_tokens", budget.available_tokens.to_string()),
                runtime_proxy_log_field(
                    "budget_mode",
                    runtime_smart_context_budget_mode_label(budget.policy.mode),
                ),
                runtime_proxy_log_field(
                    "max_inline_tool_output_bytes",
                    budget.policy.max_inline_tool_output_bytes.to_string(),
                ),
                runtime_proxy_log_field(
                    "max_rehydrate_tokens",
                    budget.policy.max_rehydrate_tokens.to_string(),
                ),
                runtime_proxy_log_field(
                    "policy_reasons",
                    runtime_smart_context_budget_policy_reason_labels(&budget.policy.reasons),
                ),
                runtime_proxy_log_field("artifacts_stored", stats.artifacts_stored.to_string()),
                runtime_proxy_log_field(
                    "tool_outputs_condensed",
                    stats.tool_outputs_condensed.to_string(),
                ),
                runtime_proxy_log_field("duplicate_texts", stats.duplicate_texts.to_string()),
                runtime_proxy_log_field(
                    "cross_turn_duplicate_texts",
                    stats.cross_turn_duplicate_texts.to_string(),
                ),
                runtime_proxy_log_field(
                    "repeat_tool_output_refs",
                    stats.repeat_tool_output_refs.to_string(),
                ),
                runtime_proxy_log_field(
                    "blob_outputs_condensed",
                    stats.blob_outputs_condensed.to_string(),
                ),
                runtime_proxy_log_field("rehydrated_refs", stats.rehydrated_refs.to_string()),
                runtime_proxy_log_field(
                    "static_context_deltas",
                    stats.static_context_deltas.to_string(),
                ),
            ],
        ),
    );
}

fn runtime_smart_context_record_rewrite_telemetry(
    shared: &RuntimeRotationProxyShared,
    record: RuntimeSmartContextRewriteTelemetryRecord,
) {
    let Some(states) = RUNTIME_SMART_CONTEXT_PROXY_STATES.get() else {
        return;
    };
    let Ok(mut states) = states.lock() else {
        return;
    };
    let Some(state) = states.get_mut(&shared.log_path) else {
        return;
    };
    if !state.enabled {
        return;
    }
    state.rewrite_telemetry_history.push(record);
    if state.rewrite_telemetry_history.len() > SMART_CONTEXT_REWRITE_TELEMETRY_HISTORY_LIMIT {
        let overflow = state
            .rewrite_telemetry_history
            .len()
            .saturating_sub(SMART_CONTEXT_REWRITE_TELEMETRY_HISTORY_LIMIT);
        state.rewrite_telemetry_history.drain(0..overflow);
    }
}

fn runtime_smart_context_telemetry_fallback_reason<'a>(
    decision: &str,
    self_check: &'a str,
) -> Option<&'a str> {
    if decision == "self_check_passthrough" {
        Some(self_check)
    } else {
        None
    }
}

fn runtime_smart_context_budget_mode_label(
    mode: runtime_proxy_crate::SmartContextBudgetMode,
) -> &'static str {
    match mode {
        runtime_proxy_crate::SmartContextBudgetMode::ExactPassThrough => "exact_pass_through",
        runtime_proxy_crate::SmartContextBudgetMode::LargeLossless => "large_lossless",
        runtime_proxy_crate::SmartContextBudgetMode::ArtifactCondensed => "artifact_condensed",
        runtime_proxy_crate::SmartContextBudgetMode::MinimalRefsOnly => "minimal_refs_only",
    }
}

fn runtime_smart_context_budget_policy_reason_labels(
    reasons: &[runtime_proxy_crate::SmartContextBudgetPolicyReason],
) -> String {
    if reasons.is_empty() {
        return "-".to_string();
    }
    reasons
        .iter()
        .map(|reason| match reason {
            runtime_proxy_crate::SmartContextBudgetPolicyReason::ExactnessRequired => {
                "exactness_required"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::StaticContextChanged => {
                "static_context_changed"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::MissingRehydrateRefs => {
                "missing_rehydrate_refs"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::UnknownTokenWindow => {
                "unknown_token_window"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::UnsafeAccounting => {
                "unsafe_accounting"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::RecentRewriteSavingsSafe => {
                "recent_rewrite_savings_safe"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::PlentyOfBudget => {
                "plenty_of_budget"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::ModerateBudget => {
                "moderate_budget"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::TightBudget => "tight_budget",
            runtime_proxy_crate::SmartContextBudgetPolicyReason::CriticalBudget => {
                "critical_budget"
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn runtime_smart_context_rewrite_self_check(
    body_bytes_before: usize,
    body_bytes_after: usize,
    stats: &RuntimeSmartContextTransformStats,
) -> &'static str {
    if stats.rehydrated_refs > 0
        && stats.tool_outputs_condensed == 0
        && stats.duplicate_texts == 0
        && stats.static_context_deltas == 0
    {
        "ok_rehydrate_exact"
    } else if body_bytes_after < body_bytes_before {
        "ok_saved"
    } else if body_bytes_after == body_bytes_before {
        "zero_savings"
    } else {
        "growth"
    }
}

fn runtime_smart_context_saved_tokens(body_bytes_before: usize, body_bytes_after: usize) -> u64 {
    runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(body_bytes_before)
        .saturating_sub(
            runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(body_bytes_after),
        )
}

fn runtime_smart_context_minified_json_body(
    value: &serde_json::Value,
    original_body: &[u8],
) -> Option<Vec<u8>> {
    let Cow::Owned(body) =
        runtime_proxy_crate::smart_context_structural_minify_json_value_body(original_body, value)
    else {
        return None;
    };
    runtime_smart_context_validated_minified_json_body(body, original_body)
}

fn runtime_smart_context_minified_json_body_from_original(original_body: &[u8]) -> Option<Vec<u8>> {
    let Cow::Owned(body) =
        runtime_proxy_crate::smart_context_structural_minify_json_body(original_body)
    else {
        return None;
    };
    runtime_smart_context_validated_minified_json_body(body, original_body)
}

fn runtime_smart_context_validated_minified_json_body(
    body: Vec<u8>,
    original_body: &[u8],
) -> Option<Vec<u8>> {
    if body.len() >= original_body.len() {
        return None;
    }
    let before = String::from_utf8_lossy(original_body);
    let after = String::from_utf8_lossy(&body);
    prodex_context::critical_signal_self_check(&before, &after)
        .passed()
        .then_some(body)
}

#[cfg(test)]
fn runtime_smart_context_should_pass_through_after_self_check(
    body_bytes_before: usize,
    body_bytes_after: usize,
    stats: &RuntimeSmartContextTransformStats,
) -> bool {
    stats.rehydrated_refs == 0
        && stats.static_context_deltas == 0
        && body_bytes_after >= body_bytes_before
}

fn runtime_smart_context_rewrite_ratio_percent(
    body_bytes_before: usize,
    body_bytes_after: usize,
) -> usize {
    if body_bytes_before == 0 {
        return 100;
    }
    body_bytes_after.saturating_mul(100) / body_bytes_before
}

fn runtime_smart_context_reason_labels(
    reasons: &[runtime_proxy_crate::SmartContextExactnessReason],
) -> String {
    if reasons.is_empty() {
        return "-".to_string();
    }
    reasons
        .iter()
        .map(|reason| match reason {
            runtime_proxy_crate::SmartContextExactnessReason::ExplicitExactMode => "exact_mode",
            runtime_proxy_crate::SmartContextExactnessReason::PreviousResponseAffinity => {
                "previous_response"
            }
            runtime_proxy_crate::SmartContextExactnessReason::TurnStateAffinity => "turn_state",
            runtime_proxy_crate::SmartContextExactnessReason::SessionAffinity => "session",
            runtime_proxy_crate::SmartContextExactnessReason::ToolOutputWithoutArtifact => {
                "tool_output_without_artifact"
            }
            runtime_proxy_crate::SmartContextExactnessReason::RehydrateRequired => "rehydrate",
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn runtime_smart_context_tier_label(
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
) -> &'static str {
    match tier {
        runtime_proxy_crate::SmartContextTokenBudgetTier::Exact => "exact",
        runtime_proxy_crate::SmartContextTokenBudgetTier::Large => "large",
        runtime_proxy_crate::SmartContextTokenBudgetTier::Condensed => "condensed",
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal => "minimal",
    }
}

#[cfg(test)]
#[path = "../../tests/src/runtime_proxy/smart_context.rs"]
mod tests;
