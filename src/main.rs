use anyhow::{Context, Result, bail};
use base64::Engine;
use chrono::{Local, TimeZone};
use clap::{Args, Parser, Subcommand};
use dirs::home_dir;
use fs2::FileExt;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Cursor, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TrySendError};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tiny_http::{
    Header as TinyHeader, ReadWrite as TinyReadWrite, Response as TinyResponse,
    Server as TinyServer, StatusCode as TinyStatusCode,
};
use tokio::runtime::{Builder as TokioRuntimeBuilder, Runtime as TokioRuntime};
use tungstenite::client::IntoClientRequest;
use tungstenite::error::UrlError as WsUrlError;
use tungstenite::handshake::derive_accept_key;
use tungstenite::http::{HeaderName as WsHeaderName, HeaderValue as WsHeaderValue};
use tungstenite::protocol::Role as WsRole;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{
    Error as WsError, HandshakeError as WsHandshakeError, Message as WsMessage,
    WebSocket as WsSocket, client_tls_with_config,
};

mod housekeeping;
mod profile_commands;
mod profile_identity;
mod quota_support;
#[path = "runtime_tuning.rs"]
mod runtime_config;
mod runtime_doctor;
mod shared_codex_fs;
#[path = "cli_render.rs"]
mod terminal_ui;
mod update_notice;

use housekeeping::*;
use profile_commands::*;
use profile_identity::*;
use quota_support::*;
use runtime_config::*;
use runtime_doctor::*;
use shared_codex_fs::*;
use terminal_ui::*;
use update_notice::*;

const DEFAULT_PRODEX_DIR: &str = ".prodex";
const DEFAULT_CODEX_DIR: &str = ".codex";
const DEFAULT_CHATGPT_BASE_URL: &str = "https://chatgpt.com/backend-api";
const DEFAULT_WATCH_INTERVAL_SECONDS: u64 = 5;
const RUN_SELECTION_NEAR_OPTIMAL_BPS: i64 = 1_000;
const RUN_SELECTION_HYSTERESIS_BPS: i64 = 500;
const RUN_SELECTION_COOLDOWN_SECONDS: i64 = 15 * 60;
const RESPONSE_PROFILE_BINDING_LIMIT: usize = 65_536;
const TURN_STATE_PROFILE_BINDING_LIMIT: usize = 4_096;
const SESSION_ID_PROFILE_BINDING_LIMIT: usize = 4_096;
const APP_STATE_LAST_RUN_RETENTION_SECONDS: i64 = if cfg!(test) { 60 } else { 90 * 24 * 60 * 60 };
const APP_STATE_SESSION_BINDING_RETENTION_SECONDS: i64 =
    if cfg!(test) { 60 } else { 30 * 24 * 60 * 60 };
const RUNTIME_SCORE_RETENTION_SECONDS: i64 = if cfg!(test) { 120 } else { 14 * 24 * 60 * 60 };
const RUNTIME_USAGE_SNAPSHOT_RETENTION_SECONDS: i64 =
    if cfg!(test) { 120 } else { 7 * 24 * 60 * 60 };
const PROD_EX_TMP_LOGIN_RETENTION_SECONDS: i64 = if cfg!(test) { 60 } else { 24 * 60 * 60 };
const ORPHAN_MANAGED_PROFILE_AUDIT_RETENTION_SECONDS: i64 =
    if cfg!(test) { 60 } else { 7 * 24 * 60 * 60 };
const RUNTIME_PROXY_LOG_RETENTION_SECONDS: i64 = if cfg!(test) { 120 } else { 7 * 24 * 60 * 60 };
const RUNTIME_PROXY_LOG_RETENTION_COUNT: usize = if cfg!(test) { 4 } else { 40 };
const RUNTIME_PREVIOUS_RESPONSE_RETRY_DELAYS_MS: [u64; 3] = [75, 200, 500];
const RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT: usize = if cfg!(test) { 4 } else { 12 };
const RUNTIME_PROXY_PRECOMMIT_BUDGET_MS: u64 = if cfg!(test) { 500 } else { 3_000 };
const RUNTIME_PROXY_PRECOMMIT_CONTINUATION_ATTEMPT_LIMIT: usize =
    RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT * 2;
const RUNTIME_PROXY_PRECOMMIT_CONTINUATION_BUDGET_MS: u64 = RUNTIME_PROXY_PRECOMMIT_BUDGET_MS * 4;
const RUNTIME_PROFILE_RETRY_BACKOFF_SECONDS: i64 = if cfg!(test) { 2 } else { 20 };
const RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS: i64 = if cfg!(test) { 2 } else { 15 };
const RUNTIME_PROFILE_TRANSPORT_BACKOFF_MAX_SECONDS: i64 = if cfg!(test) { 8 } else { 120 };
const RUNTIME_PROXY_LOCAL_OVERLOAD_BACKOFF_SECONDS: i64 = if cfg!(test) { 1 } else { 3 };
const RUNTIME_PROXY_ADMISSION_WAIT_BUDGET_MS: u64 = if cfg!(test) { 80 } else { 750 };
const RUNTIME_PROXY_ADMISSION_WAIT_POLL_MS: u64 = if cfg!(test) { 5 } else { 25 };
const RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS: u64 = if cfg!(test) { 80 } else { 750 };
const RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_POLL_MS: u64 = if cfg!(test) { 5 } else { 25 };
const RUNTIME_PROXY_PRESSURE_ADMISSION_WAIT_BUDGET_MS: u64 = if cfg!(test) { 25 } else { 200 };
const RUNTIME_PROXY_PRESSURE_LONG_LIVED_QUEUE_WAIT_BUDGET_MS: u64 =
    if cfg!(test) { 25 } else { 200 };
const RUNTIME_PROXY_PRESSURE_PRECOMMIT_BUDGET_MS: u64 = if cfg!(test) { 150 } else { 800 };
#[allow(dead_code)]
const RUNTIME_PROXY_PRESSURE_PRECOMMIT_CONTINUATION_BUDGET_MS: u64 =
    if cfg!(test) { 250 } else { 1_500 };
const RUNTIME_PROXY_PRESSURE_PRECOMMIT_ATTEMPT_LIMIT: usize = if cfg!(test) { 2 } else { 6 };
#[allow(dead_code)]
const RUNTIME_PROXY_PRESSURE_PRECOMMIT_CONTINUATION_ATTEMPT_LIMIT: usize =
    if cfg!(test) { 4 } else { 8 };
const RUNTIME_PROXY_COMPACT_OWNER_RETRY_DELAY_MS: u64 = if cfg!(test) { 5 } else { 150 };
const RUNTIME_PROFILE_INFLIGHT_SOFT_LIMIT: usize = if cfg!(test) { 1 } else { 4 };
const RUNTIME_PROFILE_INFLIGHT_HARD_LIMIT: usize = if cfg!(test) { 2 } else { 8 };
const RUNTIME_PROFILE_HEALTH_DECAY_SECONDS: i64 = if cfg!(test) { 2 } else { 60 };
const RUNTIME_PROFILE_USAGE_CACHE_FRESH_SECONDS: i64 = if cfg!(test) { 30 } else { 300 };
const UPDATE_CHECK_CACHE_TTL_SECONDS: i64 = if cfg!(test) { 5 } else { 21_600 };
const UPDATE_CHECK_STALE_CURRENT_TTL_SECONDS: i64 = if cfg!(test) { 1 } else { 300 };
const UPDATE_CHECK_HTTP_CONNECT_TIMEOUT_MS: u64 = if cfg!(test) { 200 } else { 800 };
const UPDATE_CHECK_HTTP_READ_TIMEOUT_MS: u64 = if cfg!(test) { 400 } else { 1200 };
const RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS: i64 = if cfg!(test) { 300 } else { 1800 };
const RUNTIME_PROFILE_SYNC_PROBE_FALLBACK_LIMIT: usize = 2;
const RUNTIME_STARTUP_PROBE_WARM_LIMIT: usize = 3;
const RUNTIME_STATE_SAVE_DEBOUNCE_MS: u64 = if cfg!(test) { 5 } else { 150 };
const RUNTIME_STATE_SAVE_QUEUE_PRESSURE_THRESHOLD: usize = 8;
const RUNTIME_CONTINUATION_JOURNAL_QUEUE_PRESSURE_THRESHOLD: usize = 8;
const RUNTIME_PROBE_REFRESH_QUEUE_PRESSURE_THRESHOLD: usize = 16;
const RUNTIME_BINDING_TOUCH_PERSIST_INTERVAL_SECONDS: i64 = if cfg!(test) { 1 } else { 60 };
const RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS: i64 = if cfg!(test) { 4 } else { 180 };
const RUNTIME_PROFILE_SUCCESS_STREAK_DECAY_SECONDS: i64 = if cfg!(test) { 8 } else { 300 };
const RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS: i64 = if cfg!(test) { 8 } else { 300 };
const RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY: u32 = 4;
const RUNTIME_PROFILE_CONNECT_FAILURE_HEALTH_PENALTY: u32 = 5;
const RUNTIME_PROFILE_FORWARD_FAILURE_HEALTH_PENALTY: u32 = 3;
const RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY: u32 = 2;
const RUNTIME_PROFILE_LATENCY_PENALTY_MAX: u32 = 12;
const RUNTIME_PROFILE_HEALTH_SUCCESS_RECOVERY_SCORE: u32 = 2;
const RUNTIME_PROFILE_BAD_PAIRING_PENALTY: u32 = 2;
const RUNTIME_PROFILE_HEALTH_MAX_SCORE: u32 = 16;
const RUNTIME_PROFILE_SUCCESS_STREAK_MAX: u32 = 3;
const QUOTA_HTTP_CONNECT_TIMEOUT_MS: u64 = if cfg!(test) { 250 } else { 5_000 };
const QUOTA_HTTP_READ_TIMEOUT_MS: u64 = if cfg!(test) { 500 } else { 10_000 };
// Match Codex's default Responses stream idle timeout so the local proxy stays transport-transparent.
const RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS: u64 = if cfg!(test) { 250 } else { 300_000 };
const RUNTIME_PROXY_HTTP_CONNECT_TIMEOUT_MS: u64 = if cfg!(test) { 250 } else { 5_000 };
const RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS: u64 = if cfg!(test) { 250 } else { 15_000 };
const RUNTIME_PROXY_WEBSOCKET_PRECOMMIT_PROGRESS_TIMEOUT_MS: u64 =
    if cfg!(test) { 120 } else { 8_000 };
const RUNTIME_PROXY_WEBSOCKET_PREVIOUS_RESPONSE_REUSE_STALE_MS: u64 =
    if cfg!(test) { 60_000 } else { 60_000 };
const RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS: u64 = if cfg!(test) { 50 } else { 1_000 };
const RUNTIME_PROXY_SSE_LOOKAHEAD_BYTES: usize = 8 * 1024;
const RUNTIME_PROXY_LOG_FILE_PREFIX: &str = "prodex-runtime";
const RUNTIME_PROXY_LATEST_LOG_POINTER: &str = "prodex-runtime-latest.path";
const RUNTIME_PROXY_DOCTOR_TAIL_BYTES: usize = 128 * 1024;
const INFO_RUNTIME_LOG_TAIL_BYTES: usize = if cfg!(test) { 64 * 1024 } else { 512 * 1024 };
const INFO_FORECAST_LOOKBACK_SECONDS: i64 = if cfg!(test) { 3_600 } else { 3 * 60 * 60 };
const INFO_FORECAST_MIN_SPAN_SECONDS: i64 = if cfg!(test) { 60 } else { 5 * 60 };
const INFO_RECENT_LOAD_WINDOW_SECONDS: i64 = if cfg!(test) { 600 } else { 30 * 60 };
const LAST_GOOD_FILE_SUFFIX: &str = ".last-good";
const RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_SECONDS: i64 = if cfg!(test) { 5 } else { 180 };
const RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD: u32 = 2;
const RUNTIME_CONTINUATION_SUSPECT_GRACE_SECONDS: i64 = if cfg!(test) { 5 } else { 120 };
const RUNTIME_CONTINUATION_DEAD_GRACE_SECONDS: i64 = if cfg!(test) { 5 } else { 900 };
const RUNTIME_CONTINUATION_SUSPECT_NOT_FOUND_STREAK_LIMIT: u32 = 2;
const RUNTIME_CONTINUATION_CONFIDENCE_MAX: u32 = 8;
const RUNTIME_CONTINUATION_VERIFIED_CONFIDENCE_BONUS: u32 = 2;
const RUNTIME_CONTINUATION_TOUCH_CONFIDENCE_BONUS: u32 = 1;
const RUNTIME_CONTINUATION_SUSPECT_CONFIDENCE_PENALTY: u32 = 1;
const RUNTIME_SIDECAR_STALE_SAVE_RETRY_LIMIT: usize = if cfg!(test) { 3 } else { 6 };
const RUNTIME_BROKER_READY_TIMEOUT_MS: u64 = if cfg!(test) { 3_000 } else { 15_000 };
const RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS: u64 = if cfg!(test) { 250 } else { 750 };
const RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS: u64 = if cfg!(test) { 400 } else { 1_500 };
const RUNTIME_BROKER_POLL_INTERVAL_MS: u64 = if cfg!(test) { 25 } else { 100 };
const RUNTIME_BROKER_IDLE_GRACE_SECONDS: i64 = if cfg!(test) { 1 } else { 5 };
const CLI_WIDTH: usize = 110;
const CLI_MIN_WIDTH: usize = 60;
const CLI_LABEL_WIDTH: usize = 16;
const CLI_MIN_LABEL_WIDTH: usize = 10;
const CLI_MAX_LABEL_WIDTH: usize = 24;
const CLI_TABLE_GAP: &str = "  ";
const SHARED_CODEX_DIR_NAMES: &[&str] = &[
    "sessions",
    "archived_sessions",
    "shell_snapshots",
    "memories",
    "rules",
    "skills",
];
const SHARED_CODEX_FILE_NAMES: &[&str] = &["history.jsonl", "config.toml"];
const SHARED_CODEX_SQLITE_PREFIXES: &[&str] = &["state_", "logs_"];
const SHARED_CODEX_SQLITE_SUFFIXES: &[&str] = &[".sqlite", ".sqlite-shm", ".sqlite-wal"];
static STATE_SAVE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static RUNTIME_STATE_SAVE_QUEUE: OnceLock<Arc<RuntimeStateSaveQueue>> = OnceLock::new();
static RUNTIME_CONTINUATION_JOURNAL_SAVE_QUEUE: OnceLock<Arc<RuntimeContinuationJournalSaveQueue>> =
    OnceLock::new();
static RUNTIME_PROBE_REFRESH_QUEUE: OnceLock<Arc<RuntimeProbeRefreshQueue>> = OnceLock::new();
static RUNTIME_SIDECAR_GENERATION_CACHE: OnceLock<Mutex<BTreeMap<PathBuf, u64>>> = OnceLock::new();
static RUNTIME_PERSISTENCE_MODE_BY_LOG_PATH: OnceLock<Mutex<BTreeMap<PathBuf, bool>>> =
    OnceLock::new();
static RUNTIME_BROKER_METADATA_BY_LOG_PATH: OnceLock<
    Mutex<BTreeMap<PathBuf, RuntimeBrokerMetadata>>,
> = OnceLock::new();

fn runtime_proxy_log_dir() -> PathBuf {
    env::var_os("PRODEX_RUNTIME_LOG_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
}

fn create_runtime_proxy_log_path() -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let dir = runtime_proxy_log_dir();
    let _ = fs::create_dir_all(&dir);
    dir.join(format!(
        "{RUNTIME_PROXY_LOG_FILE_PREFIX}-{}-{millis}.log",
        std::process::id()
    ))
}

fn runtime_proxy_latest_log_pointer_path() -> PathBuf {
    runtime_proxy_log_dir().join(RUNTIME_PROXY_LATEST_LOG_POINTER)
}

fn initialize_runtime_proxy_log_path() -> PathBuf {
    cleanup_runtime_proxy_log_housekeeping();
    let log_path = create_runtime_proxy_log_path();
    let _ = fs::write(
        runtime_proxy_latest_log_pointer_path(),
        format!("{}\n", log_path.display()),
    );
    runtime_proxy_log_to_path(
        &log_path,
        &format!(
            "runtime proxy log initialized pid={} cwd={}",
            std::process::id(),
            std::env::current_dir()
                .ok()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "<unknown>".to_string())
        ),
    );
    log_path
}

fn runtime_proxy_worker_count() -> usize {
    let parallelism = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4);
    usize_override(
        "PRODEX_RUNTIME_PROXY_WORKER_COUNT",
        (parallelism.saturating_mul(4)).clamp(8, 32),
    )
    .clamp(1, 64)
}

fn runtime_proxy_long_lived_worker_count() -> usize {
    let parallelism = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4);
    usize_override(
        "PRODEX_RUNTIME_PROXY_LONG_LIVED_WORKER_COUNT",
        parallelism.clamp(2, 8),
    )
    .clamp(1, 32)
}

fn runtime_proxy_long_lived_queue_capacity(worker_count: usize) -> usize {
    usize_override(
        "PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_CAPACITY",
        worker_count.saturating_mul(8).clamp(16, 128),
    )
    .max(1)
}

fn runtime_proxy_active_request_limit(
    worker_count: usize,
    long_lived_worker_count: usize,
) -> usize {
    usize_override(
        "PRODEX_RUNTIME_PROXY_ACTIVE_REQUEST_LIMIT",
        worker_count
            .saturating_add(long_lived_worker_count.saturating_mul(4))
            .clamp(16, 128),
    )
    .max(1)
}

#[derive(Debug, Clone, Copy)]
struct RuntimeProxyLaneLimits {
    responses: usize,
    compact: usize,
    websocket: usize,
    standard: usize,
}

#[derive(Debug, Clone)]
struct RuntimeProxyLaneAdmission {
    responses_active: Arc<AtomicUsize>,
    compact_active: Arc<AtomicUsize>,
    websocket_active: Arc<AtomicUsize>,
    standard_active: Arc<AtomicUsize>,
    limits: RuntimeProxyLaneLimits,
}

impl RuntimeProxyLaneAdmission {
    fn new(limits: RuntimeProxyLaneLimits) -> Self {
        Self {
            responses_active: Arc::new(AtomicUsize::new(0)),
            compact_active: Arc::new(AtomicUsize::new(0)),
            websocket_active: Arc::new(AtomicUsize::new(0)),
            standard_active: Arc::new(AtomicUsize::new(0)),
            limits,
        }
    }

    fn active_counter(&self, lane: RuntimeRouteKind) -> Arc<AtomicUsize> {
        match lane {
            RuntimeRouteKind::Responses => Arc::clone(&self.responses_active),
            RuntimeRouteKind::Compact => Arc::clone(&self.compact_active),
            RuntimeRouteKind::Websocket => Arc::clone(&self.websocket_active),
            RuntimeRouteKind::Standard => Arc::clone(&self.standard_active),
        }
    }

    fn limit(&self, lane: RuntimeRouteKind) -> usize {
        match lane {
            RuntimeRouteKind::Responses => self.limits.responses,
            RuntimeRouteKind::Compact => self.limits.compact,
            RuntimeRouteKind::Websocket => self.limits.websocket,
            RuntimeRouteKind::Standard => self.limits.standard,
        }
    }
}

fn runtime_proxy_lane_limits(
    global_limit: usize,
    worker_count: usize,
    long_lived_worker_count: usize,
) -> RuntimeProxyLaneLimits {
    let global_limit = global_limit.max(1);
    RuntimeProxyLaneLimits {
        responses: usize_override(
            "PRODEX_RUNTIME_PROXY_RESPONSES_ACTIVE_LIMIT",
            (global_limit.saturating_mul(3) / 4).clamp(4, global_limit),
        )
        .min(global_limit)
        .max(1),
        compact: usize_override(
            "PRODEX_RUNTIME_PROXY_COMPACT_ACTIVE_LIMIT",
            (global_limit / 4).clamp(2, 6).min(global_limit),
        )
        .min(global_limit)
        .max(1),
        websocket: usize_override(
            "PRODEX_RUNTIME_PROXY_WEBSOCKET_ACTIVE_LIMIT",
            long_lived_worker_count.clamp(2, global_limit),
        )
        .min(global_limit)
        .max(1),
        standard: usize_override(
            "PRODEX_RUNTIME_PROXY_STANDARD_ACTIVE_LIMIT",
            (worker_count / 2).clamp(2, 8).min(global_limit),
        )
        .min(global_limit)
        .max(1),
    }
}

fn runtime_proxy_log_to_path(log_path: &Path, message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f %:z");
    let sanitized = message.replace(['\r', '\n'], " ");
    let line = format!("[{timestamp}] {sanitized}\n");
    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        let _ = file.write_all(line.as_bytes());
        let _ = file.flush();
    }
}

fn runtime_persistence_mode_by_log_path() -> &'static Mutex<BTreeMap<PathBuf, bool>> {
    RUNTIME_PERSISTENCE_MODE_BY_LOG_PATH.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn register_runtime_proxy_persistence_mode(log_path: &Path, enabled: bool) {
    let mut modes = runtime_persistence_mode_by_log_path()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    modes.insert(log_path.to_path_buf(), enabled);
}

fn unregister_runtime_proxy_persistence_mode(log_path: &Path) {
    let mut modes = runtime_persistence_mode_by_log_path()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    modes.remove(log_path);
}

fn runtime_proxy_persistence_enabled_for_log_path(log_path: &Path) -> bool {
    runtime_persistence_mode_by_log_path()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(log_path)
        .copied()
        .unwrap_or(true)
}

fn runtime_proxy_persistence_enabled(shared: &RuntimeRotationProxyShared) -> bool {
    runtime_proxy_persistence_enabled_for_log_path(&shared.log_path)
}

fn runtime_broker_metadata_by_log_path() -> &'static Mutex<BTreeMap<PathBuf, RuntimeBrokerMetadata>>
{
    RUNTIME_BROKER_METADATA_BY_LOG_PATH.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn register_runtime_broker_metadata(log_path: &Path, metadata: RuntimeBrokerMetadata) {
    let mut metadata_by_path = runtime_broker_metadata_by_log_path()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    metadata_by_path.insert(log_path.to_path_buf(), metadata);
}

fn unregister_runtime_broker_metadata(log_path: &Path) {
    let mut metadata_by_path = runtime_broker_metadata_by_log_path()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    metadata_by_path.remove(log_path);
}

#[allow(dead_code)]
fn runtime_broker_metadata_for_log_path(log_path: &Path) -> Option<RuntimeBrokerMetadata> {
    runtime_broker_metadata_by_log_path()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(log_path)
        .cloned()
}

fn schedule_runtime_state_save(
    shared: &RuntimeRotationProxyShared,
    state: AppState,
    continuations: RuntimeContinuationStore,
    profile_scores: BTreeMap<String, RuntimeProfileHealth>,
    usage_snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot>,
    backoffs: RuntimeProfileBackoffs,
    paths: AppPaths,
    reason: &str,
) {
    if !runtime_proxy_persistence_enabled(shared) {
        runtime_proxy_log(
            shared,
            format!(
                "state_save_suppressed role=follower reason={reason} path={}",
                paths.state_file.display()
            ),
        );
        return;
    }
    let revision = shared.state_save_revision.fetch_add(1, Ordering::SeqCst) + 1;
    let queued_at = Instant::now();
    let ready_at = queued_at + runtime_state_save_debounce(reason);
    if cfg!(test) {
        runtime_proxy_log(
            shared,
            format!(
                "state_save_inline revision={} reason={} ready_in_ms={}",
                revision,
                reason,
                ready_at.saturating_duration_since(queued_at).as_millis()
            ),
        );
        match save_runtime_state_snapshot_if_latest(
            &paths,
            &state,
            &continuations,
            &profile_scores,
            &usage_snapshots,
            &backoffs,
            revision,
            &shared.state_save_revision,
        ) {
            Ok(true) => runtime_proxy_log(
                shared,
                format!(
                    "state_save_ok revision={} reason={} lag_ms=0",
                    revision, reason
                ),
            ),
            Ok(false) => runtime_proxy_log(
                shared,
                format!(
                    "state_save_skipped revision={} reason={} lag_ms=0",
                    revision, reason
                ),
            ),
            Err(err) => runtime_proxy_log(
                shared,
                format!(
                    "state_save_error revision={} reason={} lag_ms=0 stage=write error={err:#}",
                    revision, reason
                ),
            ),
        }
        if runtime_state_save_reason_requires_continuation_journal(reason) {
            schedule_runtime_continuation_journal_save(shared, continuations, paths, reason);
        }
        return;
    }
    let queue = runtime_state_save_queue();
    let mut pending = queue
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    pending.insert(
        paths.state_file.clone(),
        RuntimeStateSaveJob {
            paths: paths.clone(),
            state,
            continuations: continuations.clone(),
            profile_scores,
            usage_snapshots,
            backoffs,
            revision,
            latest_revision: Arc::clone(&shared.state_save_revision),
            log_path: shared.log_path.clone(),
            reason: reason.to_string(),
            queued_at,
            ready_at,
        },
    );
    let backlog = pending.len().saturating_sub(1);
    drop(pending);
    queue.wake.notify_one();
    runtime_proxy_log(
        shared,
        format!(
            "state_save_queued revision={} reason={} backlog={} ready_in_ms={}",
            revision,
            reason,
            backlog,
            ready_at.saturating_duration_since(queued_at).as_millis()
        ),
    );
    if runtime_proxy_queue_pressure_active(backlog, 0, 0) {
        runtime_proxy_log(
            shared,
            format!(
                "state_save_queue_backpressure revision={} reason={} backlog={backlog}",
                revision, reason
            ),
        );
    }
    if runtime_state_save_reason_requires_continuation_journal(reason) {
        schedule_runtime_continuation_journal_save(shared, continuations, paths, reason);
    }
}

fn runtime_state_save_queue() -> Arc<RuntimeStateSaveQueue> {
    Arc::clone(RUNTIME_STATE_SAVE_QUEUE.get_or_init(|| {
        let queue = Arc::new(RuntimeStateSaveQueue {
            pending: Mutex::new(BTreeMap::new()),
            wake: Condvar::new(),
            active: Arc::new(AtomicUsize::new(0)),
        });
        let worker_queue = Arc::clone(&queue);
        thread::spawn(move || runtime_state_save_worker_loop(worker_queue));
        queue
    }))
}

fn runtime_continuation_journal_save_queue() -> Arc<RuntimeContinuationJournalSaveQueue> {
    Arc::clone(RUNTIME_CONTINUATION_JOURNAL_SAVE_QUEUE.get_or_init(|| {
        let queue = Arc::new(RuntimeContinuationJournalSaveQueue {
            pending: Mutex::new(BTreeMap::new()),
            wake: Condvar::new(),
            active: Arc::new(AtomicUsize::new(0)),
        });
        let worker_queue = Arc::clone(&queue);
        thread::spawn(move || runtime_continuation_journal_save_worker_loop(worker_queue));
        queue
    }))
}

fn runtime_probe_refresh_queue() -> Arc<RuntimeProbeRefreshQueue> {
    Arc::clone(RUNTIME_PROBE_REFRESH_QUEUE.get_or_init(|| {
        let queue = Arc::new(RuntimeProbeRefreshQueue {
            pending: Mutex::new(BTreeMap::new()),
            wake: Condvar::new(),
            active: Arc::new(AtomicUsize::new(0)),
        });
        let worker_queue = Arc::clone(&queue);
        thread::spawn(move || runtime_probe_refresh_worker_loop(worker_queue));
        queue
    }))
}

fn runtime_state_save_queue_backlog() -> usize {
    runtime_state_save_queue()
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .len()
}

#[allow(dead_code)]
fn runtime_state_save_queue_active() -> usize {
    runtime_state_save_queue().active.load(Ordering::SeqCst)
}

fn runtime_continuation_journal_queue_backlog() -> usize {
    runtime_continuation_journal_save_queue()
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .len()
}

#[allow(dead_code)]
fn runtime_continuation_journal_queue_active() -> usize {
    runtime_continuation_journal_save_queue()
        .active
        .load(Ordering::SeqCst)
}

fn runtime_probe_refresh_queue_backlog() -> usize {
    runtime_probe_refresh_queue()
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .len()
}

#[allow(dead_code)]
fn runtime_probe_refresh_queue_active() -> usize {
    runtime_probe_refresh_queue().active.load(Ordering::SeqCst)
}

fn runtime_proxy_queue_pressure_active(
    state_save_backlog: usize,
    continuation_journal_backlog: usize,
    probe_refresh_backlog: usize,
) -> bool {
    state_save_backlog >= RUNTIME_STATE_SAVE_QUEUE_PRESSURE_THRESHOLD
        || continuation_journal_backlog >= RUNTIME_CONTINUATION_JOURNAL_QUEUE_PRESSURE_THRESHOLD
        || probe_refresh_backlog >= RUNTIME_PROBE_REFRESH_QUEUE_PRESSURE_THRESHOLD
}

fn schedule_runtime_continuation_journal_save(
    shared: &RuntimeRotationProxyShared,
    continuations: RuntimeContinuationStore,
    paths: AppPaths,
    reason: &str,
) {
    if !runtime_proxy_persistence_enabled(shared) {
        runtime_proxy_log(
            shared,
            format!(
                "continuation_journal_save_suppressed role=follower reason={reason} path={}",
                runtime_continuation_journal_file_path(&paths).display()
            ),
        );
        return;
    }
    if cfg!(test) {
        runtime_proxy_log(
            shared,
            format!("continuation_journal_save_inline reason={reason} backlog=0"),
        );
        let saved_at = Local::now().timestamp();
        match save_runtime_continuation_journal(&paths, &continuations, saved_at) {
            Ok(()) => runtime_proxy_log(
                shared,
                format!(
                    "continuation_journal_save_ok saved_at={} reason={} lag_ms=0",
                    saved_at, reason
                ),
            ),
            Err(err) => runtime_proxy_log(
                shared,
                format!(
                    "continuation_journal_save_error saved_at={} reason={} lag_ms=0 stage=write error={err:#}",
                    saved_at, reason
                ),
            ),
        }
        return;
    }
    let queue = runtime_continuation_journal_save_queue();
    let journal_path = runtime_continuation_journal_file_path(&paths);
    let mut pending = queue
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    pending.insert(
        journal_path,
        RuntimeContinuationJournalSaveJob {
            paths,
            continuations,
            log_path: shared.log_path.clone(),
            reason: reason.to_string(),
            saved_at: Local::now().timestamp(),
            queued_at: Instant::now(),
        },
    );
    let backlog = pending.len().saturating_sub(1);
    drop(pending);
    queue.wake.notify_one();
    runtime_proxy_log(
        shared,
        format!(
            "continuation_journal_save_queued reason={} backlog={}",
            reason, backlog
        ),
    );
    if runtime_proxy_queue_pressure_active(0, backlog, 0) {
        runtime_proxy_log(
            shared,
            format!(
                "continuation_journal_queue_backpressure reason={} backlog={backlog}",
                reason
            ),
        );
    }
}

fn runtime_state_save_worker_loop(queue: Arc<RuntimeStateSaveQueue>) {
    loop {
        let jobs = {
            let mut pending = queue
                .pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            while pending.is_empty() {
                pending = queue
                    .wake
                    .wait(pending)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
            loop {
                let now = Instant::now();
                let next_ready_at = pending.values().map(|job| job.ready_at).min();
                let Some(next_ready_at) = next_ready_at else {
                    break BTreeMap::new();
                };
                if next_ready_at <= now {
                    let due_keys = pending
                        .iter()
                        .filter_map(|(key, job)| (job.ready_at <= now).then_some(key.clone()))
                        .collect::<Vec<_>>();
                    let mut due = BTreeMap::new();
                    for key in due_keys {
                        if let Some(job) = pending.remove(&key) {
                            due.insert(key, job);
                        }
                    }
                    break due;
                }
                let wait_for = next_ready_at.saturating_duration_since(now);
                let (next_pending, _) = queue
                    .wake
                    .wait_timeout(pending, wait_for)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                pending = next_pending;
            }
        };

        for (_, job) in jobs {
            queue.active.fetch_add(1, Ordering::SeqCst);
            match save_runtime_state_snapshot_if_latest(
                &job.paths,
                &job.state,
                &job.continuations,
                &job.profile_scores,
                &job.usage_snapshots,
                &job.backoffs,
                job.revision,
                &job.latest_revision,
            ) {
                Ok(true) => runtime_proxy_log_to_path(
                    &job.log_path,
                    &format!(
                        "state_save_ok revision={} reason={} lag_ms={}",
                        job.revision,
                        job.reason,
                        job.queued_at.elapsed().as_millis()
                    ),
                ),
                Ok(false) => runtime_proxy_log_to_path(
                    &job.log_path,
                    &format!(
                        "state_save_skipped revision={} reason={} lag_ms={}",
                        job.revision,
                        job.reason,
                        job.queued_at.elapsed().as_millis()
                    ),
                ),
                Err(err) => runtime_proxy_log_to_path(
                    &job.log_path,
                    &format!(
                        "state_save_error revision={} reason={} lag_ms={} stage=write error={err:#}",
                        job.revision,
                        job.reason,
                        job.queued_at.elapsed().as_millis()
                    ),
                ),
            }
            queue.active.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

fn runtime_continuation_journal_save_worker_loop(queue: Arc<RuntimeContinuationJournalSaveQueue>) {
    loop {
        let jobs = {
            let mut pending = queue
                .pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            while pending.is_empty() {
                pending = queue
                    .wake
                    .wait(pending)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
            std::mem::take(&mut *pending)
        };

        for (_, job) in jobs {
            queue.active.fetch_add(1, Ordering::SeqCst);
            match save_runtime_continuation_journal(&job.paths, &job.continuations, job.saved_at) {
                Ok(()) => runtime_proxy_log_to_path(
                    &job.log_path,
                    &format!(
                        "continuation_journal_save_ok saved_at={} reason={} lag_ms={}",
                        job.saved_at,
                        job.reason,
                        job.queued_at.elapsed().as_millis()
                    ),
                ),
                Err(err) => runtime_proxy_log_to_path(
                    &job.log_path,
                    &format!(
                        "continuation_journal_save_error saved_at={} reason={} lag_ms={} stage=write error={err:#}",
                        job.saved_at,
                        job.reason,
                        job.queued_at.elapsed().as_millis()
                    ),
                ),
            }
            queue.active.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

fn runtime_state_save_reason_requires_continuation_journal(reason: &str) -> bool {
    [
        "response_ids:",
        "response_touch:",
        "turn_state:",
        "turn_state_touch:",
        "session_id:",
        "session_touch:",
        "compact_lineage:",
        "compact_lineage_release:",
    ]
    .into_iter()
    .any(|prefix| reason.starts_with(prefix))
}

fn runtime_state_save_debounce(reason: &str) -> Duration {
    if reason.starts_with("session_id:") {
        Duration::from_millis(RUNTIME_STATE_SAVE_DEBOUNCE_MS)
    } else {
        Duration::ZERO
    }
}

fn schedule_runtime_probe_refresh(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    codex_home: &Path,
) {
    let (state_file, upstream_base_url) = match shared.runtime.lock() {
        Ok(runtime) => (
            runtime.paths.state_file.clone(),
            runtime.upstream_base_url.clone(),
        ),
        Err(_) => return,
    };
    let queue = runtime_probe_refresh_queue();
    let mut pending = queue
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let reason = if pending.contains_key(&(state_file.clone(), profile_name.to_string())) {
        "deduped"
    } else {
        "queued"
    };
    let queued_at = Instant::now();
    pending.insert(
        (state_file.clone(), profile_name.to_string()),
        RuntimeProbeRefreshJob {
            shared: shared.clone(),
            profile_name: profile_name.to_string(),
            codex_home: codex_home.to_path_buf(),
            upstream_base_url,
            queued_at,
        },
    );
    let backlog = pending.len().saturating_sub(1);
    drop(pending);
    queue.wake.notify_one();
    runtime_proxy_log(
        shared,
        format!(
            "profile_probe_refresh_queued profile={profile_name} reason={reason} backlog={backlog}"
        ),
    );
    if runtime_proxy_queue_pressure_active(0, 0, backlog) {
        runtime_proxy_log(
            shared,
            format!("profile_probe_refresh_backpressure profile={profile_name} backlog={backlog}"),
        );
    }
}

fn runtime_profiles_needing_startup_probe_refresh(
    state: &AppState,
    current_profile: &str,
    profile_probe_cache: &BTreeMap<String, RuntimeProfileProbeCacheEntry>,
    profile_usage_snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    now: i64,
) -> Vec<String> {
    if profile_usage_snapshots.is_empty() {
        return Vec::new();
    }

    active_profile_selection_order(state, current_profile)
        .into_iter()
        .filter(|profile_name| {
            let probe_fresh = profile_probe_cache.get(profile_name).is_some_and(|entry| {
                runtime_profile_probe_cache_freshness(entry, now)
                    == RuntimeProbeCacheFreshness::Fresh
            });
            let snapshot_usable = profile_usage_snapshots
                .get(profile_name)
                .is_some_and(|snapshot| runtime_usage_snapshot_is_usable(snapshot, now));
            !probe_fresh && !snapshot_usable
        })
        .take(RUNTIME_STARTUP_PROBE_WARM_LIMIT)
        .collect()
}

fn schedule_runtime_startup_probe_warmup(shared: &RuntimeRotationProxyShared) {
    let (state, current_profile, profile_probe_cache, profile_usage_snapshots) =
        match shared.runtime.lock() {
            Ok(runtime) => (
                runtime.state.clone(),
                runtime.current_profile.clone(),
                runtime.profile_probe_cache.clone(),
                runtime.profile_usage_snapshots.clone(),
            ),
            Err(_) => return,
        };
    let refresh_profiles = runtime_profiles_needing_startup_probe_refresh(
        &state,
        &current_profile,
        &profile_probe_cache,
        &profile_usage_snapshots,
        Local::now().timestamp(),
    );
    if refresh_profiles.is_empty() {
        return;
    }

    let refresh_jobs = refresh_profiles
        .into_iter()
        .filter_map(|profile_name| {
            let profile = state.profiles.get(&profile_name)?;
            read_auth_summary(&profile.codex_home)
                .quota_compatible
                .then(|| (profile_name, profile.codex_home.clone()))
        })
        .collect::<Vec<_>>();
    if refresh_jobs.is_empty() {
        return;
    }

    runtime_proxy_log(
        shared,
        format!(
            "startup_probe_warmup queued={} profiles={}",
            refresh_jobs.len(),
            refresh_jobs
                .iter()
                .map(|(profile_name, _)| profile_name.as_str())
                .collect::<Vec<_>>()
                .join(",")
        ),
    );
    for (profile_name, codex_home) in refresh_jobs {
        schedule_runtime_probe_refresh(shared, &profile_name, &codex_home);
    }
}

fn apply_runtime_profile_probe_result(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    auth: AuthSummary,
    result: std::result::Result<UsageResponse, String>,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    runtime.profile_probe_cache.insert(
        profile_name.to_string(),
        RuntimeProfileProbeCacheEntry {
            checked_at: now,
            auth,
            result: result.clone(),
        },
    );

    let Ok(usage) = result else {
        return Ok(());
    };

    let snapshot = runtime_profile_usage_snapshot_from_usage(&usage);
    let quota_summary =
        runtime_quota_summary_from_usage_snapshot(&snapshot, RuntimeRouteKind::Responses);
    runtime
        .profile_usage_snapshots
        .insert(profile_name.to_string(), snapshot);
    if quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted {
        runtime
            .state
            .response_profile_bindings
            .retain(|_, binding| binding.profile_name != profile_name);
        runtime
            .state
            .session_profile_bindings
            .retain(|_, binding| binding.profile_name != profile_name);
        runtime.turn_state_bindings.retain(|key, binding| {
            binding.profile_name != profile_name || runtime_is_compact_turn_state_lineage_key(key)
        });
        runtime.session_id_bindings.retain(|key, binding| {
            binding.profile_name != profile_name || runtime_is_compact_session_lineage_key(key)
        });
        runtime_proxy_log(
            shared,
            format!(
                "quota_release_profile_affinity profile={profile_name} reason=usage_snapshot_exhausted {}",
                runtime_quota_summary_log_fields(quota_summary)
            ),
        );
    }
    let state_snapshot = runtime.state.clone();
    let continuations_snapshot = runtime_continuation_store_snapshot(&runtime);
    let profile_scores_snapshot = runtime.profile_health.clone();
    let usage_snapshots = runtime.profile_usage_snapshots.clone();
    let backoffs_snapshot = runtime_profile_backoffs_snapshot(&runtime);
    let paths_snapshot = runtime.paths.clone();
    drop(runtime);
    schedule_runtime_state_save(
        shared,
        state_snapshot,
        continuations_snapshot,
        profile_scores_snapshot,
        usage_snapshots,
        backoffs_snapshot,
        paths_snapshot,
        &format!("usage_snapshot:{profile_name}"),
    );
    Ok(())
}

fn runtime_probe_refresh_worker_loop(queue: Arc<RuntimeProbeRefreshQueue>) {
    loop {
        let jobs = {
            let mut pending = queue
                .pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            while pending.is_empty() {
                pending = queue
                    .wake
                    .wait(pending)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
            std::mem::take(&mut *pending)
        };

        for (_, job) in jobs {
            queue.active.fetch_add(1, Ordering::SeqCst);
            runtime_proxy_log(
                &job.shared,
                format!("profile_probe_refresh_start profile={}", job.profile_name),
            );
            let auth = read_auth_summary(&job.codex_home);
            let result = if auth.quota_compatible {
                fetch_usage(&job.codex_home, Some(job.upstream_base_url.as_str()))
                    .map_err(|err| err.to_string())
            } else {
                Err("auth mode is not quota-compatible".to_string())
            };
            let apply_result = apply_runtime_profile_probe_result(
                &job.shared,
                &job.profile_name,
                auth,
                result.clone(),
            );
            match result {
                Ok(_) => runtime_proxy_log(
                    &job.shared,
                    if let Err(err) = apply_result {
                        format!(
                            "profile_probe_refresh_error profile={} lag_ms={} error=state_update:{err:#}",
                            job.profile_name,
                            job.queued_at.elapsed().as_millis()
                        )
                    } else {
                        format!(
                            "profile_probe_refresh_ok profile={} lag_ms={}",
                            job.profile_name,
                            job.queued_at.elapsed().as_millis()
                        )
                    },
                ),
                Err(err) => runtime_proxy_log(
                    &job.shared,
                    format!(
                        "profile_probe_refresh_error profile={} lag_ms={} error={err}",
                        job.profile_name,
                        job.queued_at.elapsed().as_millis()
                    ),
                ),
            }
            queue.active.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

fn acquire_state_file_lock(paths: &AppPaths) -> Result<StateFileLock> {
    fs::create_dir_all(&paths.root)
        .with_context(|| format!("failed to create {}", paths.root.display()))?;
    let lock_path = state_lock_file_path(&paths.state_file);
    let file = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .with_context(|| format!("failed to open {}", lock_path.display()))?;
    file.lock_exclusive()
        .with_context(|| format!("failed to lock {}", lock_path.display()))?;
    Ok(StateFileLock { file })
}

fn try_acquire_runtime_owner_lock(paths: &AppPaths) -> Result<Option<StateFileLock>> {
    fs::create_dir_all(&paths.root)
        .with_context(|| format!("failed to create {}", paths.root.display()))?;
    let lock_path = runtime_owner_lock_file_path(paths);
    let file = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .with_context(|| format!("failed to open {}", lock_path.display()))?;
    match file.try_lock_exclusive() {
        Ok(()) => Ok(Some(StateFileLock { file })),
        Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(None),
        Err(err) => Err(err).with_context(|| format!("failed to lock {}", lock_path.display())),
    }
}

fn state_lock_file_path(state_file: &Path) -> PathBuf {
    json_lock_file_path(state_file)
}

fn runtime_owner_lock_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-owner.lock")
}

fn json_lock_file_path(path: &Path) -> PathBuf {
    path.with_extension("json.lock")
}

fn acquire_json_file_lock(path: &Path) -> Result<JsonFileLock> {
    let lock_path = json_lock_file_path(path);
    let file = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .with_context(|| format!("failed to open {}", lock_path.display()))?;
    file.lock_exclusive()
        .with_context(|| format!("failed to lock {}", lock_path.display()))?;
    Ok(JsonFileLock { file })
}

fn runtime_sidecar_generation_cache() -> &'static Mutex<BTreeMap<PathBuf, u64>> {
    RUNTIME_SIDECAR_GENERATION_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn runtime_sidecar_cached_generation(path: &Path) -> Option<u64> {
    runtime_sidecar_generation_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(path)
        .copied()
}

fn remember_runtime_sidecar_generation(path: &Path, generation: u64) {
    runtime_sidecar_generation_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(path.to_path_buf(), generation);
}

fn forget_runtime_sidecar_generation(path: &Path) {
    runtime_sidecar_generation_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(path);
}

#[derive(Debug)]
struct JsonFileLock {
    file: fs::File,
}

impl Drop for JsonFileLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

fn merge_last_run_selection(
    existing: &BTreeMap<String, i64>,
    incoming: &BTreeMap<String, i64>,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> BTreeMap<String, i64> {
    let mut merged = existing.clone();
    for (profile_name, timestamp) in incoming {
        merged
            .entry(profile_name.clone())
            .and_modify(|current| *current = (*current).max(*timestamp))
            .or_insert(*timestamp);
    }
    merged.retain(|profile_name, _| profiles.contains_key(profile_name));
    merged
}

fn prune_last_run_selection(
    selections: &mut BTreeMap<String, i64>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) {
    let oldest_allowed = now.saturating_sub(APP_STATE_LAST_RUN_RETENTION_SECONDS);
    selections.retain(|profile_name, timestamp| {
        profiles.contains_key(profile_name) && *timestamp >= oldest_allowed
    });
}

fn merge_profile_bindings(
    existing: &BTreeMap<String, ResponseProfileBinding>,
    incoming: &BTreeMap<String, ResponseProfileBinding>,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> BTreeMap<String, ResponseProfileBinding> {
    let mut merged = existing.clone();
    for (response_id, binding) in incoming {
        let should_replace = merged
            .get(response_id)
            .is_none_or(|current| current.bound_at <= binding.bound_at);
        if should_replace {
            merged.insert(response_id.clone(), binding.clone());
        }
    }
    merged.retain(|_, binding| profiles.contains_key(&binding.profile_name));
    merged
}

fn runtime_continuation_binding_lifecycle_rank(state: RuntimeContinuationBindingLifecycle) -> u8 {
    match state {
        RuntimeContinuationBindingLifecycle::Dead => 0,
        RuntimeContinuationBindingLifecycle::Suspect => 1,
        RuntimeContinuationBindingLifecycle::Warm => 2,
        RuntimeContinuationBindingLifecycle::Verified => 3,
    }
}

fn runtime_continuation_status_evidence_sort_key(
    status: &RuntimeContinuationBindingStatus,
) -> (u8, u32, u32, u32, u8, i64, i64, i64) {
    (
        runtime_continuation_binding_lifecycle_rank(status.state),
        status.confidence.min(RUNTIME_CONTINUATION_CONFIDENCE_MAX),
        status.success_count,
        u32::MAX.saturating_sub(status.not_found_streak),
        if status.last_verified_route.is_some() {
            1
        } else {
            0
        },
        status.last_verified_at.unwrap_or(i64::MIN),
        status.last_touched_at.unwrap_or(i64::MIN),
        status.last_not_found_at.unwrap_or(i64::MIN),
    )
}

fn runtime_continuation_status_is_more_evidenced(
    candidate: &RuntimeContinuationBindingStatus,
    current: &RuntimeContinuationBindingStatus,
) -> bool {
    runtime_continuation_status_evidence_sort_key(candidate)
        > runtime_continuation_status_evidence_sort_key(current)
}

fn runtime_continuation_status_should_replace(
    candidate: &RuntimeContinuationBindingStatus,
    current: &RuntimeContinuationBindingStatus,
) -> bool {
    match (
        runtime_continuation_status_last_event_at(candidate),
        runtime_continuation_status_last_event_at(current),
    ) {
        (Some(candidate_at), Some(current_at)) if candidate_at != current_at => {
            return candidate_at > current_at;
        }
        (Some(_), None) => return true,
        (None, Some(_)) => return false,
        _ => {}
    }

    runtime_continuation_status_is_more_evidenced(candidate, current)
}

fn runtime_continuation_status_last_event_at(
    status: &RuntimeContinuationBindingStatus,
) -> Option<i64> {
    [
        status.last_not_found_at,
        status.last_verified_at,
        status.last_touched_at,
    ]
    .into_iter()
    .flatten()
    .max()
}

fn runtime_continuation_status_is_terminal(status: &RuntimeContinuationBindingStatus) -> bool {
    status.state == RuntimeContinuationBindingLifecycle::Dead
        || status.not_found_streak >= RUNTIME_CONTINUATION_SUSPECT_NOT_FOUND_STREAK_LIMIT
        || (status.state == RuntimeContinuationBindingLifecycle::Suspect
            && status.confidence == 0
            && status.failure_count > 0)
}

fn runtime_continuation_status_should_retain_with_binding(
    status: &RuntimeContinuationBindingStatus,
    now: i64,
) -> bool {
    match status.state {
        RuntimeContinuationBindingLifecycle::Dead => false,
        RuntimeContinuationBindingLifecycle::Verified
        | RuntimeContinuationBindingLifecycle::Warm => {
            status.confidence > 0
                || status.success_count > 0
                || status.last_verified_at.is_some()
                || status.last_touched_at.is_some()
        }
        RuntimeContinuationBindingLifecycle::Suspect => {
            status.not_found_streak < RUNTIME_CONTINUATION_SUSPECT_NOT_FOUND_STREAK_LIMIT
                && status.confidence > 0
                && status.last_not_found_at.is_some_and(|last| {
                    now.saturating_sub(last) < RUNTIME_CONTINUATION_SUSPECT_GRACE_SECONDS
                })
        }
    }
}

fn runtime_continuation_status_should_retain_without_binding(
    status: &RuntimeContinuationBindingStatus,
    now: i64,
) -> bool {
    match status.state {
        RuntimeContinuationBindingLifecycle::Dead => status
            .last_not_found_at
            .or(status.last_touched_at)
            .is_some_and(|last| now.saturating_sub(last) < RUNTIME_CONTINUATION_DEAD_GRACE_SECONDS),
        _ => runtime_continuation_status_should_retain_with_binding(status, now),
    }
}

fn runtime_continuation_status_dead_at(status: &RuntimeContinuationBindingStatus) -> Option<i64> {
    (status.state == RuntimeContinuationBindingLifecycle::Dead)
        .then(|| status.last_not_found_at.or(status.last_touched_at))
        .flatten()
}

fn runtime_continuation_dead_status_shadowed_by_binding(
    binding: &ResponseProfileBinding,
    status: &RuntimeContinuationBindingStatus,
) -> bool {
    runtime_continuation_status_dead_at(status).is_some_and(|dead_at| binding.bound_at > dead_at)
}

fn merge_runtime_continuation_status_map(
    existing: &BTreeMap<String, RuntimeContinuationBindingStatus>,
    incoming: &BTreeMap<String, RuntimeContinuationBindingStatus>,
    live_bindings: &BTreeMap<String, ResponseProfileBinding>,
) -> BTreeMap<String, RuntimeContinuationBindingStatus> {
    let now = Local::now().timestamp();
    let mut merged = existing.clone();
    for (key, status) in incoming {
        let should_replace = merged
            .get(key)
            .is_none_or(|current| runtime_continuation_status_should_replace(status, current));
        if should_replace {
            merged.insert(key.clone(), status.clone());
        }
    }
    merged.retain(|key, status| {
        live_bindings.contains_key(key)
            || runtime_continuation_status_should_retain_without_binding(status, now)
    });
    merged
}

fn merge_runtime_continuation_statuses(
    existing: &RuntimeContinuationStatuses,
    incoming: &RuntimeContinuationStatuses,
    response_bindings: &BTreeMap<String, ResponseProfileBinding>,
    turn_state_bindings: &BTreeMap<String, ResponseProfileBinding>,
    session_id_bindings: &BTreeMap<String, ResponseProfileBinding>,
) -> RuntimeContinuationStatuses {
    RuntimeContinuationStatuses {
        response: merge_runtime_continuation_status_map(
            &existing.response,
            &incoming.response,
            response_bindings,
        ),
        turn_state: merge_runtime_continuation_status_map(
            &existing.turn_state,
            &incoming.turn_state,
            turn_state_bindings,
        ),
        session_id: merge_runtime_continuation_status_map(
            &existing.session_id,
            &incoming.session_id,
            session_id_bindings,
        ),
    }
}

fn compact_runtime_continuation_statuses(
    statuses: RuntimeContinuationStatuses,
    continuations: &RuntimeContinuationStore,
) -> RuntimeContinuationStatuses {
    let now = Local::now().timestamp();
    let mut merged = merge_runtime_continuation_statuses(
        &RuntimeContinuationStatuses::default(),
        &statuses,
        &continuations.response_profile_bindings,
        &continuations.turn_state_bindings,
        &continuations.session_id_bindings,
    );
    merged.response.retain(|key, status| {
        if let Some(binding) = continuations.response_profile_bindings.get(key) {
            !runtime_continuation_dead_status_shadowed_by_binding(binding, status)
                && runtime_continuation_status_should_retain_with_binding(status, now)
        } else {
            runtime_continuation_status_should_retain_without_binding(status, now)
        }
    });
    merged.turn_state.retain(|key, status| {
        if let Some(binding) = continuations.turn_state_bindings.get(key) {
            !runtime_continuation_dead_status_shadowed_by_binding(binding, status)
                && runtime_continuation_status_should_retain_with_binding(status, now)
        } else {
            runtime_continuation_status_should_retain_without_binding(status, now)
        }
    });
    merged.session_id.retain(|key, status| {
        if let Some(binding) = continuations.session_id_bindings.get(key) {
            !runtime_continuation_dead_status_shadowed_by_binding(binding, status)
                && runtime_continuation_status_should_retain_with_binding(status, now)
        } else {
            runtime_continuation_status_should_retain_without_binding(status, now)
        }
    });
    merged
}

fn runtime_continuation_binding_should_retain(
    binding: &ResponseProfileBinding,
    status: Option<&RuntimeContinuationBindingStatus>,
    now: i64,
) -> bool {
    match status {
        Some(status) if runtime_continuation_dead_status_shadowed_by_binding(binding, status) => {
            true
        }
        Some(status) if runtime_continuation_status_is_terminal(status) => false,
        Some(status) => runtime_continuation_status_should_retain_with_binding(status, now),
        None => binding.bound_at <= now,
    }
}

fn prune_profile_bindings_for_housekeeping(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
    retention_seconds: i64,
    max_entries: usize,
) {
    let oldest_allowed = now.saturating_sub(retention_seconds);
    bindings.retain(|_, binding| {
        profiles.contains_key(&binding.profile_name) && binding.bound_at >= oldest_allowed
    });
    prune_profile_bindings(bindings, max_entries);
}

fn prune_profile_bindings_for_housekeeping_without_retention(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
    profiles: &BTreeMap<String, ProfileEntry>,
) {
    bindings.retain(|_, binding| profiles.contains_key(&binding.profile_name));
}

fn compact_app_state(mut state: AppState, now: i64) -> AppState {
    state.active_profile = state
        .active_profile
        .filter(|profile_name| state.profiles.contains_key(profile_name));
    prune_last_run_selection(&mut state.last_run_selected_at, &state.profiles, now);
    prune_profile_bindings_for_housekeeping_without_retention(
        &mut state.response_profile_bindings,
        &state.profiles,
    );
    prune_profile_bindings_for_housekeeping(
        &mut state.session_profile_bindings,
        &state.profiles,
        now,
        APP_STATE_SESSION_BINDING_RETENTION_SECONDS,
        SESSION_ID_PROFILE_BINDING_LIMIT,
    );
    state
}

fn merge_runtime_state_snapshot(existing: AppState, snapshot: &AppState) -> AppState {
    let profiles = if existing.profiles.is_empty() {
        snapshot.profiles.clone()
    } else {
        existing.profiles.clone()
    };
    let active_profile = snapshot
        .active_profile
        .clone()
        .or(existing.active_profile.clone())
        .filter(|profile_name| profiles.contains_key(profile_name));

    let merged = AppState {
        active_profile,
        profiles: profiles.clone(),
        last_run_selected_at: merge_last_run_selection(
            &existing.last_run_selected_at,
            &snapshot.last_run_selected_at,
            &profiles,
        ),
        response_profile_bindings: merge_profile_bindings(
            &existing.response_profile_bindings,
            &snapshot.response_profile_bindings,
            &profiles,
        ),
        session_profile_bindings: merge_profile_bindings(
            &existing.session_profile_bindings,
            &snapshot.session_profile_bindings,
            &profiles,
        ),
    };
    compact_app_state(merged, Local::now().timestamp())
}

fn merge_app_state_for_save(existing: AppState, desired: &AppState) -> AppState {
    let active_profile = desired
        .active_profile
        .clone()
        .filter(|profile_name| desired.profiles.contains_key(profile_name));
    let merged = AppState {
        active_profile,
        profiles: desired.profiles.clone(),
        last_run_selected_at: merge_last_run_selection(
            &existing.last_run_selected_at,
            &desired.last_run_selected_at,
            &desired.profiles,
        ),
        response_profile_bindings: merge_profile_bindings(
            &existing.response_profile_bindings,
            &desired.response_profile_bindings,
            &desired.profiles,
        ),
        session_profile_bindings: merge_profile_bindings(
            &existing.session_profile_bindings,
            &desired.session_profile_bindings,
            &desired.profiles,
        ),
    };
    compact_app_state(merged, Local::now().timestamp())
}

fn write_state_json_atomic(paths: &AppPaths, json: &str) -> Result<()> {
    if runtime_take_fault_injection("PRODEX_RUNTIME_FAULT_STATE_SAVE_ERROR_ONCE") {
        bail!("injected runtime state save failure");
    }
    write_json_file_with_backup(
        &paths.state_file,
        &state_last_good_file_path(paths),
        json,
        |content| {
            let _: AppState =
                serde_json::from_str(content).context("failed to validate prodex state")?;
            Ok(())
        },
    )
}

fn runtime_scores_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-scores.json")
}

fn state_last_good_file_path(paths: &AppPaths) -> PathBuf {
    last_good_file_path(&paths.state_file)
}

fn runtime_usage_snapshots_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-usage-snapshots.json")
}

fn runtime_scores_last_good_file_path(paths: &AppPaths) -> PathBuf {
    last_good_file_path(&runtime_scores_file_path(paths))
}

fn runtime_usage_snapshots_last_good_file_path(paths: &AppPaths) -> PathBuf {
    last_good_file_path(&runtime_usage_snapshots_file_path(paths))
}

fn runtime_backoffs_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-backoffs.json")
}

fn runtime_backoffs_last_good_file_path(paths: &AppPaths) -> PathBuf {
    last_good_file_path(&runtime_backoffs_file_path(paths))
}

fn runtime_continuations_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-continuations.json")
}

fn runtime_continuations_last_good_file_path(paths: &AppPaths) -> PathBuf {
    last_good_file_path(&runtime_continuations_file_path(paths))
}

fn runtime_continuation_journal_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-continuation-journal.json")
}

fn runtime_continuation_journal_last_good_file_path(paths: &AppPaths) -> PathBuf {
    last_good_file_path(&runtime_continuation_journal_file_path(paths))
}

fn runtime_broker_registry_file_path(paths: &AppPaths, broker_key: &str) -> PathBuf {
    paths.root.join(format!("runtime-broker-{broker_key}.json"))
}

fn runtime_broker_registry_last_good_file_path(paths: &AppPaths, broker_key: &str) -> PathBuf {
    last_good_file_path(&runtime_broker_registry_file_path(paths, broker_key))
}

fn runtime_broker_lease_dir(paths: &AppPaths, broker_key: &str) -> PathBuf {
    paths
        .root
        .join(format!("runtime-broker-{broker_key}-leases"))
}

fn runtime_broker_ensure_lock_path(paths: &AppPaths, broker_key: &str) -> PathBuf {
    paths
        .root
        .join(format!("runtime-broker-{broker_key}-ensure"))
}

fn last_good_file_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("snapshot.json");
    path.with_file_name(format!("{file_name}{LAST_GOOD_FILE_SUFFIX}"))
}

fn runtime_sidecar_generation_from_content(content: &str) -> Result<u64> {
    let value: serde_json::Value =
        serde_json::from_str(content).context("failed to parse runtime sidecar json")?;
    Ok(value
        .get("generation")
        .and_then(|value| value.as_u64())
        .unwrap_or(0))
}

fn runtime_sidecar_generation_from_disk(path: &Path, backup_path: &Path) -> Result<u64> {
    match fs::read_to_string(path) {
        Ok(content) => runtime_sidecar_generation_from_content(&content).or_else(|primary_err| {
            match fs::read_to_string(backup_path) {
                Ok(backup_content) => runtime_sidecar_generation_from_content(&backup_content)
                    .with_context(|| {
                        format!(
                            "failed to parse {} after primary load error: {primary_err:#}",
                            backup_path.display()
                        )
                    }),
                Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(0),
                Err(err) => {
                    Err(err).with_context(|| format!("failed to read {}", backup_path.display()))
                }
            }
        }),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            match fs::read_to_string(backup_path) {
                Ok(backup_content) => runtime_sidecar_generation_from_content(&backup_content)
                    .with_context(|| format!("failed to parse {}", backup_path.display())),
                Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(0),
                Err(err) => {
                    Err(err).with_context(|| format!("failed to read {}", backup_path.display()))
                }
            }
        }
        Err(err) => Err(err).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn parse_versioned_json_or_raw<T>(content: &str) -> Result<(T, u64)>
where
    T: for<'de> Deserialize<'de>,
{
    match serde_json::from_str::<VersionedJson<T>>(content) {
        Ok(versioned) => Ok((versioned.value, versioned.generation)),
        Err(_) => Ok((serde_json::from_str::<T>(content)?, 0)),
    }
}

fn read_versioned_json_file_with_backup<T>(
    path: &Path,
    backup_path: &Path,
) -> Result<RecoveredVersionedLoad<T>>
where
    T: for<'de> Deserialize<'de>,
{
    let primary =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()));
    match primary.and_then(|content| {
        parse_versioned_json_or_raw::<T>(&content)
            .with_context(|| format!("failed to parse {}", path.display()))
    }) {
        Ok((value, generation)) => Ok(RecoveredVersionedLoad {
            value,
            generation,
            recovered_from_backup: false,
        }),
        Err(primary_err) => {
            let backup_content = fs::read_to_string(backup_path)
                .with_context(|| format!("failed to read {}", backup_path.display()))?;
            let (value, generation) = parse_versioned_json_or_raw::<T>(&backup_content)
                .with_context(|| {
                    format!(
                        "failed to parse {} after primary load error: {primary_err:#}",
                        backup_path.display()
                    )
                })?;
            Ok(RecoveredVersionedLoad {
                value,
                generation,
                recovered_from_backup: true,
            })
        }
    }
}

fn write_versioned_json_file_with_backup<T>(
    path: &Path,
    backup_path: &Path,
    generation: u64,
    value: &T,
) -> Result<()>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    let json = serde_json::to_string_pretty(&VersionedJson { generation, value })
        .context("failed to serialize runtime sidecar")?;
    write_json_file_with_backup(path, backup_path, &json, |content| {
        let _: VersionedJson<T> =
            serde_json::from_str(content).context("failed to validate runtime sidecar")?;
        Ok(())
    })
}

fn save_versioned_json_file_with_fence<T>(path: &Path, backup_path: &Path, value: &T) -> Result<()>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let _lock = acquire_json_file_lock(path)?;
    let cached_generation = runtime_sidecar_cached_generation(path);
    let expected_generation = cached_generation
        .unwrap_or_else(|| runtime_sidecar_generation_from_disk(path, backup_path).unwrap_or(0));
    let current_generation = runtime_sidecar_generation_from_disk(path, backup_path)?;
    if current_generation != expected_generation {
        if current_generation == 0
            && expected_generation > 0
            && cached_generation.is_some()
            && !path.exists()
            && !backup_path.exists()
        {
            forget_runtime_sidecar_generation(path);
            return save_versioned_json_file_with_fence(path, backup_path, value);
        }
        bail!(
            "stale runtime sidecar generation for {} expected={} current={}",
            path.display(),
            expected_generation,
            current_generation
        );
    }
    let next_generation = current_generation.saturating_add(1);
    write_versioned_json_file_with_backup(path, backup_path, next_generation, value)?;
    remember_runtime_sidecar_generation(path, next_generation);
    Ok(())
}

fn runtime_sidecar_generation_error_is_stale(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        cause
            .to_string()
            .contains("stale runtime sidecar generation")
    })
}

fn write_json_file_with_backup(
    path: &Path,
    backup_path: &Path,
    json: &str,
    validate: impl Fn(&str) -> Result<()>,
) -> Result<()> {
    let temp_file = unique_state_temp_file_path(path);
    fs::write(&temp_file, json)
        .with_context(|| format!("failed to write {}", temp_file.display()))?;
    validate(json).with_context(|| format!("failed to validate staged {}", temp_file.display()))?;
    fs::rename(&temp_file, path)
        .with_context(|| format!("failed to replace {}", path.display()))?;
    let written = fs::read_to_string(path)
        .with_context(|| format!("failed to re-read {}", path.display()))?;
    validate(&written).with_context(|| format!("failed to validate {}", path.display()))?;
    fs::write(backup_path, &written)
        .with_context(|| format!("failed to refresh {}", backup_path.display()))?;
    Ok(())
}

fn load_json_file_with_backup<T>(path: &Path, backup_path: &Path) -> Result<RecoveredLoad<T>>
where
    T: for<'de> Deserialize<'de>,
{
    let primary =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()));
    match primary.and_then(|content| {
        serde_json::from_str::<T>(&content)
            .with_context(|| format!("failed to parse {}", path.display()))
    }) {
        Ok(value) => Ok(RecoveredLoad {
            value,
            recovered_from_backup: false,
        }),
        Err(primary_err) => {
            let backup_content = fs::read_to_string(backup_path)
                .with_context(|| format!("failed to read {}", backup_path.display()))?;
            let value = serde_json::from_str::<T>(&backup_content).with_context(|| {
                format!(
                    "failed to parse {} after primary load error: {primary_err:#}",
                    backup_path.display()
                )
            })?;
            Ok(RecoveredLoad {
                value,
                recovered_from_backup: true,
            })
        }
    }
}

fn update_check_cache_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("update-check.json")
}

fn runtime_profile_score_profile_name(key: &str) -> &str {
    key.rsplit(':').next().unwrap_or(key)
}

fn compact_runtime_profile_scores(
    mut scores: BTreeMap<String, RuntimeProfileHealth>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> BTreeMap<String, RuntimeProfileHealth> {
    let oldest_allowed = now.saturating_sub(RUNTIME_SCORE_RETENTION_SECONDS);
    scores.retain(|key, value| {
        profiles.contains_key(runtime_profile_score_profile_name(key))
            && value.updated_at >= oldest_allowed
    });
    scores
}

fn compact_runtime_usage_snapshots(
    mut snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> BTreeMap<String, RuntimeProfileUsageSnapshot> {
    let oldest_allowed = now.saturating_sub(RUNTIME_USAGE_SNAPSHOT_RETENTION_SECONDS);
    snapshots.retain(|profile_name, snapshot| {
        profiles.contains_key(profile_name) && snapshot.checked_at >= oldest_allowed
    });
    snapshots
}

fn compact_runtime_profile_backoffs(
    mut backoffs: RuntimeProfileBackoffs,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> RuntimeProfileBackoffs {
    backoffs
        .retry_backoff_until
        .retain(|profile_name, until| profiles.contains_key(profile_name) && *until > now);
    backoffs
        .transport_backoff_until
        .retain(|profile_name, until| profiles.contains_key(profile_name) && *until > now);
    backoffs
        .route_circuit_open_until
        .retain(|route_profile_key, _| {
            profiles.contains_key(runtime_profile_route_circuit_profile_name(
                route_profile_key,
            ))
        });
    backoffs
}

fn runtime_continuation_store_from_app_state(state: &AppState) -> RuntimeContinuationStore {
    RuntimeContinuationStore {
        response_profile_bindings: state.response_profile_bindings.clone(),
        session_profile_bindings: state.session_profile_bindings.clone(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: runtime_external_session_id_bindings(&state.session_profile_bindings),
        statuses: RuntimeContinuationStatuses::default(),
    }
}

fn compact_runtime_continuation_store(
    mut continuations: RuntimeContinuationStore,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> RuntimeContinuationStore {
    let now = Local::now().timestamp();
    let response_statuses = continuations.statuses.response.clone();
    let turn_state_statuses = continuations.statuses.turn_state.clone();
    let session_id_statuses = continuations.statuses.session_id.clone();
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
            runtime_continuation_binding_should_retain(binding, response_statuses.get(key), now)
        });
    continuations.turn_state_bindings.retain(|key, binding| {
        runtime_continuation_binding_should_retain(binding, turn_state_statuses.get(key), now)
    });
    continuations
        .session_profile_bindings
        .retain(|key, binding| {
            runtime_continuation_binding_should_retain(binding, session_id_statuses.get(key), now)
        });
    continuations.session_id_bindings.retain(|key, binding| {
        runtime_continuation_binding_should_retain(binding, session_id_statuses.get(key), now)
    });
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
    let statuses = continuations.statuses.clone();
    continuations.statuses = compact_runtime_continuation_statuses(statuses, &continuations);
    continuations
}

fn merge_runtime_continuation_store(
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

fn load_runtime_continuations_with_recovery(
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

fn save_runtime_continuations_for_profiles(
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

fn load_runtime_continuation_journal_with_recovery(
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

fn save_runtime_continuation_journal(
    paths: &AppPaths,
    continuations: &RuntimeContinuationStore,
    saved_at: i64,
) -> Result<()> {
    let profiles = AppState::load(paths)
        .map(|state| state.profiles)
        .unwrap_or_default();
    let incoming = compact_runtime_continuation_store(continuations.clone(), &profiles);
    for attempt in 0..=RUNTIME_SIDECAR_STALE_SAVE_RETRY_LIMIT {
        let existing = load_runtime_continuation_journal_with_recovery(paths, &profiles)?;
        let journal = RuntimeContinuationJournal {
            saved_at: saved_at.max(existing.value.saved_at),
            continuations: merge_runtime_continuation_store(
                &existing.value.continuations,
                &incoming,
                &profiles,
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

fn runtime_profile_route_key_parts<'a>(key: &'a str, prefix: &str) -> Option<(&'a str, &'a str)> {
    let rest = key.strip_prefix(prefix)?;
    let (route, profile_name) = rest.split_once(':')?;
    Some((route, profile_name))
}

fn merge_runtime_profile_scores(
    existing: &BTreeMap<String, RuntimeProfileHealth>,
    incoming: &BTreeMap<String, RuntimeProfileHealth>,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> BTreeMap<String, RuntimeProfileHealth> {
    let mut merged = existing.clone();
    for (key, value) in incoming {
        let should_replace = merged
            .get(key)
            .is_none_or(|current| current.updated_at <= value.updated_at);
        if should_replace {
            merged.insert(key.clone(), value.clone());
        }
    }
    compact_runtime_profile_scores(merged, profiles, Local::now().timestamp())
}

fn load_runtime_profile_scores(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<BTreeMap<String, RuntimeProfileHealth>> {
    let path = runtime_scores_file_path(paths);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let loaded = read_versioned_json_file_with_backup::<BTreeMap<String, RuntimeProfileHealth>>(
        &path,
        &runtime_scores_last_good_file_path(paths),
    )?;
    remember_runtime_sidecar_generation(&path, loaded.generation);
    let scores = loaded.value;
    Ok(compact_runtime_profile_scores(
        scores,
        profiles,
        Local::now().timestamp(),
    ))
}

fn load_runtime_profile_scores_with_recovery(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<RecoveredLoad<BTreeMap<String, RuntimeProfileHealth>>> {
    let path = runtime_scores_file_path(paths);
    if !path.exists() && !runtime_scores_last_good_file_path(paths).exists() {
        return Ok(RecoveredLoad {
            value: BTreeMap::new(),
            recovered_from_backup: false,
        });
    }
    let loaded = read_versioned_json_file_with_backup::<BTreeMap<String, RuntimeProfileHealth>>(
        &path,
        &runtime_scores_last_good_file_path(paths),
    )?;
    remember_runtime_sidecar_generation(&path, loaded.generation);
    Ok(RecoveredLoad {
        value: compact_runtime_profile_scores(loaded.value, profiles, Local::now().timestamp()),
        recovered_from_backup: loaded.recovered_from_backup,
    })
}

fn save_runtime_profile_scores(
    paths: &AppPaths,
    scores: &BTreeMap<String, RuntimeProfileHealth>,
) -> Result<()> {
    let path = runtime_scores_file_path(paths);
    let profiles = AppState::load(paths)
        .map(|state| state.profiles)
        .unwrap_or_default();
    let compacted =
        compact_runtime_profile_scores(scores.clone(), &profiles, Local::now().timestamp());
    save_versioned_json_file_with_fence(
        &path,
        &runtime_scores_last_good_file_path(paths),
        &compacted,
    )?;
    Ok(())
}

fn merge_runtime_usage_snapshots(
    existing: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    incoming: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> BTreeMap<String, RuntimeProfileUsageSnapshot> {
    let mut merged = existing.clone();
    for (profile_name, snapshot) in incoming {
        let should_replace = merged
            .get(profile_name)
            .is_none_or(|current| current.checked_at <= snapshot.checked_at);
        if should_replace {
            merged.insert(profile_name.clone(), snapshot.clone());
        }
    }
    compact_runtime_usage_snapshots(merged, profiles, Local::now().timestamp())
}

fn load_runtime_usage_snapshots(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<BTreeMap<String, RuntimeProfileUsageSnapshot>> {
    let path = runtime_usage_snapshots_file_path(paths);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let loaded = read_versioned_json_file_with_backup::<
        BTreeMap<String, RuntimeProfileUsageSnapshot>,
    >(&path, &runtime_usage_snapshots_last_good_file_path(paths))?;
    remember_runtime_sidecar_generation(&path, loaded.generation);
    let snapshots = loaded.value;
    Ok(compact_runtime_usage_snapshots(
        snapshots,
        profiles,
        Local::now().timestamp(),
    ))
}

fn load_runtime_usage_snapshots_with_recovery(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<RecoveredLoad<BTreeMap<String, RuntimeProfileUsageSnapshot>>> {
    let path = runtime_usage_snapshots_file_path(paths);
    if !path.exists() && !runtime_usage_snapshots_last_good_file_path(paths).exists() {
        return Ok(RecoveredLoad {
            value: BTreeMap::new(),
            recovered_from_backup: false,
        });
    }
    let loaded = read_versioned_json_file_with_backup::<
        BTreeMap<String, RuntimeProfileUsageSnapshot>,
    >(&path, &runtime_usage_snapshots_last_good_file_path(paths))?;
    remember_runtime_sidecar_generation(&path, loaded.generation);
    Ok(RecoveredLoad {
        value: compact_runtime_usage_snapshots(loaded.value, profiles, Local::now().timestamp()),
        recovered_from_backup: loaded.recovered_from_backup,
    })
}

fn save_runtime_usage_snapshots(
    paths: &AppPaths,
    snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
) -> Result<()> {
    let path = runtime_usage_snapshots_file_path(paths);
    let profiles = AppState::load(paths)
        .map(|state| state.profiles)
        .unwrap_or_default();
    let compacted =
        compact_runtime_usage_snapshots(snapshots.clone(), &profiles, Local::now().timestamp());
    save_versioned_json_file_with_fence(
        &path,
        &runtime_usage_snapshots_last_good_file_path(paths),
        &compacted,
    )?;
    Ok(())
}

fn merge_runtime_profile_backoffs(
    existing: &RuntimeProfileBackoffs,
    incoming: &RuntimeProfileBackoffs,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> RuntimeProfileBackoffs {
    let mut merged = existing.clone();
    for (profile_name, until) in &incoming.retry_backoff_until {
        merged
            .retry_backoff_until
            .entry(profile_name.clone())
            .and_modify(|current| *current = (*current).max(*until))
            .or_insert(*until);
    }
    for (profile_name, until) in &incoming.transport_backoff_until {
        merged
            .transport_backoff_until
            .entry(profile_name.clone())
            .and_modify(|current| *current = (*current).max(*until))
            .or_insert(*until);
    }
    for (route_profile_key, until) in &incoming.route_circuit_open_until {
        merged
            .route_circuit_open_until
            .entry(route_profile_key.clone())
            .and_modify(|current| *current = (*current).max(*until))
            .or_insert(*until);
    }
    merged
        .retry_backoff_until
        .retain(|profile_name, until| profiles.contains_key(profile_name) && *until > now);
    merged
        .transport_backoff_until
        .retain(|profile_name, until| profiles.contains_key(profile_name) && *until > now);
    merged
        .route_circuit_open_until
        .retain(|route_profile_key, _| {
            profiles.contains_key(runtime_profile_route_circuit_profile_name(
                route_profile_key,
            ))
        });
    compact_runtime_profile_backoffs(merged, profiles, now)
}

fn load_runtime_profile_backoffs(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<RuntimeProfileBackoffs> {
    let path = runtime_backoffs_file_path(paths);
    if !path.exists() {
        return Ok(RuntimeProfileBackoffs::default());
    }
    let loaded = read_versioned_json_file_with_backup::<RuntimeProfileBackoffs>(
        &path,
        &runtime_backoffs_last_good_file_path(paths),
    )?;
    remember_runtime_sidecar_generation(&path, loaded.generation);
    let backoffs = loaded.value;
    Ok(compact_runtime_profile_backoffs(
        backoffs,
        profiles,
        Local::now().timestamp(),
    ))
}

fn load_runtime_profile_backoffs_with_recovery(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<RecoveredLoad<RuntimeProfileBackoffs>> {
    let path = runtime_backoffs_file_path(paths);
    if !path.exists() && !runtime_backoffs_last_good_file_path(paths).exists() {
        return Ok(RecoveredLoad {
            value: RuntimeProfileBackoffs::default(),
            recovered_from_backup: false,
        });
    }
    let loaded = read_versioned_json_file_with_backup::<RuntimeProfileBackoffs>(
        &path,
        &runtime_backoffs_last_good_file_path(paths),
    )?;
    remember_runtime_sidecar_generation(&path, loaded.generation);
    Ok(RecoveredLoad {
        value: compact_runtime_profile_backoffs(loaded.value, profiles, Local::now().timestamp()),
        recovered_from_backup: loaded.recovered_from_backup,
    })
}

fn save_runtime_profile_backoffs(
    paths: &AppPaths,
    backoffs: &RuntimeProfileBackoffs,
) -> Result<()> {
    let path = runtime_backoffs_file_path(paths);
    let profiles = AppState::load(paths)
        .map(|state| state.profiles)
        .unwrap_or_default();
    let compacted =
        compact_runtime_profile_backoffs(backoffs.clone(), &profiles, Local::now().timestamp());
    save_versioned_json_file_with_fence(
        &path,
        &runtime_backoffs_last_good_file_path(paths),
        &compacted,
    )?;
    Ok(())
}

fn save_runtime_state_snapshot_if_latest(
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
        if latest_revision.load(Ordering::SeqCst) != revision {
            return Ok(false);
        }

        let _lock = acquire_state_file_lock(paths)?;

        if latest_revision.load(Ordering::SeqCst) != revision {
            return Ok(false);
        }

        let existing = AppState::load(paths)?;
        let merged = merge_runtime_state_snapshot(existing, snapshot);
        let existing_continuations =
            load_runtime_continuations_with_recovery(paths, &merged.profiles)?;
        let merged_continuations = merge_runtime_continuation_store(
            &existing_continuations.value,
            continuations,
            &merged.profiles,
        );
        let mut merged = merged;
        merged.response_profile_bindings = merged_continuations.response_profile_bindings.clone();
        merged.session_profile_bindings = merged_continuations.session_profile_bindings.clone();
        let json =
            serde_json::to_string_pretty(&merged).context("failed to serialize prodex state")?;
        let existing_scores = load_runtime_profile_scores(paths, &merged.profiles)?;
        let merged_scores =
            merge_runtime_profile_scores(&existing_scores, profile_scores, &merged.profiles);
        let existing_usage_snapshots = load_runtime_usage_snapshots(paths, &merged.profiles)?;
        let merged_usage_snapshots = merge_runtime_usage_snapshots(
            &existing_usage_snapshots,
            usage_snapshots,
            &merged.profiles,
        );
        let existing_backoffs = load_runtime_profile_backoffs(paths, &merged.profiles)?;
        let merged_backoffs = merge_runtime_profile_backoffs(
            &existing_backoffs,
            backoffs,
            &merged.profiles,
            Local::now().timestamp(),
        );

        if latest_revision.load(Ordering::SeqCst) != revision {
            return Ok(false);
        }

        let save_result = (|| -> Result<()> {
            // Continuations are restored as the stronger source of truth on startup,
            // so persist them before the state snapshot to reduce crash windows where
            // a newer state file could be overwritten by an older continuation sidecar.
            save_runtime_continuations_for_profiles(
                paths,
                &merged_continuations,
                &merged.profiles,
            )?;
            write_state_json_atomic(paths, &json)?;
            save_runtime_profile_scores(paths, &merged_scores)?;
            save_runtime_usage_snapshots(paths, &merged_usage_snapshots)?;
            save_runtime_profile_backoffs(paths, &merged_backoffs)?;
            Ok(())
        })();
        match save_result {
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

#[derive(Parser, Debug)]
#[command(
    name = "prodex",
    version,
    about = "Manage multiple Codex profiles backed by isolated CODEX_HOME directories."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(subcommand)]
    Profile(ProfileCommands),
    #[command(name = "use")]
    UseProfile(ProfileSelector),
    Current,
    #[command(name = "info")]
    Info(InfoArgs),
    Doctor(DoctorArgs),
    #[command(trailing_var_arg = true)]
    Login(CodexPassthroughArgs),
    Logout(ProfileSelector),
    Quota(QuotaArgs),
    #[command(trailing_var_arg = true)]
    Run(RunArgs),
    #[command(name = "__runtime-broker", hide = true)]
    RuntimeBroker(RuntimeBrokerArgs),
}

#[derive(Subcommand, Debug)]
enum ProfileCommands {
    Add(AddProfileArgs),
    ImportCurrent(ImportCurrentArgs),
    List,
    Remove(RemoveProfileArgs),
    Use(ProfileSelector),
}

#[derive(Args, Debug)]
struct AddProfileArgs {
    name: String,
    #[arg(long)]
    codex_home: Option<PathBuf>,
    #[arg(long)]
    copy_from: Option<PathBuf>,
    #[arg(long)]
    copy_current: bool,
    #[arg(long)]
    activate: bool,
}

#[derive(Args, Debug)]
struct ImportCurrentArgs {
    #[arg(default_value = "default")]
    name: String,
}

#[derive(Args, Debug)]
struct RemoveProfileArgs {
    name: String,
    #[arg(long)]
    delete_home: bool,
}

#[derive(Args, Debug, Clone)]
struct ProfileSelector {
    #[arg(short, long)]
    profile: Option<String>,
}

#[derive(Args, Debug)]
struct CodexPassthroughArgs {
    #[arg(short, long)]
    profile: Option<String>,
    #[arg(value_name = "CODEX_ARG", allow_hyphen_values = true)]
    codex_args: Vec<OsString>,
}

#[derive(Args, Debug)]
struct QuotaArgs {
    #[arg(short, long)]
    profile: Option<String>,
    #[arg(long)]
    all: bool,
    #[arg(long)]
    detail: bool,
    #[arg(long)]
    raw: bool,
    #[arg(long, hide = true)]
    watch: bool,
    #[arg(long, conflicts_with = "watch")]
    once: bool,
    #[arg(long)]
    base_url: Option<String>,
}

#[derive(Args, Debug, Default)]
struct InfoArgs {}

#[derive(Args, Debug)]
struct DoctorArgs {
    #[arg(long)]
    quota: bool,
    #[arg(long)]
    runtime: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
struct RunArgs {
    #[arg(short, long)]
    profile: Option<String>,
    #[arg(long, conflicts_with = "no_auto_rotate")]
    auto_rotate: bool,
    #[arg(long)]
    no_auto_rotate: bool,
    #[arg(long)]
    skip_quota_check: bool,
    #[arg(long)]
    base_url: Option<String>,
    #[arg(value_name = "CODEX_ARG", allow_hyphen_values = true)]
    codex_args: Vec<OsString>,
}

#[derive(Args, Debug)]
struct RuntimeBrokerArgs {
    #[arg(long)]
    current_profile: String,
    #[arg(long)]
    upstream_base_url: String,
    #[arg(long, default_value_t = false)]
    include_code_review: bool,
    #[arg(long)]
    broker_key: String,
    #[arg(long)]
    instance_token: String,
    #[arg(long)]
    admin_token: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AppState {
    active_profile: Option<String>,
    #[serde(default)]
    profiles: BTreeMap<String, ProfileEntry>,
    #[serde(default)]
    last_run_selected_at: BTreeMap<String, i64>,
    #[serde(default)]
    response_profile_bindings: BTreeMap<String, ResponseProfileBinding>,
    #[serde(default)]
    session_profile_bindings: BTreeMap<String, ResponseProfileBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProfileEntry {
    codex_home: PathBuf,
    managed: bool,
    #[serde(default)]
    email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ResponseProfileBinding {
    profile_name: String,
    bound_at: i64,
}

#[derive(Debug, Clone)]
struct AppPaths {
    root: PathBuf,
    state_file: PathBuf,
    managed_profiles_root: PathBuf,
    shared_codex_root: PathBuf,
    legacy_shared_codex_root: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
struct UsageResponse {
    email: Option<String>,
    plan_type: Option<String>,
    rate_limit: Option<WindowPair>,
    code_review_rate_limit: Option<WindowPair>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    additional_rate_limits: Vec<AdditionalRateLimit>,
}

#[derive(Debug, Clone, Deserialize)]
struct WindowPair {
    primary_window: Option<UsageWindow>,
    secondary_window: Option<UsageWindow>,
}

#[derive(Debug, Clone, Deserialize)]
struct AdditionalRateLimit {
    limit_name: Option<String>,
    metered_feature: Option<String>,
    rate_limit: WindowPair,
}

#[derive(Debug, Clone, Deserialize)]
struct UsageWindow {
    used_percent: Option<i64>,
    reset_at: Option<i64>,
    limit_window_seconds: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
struct StoredAuth {
    auth_mode: Option<String>,
    tokens: Option<StoredTokens>,
    #[serde(rename = "OPENAI_API_KEY")]
    openai_api_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct StoredTokens {
    access_token: Option<String>,
    account_id: Option<String>,
    id_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct IdTokenClaims {
    #[serde(default)]
    email: Option<String>,
    #[serde(rename = "https://api.openai.com/profile", default)]
    profile: Option<IdTokenProfileClaims>,
}

#[derive(Debug, Clone, Deserialize)]
struct IdTokenProfileClaims {
    #[serde(default)]
    email: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InfoQuotaSource {
    LiveProbe,
    PersistedSnapshot,
}

#[derive(Debug, Clone)]
struct InfoQuotaAggregate {
    quota_compatible_profiles: usize,
    live_profiles: usize,
    snapshot_profiles: usize,
    unavailable_profiles: usize,
    five_hour_pool_remaining: i64,
    weekly_pool_remaining: i64,
    earliest_five_hour_reset_at: Option<i64>,
    earliest_weekly_reset_at: Option<i64>,
}

impl InfoQuotaAggregate {
    fn profiles_with_data(&self) -> usize {
        self.live_profiles + self.snapshot_profiles
    }
}

#[derive(Debug, Clone)]
struct ProcessRow {
    pid: u32,
    command: String,
    args: Vec<String>,
}

#[derive(Debug, Clone)]
struct ProdexProcessInfo {
    pid: u32,
    runtime: bool,
}

#[derive(Debug, Clone)]
struct InfoRuntimeQuotaObservation {
    timestamp: i64,
    profile: String,
    five_hour_remaining: i64,
    weekly_remaining: i64,
}

#[derive(Debug, Clone, Default)]
struct InfoRuntimeLoadSummary {
    log_count: usize,
    observations: Vec<InfoRuntimeQuotaObservation>,
    active_inflight_units: usize,
    recent_selection_events: usize,
    recent_first_timestamp: Option<i64>,
    recent_last_timestamp: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
struct InfoRunwayEstimate {
    burn_per_hour: f64,
    observed_profiles: usize,
    observed_span_seconds: i64,
    exhaust_at: i64,
}

#[derive(Debug, Clone, Copy)]
enum InfoQuotaWindow {
    FiveHour,
    Weekly,
}
struct ProfileEmailLookupJob {
    name: String,
    codex_home: PathBuf,
}

#[derive(Debug)]
struct RunProfileProbeJob {
    name: String,
    order_index: usize,
    codex_home: PathBuf,
}

#[derive(Debug)]
struct RunProfileProbeReport {
    name: String,
    order_index: usize,
    auth: AuthSummary,
    result: std::result::Result<UsageResponse, String>,
}

#[derive(Debug, Clone)]
struct ReadyProfileCandidate {
    name: String,
    usage: UsageResponse,
    order_index: usize,
    preferred: bool,
    quota_source: RuntimeQuotaSource,
}

#[derive(Debug, Clone, Copy)]
struct MainWindowSnapshot {
    remaining_percent: i64,
    reset_at: i64,
    pressure_score: i64,
}

#[derive(Debug, Clone, Copy)]
struct ReadyProfileScore {
    total_pressure: i64,
    weekly_pressure: i64,
    five_hour_pressure: i64,
    reserve_floor: i64,
    weekly_remaining: i64,
    five_hour_remaining: i64,
    weekly_reset_at: i64,
    five_hour_reset_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum RuntimeQuotaWindowStatus {
    Ready,
    Thin,
    Critical,
    Exhausted,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
struct RuntimeQuotaWindowSummary {
    status: RuntimeQuotaWindowStatus,
    remaining_percent: i64,
    reset_at: i64,
}

#[derive(Debug, Clone, Copy)]
struct RuntimeQuotaSummary {
    five_hour: RuntimeQuotaWindowSummary,
    weekly: RuntimeQuotaWindowSummary,
    route_band: RuntimeQuotaPressureBand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RuntimeQuotaPressureBand {
    Healthy,
    Thin,
    Critical,
    Exhausted,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeQuotaSource {
    LiveProbe,
    PersistedSnapshot,
}

#[derive(Debug, Clone)]
struct RuntimeProxyRequest {
    method: String,
    path_and_query: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

#[derive(Debug, Clone)]
struct RecoveredLoad<T> {
    value: T,
    recovered_from_backup: bool,
}

#[derive(Debug, Clone)]
struct RecoveredVersionedLoad<T> {
    value: T,
    generation: u64,
    recovered_from_backup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VersionedJson<T> {
    #[serde(default)]
    generation: u64,
    value: T,
}

#[derive(Debug, Clone)]
struct RuntimeRotationProxyShared {
    async_client: reqwest::Client,
    async_runtime: Arc<TokioRuntime>,
    runtime: Arc<Mutex<RuntimeRotationState>>,
    log_path: PathBuf,
    request_sequence: Arc<AtomicU64>,
    state_save_revision: Arc<AtomicU64>,
    local_overload_backoff_until: Arc<AtomicU64>,
    active_request_count: Arc<AtomicUsize>,
    active_request_limit: usize,
    lane_admission: RuntimeProxyLaneAdmission,
}

#[derive(Debug)]
struct RuntimeStateSaveQueue {
    pending: Mutex<BTreeMap<PathBuf, RuntimeStateSaveJob>>,
    wake: Condvar,
    active: Arc<AtomicUsize>,
}

#[derive(Debug)]
struct RuntimeContinuationJournalSaveQueue {
    pending: Mutex<BTreeMap<PathBuf, RuntimeContinuationJournalSaveJob>>,
    wake: Condvar,
    active: Arc<AtomicUsize>,
}

#[derive(Debug)]
struct RuntimeStateSaveJob {
    paths: AppPaths,
    state: AppState,
    continuations: RuntimeContinuationStore,
    profile_scores: BTreeMap<String, RuntimeProfileHealth>,
    usage_snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot>,
    backoffs: RuntimeProfileBackoffs,
    revision: u64,
    latest_revision: Arc<AtomicU64>,
    log_path: PathBuf,
    reason: String,
    queued_at: Instant,
    ready_at: Instant,
}

#[derive(Debug)]
struct RuntimeContinuationJournalSaveJob {
    paths: AppPaths,
    continuations: RuntimeContinuationStore,
    log_path: PathBuf,
    reason: String,
    saved_at: i64,
    queued_at: Instant,
}

#[derive(Debug)]
struct RuntimeProbeRefreshQueue {
    pending: Mutex<BTreeMap<(PathBuf, String), RuntimeProbeRefreshJob>>,
    wake: Condvar,
    active: Arc<AtomicUsize>,
}

#[derive(Debug, Clone)]
struct RuntimeProbeRefreshJob {
    shared: RuntimeRotationProxyShared,
    profile_name: String,
    codex_home: PathBuf,
    upstream_base_url: String,
    queued_at: Instant,
}

struct StateFileLock {
    file: fs::File,
}

impl Drop for StateFileLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

#[derive(Debug, Clone)]
struct RuntimeRotationState {
    paths: AppPaths,
    state: AppState,
    upstream_base_url: String,
    include_code_review: bool,
    current_profile: String,
    turn_state_bindings: BTreeMap<String, ResponseProfileBinding>,
    session_id_bindings: BTreeMap<String, ResponseProfileBinding>,
    continuation_statuses: RuntimeContinuationStatuses,
    profile_probe_cache: BTreeMap<String, RuntimeProfileProbeCacheEntry>,
    profile_usage_snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot>,
    profile_retry_backoff_until: BTreeMap<String, i64>,
    profile_transport_backoff_until: BTreeMap<String, i64>,
    profile_route_circuit_open_until: BTreeMap<String, i64>,
    profile_inflight: BTreeMap<String, usize>,
    profile_health: BTreeMap<String, RuntimeProfileHealth>,
}

#[derive(Debug, Clone)]
struct RuntimeProfileProbeCacheEntry {
    checked_at: i64,
    auth: AuthSummary,
    result: std::result::Result<UsageResponse, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuntimeProfileUsageSnapshot {
    checked_at: i64,
    five_hour_status: RuntimeQuotaWindowStatus,
    five_hour_remaining_percent: i64,
    five_hour_reset_at: i64,
    weekly_status: RuntimeQuotaWindowStatus,
    weekly_remaining_percent: i64,
    weekly_reset_at: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RuntimeProfileBackoffs {
    retry_backoff_until: BTreeMap<String, i64>,
    transport_backoff_until: BTreeMap<String, i64>,
    route_circuit_open_until: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
struct RuntimeContinuationJournal {
    #[serde(default)]
    saved_at: i64,
    #[serde(default)]
    continuations: RuntimeContinuationStore,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
struct RuntimeContinuationStore {
    #[serde(default)]
    response_profile_bindings: BTreeMap<String, ResponseProfileBinding>,
    #[serde(default)]
    session_profile_bindings: BTreeMap<String, ResponseProfileBinding>,
    #[serde(default)]
    turn_state_bindings: BTreeMap<String, ResponseProfileBinding>,
    #[serde(default)]
    session_id_bindings: BTreeMap<String, ResponseProfileBinding>,
    #[serde(default)]
    statuses: RuntimeContinuationStatuses,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
struct RuntimeContinuationStatuses {
    #[serde(default)]
    response: BTreeMap<String, RuntimeContinuationBindingStatus>,
    #[serde(default)]
    turn_state: BTreeMap<String, RuntimeContinuationBindingStatus>,
    #[serde(default)]
    session_id: BTreeMap<String, RuntimeContinuationBindingStatus>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
enum RuntimeContinuationBindingLifecycle {
    #[default]
    Warm,
    Verified,
    Suspect,
    Dead,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
struct RuntimeContinuationBindingStatus {
    #[serde(default)]
    state: RuntimeContinuationBindingLifecycle,
    #[serde(default)]
    confidence: u32,
    #[serde(default)]
    last_touched_at: Option<i64>,
    #[serde(default)]
    last_verified_at: Option<i64>,
    #[serde(default)]
    last_verified_route: Option<String>,
    #[serde(default)]
    last_not_found_at: Option<i64>,
    #[serde(default)]
    not_found_streak: u32,
    #[serde(default)]
    success_count: u32,
    #[serde(default)]
    failure_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeProbeCacheFreshness {
    Fresh,
    StaleUsable,
    Expired,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RuntimeProfileHealth {
    score: u32,
    updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeRouteKind {
    Responses,
    Compact,
    Websocket,
    Standard,
}

#[derive(Debug, Clone, Default)]
struct RuntimeDoctorSummary {
    log_path: Option<PathBuf>,
    pointer_exists: bool,
    log_exists: bool,
    line_count: usize,
    marker_counts: BTreeMap<&'static str, usize>,
    last_marker_line: Option<String>,
    marker_last_fields: BTreeMap<&'static str, BTreeMap<String, String>>,
    facet_counts: BTreeMap<String, BTreeMap<String, usize>>,
    previous_response_not_found_by_route: BTreeMap<String, usize>,
    previous_response_not_found_by_transport: BTreeMap<String, usize>,
    first_timestamp: Option<String>,
    last_timestamp: Option<String>,
    selection_pressure: String,
    transport_pressure: String,
    persistence_pressure: String,
    quota_freshness_pressure: String,
    startup_audit_pressure: String,
    persisted_retry_backoffs: usize,
    persisted_transport_backoffs: usize,
    persisted_route_circuits: usize,
    persisted_usage_snapshots: usize,
    persisted_response_bindings: usize,
    persisted_session_bindings: usize,
    persisted_turn_state_bindings: usize,
    persisted_session_id_bindings: usize,
    persisted_verified_continuations: usize,
    persisted_warm_continuations: usize,
    persisted_suspect_continuations: usize,
    persisted_dead_continuations: usize,
    persisted_continuation_journal_response_bindings: usize,
    persisted_continuation_journal_session_bindings: usize,
    persisted_continuation_journal_turn_state_bindings: usize,
    persisted_continuation_journal_session_id_bindings: usize,
    state_save_queue_backlog: Option<usize>,
    state_save_lag_ms: Option<u64>,
    continuation_journal_save_backlog: Option<usize>,
    continuation_journal_save_lag_ms: Option<u64>,
    profile_probe_refresh_backlog: Option<usize>,
    profile_probe_refresh_lag_ms: Option<u64>,
    continuation_journal_saved_at: Option<i64>,
    suspect_continuation_bindings: Vec<String>,
    failure_class_counts: BTreeMap<String, usize>,
    stale_persisted_usage_snapshots: usize,
    recovered_state_file: bool,
    recovered_scores_file: bool,
    recovered_usage_snapshots_file: bool,
    recovered_backoffs_file: bool,
    recovered_continuations_file: bool,
    recovered_continuation_journal_file: bool,
    last_good_backups_present: usize,
    degraded_routes: Vec<String>,
    orphan_managed_dirs: Vec<String>,
    profiles: Vec<RuntimeDoctorProfileSummary>,
    diagnosis: String,
}

const RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX: &str = "__compact_session__:";
const RUNTIME_COMPACT_TURN_STATE_LINEAGE_PREFIX: &str = "__compact_turn_state__:";

#[derive(Debug, Clone, Default)]
struct RuntimeDoctorProfileSummary {
    profile: String,
    quota_freshness: String,
    quota_age_seconds: i64,
    retry_backoff_until: Option<i64>,
    transport_backoff_until: Option<i64>,
    routes: Vec<RuntimeDoctorRouteSummary>,
}

#[derive(Debug, Clone, Default)]
struct RuntimeDoctorRouteSummary {
    route: String,
    circuit_state: String,
    circuit_until: Option<i64>,
    health_score: u32,
    bad_pairing_score: u32,
    performance_score: u32,
    quota_band: String,
    five_hour_status: String,
    weekly_status: String,
}

struct RuntimeRotationProxy {
    server: Arc<TinyServer>,
    shutdown: Arc<AtomicBool>,
    worker_threads: Vec<thread::JoinHandle<()>>,
    accept_worker_count: usize,
    listen_addr: std::net::SocketAddr,
    log_path: PathBuf,
    active_request_count: Arc<AtomicUsize>,
    owner_lock: Option<StateFileLock>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct RuntimeBrokerRegistry {
    pid: u32,
    listen_addr: String,
    started_at: i64,
    upstream_base_url: String,
    include_code_review: bool,
    current_profile: String,
    instance_token: String,
    admin_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct RuntimeBrokerHealth {
    pid: u32,
    started_at: i64,
    current_profile: String,
    include_code_review: bool,
    active_requests: usize,
    instance_token: String,
    persistence_role: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct RuntimeBrokerMetadata {
    started_at: i64,
    current_profile: String,
    include_code_review: bool,
    instance_token: String,
    admin_token: String,
}

#[derive(Debug)]
struct RuntimeBrokerLease {
    path: PathBuf,
}

#[derive(Debug)]
struct RuntimeProxyEndpoint {
    listen_addr: std::net::SocketAddr,
    _lease: Option<RuntimeBrokerLease>,
}

type RuntimeLocalWebSocket = WsSocket<Box<dyn TinyReadWrite + Send>>;
type RuntimeUpstreamWebSocket = WsSocket<MaybeTlsStream<TcpStream>>;

fn runtime_set_upstream_websocket_io_timeout(
    socket: &mut RuntimeUpstreamWebSocket,
    timeout: Option<Duration>,
) -> io::Result<()> {
    match socket.get_mut() {
        MaybeTlsStream::Plain(stream) => {
            stream.set_read_timeout(timeout)?;
            stream.set_write_timeout(timeout)?;
        }
        MaybeTlsStream::Rustls(stream) => {
            stream.sock.set_read_timeout(timeout)?;
            stream.sock.set_write_timeout(timeout)?;
        }
        _ => {}
    }
    Ok(())
}

fn runtime_websocket_timeout_error(err: &WsError) -> bool {
    matches!(
        err,
        WsError::Io(io_err)
            if matches!(
                io_err.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            )
    )
}

enum RuntimeResponsesAttempt {
    Success {
        profile_name: String,
        response: RuntimeResponsesReply,
    },
    QuotaBlocked {
        profile_name: String,
        response: RuntimeResponsesReply,
    },
    PreviousResponseNotFound {
        profile_name: String,
        response: RuntimeResponsesReply,
        turn_state: Option<String>,
    },
    LocalSelectionBlocked {
        profile_name: String,
    },
}

enum RuntimeStandardAttempt {
    Success {
        profile_name: String,
        response: tiny_http::ResponseBox,
    },
    RetryableFailure {
        profile_name: String,
        response: tiny_http::ResponseBox,
        overload: bool,
    },
    LocalSelectionBlocked {
        profile_name: String,
    },
}

#[derive(Debug)]
enum RuntimeSseInspection {
    Commit {
        prelude: Vec<u8>,
        response_ids: Vec<String>,
    },
    QuotaBlocked(Vec<u8>),
    PreviousResponseNotFound(Vec<u8>),
}

#[derive(Default)]
struct RuntimeSseTapState {
    line: Vec<u8>,
    data_lines: Vec<String>,
    remembered_response_ids: BTreeSet<String>,
}

enum RuntimeResponsesReply {
    Buffered(tiny_http::ResponseBox),
    Streaming(RuntimeStreamingResponse),
}

struct RuntimeStreamingResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: Box<dyn Read + Send>,
    request_id: u64,
    profile_name: String,
    log_path: PathBuf,
    shared: RuntimeRotationProxyShared,
    _inflight_guard: Option<RuntimeProfileInFlightGuard>,
}

struct RuntimeProfileInFlightGuard {
    shared: RuntimeRotationProxyShared,
    profile_name: String,
    context: &'static str,
    weight: usize,
}

struct RuntimeProxyActiveRequestGuard {
    active_request_count: Arc<AtomicUsize>,
    lane_active_count: Arc<AtomicUsize>,
}

impl Drop for RuntimeProxyActiveRequestGuard {
    fn drop(&mut self) {
        self.active_request_count.fetch_sub(1, Ordering::SeqCst);
        self.lane_active_count.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Drop for RuntimeProfileInFlightGuard {
    fn drop(&mut self) {
        if let Ok(mut runtime) = self.shared.runtime.lock() {
            let remaining =
                if let Some(count) = runtime.profile_inflight.get_mut(&self.profile_name) {
                    *count = count.saturating_sub(self.weight);
                    let remaining = *count;
                    if remaining == 0 {
                        runtime.profile_inflight.remove(&self.profile_name);
                    }
                    remaining
                } else {
                    0
                };
            drop(runtime);
            runtime_proxy_log(
                &self.shared,
                format!(
                    "profile_inflight profile={} count={} weight={} context={} event=release",
                    self.profile_name, remaining, self.weight, self.context
                ),
            );
        }
    }
}

enum RuntimePrefetchChunk {
    Data(Vec<u8>),
    End,
    Error(io::ErrorKind, String),
}

struct RuntimePrefetchStream {
    receiver: Receiver<RuntimePrefetchChunk>,
    backlog: VecDeque<RuntimePrefetchChunk>,
}

struct RuntimePrefetchReader {
    receiver: Receiver<RuntimePrefetchChunk>,
    backlog: VecDeque<RuntimePrefetchChunk>,
    pending: Cursor<Vec<u8>>,
    finished: bool,
}

#[derive(Debug)]
enum RuntimeWebsocketAttempt {
    Delivered,
    QuotaBlocked {
        profile_name: String,
        payload: RuntimeWebsocketErrorPayload,
    },
    LocalSelectionBlocked {
        profile_name: String,
    },
    PreviousResponseNotFound {
        profile_name: String,
        payload: RuntimeWebsocketErrorPayload,
        turn_state: Option<String>,
    },
    ReuseWatchdogTripped {
        profile_name: String,
        event: &'static str,
    },
}

enum RuntimeUpstreamFailureResponse {
    Http(RuntimeResponsesReply),
    Websocket(RuntimeWebsocketErrorPayload),
}

#[derive(Debug)]
enum RuntimeWebsocketConnectResult {
    Connected {
        socket: RuntimeUpstreamWebSocket,
        turn_state: Option<String>,
    },
    QuotaBlocked(RuntimeWebsocketErrorPayload),
}

#[derive(Debug, Clone)]
enum RuntimeWebsocketErrorPayload {
    Text(String),
    Binary(Vec<u8>),
    Empty,
}

fn runtime_proxy_log(shared: &RuntimeRotationProxyShared, message: impl AsRef<str>) {
    runtime_proxy_log_to_path(&shared.log_path, message.as_ref());
}

fn runtime_proxy_next_request_id(shared: &RuntimeRotationProxyShared) -> u64 {
    shared.request_sequence.fetch_add(1, Ordering::Relaxed)
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    if !matches!(cli.command, Commands::RuntimeBroker(_)) {
        let _ = show_update_notice_if_available(&cli.command);
    }
    match cli.command {
        Commands::Profile(command) => handle_profile_command(command),
        Commands::UseProfile(selector) => handle_set_active_profile(selector),
        Commands::Current => handle_current_profile(),
        Commands::Info(args) => handle_info(args),
        Commands::Doctor(args) => handle_doctor(args),
        Commands::Login(args) => handle_codex_login(args),
        Commands::Logout(selector) => handle_codex_logout(selector),
        Commands::Quota(args) => handle_quota(args),
        Commands::Run(args) => handle_run(args),
        Commands::RuntimeBroker(args) => handle_runtime_broker(args),
    }
}

fn handle_profile_command(command: ProfileCommands) -> Result<()> {
    match command {
        ProfileCommands::Add(args) => handle_add_profile(args),
        ProfileCommands::ImportCurrent(args) => handle_import_current_profile(args),
        ProfileCommands::List => handle_list_profiles(),
        ProfileCommands::Remove(args) => handle_remove_profile(args),
        ProfileCommands::Use(selector) => handle_set_active_profile(selector),
    }
}

fn handle_info(_args: InfoArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let now = Local::now().timestamp();
    let quota = collect_info_quota_aggregate(&paths, &state, now);
    let processes = collect_prodex_processes();
    let runtime_logs = collect_active_runtime_log_paths(&processes);
    let runtime_load = collect_info_runtime_load_summary(&runtime_logs, now);
    let runtime_process_count = processes.iter().filter(|process| process.runtime).count();
    let five_hour_runway = estimate_info_runway(
        &runtime_load.observations,
        InfoQuotaWindow::FiveHour,
        quota.five_hour_pool_remaining,
        now,
    );
    let weekly_runway = estimate_info_runway(
        &runtime_load.observations,
        InfoQuotaWindow::Weekly,
        quota.weekly_pool_remaining,
        now,
    );

    let fields = vec![
        ("Profiles".to_string(), state.profiles.len().to_string()),
        (
            "Active profile".to_string(),
            state.active_profile.as_deref().unwrap_or("-").to_string(),
        ),
        (
            "Prodex processes".to_string(),
            format_info_process_summary(&processes),
        ),
        (
            "Recent load".to_string(),
            format_info_load_summary(&runtime_load, runtime_process_count),
        ),
        (
            "Quota data".to_string(),
            format_info_quota_data_summary(&quota),
        ),
        (
            "5h remaining pool".to_string(),
            format_info_pool_remaining(
                quota.five_hour_pool_remaining,
                quota.profiles_with_data(),
                quota.earliest_five_hour_reset_at,
            ),
        ),
        (
            "Weekly remaining pool".to_string(),
            format_info_pool_remaining(
                quota.weekly_pool_remaining,
                quota.profiles_with_data(),
                quota.earliest_weekly_reset_at,
            ),
        ),
        (
            "5h runway".to_string(),
            format_info_runway(
                quota.profiles_with_data(),
                quota.five_hour_pool_remaining,
                quota.earliest_five_hour_reset_at,
                five_hour_runway.as_ref(),
                now,
            ),
        ),
        (
            "Weekly runway".to_string(),
            format_info_runway(
                quota.profiles_with_data(),
                quota.weekly_pool_remaining,
                quota.earliest_weekly_reset_at,
                weekly_runway.as_ref(),
                now,
            ),
        ),
    ];
    print_panel("Info", &fields);
    Ok(())
}

fn collect_info_quota_aggregate(
    paths: &AppPaths,
    state: &AppState,
    now: i64,
) -> InfoQuotaAggregate {
    if state.profiles.is_empty() {
        return InfoQuotaAggregate {
            quota_compatible_profiles: 0,
            live_profiles: 0,
            snapshot_profiles: 0,
            unavailable_profiles: 0,
            five_hour_pool_remaining: 0,
            weekly_pool_remaining: 0,
            earliest_five_hour_reset_at: None,
            earliest_weekly_reset_at: None,
        };
    }

    let persisted_usage_snapshots =
        load_runtime_usage_snapshots(paths, &state.profiles).unwrap_or_default();
    let reports =
        collect_run_profile_reports(state, state.profiles.keys().cloned().collect(), None);
    build_info_quota_aggregate(&reports, &persisted_usage_snapshots, now)
}

fn build_info_quota_aggregate(
    reports: &[RunProfileProbeReport],
    persisted_usage_snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    now: i64,
) -> InfoQuotaAggregate {
    let mut aggregate = InfoQuotaAggregate {
        quota_compatible_profiles: reports
            .iter()
            .filter(|report| report.auth.quota_compatible)
            .count(),
        live_profiles: 0,
        snapshot_profiles: 0,
        unavailable_profiles: 0,
        five_hour_pool_remaining: 0,
        weekly_pool_remaining: 0,
        earliest_five_hour_reset_at: None,
        earliest_weekly_reset_at: None,
    };

    for report in reports {
        if !report.auth.quota_compatible {
            continue;
        }

        let usage = match &report.result {
            Ok(usage) => Some((usage.clone(), InfoQuotaSource::LiveProbe)),
            Err(_) => persisted_usage_snapshots
                .get(&report.name)
                .filter(|snapshot| runtime_usage_snapshot_is_usable(snapshot, now))
                .map(|snapshot| {
                    (
                        usage_from_runtime_usage_snapshot(snapshot),
                        InfoQuotaSource::PersistedSnapshot,
                    )
                }),
        };

        let Some((usage, source)) = usage else {
            aggregate.unavailable_profiles += 1;
            continue;
        };

        let Some((five_hour, weekly)) = info_main_window_snapshots(&usage) else {
            aggregate.unavailable_profiles += 1;
            continue;
        };

        match source {
            InfoQuotaSource::LiveProbe => aggregate.live_profiles += 1,
            InfoQuotaSource::PersistedSnapshot => aggregate.snapshot_profiles += 1,
        }
        aggregate.five_hour_pool_remaining += five_hour.remaining_percent;
        aggregate.weekly_pool_remaining += weekly.remaining_percent;
        if five_hour.reset_at != i64::MAX {
            aggregate.earliest_five_hour_reset_at = Some(
                aggregate
                    .earliest_five_hour_reset_at
                    .map_or(five_hour.reset_at, |current| {
                        current.min(five_hour.reset_at)
                    }),
            );
        }
        if weekly.reset_at != i64::MAX {
            aggregate.earliest_weekly_reset_at = Some(
                aggregate
                    .earliest_weekly_reset_at
                    .map_or(weekly.reset_at, |current| current.min(weekly.reset_at)),
            );
        }
    }

    aggregate
}

fn info_main_window_snapshots(
    usage: &UsageResponse,
) -> Option<(MainWindowSnapshot, MainWindowSnapshot)> {
    Some((
        required_main_window_snapshot(usage, "5h")?,
        required_main_window_snapshot(usage, "weekly")?,
    ))
}

fn collect_prodex_processes() -> Vec<ProdexProcessInfo> {
    let current_pid = std::process::id();
    let current_basename = std::env::current_exe().ok().and_then(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(ToOwned::to_owned)
    });

    let mut processes = collect_process_rows()
        .into_iter()
        .filter_map(|row| {
            classify_prodex_process_row(row, current_pid, current_basename.as_deref())
        })
        .collect::<Vec<_>>();
    processes.sort_by_key(|process| process.pid);
    processes
}

fn collect_process_rows() -> Vec<ProcessRow> {
    collect_process_rows_from_proc()
        .or_else(|| collect_process_rows_from_ps().ok())
        .unwrap_or_default()
}

fn collect_process_rows_from_proc() -> Option<Vec<ProcessRow>> {
    let mut rows = Vec::new();
    let entries = fs::read_dir("/proc").ok()?;
    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let dir = entry.path();
        let Some(command) = fs::read_to_string(dir.join("comm"))
            .ok()
            .map(|value| value.trim().to_string())
        else {
            continue;
        };
        let Some(args_bytes) = fs::read(dir.join("cmdline")).ok() else {
            continue;
        };
        let args = args_bytes
            .split(|byte| *byte == 0)
            .filter_map(|chunk| {
                if chunk.is_empty() {
                    return None;
                }
                String::from_utf8(chunk.to_vec()).ok()
            })
            .collect::<Vec<_>>();
        rows.push(ProcessRow { pid, command, args });
    }
    Some(rows)
}

fn collect_process_rows_from_ps() -> Result<Vec<ProcessRow>> {
    let output = Command::new("ps")
        .args(["-Ao", "pid=,comm=,args="])
        .output()
        .context("failed to execute ps for prodex process listing")?;
    if !output.status.success() {
        bail!("ps returned exit status {}", output.status);
    }
    let text = String::from_utf8(output.stdout).context("ps output was not valid UTF-8")?;
    Ok(parse_ps_process_rows(&text))
}

fn parse_ps_process_rows(text: &str) -> Vec<ProcessRow> {
    let mut rows = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let tokens = trimmed.split_whitespace().collect::<Vec<_>>();
        if tokens.len() < 2 {
            continue;
        }
        let Ok(pid) = tokens[0].parse::<u32>() else {
            continue;
        };
        rows.push(ProcessRow {
            pid,
            command: tokens[1].to_string(),
            args: tokens
                .iter()
                .skip(2)
                .map(|token| (*token).to_string())
                .collect(),
        });
    }
    rows
}

fn classify_prodex_process_row(
    row: ProcessRow,
    current_pid: u32,
    current_basename: Option<&str>,
) -> Option<ProdexProcessInfo> {
    if row.pid == current_pid || !is_prodex_process_row(&row, current_basename) {
        return None;
    }

    Some(ProdexProcessInfo {
        pid: row.pid,
        runtime: prodex_process_row_is_runtime(&row, current_basename),
    })
}

fn is_prodex_process_row(row: &ProcessRow, current_basename: Option<&str>) -> bool {
    let command_base = process_basename(&row.command);
    process_basename_matches(command_base, current_basename)
        || prodex_process_row_argv_span(row, current_basename).is_some()
}

fn prodex_process_row_is_runtime(row: &ProcessRow, current_basename: Option<&str>) -> bool {
    prodex_process_row_argv_span(row, current_basename)
        .and_then(|args| args.get(1))
        .is_some_and(|arg| arg == "run" || arg == "__runtime-broker")
}

fn prodex_process_row_argv_span<'a>(
    row: &'a ProcessRow,
    current_basename: Option<&str>,
) -> Option<&'a [String]> {
    row.args.iter().enumerate().find_map(|(index, arg)| {
        process_basename_matches(process_basename(arg), current_basename)
            .then_some(&row.args[index..])
    })
}

fn process_basename_matches(candidate: &str, current_basename: Option<&str>) -> bool {
    candidate == "prodex" || current_basename.is_some_and(|name| candidate == name)
}

fn process_basename(input: &str) -> &str {
    Path::new(input)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(input)
}

fn collect_active_runtime_log_paths(processes: &[ProdexProcessInfo]) -> Vec<PathBuf> {
    let runtime_pids = processes
        .iter()
        .filter(|process| process.runtime)
        .map(|process| process.pid)
        .collect::<BTreeSet<_>>();
    if runtime_pids.is_empty() {
        return Vec::new();
    }

    let mut latest_logs = BTreeMap::new();
    for path in prodex_runtime_log_paths_in_dir(&runtime_proxy_log_dir()) {
        let Some(pid) = runtime_log_pid_from_path(&path) else {
            continue;
        };
        if runtime_pids.contains(&pid) {
            latest_logs.insert(pid, path);
        }
    }
    latest_logs.into_values().collect()
}

fn runtime_log_pid_from_path(path: &Path) -> Option<u32> {
    let name = path.file_name()?.to_str()?;
    let rest = name.strip_prefix(&format!("{RUNTIME_PROXY_LOG_FILE_PREFIX}-"))?;
    let (pid, _) = rest.split_once('-')?;
    pid.parse::<u32>().ok()
}

fn collect_info_runtime_load_summary(log_paths: &[PathBuf], now: i64) -> InfoRuntimeLoadSummary {
    let mut summary = InfoRuntimeLoadSummary::default();
    summary.log_count = log_paths.len();

    for path in log_paths {
        let Ok(tail) = read_runtime_log_tail(path, INFO_RUNTIME_LOG_TAIL_BYTES) else {
            continue;
        };
        let log_summary = collect_info_runtime_load_summary_from_text(
            &String::from_utf8_lossy(&tail),
            now,
            INFO_RECENT_LOAD_WINDOW_SECONDS,
            INFO_FORECAST_LOOKBACK_SECONDS,
        );
        summary.observations.extend(log_summary.observations);
        summary.active_inflight_units += log_summary.active_inflight_units;
        summary.recent_selection_events += log_summary.recent_selection_events;
        summary.recent_first_timestamp = match (
            summary.recent_first_timestamp,
            log_summary.recent_first_timestamp,
        ) {
            (Some(current), Some(candidate)) => Some(current.min(candidate)),
            (current, candidate) => current.or(candidate),
        };
        summary.recent_last_timestamp = match (
            summary.recent_last_timestamp,
            log_summary.recent_last_timestamp,
        ) {
            (Some(current), Some(candidate)) => Some(current.max(candidate)),
            (current, candidate) => current.or(candidate),
        };
    }

    summary
        .observations
        .sort_by_key(|observation| (observation.timestamp, observation.profile.clone()));
    summary
}

fn collect_info_runtime_load_summary_from_text(
    text: &str,
    now: i64,
    recent_window_seconds: i64,
    lookback_seconds: i64,
) -> InfoRuntimeLoadSummary {
    let recent_cutoff = now.saturating_sub(recent_window_seconds);
    let lookback_cutoff = now.saturating_sub(lookback_seconds);
    let mut summary = InfoRuntimeLoadSummary::default();
    let mut latest_inflight_counts = BTreeMap::new();

    for line in text.lines() {
        if let Some((profile, count)) = info_runtime_inflight_from_line(line) {
            latest_inflight_counts.insert(profile, count);
        }
        let Some(observation) = info_runtime_selection_observation_from_line(line) else {
            continue;
        };
        if observation.timestamp >= lookback_cutoff {
            summary.observations.push(observation.clone());
        }
        if observation.timestamp >= recent_cutoff {
            summary.recent_selection_events += 1;
            summary.recent_first_timestamp = Some(
                summary
                    .recent_first_timestamp
                    .map_or(observation.timestamp, |current| {
                        current.min(observation.timestamp)
                    }),
            );
            summary.recent_last_timestamp = Some(
                summary
                    .recent_last_timestamp
                    .map_or(observation.timestamp, |current| {
                        current.max(observation.timestamp)
                    }),
            );
        }
    }

    summary.active_inflight_units = latest_inflight_counts.values().sum();
    summary
}

fn info_runtime_selection_observation_from_line(line: &str) -> Option<InfoRuntimeQuotaObservation> {
    if !line.contains("selection_pick") && !line.contains("selection_keep_current") {
        return None;
    }
    let timestamp = runtime_log_timestamp_epoch(line)?;
    let fields = info_runtime_parse_fields(line);
    let profile = fields.get("profile")?;
    if profile == "none" {
        return None;
    }
    Some(InfoRuntimeQuotaObservation {
        timestamp,
        profile: profile.clone(),
        five_hour_remaining: fields.get("five_hour_remaining")?.parse::<i64>().ok()?,
        weekly_remaining: fields.get("weekly_remaining")?.parse::<i64>().ok()?,
    })
}

fn info_runtime_inflight_from_line(line: &str) -> Option<(String, usize)> {
    if !line.contains("profile_inflight ") {
        return None;
    }
    let fields = info_runtime_parse_fields(line);
    Some((
        fields.get("profile")?.clone(),
        fields.get("count")?.parse::<usize>().ok()?,
    ))
}

fn runtime_log_timestamp_epoch(line: &str) -> Option<i64> {
    let timestamp = info_runtime_line_timestamp(line)?;
    chrono::DateTime::parse_from_str(&timestamp, "%Y-%m-%d %H:%M:%S%.f %:z")
        .or_else(|_| chrono::DateTime::parse_from_str(&timestamp, "%Y-%m-%d %H:%M:%S %:z"))
        .ok()
        .map(|datetime| datetime.timestamp())
}

fn info_runtime_line_timestamp(line: &str) -> Option<String> {
    let end = line.find("] ")?;
    line.strip_prefix('[')
        .and_then(|trimmed| trimmed.get(..end.saturating_sub(1)))
        .map(ToString::to_string)
}

fn info_runtime_parse_fields(line: &str) -> BTreeMap<String, String> {
    let message = line
        .split_once("] ")
        .map(|(_, message)| message)
        .unwrap_or(line)
        .trim();
    let mut fields = BTreeMap::new();
    for token in message.split_whitespace() {
        let Some((key, value)) = token.split_once('=') else {
            continue;
        };
        if key.is_empty() || value.is_empty() {
            continue;
        }
        fields.insert(key.to_string(), value.trim_matches('"').to_string());
    }
    fields
}

fn estimate_info_runway(
    observations: &[InfoRuntimeQuotaObservation],
    window: InfoQuotaWindow,
    current_remaining: i64,
    now: i64,
) -> Option<InfoRunwayEstimate> {
    if current_remaining <= 0 {
        return Some(InfoRunwayEstimate {
            burn_per_hour: 0.0,
            observed_profiles: 0,
            observed_span_seconds: 0,
            exhaust_at: now,
        });
    }

    let mut by_profile = BTreeMap::<String, Vec<&InfoRuntimeQuotaObservation>>::new();
    for observation in observations {
        by_profile
            .entry(observation.profile.clone())
            .or_default()
            .push(observation);
    }

    let mut burn_per_hour = 0.0;
    let mut observed_profiles = 0;
    let mut earliest = i64::MAX;
    let mut latest = i64::MIN;

    for profile_observations in by_profile.values_mut() {
        profile_observations.sort_by_key(|observation| observation.timestamp);
        let Some((profile_burn_per_hour, start, end)) =
            info_profile_window_burn_rate(profile_observations, window)
        else {
            continue;
        };
        burn_per_hour += profile_burn_per_hour;
        observed_profiles += 1;
        earliest = earliest.min(start);
        latest = latest.max(end);
    }

    if burn_per_hour <= 0.0 || observed_profiles == 0 || earliest == i64::MAX || latest == i64::MIN
    {
        return None;
    }

    let seconds_until_exhaustion =
        ((current_remaining as f64 / burn_per_hour) * 3600.0).ceil() as i64;

    Some(InfoRunwayEstimate {
        burn_per_hour,
        observed_profiles,
        observed_span_seconds: latest.saturating_sub(earliest),
        exhaust_at: now.saturating_add(seconds_until_exhaustion.max(0)),
    })
}

fn info_profile_window_burn_rate(
    observations: &[&InfoRuntimeQuotaObservation],
    window: InfoQuotaWindow,
) -> Option<(f64, i64, i64)> {
    if observations.len() < 2 {
        return None;
    }

    let latest = observations.last()?;
    let mut earliest = *latest;
    let mut current_remaining = info_observation_window_remaining(latest, window);

    for observation in observations.iter().rev().skip(1) {
        let remaining = info_observation_window_remaining(observation, window);
        if remaining < current_remaining {
            break;
        }
        earliest = *observation;
        current_remaining = remaining;
    }

    let earliest_remaining = info_observation_window_remaining(earliest, window);
    let latest_remaining = info_observation_window_remaining(latest, window);
    let burned = earliest_remaining.saturating_sub(latest_remaining);
    let span_seconds = latest.timestamp.saturating_sub(earliest.timestamp);
    if burned <= 0 || span_seconds < INFO_FORECAST_MIN_SPAN_SECONDS {
        return None;
    }

    Some((
        burned as f64 * 3600.0 / span_seconds as f64,
        earliest.timestamp,
        latest.timestamp,
    ))
}

fn info_observation_window_remaining(
    observation: &InfoRuntimeQuotaObservation,
    window: InfoQuotaWindow,
) -> i64 {
    match window {
        InfoQuotaWindow::FiveHour => observation.five_hour_remaining,
        InfoQuotaWindow::Weekly => observation.weekly_remaining,
    }
}

fn format_info_process_summary(processes: &[ProdexProcessInfo]) -> String {
    if processes.is_empty() {
        return "No".to_string();
    }

    let runtime_count = processes.iter().filter(|process| process.runtime).count();
    let pid_list = processes
        .iter()
        .take(6)
        .map(|process| process.pid.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let remaining = processes.len().saturating_sub(6);
    let extra = if remaining > 0 {
        format!(" (+{remaining} more)")
    } else {
        String::new()
    };

    format!(
        "Yes ({} total, {} runtime; pids: {}{})",
        processes.len(),
        runtime_count,
        pid_list,
        extra
    )
}

fn format_info_load_summary(
    summary: &InfoRuntimeLoadSummary,
    runtime_process_count: usize,
) -> String {
    if runtime_process_count == 0 {
        return "No active prodex runtime detected".to_string();
    }
    if summary.log_count == 0 {
        return "Runtime process detected, but no matching runtime log was found".to_string();
    }
    if summary.recent_selection_events == 0 {
        return format!(
            "{} active runtime log(s); no selection activity observed in the sampled window; inflight units {}",
            summary.log_count, summary.active_inflight_units
        );
    }
    if summary.recent_selection_events == 1 {
        return format!(
            "1 selection event observed in the sampled window; inflight units {}; {} active runtime log(s)",
            summary.active_inflight_units, summary.log_count
        );
    }

    let activity_span = summary
        .recent_first_timestamp
        .zip(summary.recent_last_timestamp)
        .map(|(start, end)| format_relative_duration(end.saturating_sub(start)))
        .unwrap_or_else(|| format!("{}m", INFO_RECENT_LOAD_WINDOW_SECONDS / 60));

    format!(
        "{} selection event(s) over {}; inflight units {}; {} active runtime log(s)",
        summary.recent_selection_events,
        activity_span,
        summary.active_inflight_units,
        summary.log_count
    )
}

fn format_info_quota_data_summary(aggregate: &InfoQuotaAggregate) -> String {
    if aggregate.quota_compatible_profiles == 0 {
        return "No quota-compatible profiles".to_string();
    }

    format!(
        "{} quota-compatible profile(s): live={}, snapshot={}, unavailable={}",
        aggregate.quota_compatible_profiles,
        aggregate.live_profiles,
        aggregate.snapshot_profiles,
        aggregate.unavailable_profiles
    )
}

fn format_info_pool_remaining(
    total_remaining: i64,
    profiles_with_data: usize,
    earliest_reset_at: Option<i64>,
) -> String {
    if profiles_with_data == 0 {
        return "Unavailable".to_string();
    }

    let mut value = format!("{total_remaining}% across {profiles_with_data} profile(s)");
    if let Some(reset_at) = earliest_reset_at {
        value.push_str(&format!(
            "; earliest reset {}",
            format_precise_reset_time(Some(reset_at))
        ));
    }
    value
}

fn format_info_runway(
    profiles_with_data: usize,
    current_remaining: i64,
    earliest_reset_at: Option<i64>,
    estimate: Option<&InfoRunwayEstimate>,
    now: i64,
) -> String {
    if profiles_with_data == 0 {
        return "Unavailable".to_string();
    }
    if current_remaining <= 0 {
        return "Exhausted".to_string();
    }

    let Some(estimate) = estimate else {
        return "Unavailable (no recent quota decay observed in active runtime logs)".to_string();
    };

    let observed = format_relative_duration(estimate.observed_span_seconds);
    let burn = format!("{:.1}", estimate.burn_per_hour);
    if let Some(reset_at) = earliest_reset_at
        && reset_at <= estimate.exhaust_at
    {
        return format!(
            "Earliest reset {} arrives before the no-reset runway (~{} at {} aggregated-%/h, {} profile(s), observed over {})",
            format_precise_reset_time(Some(reset_at)),
            format_relative_duration(estimate.exhaust_at.saturating_sub(now)),
            burn,
            estimate.observed_profiles,
            observed
        );
    }

    format!(
        "{} (~{}) at {} aggregated-%/h from {} profile(s), observed over {}, no-reset estimate",
        format_precise_reset_time(Some(estimate.exhaust_at)),
        format_relative_duration(estimate.exhaust_at.saturating_sub(now)),
        burn,
        estimate.observed_profiles,
        observed
    )
}

fn format_relative_duration(seconds: i64) -> String {
    let seconds = seconds.max(0);
    if seconds == 0 {
        return "now".to_string();
    }

    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;

    if days > 0 {
        if hours > 0 {
            format!("{days}d {hours}h")
        } else {
            format!("{days}d")
        }
    } else if hours > 0 {
        if minutes > 0 {
            format!("{hours}h {minutes}m")
        } else {
            format!("{hours}h")
        }
    } else if minutes > 0 {
        format!("{minutes}m")
    } else {
        "<1m".to_string()
    }
}
fn handle_doctor(args: DoctorArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let codex_home = default_codex_home()?;

    if args.runtime && args.json {
        let summary = collect_runtime_doctor_summary();
        let json = serde_json::to_string_pretty(&runtime_doctor_json_value(&summary))
            .context("failed to serialize runtime doctor summary")?;
        println!("{json}");
        return Ok(());
    }

    let summary_fields = vec![
        ("Prodex root".to_string(), paths.root.display().to_string()),
        (
            "State file".to_string(),
            format!(
                "{} ({})",
                paths.state_file.display(),
                if paths.state_file.exists() {
                    "exists"
                } else {
                    "missing"
                }
            ),
        ),
        (
            "Profiles root".to_string(),
            paths.managed_profiles_root.display().to_string(),
        ),
        (
            "Default CODEX_HOME".to_string(),
            format!(
                "{} ({})",
                codex_home.display(),
                if codex_home.exists() {
                    "exists"
                } else {
                    "missing"
                }
            ),
        ),
        (
            "Codex binary".to_string(),
            format_binary_resolution(&codex_bin()),
        ),
        (
            "Quota endpoint".to_string(),
            usage_url(&quota_base_url(None)),
        ),
        ("Profiles".to_string(), state.profiles.len().to_string()),
        (
            "Active profile".to_string(),
            state.active_profile.as_deref().unwrap_or("-").to_string(),
        ),
    ];
    print_panel("Doctor", &summary_fields);

    if args.runtime {
        println!();
        print_panel("Runtime Proxy", &runtime_doctor_fields());
    }

    if state.profiles.is_empty() {
        return Ok(());
    }

    for report in collect_doctor_profile_reports(&state, args.quota) {
        let kind = if report.summary.managed {
            "managed"
        } else {
            "external"
        };

        println!();
        let mut fields = vec![
            (
                "Current".to_string(),
                if report.summary.active {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
            ),
            ("Kind".to_string(), kind.to_string()),
            ("Auth".to_string(), report.summary.auth.label),
            (
                "Email".to_string(),
                report.summary.email.as_deref().unwrap_or("-").to_string(),
            ),
            (
                "Path".to_string(),
                report.summary.codex_home.display().to_string(),
            ),
            (
                "Exists".to_string(),
                if report.summary.codex_home.exists() {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
            ),
        ];

        if let Some(quota) = report.quota {
            match quota {
                Ok(usage) => {
                    let blocked = collect_blocked_limits(&usage, false);
                    fields.push((
                        "Quota".to_string(),
                        if blocked.is_empty() {
                            "Ready".to_string()
                        } else {
                            format!("Blocked ({})", format_blocked_limits(&blocked))
                        },
                    ));
                    fields.push(("Main".to_string(), format_main_windows(&usage)));
                }
                Err(err) => {
                    fields.push((
                        "Quota".to_string(),
                        format!("Error ({})", first_line_of_error(&err.to_string())),
                    ));
                }
            }
        }
        print_panel(&format!("Profile {}", report.summary.name), &fields);
    }

    Ok(())
}

fn deserialize_null_default<'de, D, T>(deserializer: D) -> std::result::Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

fn handle_quota(args: QuotaArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;

    if args.all {
        if state.profiles.is_empty() {
            bail!("no profiles configured");
        }
        if quota_watch_enabled(&args) {
            return watch_all_quotas(&paths, args.base_url.as_deref(), args.detail);
        }
        let reports = collect_quota_reports(&state, args.base_url.as_deref());
        print_quota_reports(&reports, args.detail);
        return Ok(());
    }

    let profile_name = resolve_profile_name(&state, args.profile.as_deref())?;
    let codex_home = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?
        .codex_home
        .clone();

    if args.raw {
        let usage = fetch_usage_json(&codex_home, args.base_url.as_deref())?;
        println!(
            "{}",
            serde_json::to_string_pretty(&usage).context("failed to render usage JSON")?
        );
        return Ok(());
    }

    if quota_watch_enabled(&args) {
        return watch_quota(&profile_name, &codex_home, args.base_url.as_deref());
    }

    let usage = fetch_usage(&codex_home, args.base_url.as_deref())?;
    println!("{}", render_profile_quota(&profile_name, &usage));
    Ok(())
}

fn handle_run(args: RunArgs) -> Result<()> {
    let codex_args = normalize_run_codex_args(&args.codex_args);
    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;
    let profile_name = resolve_profile_name(&state, args.profile.as_deref())?;
    let mut selected_profile_name = profile_name.clone();
    let explicit_profile_requested = args.profile.is_some();
    let allow_auto_rotate = !args.no_auto_rotate;
    let include_code_review = is_review_invocation(&codex_args);
    let mut codex_home = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?
        .codex_home
        .clone();

    if !args.skip_quota_check {
        if allow_auto_rotate && !explicit_profile_requested && state.profiles.len() > 1 {
            let persisted_usage_snapshots =
                load_runtime_usage_snapshots(&paths, &state.profiles).unwrap_or_default();
            let reports = collect_run_profile_reports(
                &state,
                active_profile_selection_order(&state, &profile_name),
                args.base_url.as_deref(),
            );
            let ready_candidates = ready_profile_candidates(
                &reports,
                include_code_review,
                Some(&profile_name),
                &state,
                Some(&persisted_usage_snapshots),
            );
            let selected_report = reports.iter().find(|report| report.name == profile_name);

            if let Some(best_candidate) = ready_candidates.first() {
                if best_candidate.name != profile_name {
                    print_wrapped_stderr(&section_header("Quota Preflight"));
                    let mut selection_message = format!(
                        "Using profile '{}' ({})",
                        best_candidate.name,
                        format_main_windows_compact(&best_candidate.usage)
                    );
                    if let Some(report) = selected_report {
                        match &report.result {
                            Ok(usage) => {
                                let blocked = collect_blocked_limits(usage, include_code_review);
                                if !blocked.is_empty() {
                                    print_wrapped_stderr(&format!(
                                        "Quota preflight blocked profile '{}': {}",
                                        profile_name,
                                        format_blocked_limits(&blocked)
                                    ));
                                    selection_message = format!(
                                        "Auto-rotating to profile '{}' using quota-pressure scoring ({}).",
                                        best_candidate.name,
                                        format_main_windows_compact(&best_candidate.usage)
                                    );
                                } else {
                                    selection_message = format!(
                                        "Auto-selecting profile '{}' over active profile '{}' using quota-pressure scoring ({}).",
                                        best_candidate.name,
                                        profile_name,
                                        format_main_windows_compact(&best_candidate.usage)
                                    );
                                }
                            }
                            Err(err) => {
                                print_wrapped_stderr(&format!(
                                    "Warning: quota preflight failed for '{}': {err}",
                                    profile_name
                                ));
                                selection_message = format!(
                                    "Using ready profile '{}' after quota preflight failed ({})",
                                    best_candidate.name,
                                    format_main_windows_compact(&best_candidate.usage)
                                );
                            }
                        }
                    }

                    codex_home = state
                        .profiles
                        .get(&best_candidate.name)
                        .with_context(|| format!("profile '{}' is missing", best_candidate.name))?
                        .codex_home
                        .clone();
                    selected_profile_name = best_candidate.name.clone();
                    state.active_profile = Some(best_candidate.name.clone());
                    state.save(&paths)?;
                    print_wrapped_stderr(&selection_message);
                }
            } else if let Some(report) = selected_report {
                match &report.result {
                    Ok(usage) => {
                        let blocked = collect_blocked_limits(usage, include_code_review);
                        print_wrapped_stderr(&section_header("Quota Preflight"));
                        print_wrapped_stderr(&format!(
                            "Quota preflight blocked profile '{}': {}",
                            profile_name,
                            format_blocked_limits(&blocked)
                        ));
                        print_wrapped_stderr("No ready profile was found.");
                        print_wrapped_stderr(&format!(
                            "Inspect with `prodex quota --profile {}` or bypass with `prodex run --skip-quota-check`.",
                            profile_name
                        ));
                        std::process::exit(2);
                    }
                    Err(err) => {
                        print_wrapped_stderr(&section_header("Quota Preflight"));
                        print_wrapped_stderr(&format!(
                            "Warning: quota preflight failed for '{}': {err:#}",
                            profile_name
                        ));
                        print_wrapped_stderr("Continuing without quota gate.");
                    }
                }
            }
        } else {
            match fetch_usage(&codex_home, args.base_url.as_deref()) {
                Ok(usage) => {
                    let blocked = collect_blocked_limits(&usage, include_code_review);
                    if !blocked.is_empty() {
                        let alternatives = find_ready_profiles(
                            &state,
                            &profile_name,
                            args.base_url.as_deref(),
                            include_code_review,
                        );

                        print_wrapped_stderr(&section_header("Quota Preflight"));
                        print_wrapped_stderr(&format!(
                            "Quota preflight blocked profile '{}': {}",
                            profile_name,
                            format_blocked_limits(&blocked)
                        ));

                        if allow_auto_rotate {
                            if let Some(next_profile) = alternatives.first() {
                                let next_profile = next_profile.clone();
                                codex_home = state
                                    .profiles
                                    .get(&next_profile)
                                    .with_context(|| {
                                        format!("profile '{}' is missing", next_profile)
                                    })?
                                    .codex_home
                                    .clone();
                                selected_profile_name = next_profile.clone();
                                state.active_profile = Some(next_profile.clone());
                                state.save(&paths)?;
                                print_wrapped_stderr(&format!(
                                    "Auto-rotating to profile '{}'.",
                                    next_profile
                                ));
                            } else {
                                print_wrapped_stderr("No other ready profile was found.");
                                print_wrapped_stderr(&format!(
                                    "Inspect with `prodex quota --profile {}` or bypass with `prodex run --skip-quota-check`.",
                                    profile_name
                                ));
                                std::process::exit(2);
                            }
                        } else {
                            if !alternatives.is_empty() {
                                print_wrapped_stderr(&format!(
                                    "Other profiles that look ready: {}",
                                    alternatives.join(", ")
                                ));
                                print_wrapped_stderr(
                                    "Rerun without `--no-auto-rotate` to allow fallback.",
                                );
                            }
                            print_wrapped_stderr(&format!(
                                "Inspect with `prodex quota --profile {}` or bypass with `prodex run --skip-quota-check`.",
                                profile_name
                            ));
                            std::process::exit(2);
                        }
                    }
                }
                Err(err) => {
                    print_wrapped_stderr(&section_header("Quota Preflight"));
                    print_wrapped_stderr(&format!(
                        "Warning: quota preflight failed for '{}': {err:#}",
                        profile_name
                    ));
                    print_wrapped_stderr("Continuing without quota gate.");
                }
            }
        }
    }

    record_run_selection(&mut state, &selected_profile_name);
    state.save(&paths)?;

    if state
        .profiles
        .get(&selected_profile_name)
        .with_context(|| format!("profile '{}' is missing", selected_profile_name))?
        .managed
    {
        prepare_managed_codex_home(&paths, &codex_home)?;
    }

    let runtime_upstream_base_url = quota_base_url(args.base_url.as_deref());
    let runtime_proxy = if should_enable_runtime_rotation_proxy(
        &state,
        &selected_profile_name,
        allow_auto_rotate,
    ) {
        let proxy = ensure_runtime_rotation_proxy_endpoint(
            &paths,
            &selected_profile_name,
            runtime_upstream_base_url.as_str(),
            include_code_review,
        )?;
        Some(proxy)
    } else {
        None
    };
    let runtime_args = runtime_proxy
        .as_ref()
        .map(|proxy| runtime_proxy_codex_args(proxy.listen_addr, &codex_args))
        .unwrap_or(codex_args);

    let status = run_child(&codex_bin(), &runtime_args, &codex_home)?;
    exit_with_status(status)
}

fn handle_runtime_broker(args: RuntimeBrokerArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let proxy = start_runtime_rotation_proxy(
        &paths,
        &state,
        &args.current_profile,
        args.upstream_base_url.clone(),
        args.include_code_review,
    )?;
    if proxy.owner_lock.is_none() {
        return Ok(());
    }

    let metadata = RuntimeBrokerMetadata {
        started_at: Local::now().timestamp(),
        current_profile: args.current_profile.clone(),
        include_code_review: args.include_code_review,
        instance_token: args.instance_token.clone(),
        admin_token: args.admin_token.clone(),
    };
    register_runtime_broker_metadata(&proxy.log_path, metadata.clone());
    let registry = RuntimeBrokerRegistry {
        pid: std::process::id(),
        listen_addr: proxy.listen_addr.to_string(),
        started_at: metadata.started_at,
        upstream_base_url: args.upstream_base_url.clone(),
        include_code_review: args.include_code_review,
        current_profile: args.current_profile.clone(),
        instance_token: args.instance_token.clone(),
        admin_token: args.admin_token.clone(),
    };
    save_runtime_broker_registry(&paths, &args.broker_key, &registry)?;
    runtime_proxy_log_to_path(
        &proxy.log_path,
        &format!(
            "runtime_broker_started listen_addr={} broker_key={} current_profile={} include_code_review={}",
            proxy.listen_addr, args.broker_key, args.current_profile, args.include_code_review
        ),
    );

    let startup_grace_until = metadata
        .started_at
        .saturating_add(runtime_broker_startup_grace_seconds());
    let mut idle_started_at = None::<i64>;
    loop {
        let live_leases = cleanup_runtime_broker_stale_leases(&paths, &args.broker_key);
        let active_requests = proxy.active_request_count.load(Ordering::SeqCst);
        if live_leases > 0 || active_requests > 0 {
            idle_started_at = None;
        } else {
            let now = Local::now().timestamp();
            if now < startup_grace_until {
                idle_started_at = None;
                thread::sleep(Duration::from_millis(RUNTIME_BROKER_POLL_INTERVAL_MS));
                continue;
            }
            let idle_since = idle_started_at.get_or_insert(now);
            if now.saturating_sub(*idle_since) >= RUNTIME_BROKER_IDLE_GRACE_SECONDS {
                runtime_proxy_log_to_path(
                    &proxy.log_path,
                    &format!(
                        "runtime_broker_idle_shutdown broker_key={} idle_seconds={}",
                        args.broker_key,
                        now.saturating_sub(*idle_since)
                    ),
                );
                break;
            }
        }
        thread::sleep(Duration::from_millis(RUNTIME_BROKER_POLL_INTERVAL_MS));
    }

    drop(proxy);
    remove_runtime_broker_registry_if_token_matches(&paths, &args.broker_key, &args.instance_token);
    Ok(())
}

fn normalize_run_codex_args(codex_args: &[OsString]) -> Vec<OsString> {
    let Some(first) = codex_args.first().and_then(|arg| arg.to_str()) else {
        return codex_args.to_vec();
    };
    if !looks_like_codex_session_id(first) {
        return codex_args.to_vec();
    }

    let mut normalized = Vec::with_capacity(codex_args.len() + 1);
    normalized.push(OsString::from("resume"));
    normalized.extend(codex_args.iter().cloned());
    normalized
}

fn looks_like_codex_session_id(value: &str) -> bool {
    let parts = value.split('-').collect::<Vec<_>>();
    if parts.len() != 5 {
        return false;
    }
    let expected_lengths = [8usize, 4, 4, 4, 12];
    parts.iter().zip(expected_lengths).all(|(part, expected)| {
        part.len() == expected && part.chars().all(|ch| ch.is_ascii_hexdigit())
    })
}

fn record_run_selection(state: &mut AppState, profile_name: &str) {
    state
        .last_run_selected_at
        .retain(|name, _| state.profiles.contains_key(name));
    state
        .last_run_selected_at
        .insert(profile_name.to_string(), Local::now().timestamp());
}

fn resolve_profile_name(state: &AppState, requested: Option<&str>) -> Result<String> {
    if let Some(name) = requested {
        if state.profiles.contains_key(name) {
            return Ok(name.to_string());
        }
        bail!("profile '{}' does not exist", name);
    }

    if let Some(active) = state.active_profile.as_deref() {
        if state.profiles.contains_key(active) {
            return Ok(active.to_string());
        }
        bail!("active profile '{}' no longer exists", active);
    }

    if state.profiles.len() == 1 {
        let (name, _) = state
            .profiles
            .iter()
            .next()
            .context("single profile lookup failed unexpectedly")?;
        return Ok(name.clone());
    }

    bail!("no active profile selected; use `prodex profile use <name>` or pass --profile")
}

fn ensure_path_is_unique(state: &AppState, candidate: &Path) -> Result<()> {
    for (name, profile) in &state.profiles {
        if same_path(&profile.codex_home, candidate) {
            bail!(
                "path {} is already used by profile '{}'",
                candidate.display(),
                name
            );
        }
    }
    Ok(())
}

fn validate_profile_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("profile name cannot be empty");
    }

    if name.contains(std::path::MAIN_SEPARATOR) {
        bail!("profile name cannot contain path separators");
    }

    if name == "." || name == ".." {
        bail!("profile name cannot be '.' or '..'");
    }

    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        bail!("profile name may only contain letters, numbers, '.', '_' or '-'");
    }

    Ok(())
}

fn should_enable_runtime_rotation_proxy(
    state: &AppState,
    selected_profile_name: &str,
    allow_auto_rotate: bool,
) -> bool {
    if !allow_auto_rotate || state.profiles.len() <= 1 {
        return false;
    }

    let Some(selected_profile) = state.profiles.get(selected_profile_name) else {
        return false;
    };
    if !read_auth_summary(&selected_profile.codex_home).quota_compatible {
        return false;
    }

    state
        .profiles
        .values()
        .filter(|profile| read_auth_summary(&profile.codex_home).quota_compatible)
        .take(2)
        .count()
        > 1
}

fn runtime_proxy_codex_args(
    listen_addr: std::net::SocketAddr,
    user_args: &[OsString],
) -> Vec<OsString> {
    let proxy_chatgpt_base = format!("http://{listen_addr}/backend-api");
    let proxy_openai_base = format!("http://{listen_addr}/backend-api/codex");
    let overrides = [
        format!(
            "chatgpt_base_url={}",
            toml_string_literal(&proxy_chatgpt_base)
        ),
        format!(
            "openai_base_url={}",
            toml_string_literal(&proxy_openai_base),
        ),
    ];

    let mut args = Vec::with_capacity((overrides.len() * 2) + user_args.len());
    for override_entry in overrides {
        args.push(OsString::from("-c"));
        args.push(OsString::from(override_entry));
    }
    args.extend(user_args.iter().cloned());
    args
}

fn start_runtime_rotation_proxy(
    paths: &AppPaths,
    state: &AppState,
    current_profile: &str,
    upstream_base_url: String,
    include_code_review: bool,
) -> Result<RuntimeRotationProxy> {
    let server = Arc::new(
        TinyServer::http("127.0.0.1:0")
            .map_err(|err| anyhow::anyhow!("failed to bind runtime auto-rotate proxy: {err}"))?,
    );
    let listen_addr = server
        .server_addr()
        .to_ip()
        .context("runtime auto-rotate proxy did not expose a TCP listen address")?;
    let log_path = initialize_runtime_proxy_log_path();
    let owner_lock = try_acquire_runtime_owner_lock(paths)?;
    let persistence_enabled = owner_lock.is_some();
    let async_runtime = Arc::new(
        TokioRuntimeBuilder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .context("failed to build runtime auto-rotate async runtime")?,
    );
    let worker_count = runtime_proxy_worker_count();
    let long_lived_worker_count = runtime_proxy_long_lived_worker_count();
    let long_lived_queue_capacity =
        runtime_proxy_long_lived_queue_capacity(long_lived_worker_count);
    let active_request_limit =
        runtime_proxy_active_request_limit(worker_count, long_lived_worker_count);
    let lane_admission = RuntimeProxyLaneAdmission::new(runtime_proxy_lane_limits(
        active_request_limit,
        worker_count,
        long_lived_worker_count,
    ));
    let persisted_state = AppState::load_with_recovery(paths).unwrap_or(RecoveredLoad {
        value: state.clone(),
        recovered_from_backup: false,
    });
    let mut restored_state = merge_runtime_state_snapshot(state.clone(), &persisted_state.value);
    let persisted_continuations =
        load_runtime_continuations_with_recovery(paths, &restored_state.profiles).unwrap_or(
            RecoveredLoad {
                value: RuntimeContinuationStore::default(),
                recovered_from_backup: false,
            },
        );
    let continuation_journal =
        load_runtime_continuation_journal_with_recovery(paths, &restored_state.profiles).unwrap_or(
            RecoveredLoad {
                value: RuntimeContinuationJournal::default(),
                recovered_from_backup: false,
            },
        );
    let fallback_continuations = runtime_continuation_store_from_app_state(&restored_state);
    let restored_continuations = merge_runtime_continuation_store(
        &merge_runtime_continuation_store(
            &fallback_continuations,
            &persisted_continuations.value,
            &restored_state.profiles,
        ),
        &continuation_journal.value.continuations,
        &restored_state.profiles,
    );
    let continuation_sidecar_present = runtime_continuations_file_path(paths).exists()
        || runtime_continuations_last_good_file_path(paths).exists();
    let continuation_migration_needed = !continuation_sidecar_present
        && (restored_continuations != RuntimeContinuationStore::default());
    let restored_session_id_bindings = merge_profile_bindings(
        &restored_continuations.session_profile_bindings,
        &runtime_external_session_id_bindings(&restored_continuations.session_id_bindings),
        &restored_state.profiles,
    );
    restored_state.response_profile_bindings =
        restored_continuations.response_profile_bindings.clone();
    restored_state.session_profile_bindings = restored_session_id_bindings.clone();
    let persisted_profile_scores =
        load_runtime_profile_scores_with_recovery(paths, &restored_state.profiles).unwrap_or(
            RecoveredLoad {
                value: BTreeMap::new(),
                recovered_from_backup: false,
            },
        );
    let persisted_usage_snapshots =
        load_runtime_usage_snapshots_with_recovery(paths, &restored_state.profiles).unwrap_or(
            RecoveredLoad {
                value: BTreeMap::new(),
                recovered_from_backup: false,
            },
        );
    let persisted_backoffs =
        load_runtime_profile_backoffs_with_recovery(paths, &restored_state.profiles).unwrap_or(
            RecoveredLoad {
                value: RuntimeProfileBackoffs::default(),
                recovered_from_backup: false,
            },
        );
    let persisted_profile_scores_count = persisted_profile_scores.value.len();
    let persisted_usage_snapshots_count = persisted_usage_snapshots.value.len();
    let persisted_response_binding_count = restored_continuations.response_profile_bindings.len();
    let persisted_session_binding_count = restored_continuations.session_profile_bindings.len();
    let persisted_turn_state_binding_count = restored_continuations.turn_state_bindings.len();
    let persisted_session_id_binding_count = restored_session_id_bindings.len();
    let persisted_retry_backoffs_count = persisted_backoffs.value.retry_backoff_until.len();
    let persisted_transport_backoffs_count = persisted_backoffs.value.transport_backoff_until.len();
    let persisted_route_circuit_count = persisted_backoffs.value.route_circuit_open_until.len();
    let expired_usage_snapshot_count = persisted_usage_snapshots
        .value
        .values()
        .filter(|snapshot| !runtime_usage_snapshot_is_usable(snapshot, Local::now().timestamp()))
        .count();
    let restored_global_scores_count = persisted_profile_scores
        .value
        .keys()
        .filter(|key| !key.starts_with("__route_"))
        .count();
    let restored_route_scores_count = persisted_profile_scores
        .value
        .keys()
        .filter(|key| key.starts_with("__route_health__"))
        .count();
    let restored_bad_pairing_count = persisted_profile_scores
        .value
        .keys()
        .filter(|key| key.starts_with("__route_bad_pairing__"))
        .count();
    let restored_success_streak_count = persisted_profile_scores
        .value
        .keys()
        .filter(|key| key.starts_with("__route_success__"))
        .count();
    let shared = RuntimeRotationProxyShared {
        async_client: reqwest::Client::builder()
            .connect_timeout(Duration::from_millis(
                runtime_proxy_http_connect_timeout_ms(),
            ))
            .read_timeout(Duration::from_millis(runtime_proxy_stream_idle_timeout_ms()))
            .build()
            .context("failed to build runtime auto-rotate async HTTP client")?,
        async_runtime,
        log_path: log_path.clone(),
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit,
        lane_admission: lane_admission.clone(),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths: paths.clone(),
            state: restored_state.clone(),
            upstream_base_url: upstream_base_url.clone(),
            include_code_review,
            current_profile: current_profile.to_string(),
            turn_state_bindings: restored_continuations.turn_state_bindings.clone(),
            session_id_bindings: restored_session_id_bindings,
            continuation_statuses: restored_continuations.statuses.clone(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: persisted_usage_snapshots.value,
            profile_retry_backoff_until: persisted_backoffs.value.retry_backoff_until,
            profile_transport_backoff_until: persisted_backoffs.value.transport_backoff_until,
            profile_route_circuit_open_until: persisted_backoffs.value.route_circuit_open_until,
            profile_inflight: BTreeMap::new(),
            profile_health: persisted_profile_scores.value,
        })),
    };
    register_runtime_proxy_persistence_mode(&log_path, persistence_enabled);
    runtime_proxy_log_to_path(
        &log_path,
        &format!(
            "runtime proxy started listen_addr={listen_addr} current_profile={current_profile} include_code_review={include_code_review} upstream_base_url={upstream_base_url} persistence_mode={}",
            if persistence_enabled {
                "owner"
            } else {
                "follower"
            }
        ),
    );
    runtime_proxy_log_to_path(
        &log_path,
        &format!(
            "runtime_proxy_restore_counts persisted_scores={} persisted_usage_snapshots={} expired_usage_snapshots={} response_bindings={} session_bindings={} turn_state_bindings={} session_id_bindings={} retry_backoffs={} transport_backoffs={} route_circuits={} global_scores={} route_scores={} bad_pairing_scores={} success_streak_scores={} recovered_state={} recovered_continuations={} recovered_scores={} recovered_usage_snapshots={} recovered_backoffs={} recovered_continuation_journal={}",
            persisted_profile_scores_count,
            persisted_usage_snapshots_count,
            expired_usage_snapshot_count,
            persisted_response_binding_count,
            persisted_session_binding_count,
            persisted_turn_state_binding_count,
            persisted_session_id_binding_count,
            persisted_retry_backoffs_count,
            persisted_transport_backoffs_count,
            persisted_route_circuit_count,
            restored_global_scores_count,
            restored_route_scores_count,
            restored_bad_pairing_count,
            restored_success_streak_count,
            persisted_state.recovered_from_backup,
            persisted_continuations.recovered_from_backup,
            persisted_profile_scores.recovered_from_backup,
            persisted_usage_snapshots.recovered_from_backup,
            persisted_backoffs.recovered_from_backup,
            continuation_journal.recovered_from_backup,
        ),
    );
    audit_runtime_proxy_startup_state(&shared);
    schedule_runtime_startup_probe_warmup(&shared);
    if continuation_migration_needed && let Ok(runtime) = shared.runtime.lock() {
        schedule_runtime_state_save(
            &shared,
            runtime.state.clone(),
            runtime_continuation_store_snapshot(&runtime),
            runtime.profile_health.clone(),
            runtime.profile_usage_snapshots.clone(),
            runtime_profile_backoffs_snapshot(&runtime),
            runtime.paths.clone(),
            "startup_continuation_migration",
        );
    }
    let shutdown = Arc::new(AtomicBool::new(false));
    let mut worker_threads = Vec::new();
    let (long_lived_sender, long_lived_receiver) =
        mpsc::sync_channel::<tiny_http::Request>(long_lived_queue_capacity);
    let long_lived_receiver = Arc::new(Mutex::new(long_lived_receiver));
    runtime_proxy_log_to_path(
        &log_path,
        &format!(
            "runtime proxy worker_count={worker_count} long_lived_worker_count={long_lived_worker_count} long_lived_queue_capacity={long_lived_queue_capacity} active_request_limit={active_request_limit} lane_limits=responses:{} compact:{} websocket:{} standard:{}",
            lane_admission.limits.responses,
            lane_admission.limits.compact,
            lane_admission.limits.websocket,
            lane_admission.limits.standard
        ),
    );

    for _ in 0..long_lived_worker_count {
        let shutdown = Arc::clone(&shutdown);
        let shared = shared.clone();
        let receiver = Arc::clone(&long_lived_receiver);
        worker_threads.push(thread::spawn(move || {
            while !shutdown.load(Ordering::SeqCst) {
                let request = {
                    let guard = receiver.lock();
                    let Ok(receiver) = guard else {
                        break;
                    };
                    receiver.recv_timeout(Duration::from_millis(200))
                };
                match request {
                    Ok(request) => handle_runtime_rotation_proxy_request(request, &shared),
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        }));
    }

    for _ in 0..worker_count {
        let server: Arc<TinyServer> = Arc::clone(&server);
        let shutdown = Arc::clone(&shutdown);
        let shared = shared.clone();
        let long_lived_sender = long_lived_sender.clone();
        worker_threads.push(thread::spawn(move || {
            while !shutdown.load(Ordering::SeqCst) {
                match server.recv_timeout(Duration::from_millis(200)) {
                    Ok(Some(request)) => {
                        let long_lived = is_tiny_http_websocket_upgrade(&request)
                            || is_runtime_responses_path(request.url());
                        if long_lived {
                            match enqueue_runtime_proxy_long_lived_request_with_wait(
                                &long_lived_sender,
                                request,
                                &shared,
                            ) {
                                Ok(()) => {}
                                Err((RuntimeProxyQueueRejection::Full, request)) => {
                                    mark_runtime_proxy_local_overload(
                                        &shared,
                                        "long_lived_queue_full",
                                    );
                                    reject_runtime_proxy_overloaded_request(
                                        request,
                                        &shared,
                                        "long_lived_queue_full",
                                    );
                                }
                                Err((RuntimeProxyQueueRejection::Disconnected, request)) => {
                                    mark_runtime_proxy_local_overload(
                                        &shared,
                                        "long_lived_queue_disconnected",
                                    );
                                    reject_runtime_proxy_overloaded_request(
                                        request,
                                        &shared,
                                        "long_lived_queue_disconnected",
                                    );
                                }
                            }
                        } else {
                            handle_runtime_rotation_proxy_request(request, &shared);
                        }
                    }
                    Ok(None) => {}
                    Err(_) if shutdown.load(Ordering::SeqCst) => break,
                    Err(_) => {}
                }
            }
        }));
    }

    Ok(RuntimeRotationProxy {
        server,
        shutdown,
        worker_threads,
        accept_worker_count: worker_count,
        listen_addr,
        log_path,
        active_request_count: Arc::clone(&shared.active_request_count),
        owner_lock,
    })
}

impl Drop for RuntimeRotationProxy {
    fn drop(&mut self) {
        unregister_runtime_proxy_persistence_mode(&self.log_path);
        unregister_runtime_broker_metadata(&self.log_path);
        self.shutdown.store(true, Ordering::SeqCst);
        for _ in 0..self.accept_worker_count {
            self.server.unblock();
        }
        for worker in self.worker_threads.drain(..) {
            let _ = worker.join();
        }
        let _ = self.owner_lock.take();
    }
}

fn reject_runtime_proxy_overloaded_request(
    request: tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
    reason: &str,
) {
    let path = request.url().to_string();
    let websocket = is_tiny_http_websocket_upgrade(&request);
    runtime_proxy_log(
        shared,
        format!(
            "runtime_proxy_queue_overloaded transport={} path={} reason={reason}",
            if websocket { "websocket" } else { "http" },
            path
        ),
    );
    let response = if websocket {
        build_runtime_proxy_text_response(
            503,
            "Runtime auto-rotate proxy is temporarily saturated. Retry the request.",
        )
    } else if is_runtime_responses_path(&path) || is_runtime_compact_path(&path) {
        build_runtime_proxy_json_error_response(
            503,
            "service_unavailable",
            "Runtime auto-rotate proxy is temporarily saturated. Retry the request.",
        )
    } else {
        build_runtime_proxy_text_response(
            503,
            "Runtime auto-rotate proxy is temporarily saturated. Retry the request.",
        )
    };
    let _ = request.respond(response);
}

fn runtime_proxy_admin_token(request: &tiny_http::Request) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|header| header.field.equiv("X-Prodex-Admin-Token"))
        .map(|header| header.value.as_str().trim().to_string())
        .filter(|value| !value.is_empty())
}

fn build_runtime_proxy_json_response(status: u16, body: String) -> tiny_http::ResponseBox {
    let mut response = TinyResponse::from_string(body).with_status_code(status);
    if let Ok(header) = TinyHeader::from_bytes("Content-Type", "application/json") {
        response = response.with_header(header);
    }
    response.boxed()
}

fn update_runtime_broker_current_profile(log_path: &Path, current_profile: &str) {
    let mut metadata_by_path = runtime_broker_metadata_by_log_path()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(metadata) = metadata_by_path.get_mut(log_path) {
        metadata.current_profile = current_profile.to_string();
    }
}

fn handle_runtime_proxy_admin_request(
    request: &mut tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
) -> Option<tiny_http::ResponseBox> {
    let path = path_without_query(request.url());
    if path != "/__prodex/runtime/health" && path != "/__prodex/runtime/activate" {
        return None;
    }

    let Some(metadata) = runtime_broker_metadata_for_log_path(&shared.log_path) else {
        return Some(build_runtime_proxy_json_error_response(
            404,
            "not_found",
            "runtime broker admin endpoint is not enabled for this proxy",
        ));
    };
    if runtime_proxy_admin_token(request).as_deref() != Some(metadata.admin_token.as_str()) {
        return Some(build_runtime_proxy_json_error_response(
            403,
            "forbidden",
            "missing or invalid runtime broker admin token",
        ));
    }

    if path == "/__prodex/runtime/health" {
        let health = RuntimeBrokerHealth {
            pid: std::process::id(),
            started_at: metadata.started_at,
            current_profile: metadata.current_profile,
            include_code_review: metadata.include_code_review,
            active_requests: shared.active_request_count.load(Ordering::SeqCst),
            instance_token: metadata.instance_token,
            persistence_role: if runtime_proxy_persistence_enabled(shared) {
                "owner".to_string()
            } else {
                "follower".to_string()
            },
        };
        let body = serde_json::to_string(&health).ok()?;
        return Some(build_runtime_proxy_json_response(200, body));
    }

    if request.method().as_str() != "POST" {
        return Some(build_runtime_proxy_json_error_response(
            405,
            "method_not_allowed",
            "runtime broker activation requires POST",
        ));
    }

    let mut body = Vec::new();
    if let Err(err) = request.as_reader().read_to_end(&mut body) {
        return Some(build_runtime_proxy_json_error_response(
            400,
            "invalid_request",
            &format!("failed to read runtime broker activation body: {err}"),
        ));
    }
    let current_profile = match serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|value| {
            value
                .get("current_profile")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .map(str::to_string)
        })
        .filter(|value| !value.is_empty())
    {
        Some(current_profile) => current_profile,
        None => {
            return Some(build_runtime_proxy_json_error_response(
                400,
                "invalid_request",
                "runtime broker activation requires a non-empty current_profile",
            ));
        }
    };

    let update_result = (|| -> Result<()> {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        runtime.current_profile = current_profile.clone();
        runtime.state.active_profile = Some(current_profile.clone());
        Ok(())
    })();
    if let Err(err) = update_result {
        return Some(build_runtime_proxy_json_error_response(
            500,
            "internal_error",
            &err.to_string(),
        ));
    }
    update_runtime_broker_current_profile(&shared.log_path, &current_profile);
    runtime_proxy_log(
        shared,
        format!("runtime_broker_activate current_profile={current_profile}"),
    );
    Some(build_runtime_proxy_json_response(
        200,
        serde_json::json!({
            "ok": true,
            "current_profile": current_profile,
        })
        .to_string(),
    ))
}

fn mark_runtime_proxy_local_overload(shared: &RuntimeRotationProxyShared, reason: &str) {
    let now = Local::now().timestamp().max(0) as u64;
    let until = now.saturating_add(RUNTIME_PROXY_LOCAL_OVERLOAD_BACKOFF_SECONDS.max(1) as u64);
    let current = shared.local_overload_backoff_until.load(Ordering::SeqCst);
    if until > current {
        shared
            .local_overload_backoff_until
            .store(until, Ordering::SeqCst);
    }
    runtime_proxy_log(
        shared,
        format!("runtime_proxy_overload_backoff until={until} reason={reason}"),
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeProxyAdmissionRejection {
    GlobalLimit,
    LaneLimit(RuntimeRouteKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeProxyQueueRejection {
    Full,
    Disconnected,
}

fn runtime_proxy_request_lane(path: &str, websocket: bool) -> RuntimeRouteKind {
    if websocket {
        RuntimeRouteKind::Websocket
    } else if is_runtime_compact_path(path) {
        RuntimeRouteKind::Compact
    } else if is_runtime_responses_path(path) {
        RuntimeRouteKind::Responses
    } else {
        RuntimeRouteKind::Standard
    }
}

fn runtime_proxy_pressure_mode_active(shared: &RuntimeRotationProxyShared) -> bool {
    let now = Local::now().timestamp().max(0) as u64;
    shared.local_overload_backoff_until.load(Ordering::SeqCst) > now
        || runtime_proxy_queue_pressure_active(
            runtime_state_save_queue_backlog(),
            runtime_continuation_journal_queue_backlog(),
            runtime_probe_refresh_queue_backlog(),
        )
}

fn runtime_proxy_should_shed_fresh_compact_request(
    pressure_mode: bool,
    session_profile: Option<&str>,
) -> bool {
    pressure_mode && session_profile.is_none()
}

fn audit_runtime_proxy_startup_state(shared: &RuntimeRotationProxyShared) {
    let Ok(mut runtime) = shared.runtime.lock() else {
        return;
    };
    let now = Local::now().timestamp();
    let orphan_managed_dirs = collect_orphan_managed_profile_dirs(&runtime.paths, &runtime.state);
    let missing_managed_dirs = runtime
        .state
        .profiles
        .values()
        .filter(|profile| profile.managed && !profile.codex_home.exists())
        .count();
    let valid_profiles = runtime
        .state
        .profiles
        .iter()
        .filter(|(_, profile)| !profile.managed || profile.codex_home.exists())
        .map(|(name, _)| name.clone())
        .collect::<BTreeSet<_>>();
    let stale_response_bindings = runtime
        .state
        .response_profile_bindings
        .values()
        .filter(|binding| !valid_profiles.contains(&binding.profile_name))
        .count();
    let stale_session_bindings = runtime
        .state
        .session_profile_bindings
        .values()
        .filter(|binding| !valid_profiles.contains(&binding.profile_name))
        .count();
    let stale_probe_cache = runtime
        .profile_probe_cache
        .keys()
        .filter(|profile_name| !valid_profiles.contains(*profile_name))
        .count();
    let stale_usage_snapshots = runtime
        .profile_usage_snapshots
        .keys()
        .filter(|profile_name| !valid_profiles.contains(*profile_name))
        .count();
    let stale_retry_backoffs = runtime
        .profile_retry_backoff_until
        .keys()
        .filter(|profile_name| !valid_profiles.contains(*profile_name))
        .count();
    let stale_transport_backoffs = runtime
        .profile_transport_backoff_until
        .keys()
        .filter(|profile_name| !valid_profiles.contains(*profile_name))
        .count();
    let stale_route_circuits = runtime
        .profile_route_circuit_open_until
        .keys()
        .filter(|key| !valid_profiles.contains(runtime_profile_route_circuit_profile_name(key)))
        .count();
    let stale_health_scores = runtime
        .profile_health
        .keys()
        .filter(|key| !valid_profiles.contains(runtime_profile_score_profile_name(key)))
        .count();
    let active_profile_missing_dir = runtime
        .state
        .active_profile
        .as_deref()
        .and_then(|name| runtime.state.profiles.get(name))
        .is_some_and(|profile| profile.managed && !profile.codex_home.exists());

    runtime
        .state
        .response_profile_bindings
        .retain(|_, binding| valid_profiles.contains(&binding.profile_name));
    runtime
        .state
        .session_profile_bindings
        .retain(|_, binding| valid_profiles.contains(&binding.profile_name));
    runtime
        .turn_state_bindings
        .retain(|_, binding| valid_profiles.contains(&binding.profile_name));
    runtime
        .session_id_bindings
        .retain(|_, binding| valid_profiles.contains(&binding.profile_name));
    runtime
        .profile_probe_cache
        .retain(|profile_name, _| valid_profiles.contains(profile_name));
    runtime
        .profile_usage_snapshots
        .retain(|profile_name, _| valid_profiles.contains(profile_name));
    runtime
        .profile_retry_backoff_until
        .retain(|profile_name, _| valid_profiles.contains(profile_name));
    runtime
        .profile_transport_backoff_until
        .retain(|profile_name, _| valid_profiles.contains(profile_name));
    runtime
        .profile_route_circuit_open_until
        .retain(|key, _| valid_profiles.contains(runtime_profile_route_circuit_profile_name(key)));
    runtime
        .profile_health
        .retain(|key, _| valid_profiles.contains(runtime_profile_score_profile_name(key)));
    let route_circuit_count_after_profile_prune = runtime.profile_route_circuit_open_until.len();
    prune_runtime_profile_route_circuits(&mut runtime, now);
    let expired_route_circuits = route_circuit_count_after_profile_prune
        .saturating_sub(runtime.profile_route_circuit_open_until.len());
    let state_snapshot = runtime.state.clone();
    let continuations_snapshot = runtime_continuation_store_snapshot(&runtime);
    let profile_scores_snapshot = runtime.profile_health.clone();
    let usage_snapshots = runtime.profile_usage_snapshots.clone();
    let backoffs_snapshot = runtime_profile_backoffs_snapshot(&runtime);
    let paths_snapshot = runtime.paths.clone();
    let changed = stale_response_bindings > 0
        || stale_session_bindings > 0
        || stale_probe_cache > 0
        || stale_usage_snapshots > 0
        || stale_retry_backoffs > 0
        || stale_transport_backoffs > 0
        || stale_route_circuits > 0
        || expired_route_circuits > 0
        || stale_health_scores > 0;
    drop(runtime);

    runtime_proxy_log(
        shared,
        format!(
            "runtime_proxy_startup_audit missing_managed_dirs={missing_managed_dirs} orphan_managed_dirs={} stale_response_bindings={stale_response_bindings} stale_session_bindings={stale_session_bindings} stale_probe_cache={stale_probe_cache} stale_usage_snapshots={stale_usage_snapshots} stale_retry_backoffs={stale_retry_backoffs} stale_transport_backoffs={stale_transport_backoffs} stale_route_circuits={stale_route_circuits} expired_route_circuits={expired_route_circuits} stale_health_scores={stale_health_scores} active_profile_missing_dir={active_profile_missing_dir}",
            orphan_managed_dirs.len(),
        ),
    );
    if changed {
        schedule_runtime_state_save(
            shared,
            state_snapshot,
            continuations_snapshot,
            profile_scores_snapshot,
            usage_snapshots,
            backoffs_snapshot,
            paths_snapshot,
            "startup_audit",
        );
    }
}

fn try_acquire_runtime_proxy_active_request_slot(
    shared: &RuntimeRotationProxyShared,
    transport: &str,
    path: &str,
) -> Result<RuntimeProxyActiveRequestGuard, RuntimeProxyAdmissionRejection> {
    let lane = runtime_proxy_request_lane(path, transport == "websocket");
    let lane_active_count = shared.lane_admission.active_counter(lane);
    let lane_limit = shared.lane_admission.limit(lane);
    loop {
        let active = shared.active_request_count.load(Ordering::SeqCst);
        if active >= shared.active_request_limit {
            runtime_proxy_log(
                shared,
                format!(
                    "runtime_proxy_active_limit_reached transport={transport} path={path} active={active} limit={}",
                    shared.active_request_limit
                ),
            );
            return Err(RuntimeProxyAdmissionRejection::GlobalLimit);
        }
        let lane_active = lane_active_count.load(Ordering::SeqCst);
        if lane_active >= lane_limit {
            runtime_proxy_log(
                shared,
                format!(
                    "runtime_proxy_lane_limit_reached transport={transport} path={path} lane={} active={lane_active} limit={lane_limit}",
                    runtime_route_kind_label(lane)
                ),
            );
            return Err(RuntimeProxyAdmissionRejection::LaneLimit(lane));
        }
        if shared
            .active_request_count
            .compare_exchange(
                active,
                active.saturating_add(1),
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_ok()
        {
            if lane_active_count
                .compare_exchange(
                    lane_active,
                    lane_active.saturating_add(1),
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                )
                .is_ok()
            {
                return Ok(RuntimeProxyActiveRequestGuard {
                    active_request_count: Arc::clone(&shared.active_request_count),
                    lane_active_count,
                });
            }
            shared.active_request_count.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

fn acquire_runtime_proxy_active_request_slot_with_wait(
    shared: &RuntimeRotationProxyShared,
    transport: &str,
    path: &str,
) -> Result<RuntimeProxyActiveRequestGuard, RuntimeProxyAdmissionRejection> {
    let started_at = Instant::now();
    let pressure_mode = runtime_proxy_pressure_mode_active(shared);
    let budget = Duration::from_millis(if pressure_mode {
        runtime_proxy_pressure_admission_wait_budget_ms()
    } else {
        runtime_proxy_admission_wait_budget_ms()
    });
    let poll = Duration::from_millis(runtime_proxy_admission_wait_poll_ms().max(1));
    let mut waited = false;
    loop {
        match try_acquire_runtime_proxy_active_request_slot(shared, transport, path) {
            Ok(guard) => {
                if waited {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "runtime_proxy_admission_recovered transport={transport} path={path} waited_ms={}",
                            started_at.elapsed().as_millis()
                        ),
                    );
                }
                return Ok(guard);
            }
            Err(rejection) => {
                let elapsed = started_at.elapsed();
                if elapsed >= budget {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "runtime_proxy_admission_wait_exhausted transport={transport} path={path} waited_ms={} reason={} pressure_mode={pressure_mode}",
                            elapsed.as_millis(),
                            match rejection {
                                RuntimeProxyAdmissionRejection::GlobalLimit =>
                                    "active_request_limit",
                                RuntimeProxyAdmissionRejection::LaneLimit(lane) =>
                                    runtime_route_kind_label(lane),
                            }
                        ),
                    );
                    return Err(rejection);
                }
                if !waited {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "runtime_proxy_admission_wait_started transport={transport} path={path} budget_ms={} poll_ms={} reason={} pressure_mode={pressure_mode}",
                            budget.as_millis(),
                            poll.as_millis(),
                            match rejection {
                                RuntimeProxyAdmissionRejection::GlobalLimit =>
                                    "active_request_limit",
                                RuntimeProxyAdmissionRejection::LaneLimit(lane) =>
                                    runtime_route_kind_label(lane),
                            }
                        ),
                    );
                }
                waited = true;
                thread::sleep(poll.min(budget.saturating_sub(elapsed)));
            }
        }
    }
}

fn wait_for_runtime_proxy_queue_capacity<T, F>(
    mut item: T,
    shared: &RuntimeRotationProxyShared,
    transport: &str,
    path: &str,
    mut try_enqueue: F,
) -> Result<(), (RuntimeProxyQueueRejection, T)>
where
    F: FnMut(T) -> Result<(), (RuntimeProxyQueueRejection, T)>,
{
    let started_at = Instant::now();
    let pressure_mode = runtime_proxy_pressure_mode_active(shared);
    let budget = Duration::from_millis(if pressure_mode {
        runtime_proxy_pressure_long_lived_queue_wait_budget_ms()
    } else {
        runtime_proxy_long_lived_queue_wait_budget_ms()
    });
    let poll = Duration::from_millis(runtime_proxy_long_lived_queue_wait_poll_ms().max(1));
    let mut waited = false;
    loop {
        match try_enqueue(item) {
            Ok(()) => {
                if waited {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "runtime_proxy_queue_recovered transport={transport} path={path} waited_ms={}",
                            started_at.elapsed().as_millis()
                        ),
                    );
                }
                return Ok(());
            }
            Err((RuntimeProxyQueueRejection::Full, returned_item)) => {
                item = returned_item;
                let elapsed = started_at.elapsed();
                if elapsed >= budget {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "runtime_proxy_queue_wait_exhausted transport={transport} path={path} waited_ms={} reason=long_lived_queue_full pressure_mode={pressure_mode}",
                            elapsed.as_millis()
                        ),
                    );
                    return Err((RuntimeProxyQueueRejection::Full, item));
                }
                if !waited {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "runtime_proxy_queue_wait_started transport={transport} path={path} budget_ms={} poll_ms={} reason=long_lived_queue_full pressure_mode={pressure_mode}",
                            budget.as_millis(),
                            poll.as_millis()
                        ),
                    );
                }
                waited = true;
                thread::sleep(poll.min(budget.saturating_sub(elapsed)));
            }
            Err((RuntimeProxyQueueRejection::Disconnected, returned_item)) => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "runtime_proxy_queue_wait_exhausted transport={transport} path={path} waited_ms={} reason=long_lived_queue_disconnected",
                        started_at.elapsed().as_millis()
                    ),
                );
                return Err((RuntimeProxyQueueRejection::Disconnected, returned_item));
            }
        }
    }
}

fn enqueue_runtime_proxy_long_lived_request_with_wait(
    sender: &mpsc::SyncSender<tiny_http::Request>,
    request: tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
) -> Result<(), (RuntimeProxyQueueRejection, tiny_http::Request)> {
    let path = request.url().to_string();
    let transport = if is_tiny_http_websocket_upgrade(&request) {
        "websocket"
    } else {
        "http"
    };
    wait_for_runtime_proxy_queue_capacity(request, shared, transport, &path, |request| match sender
        .try_send(request)
    {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(returned_request)) => {
            Err((RuntimeProxyQueueRejection::Full, returned_request))
        }
        Err(TrySendError::Disconnected(returned_request)) => {
            Err((RuntimeProxyQueueRejection::Disconnected, returned_request))
        }
    })
}

fn handle_runtime_rotation_proxy_request(
    mut request: tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
) {
    if let Some(response) = handle_runtime_proxy_admin_request(&mut request, shared) {
        let _ = request.respond(response);
        return;
    }
    let request_path = request.url().to_string();
    let request_transport = if is_tiny_http_websocket_upgrade(&request) {
        "websocket"
    } else {
        "http"
    };
    let _active_request_guard = match acquire_runtime_proxy_active_request_slot_with_wait(
        shared,
        request_transport,
        &request_path,
    ) {
        Ok(guard) => guard,
        Err(RuntimeProxyAdmissionRejection::GlobalLimit) => {
            mark_runtime_proxy_local_overload(shared, "active_request_limit");
            reject_runtime_proxy_overloaded_request(request, shared, "active_request_limit");
            return;
        }
        Err(RuntimeProxyAdmissionRejection::LaneLimit(lane)) => {
            let reason = format!("lane_limit:{}", runtime_route_kind_label(lane));
            mark_runtime_proxy_local_overload(shared, &reason);
            reject_runtime_proxy_overloaded_request(request, shared, &reason);
            return;
        }
    };
    let request_id = runtime_proxy_next_request_id(shared);
    if is_tiny_http_websocket_upgrade(&request) {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket upgrade path={}",
                request.url()
            ),
        );
        proxy_runtime_responses_websocket_request(request_id, request, shared);
        return;
    }

    let captured = match capture_runtime_proxy_request(&mut request) {
        Ok(captured) => captured,
        Err(err) => {
            runtime_proxy_log(
                shared,
                format!("request={request_id} transport=http capture_error={err}"),
            );
            let _ = request.respond(build_runtime_proxy_text_response(502, &err.to_string()));
            return;
        }
    };
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http path={} previous_response_id={:?} turn_state={:?} body_bytes={}",
            captured.path_and_query,
            runtime_request_previous_response_id(&captured),
            runtime_request_turn_state(&captured),
            captured.body.len()
        ),
    );

    if is_runtime_responses_path(&captured.path_and_query) {
        let response = match proxy_runtime_responses_request(request_id, &captured, shared) {
            Ok(response) => response,
            Err(err) => {
                if is_runtime_proxy_transport_failure(&err) {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http responses_transport_failure={err:#}"
                        ),
                    );
                    return;
                } else {
                    runtime_proxy_log(
                        shared,
                        format!("request={request_id} transport=http responses_error={err:#}"),
                    );
                    RuntimeResponsesReply::Buffered(build_runtime_proxy_text_response(
                        502,
                        &err.to_string(),
                    ))
                }
            }
        };
        match response {
            RuntimeResponsesReply::Buffered(response) => {
                let _ = request.respond(response);
            }
            RuntimeResponsesReply::Streaming(response) => {
                let writer = request.into_writer();
                let _ = write_runtime_streaming_response(writer, response);
            }
        }
        return;
    }

    let response = match proxy_runtime_standard_request(request_id, &captured, shared) {
        Ok(response) => response,
        Err(err) => {
            if is_runtime_proxy_transport_failure(&err) {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http standard_transport_failure={err:#}"
                    ),
                );
                return;
            } else {
                runtime_proxy_log(
                    shared,
                    format!("request={request_id} transport=http standard_error={err:#}"),
                );
                build_runtime_proxy_text_response(502, &err.to_string())
            }
        }
    };
    let _ = request.respond(response);
}

fn is_tiny_http_websocket_upgrade(request: &tiny_http::Request) -> bool {
    request.headers().iter().any(|header| {
        header.field.equiv("Upgrade") && header.value.as_str().eq_ignore_ascii_case("websocket")
    })
}

fn capture_runtime_proxy_request(request: &mut tiny_http::Request) -> Result<RuntimeProxyRequest> {
    let mut body = Vec::new();
    request
        .as_reader()
        .read_to_end(&mut body)
        .context("failed to read proxied Codex request body")?;

    Ok(RuntimeProxyRequest {
        method: request.method().as_str().to_string(),
        path_and_query: request.url().to_string(),
        headers: runtime_proxy_request_headers(request),
        body,
    })
}

fn capture_runtime_proxy_websocket_request(request: &tiny_http::Request) -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: request.method().as_str().to_string(),
        path_and_query: request.url().to_string(),
        headers: runtime_proxy_request_headers(request),
        body: Vec::new(),
    }
}

fn runtime_proxy_request_headers(request: &tiny_http::Request) -> Vec<(String, String)> {
    request
        .headers()
        .iter()
        .map(|header| {
            (
                header.field.as_str().as_str().to_string(),
                header.value.as_str().to_string(),
            )
        })
        .collect()
}

fn runtime_request_previous_response_id(request: &RuntimeProxyRequest) -> Option<String> {
    runtime_request_previous_response_id_from_bytes(&request.body)
}

fn runtime_request_previous_response_id_from_bytes(body: &[u8]) -> Option<String> {
    if body.is_empty() {
        return None;
    }

    let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    runtime_request_previous_response_id_from_value(&value)
}

fn runtime_request_previous_response_id_from_text(request_text: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(request_text).ok()?;
    runtime_request_previous_response_id_from_value(&value)
}

fn runtime_request_previous_response_id_from_value(value: &serde_json::Value) -> Option<String> {
    value
        .get("previous_response_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn runtime_request_without_previous_response_id(
    request: &RuntimeProxyRequest,
) -> Option<RuntimeProxyRequest> {
    let mut value = serde_json::from_slice::<serde_json::Value>(&request.body).ok()?;
    let object = value.as_object_mut()?;
    let removed = object.remove("previous_response_id")?;
    if removed.as_str().map(str::trim).is_none_or(str::is_empty) {
        return None;
    }
    let body = serde_json::to_vec(&value).ok()?;
    Some(RuntimeProxyRequest {
        method: request.method.clone(),
        path_and_query: request.path_and_query.clone(),
        headers: request.headers.clone(),
        body,
    })
}

fn runtime_request_without_turn_state_header(request: &RuntimeProxyRequest) -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: request.method.clone(),
        path_and_query: request.path_and_query.clone(),
        headers: request
            .headers
            .iter()
            .filter(|(name, _)| !name.eq_ignore_ascii_case("x-codex-turn-state"))
            .cloned()
            .collect(),
        body: request.body.clone(),
    }
}

fn runtime_request_without_previous_response_affinity(
    request: &RuntimeProxyRequest,
) -> Option<RuntimeProxyRequest> {
    let request = runtime_request_without_previous_response_id(request)?;
    Some(runtime_request_without_turn_state_header(&request))
}

fn runtime_request_value_requires_previous_response_affinity(value: &serde_json::Value) -> bool {
    value
        .get("input")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                let Some(object) = item.as_object() else {
                    return false;
                };
                let item_type = object
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                let has_call_id = object
                    .get("call_id")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|call_id| !call_id.trim().is_empty());
                has_call_id && item_type.ends_with("_call_output")
            })
        })
}

fn runtime_request_requires_previous_response_affinity(request: &RuntimeProxyRequest) -> bool {
    serde_json::from_slice::<serde_json::Value>(&request.body)
        .map(|value| runtime_request_value_requires_previous_response_affinity(&value))
        .unwrap_or(false)
}

fn runtime_request_text_without_previous_response_id(request_text: &str) -> Option<String> {
    let mut value = serde_json::from_str::<serde_json::Value>(request_text).ok()?;
    let object = value.as_object_mut()?;
    let removed = object.remove("previous_response_id")?;
    if removed.as_str().map(str::trim).is_none_or(str::is_empty) {
        return None;
    }
    serde_json::to_string(&value).ok()
}

fn runtime_request_text_requires_previous_response_affinity(request_text: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(request_text)
        .map(|value| runtime_request_value_requires_previous_response_affinity(&value))
        .unwrap_or(false)
}

fn runtime_request_turn_state(request: &RuntimeProxyRequest) -> Option<String> {
    request.headers.iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("x-codex-turn-state")
            .then(|| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn runtime_request_session_id_from_value(value: &serde_json::Value) -> Option<String> {
    value
        .get("session_id")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            value
                .get("client_metadata")
                .and_then(|metadata| metadata.get("session_id"))
                .and_then(serde_json::Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn runtime_request_session_id_from_turn_metadata(request: &RuntimeProxyRequest) -> Option<String> {
    request
        .headers
        .iter()
        .find_map(|(name, value)| {
            name.eq_ignore_ascii_case("x-codex-turn-metadata")
                .then(|| value.as_str())
        })
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .and_then(|value| runtime_request_session_id_from_value(&value))
}

fn runtime_request_session_id_from_text(request_text: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(request_text)
        .ok()
        .and_then(|value| runtime_request_session_id_from_value(&value))
}

fn runtime_request_session_id(request: &RuntimeProxyRequest) -> Option<String> {
    request
        .headers
        .iter()
        .find_map(|(name, value)| {
            name.eq_ignore_ascii_case("session_id")
                .then(|| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .or_else(|| runtime_request_session_id_from_turn_metadata(request))
        .or_else(|| {
            serde_json::from_slice::<serde_json::Value>(&request.body)
                .ok()
                .and_then(|value| runtime_request_session_id_from_value(&value))
        })
}

fn runtime_binding_touch_should_persist(bound_at: i64, now: i64) -> bool {
    now.saturating_sub(bound_at) >= RUNTIME_BINDING_TOUCH_PERSIST_INTERVAL_SECONDS
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeContinuationBindingKind {
    Response,
    TurnState,
    SessionId,
}

fn runtime_continuation_status_map(
    statuses: &RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
) -> &BTreeMap<String, RuntimeContinuationBindingStatus> {
    match kind {
        RuntimeContinuationBindingKind::Response => &statuses.response,
        RuntimeContinuationBindingKind::TurnState => &statuses.turn_state,
        RuntimeContinuationBindingKind::SessionId => &statuses.session_id,
    }
}

fn runtime_continuation_status_map_mut(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
) -> &mut BTreeMap<String, RuntimeContinuationBindingStatus> {
    match kind {
        RuntimeContinuationBindingKind::Response => &mut statuses.response,
        RuntimeContinuationBindingKind::TurnState => &mut statuses.turn_state,
        RuntimeContinuationBindingKind::SessionId => &mut statuses.session_id,
    }
}

fn runtime_continuation_status_touches(
    status: &mut RuntimeContinuationBindingStatus,
    now: i64,
) -> bool {
    let previous = status.clone();
    status.last_touched_at = Some(now);
    if status.state == RuntimeContinuationBindingLifecycle::Suspect {
        if status.last_not_found_at.is_some_and(|last| {
            now.saturating_sub(last) >= RUNTIME_CONTINUATION_SUSPECT_GRACE_SECONDS
        }) {
            status.state = RuntimeContinuationBindingLifecycle::Warm;
            status.not_found_streak = 0;
            status.last_not_found_at = None;
        }
        status.confidence = status
            .confidence
            .saturating_add(RUNTIME_CONTINUATION_TOUCH_CONFIDENCE_BONUS)
            .min(RUNTIME_CONTINUATION_CONFIDENCE_MAX);
    } else if status.state != RuntimeContinuationBindingLifecycle::Dead {
        status.confidence = status
            .confidence
            .saturating_add(RUNTIME_CONTINUATION_TOUCH_CONFIDENCE_BONUS)
            .min(RUNTIME_CONTINUATION_CONFIDENCE_MAX);
    }
    *status != previous
}

fn runtime_mark_continuation_status_touched(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
) -> bool {
    let status = runtime_continuation_status_map_mut(statuses, kind)
        .entry(key.to_string())
        .or_default();
    runtime_continuation_status_touches(status, now)
}

fn runtime_mark_continuation_status_verified(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
    verified_route: Option<RuntimeRouteKind>,
) -> bool {
    let status = runtime_continuation_status_map_mut(statuses, kind)
        .entry(key.to_string())
        .or_default();
    let previous = status.clone();
    status.state = RuntimeContinuationBindingLifecycle::Verified;
    status.last_touched_at = Some(now);
    status.last_verified_at = Some(now);
    status.last_verified_route =
        verified_route.map(|route_kind| runtime_route_kind_label(route_kind).to_string());
    status.last_not_found_at = None;
    status.not_found_streak = 0;
    status.success_count = status.success_count.saturating_add(1);
    status.failure_count = 0;
    status.confidence = status
        .confidence
        .saturating_add(RUNTIME_CONTINUATION_VERIFIED_CONFIDENCE_BONUS)
        .min(RUNTIME_CONTINUATION_CONFIDENCE_MAX);
    *status != previous
}

fn runtime_mark_continuation_status_suspect(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
) -> bool {
    let status = runtime_continuation_status_map_mut(statuses, kind)
        .entry(key.to_string())
        .or_default();
    let previous = status.clone();
    status.not_found_streak = status.not_found_streak.saturating_add(1);
    status.last_touched_at = Some(now);
    status.last_not_found_at = Some(now);
    status.failure_count = status.failure_count.saturating_add(1);
    let previous_confidence = status.confidence;
    status.confidence = status
        .confidence
        .saturating_sub(RUNTIME_CONTINUATION_SUSPECT_CONFIDENCE_PENALTY);
    if previous_confidence == 0 {
        status.confidence = 1;
    }
    status.state = if status.not_found_streak >= RUNTIME_CONTINUATION_SUSPECT_NOT_FOUND_STREAK_LIMIT
        || (previous_confidence > 0 && status.confidence == 0)
    {
        RuntimeContinuationBindingLifecycle::Dead
    } else {
        RuntimeContinuationBindingLifecycle::Suspect
    };
    *status != previous
}

fn runtime_mark_continuation_status_dead(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
) -> bool {
    let status = runtime_continuation_status_map_mut(statuses, kind)
        .entry(key.to_string())
        .or_default();
    let previous = status.clone();
    status.state = RuntimeContinuationBindingLifecycle::Dead;
    status.confidence = 0;
    status.last_touched_at = Some(now);
    status.last_not_found_at = Some(now);
    status.not_found_streak = status
        .not_found_streak
        .max(RUNTIME_CONTINUATION_SUSPECT_NOT_FOUND_STREAK_LIMIT);
    status.failure_count = status.failure_count.saturating_add(1);
    *status != previous
}

fn runtime_continuation_status_recently_suspect(
    statuses: &RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
) -> bool {
    runtime_continuation_status_map(statuses, kind)
        .get(key)
        .is_some_and(|status| {
            status.state == RuntimeContinuationBindingLifecycle::Suspect
                && !runtime_continuation_status_is_terminal(status)
                && status.last_not_found_at.is_some_and(|last| {
                    now.saturating_sub(last) < RUNTIME_CONTINUATION_SUSPECT_GRACE_SECONDS
                })
        })
}

fn runtime_continuation_status_label(status: &RuntimeContinuationBindingStatus) -> &'static str {
    match status.state {
        RuntimeContinuationBindingLifecycle::Warm => "warm",
        RuntimeContinuationBindingLifecycle::Verified => "verified",
        RuntimeContinuationBindingLifecycle::Suspect => "suspect",
        RuntimeContinuationBindingLifecycle::Dead => "dead",
    }
}

fn runtime_compact_session_lineage_key(session_id: &str) -> String {
    format!("{RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX}{session_id}")
}

fn runtime_compact_turn_state_lineage_key(turn_state: &str) -> String {
    format!("{RUNTIME_COMPACT_TURN_STATE_LINEAGE_PREFIX}{turn_state}")
}

fn runtime_is_compact_session_lineage_key(key: &str) -> bool {
    key.starts_with(RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX)
}

fn runtime_is_compact_turn_state_lineage_key(key: &str) -> bool {
    key.starts_with(RUNTIME_COMPACT_TURN_STATE_LINEAGE_PREFIX)
}

fn runtime_external_session_id_bindings(
    bindings: &BTreeMap<String, ResponseProfileBinding>,
) -> BTreeMap<String, ResponseProfileBinding> {
    bindings
        .iter()
        .filter(|(key, _)| !runtime_is_compact_session_lineage_key(key))
        .map(|(key, binding)| (key.clone(), binding.clone()))
        .collect()
}

fn runtime_touch_compact_lineage_binding(
    shared: &RuntimeRotationProxyShared,
    runtime: &mut RuntimeRotationState,
    key: &str,
    reason: &str,
    session_binding: bool,
) -> Option<String> {
    let now = Local::now().timestamp();
    let status_kind = if session_binding {
        RuntimeContinuationBindingKind::SessionId
    } else {
        RuntimeContinuationBindingKind::TurnState
    };
    if runtime_continuation_status_recently_suspect(
        &runtime.continuation_statuses,
        status_kind,
        key,
        now,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=compact affinity={} profile=- reason=continuation_recent_suspect key={key}",
                if session_binding {
                    "compact_session"
                } else {
                    "compact_turn_state"
                }
            ),
        );
        return None;
    }
    if runtime_continuation_status_map(&runtime.continuation_statuses, status_kind)
        .get(key)
        .is_some_and(runtime_continuation_status_is_terminal)
    {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=compact affinity={} profile=- reason=continuation_dead key={key}",
                if session_binding {
                    "compact_session"
                } else {
                    "compact_turn_state"
                }
            ),
        );
        return None;
    }
    let bindings = if session_binding {
        &mut runtime.session_id_bindings
    } else {
        &mut runtime.turn_state_bindings
    };
    let profile_name = bindings
        .get(key)
        .map(|binding| binding.profile_name.clone())
        .filter(|profile_name| runtime.state.profiles.contains_key(profile_name));
    let mut touched = false;
    if let Some(profile_name) = profile_name.as_deref()
        && let Some(binding) = bindings.get_mut(key)
        && binding.profile_name == profile_name
    {
        if runtime_binding_touch_should_persist(binding.bound_at, now) {
            touched = true;
        }
        if binding.bound_at < now {
            binding.bound_at = now;
        }
        touched = runtime_mark_continuation_status_touched(
            &mut runtime.continuation_statuses,
            status_kind,
            key,
            now,
        ) || touched;
    }
    if touched {
        schedule_runtime_binding_touch_save(shared, runtime, reason);
    }
    profile_name
}

fn runtime_compact_followup_bound_profile(
    shared: &RuntimeRotationProxyShared,
    turn_state: Option<&str>,
    session_id: Option<&str>,
) -> Result<Option<(String, &'static str)>> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    if let Some(turn_state) = turn_state.map(str::trim).filter(|value| !value.is_empty()) {
        let key = runtime_compact_turn_state_lineage_key(turn_state);
        if let Some(profile_name) = runtime_touch_compact_lineage_binding(
            shared,
            &mut runtime,
            &key,
            &format!("compact_turn_state_touch:{turn_state}"),
            false,
        ) {
            return Ok(Some((profile_name, "turn_state")));
        }
    }
    if let Some(session_id) = session_id.map(str::trim).filter(|value| !value.is_empty()) {
        let key = runtime_compact_session_lineage_key(session_id);
        if let Some(profile_name) = runtime_touch_compact_lineage_binding(
            shared,
            &mut runtime,
            &key,
            &format!("compact_session_touch:{session_id}"),
            true,
        ) {
            return Ok(Some((profile_name, "session_id")));
        }
    }
    Ok(None)
}

fn runtime_previous_response_negative_cache_key(
    previous_response_id: &str,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__previous_response_not_found__:{}:{}:{profile_name}",
        runtime_route_kind_label(route_kind),
        previous_response_id
    )
}

fn runtime_previous_response_negative_cache_failures(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    previous_response_id: &str,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> u32 {
    runtime_profile_effective_score_from_map(
        profile_health,
        &runtime_previous_response_negative_cache_key(
            previous_response_id,
            profile_name,
            route_kind,
        ),
        now,
        RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_SECONDS,
    )
}

fn runtime_previous_response_negative_cache_active(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    previous_response_id: &str,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    runtime_previous_response_negative_cache_failures(
        profile_health,
        previous_response_id,
        profile_name,
        route_kind,
        now,
    ) > 0
}

fn clear_runtime_previous_response_negative_cache(
    runtime: &mut RuntimeRotationState,
    previous_response_id: &str,
    profile_name: &str,
) -> bool {
    let mut changed = false;
    for route_kind in [
        RuntimeRouteKind::Responses,
        RuntimeRouteKind::Websocket,
        RuntimeRouteKind::Compact,
        RuntimeRouteKind::Standard,
    ] {
        changed = runtime
            .profile_health
            .remove(&runtime_previous_response_negative_cache_key(
                previous_response_id,
                profile_name,
                route_kind,
            ))
            .is_some()
            || changed;
    }
    changed
}

fn note_runtime_previous_response_not_found(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    previous_response_id: Option<&str>,
    route_kind: RuntimeRouteKind,
) -> Result<u32> {
    let Some(previous_response_id) = previous_response_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(0);
    };

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let key = runtime_previous_response_negative_cache_key(
        previous_response_id,
        profile_name,
        route_kind,
    );
    let next_failures = runtime_profile_effective_score_from_map(
        &runtime.profile_health,
        &key,
        now,
        RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_SECONDS,
    )
    .saturating_add(1)
    .min(RUNTIME_PROFILE_HEALTH_MAX_SCORE);
    runtime.profile_health.insert(
        key,
        RuntimeProfileHealth {
            score: next_failures,
            updated_at: now,
        },
    );
    let _ = runtime_mark_continuation_status_suspect(
        &mut runtime.continuation_statuses,
        RuntimeContinuationBindingKind::Response,
        previous_response_id,
        now,
    );
    let state_snapshot = runtime.state.clone();
    let continuations_snapshot = runtime_continuation_store_snapshot(&runtime);
    let profile_scores_snapshot = runtime.profile_health.clone();
    let usage_snapshots = runtime.profile_usage_snapshots.clone();
    let backoffs_snapshot = runtime_profile_backoffs_snapshot(&runtime);
    let paths_snapshot = runtime.paths.clone();
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "previous_response_negative_cache profile={profile_name} route={} response_id={} failures={next_failures}",
            runtime_route_kind_label(route_kind),
            previous_response_id,
        ),
    );
    schedule_runtime_state_save(
        shared,
        state_snapshot,
        continuations_snapshot,
        profile_scores_snapshot,
        usage_snapshots,
        backoffs_snapshot,
        paths_snapshot,
        &format!(
            "previous_response_negative_cache:{profile_name}:{}",
            runtime_route_kind_label(route_kind)
        ),
    );
    if next_failures >= RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD {
        let _ = bump_runtime_profile_bad_pairing_score(
            shared,
            profile_name,
            route_kind,
            1,
            "previous_response_not_found",
        );
    }
    Ok(next_failures)
}

fn schedule_runtime_binding_touch_save(
    shared: &RuntimeRotationProxyShared,
    runtime: &RuntimeRotationState,
    reason: &str,
) {
    schedule_runtime_state_save(
        shared,
        runtime.state.clone(),
        runtime_continuation_store_snapshot(runtime),
        runtime.profile_health.clone(),
        runtime.profile_usage_snapshots.clone(),
        runtime_profile_backoffs_snapshot(runtime),
        runtime.paths.clone(),
        reason,
    );
}

fn runtime_response_bound_profile(
    shared: &RuntimeRotationProxyShared,
    previous_response_id: &str,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let profile_name = runtime
        .state
        .response_profile_bindings
        .get(previous_response_id)
        .map(|binding| binding.profile_name.clone())
        .filter(|profile_name| runtime.state.profiles.contains_key(profile_name));
    if runtime_continuation_status_map(
        &runtime.continuation_statuses,
        RuntimeContinuationBindingKind::Response,
    )
    .get(previous_response_id)
    .is_some_and(runtime_continuation_status_is_terminal)
    {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=previous_response profile=- reason=continuation_dead response_id={previous_response_id}",
                runtime_route_kind_label(route_kind),
            ),
        );
        return Ok(None);
    }
    if runtime_continuation_status_recently_suspect(
        &runtime.continuation_statuses,
        RuntimeContinuationBindingKind::Response,
        previous_response_id,
        now,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=previous_response profile=- reason=continuation_suspect response_id={previous_response_id}",
                runtime_route_kind_label(route_kind),
            ),
        );
        return Ok(None);
    }
    if let Some(profile_name) = profile_name.as_deref()
        && runtime_previous_response_negative_cache_active(
            &runtime.profile_health,
            previous_response_id,
            profile_name,
            route_kind,
            now,
        )
    {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=previous_response profile={} reason=negative_cache response_id={}",
                runtime_route_kind_label(route_kind),
                profile_name,
                previous_response_id,
            ),
        );
        return Ok(None);
    }
    let mut touched = false;
    if let Some(profile_name) = profile_name.as_deref()
        && let Some(binding) = runtime
            .state
            .response_profile_bindings
            .get_mut(previous_response_id)
        && binding.profile_name == profile_name
    {
        if runtime_binding_touch_should_persist(binding.bound_at, now) {
            touched = true;
        }
        if binding.bound_at < now {
            binding.bound_at = now;
        }
        touched = runtime_mark_continuation_status_touched(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
            previous_response_id,
            now,
        ) || touched;
    }
    if touched {
        schedule_runtime_binding_touch_save(
            shared,
            &runtime,
            &format!("response_touch:{previous_response_id}"),
        );
    }
    Ok(profile_name)
}

fn runtime_turn_state_bound_profile(
    shared: &RuntimeRotationProxyShared,
    turn_state: &str,
) -> Result<Option<String>> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    if runtime_continuation_status_map(
        &runtime.continuation_statuses,
        RuntimeContinuationBindingKind::TurnState,
    )
    .get(turn_state)
    .is_some_and(runtime_continuation_status_is_terminal)
    {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=responses affinity=turn_state profile=- reason=continuation_dead turn_state={turn_state}",
            ),
        );
        return Ok(None);
    }
    if runtime_continuation_status_recently_suspect(
        &runtime.continuation_statuses,
        RuntimeContinuationBindingKind::TurnState,
        turn_state,
        now,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=responses affinity=turn_state profile=- reason=continuation_suspect turn_state={turn_state}",
            ),
        );
        return Ok(None);
    }
    let profile_name = runtime
        .turn_state_bindings
        .get(turn_state)
        .map(|binding| binding.profile_name.clone())
        .filter(|profile_name| runtime.state.profiles.contains_key(profile_name));
    let mut touched = false;
    if let Some(profile_name) = profile_name.as_deref()
        && let Some(binding) = runtime.turn_state_bindings.get_mut(turn_state)
        && binding.profile_name == profile_name
    {
        if runtime_binding_touch_should_persist(binding.bound_at, now) {
            touched = true;
        }
        if binding.bound_at < now {
            binding.bound_at = now;
        }
        touched = runtime_mark_continuation_status_touched(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::TurnState,
            turn_state,
            now,
        ) || touched;
    }
    if touched {
        schedule_runtime_binding_touch_save(
            shared,
            &runtime,
            &format!("turn_state_touch:{turn_state}"),
        );
    }
    Ok(profile_name)
}

fn runtime_session_bound_profile(
    shared: &RuntimeRotationProxyShared,
    session_id: &str,
) -> Result<Option<String>> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    if runtime_continuation_status_map(
        &runtime.continuation_statuses,
        RuntimeContinuationBindingKind::SessionId,
    )
    .get(session_id)
    .is_some_and(runtime_continuation_status_is_terminal)
    {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=compact affinity=session_id profile=- reason=continuation_dead session_id={session_id}",
            ),
        );
        return Ok(None);
    }
    if runtime_continuation_status_recently_suspect(
        &runtime.continuation_statuses,
        RuntimeContinuationBindingKind::SessionId,
        session_id,
        now,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=compact affinity=session_id profile=- reason=continuation_suspect session_id={session_id}",
            ),
        );
        return Ok(None);
    }
    let profile_name = runtime
        .session_id_bindings
        .get(session_id)
        .map(|binding| binding.profile_name.clone())
        .filter(|profile_name| runtime.state.profiles.contains_key(profile_name));
    let mut touched = false;
    if let Some(profile_name) = profile_name.as_deref() {
        if let Some(binding) = runtime.session_id_bindings.get_mut(session_id)
            && binding.profile_name == profile_name
        {
            if runtime_binding_touch_should_persist(binding.bound_at, now) {
                touched = true;
            }
            if binding.bound_at < now {
                binding.bound_at = now;
            }
        }
        if let Some(binding) = runtime.state.session_profile_bindings.get_mut(session_id)
            && binding.profile_name == profile_name
        {
            if runtime_binding_touch_should_persist(binding.bound_at, now) {
                touched = true;
            }
            if binding.bound_at < now {
                binding.bound_at = now;
            }
        }
        touched = runtime_mark_continuation_status_touched(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::SessionId,
            session_id,
            now,
        ) || touched;
    }
    if touched {
        schedule_runtime_binding_touch_save(
            shared,
            &runtime,
            &format!("session_touch:{session_id}"),
        );
    }
    Ok(profile_name)
}

fn remember_runtime_turn_state(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state: Option<&str>,
    verified_route: RuntimeRouteKind,
) -> Result<()> {
    let Some(turn_state) = turn_state.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let bound_at = Local::now().timestamp();
    let mut changed = false;
    let should_update = runtime
        .turn_state_bindings
        .get(turn_state)
        .is_none_or(|binding| binding.profile_name != profile_name);
    if should_update {
        runtime.turn_state_bindings.insert(
            turn_state.to_string(),
            ResponseProfileBinding {
                profile_name: profile_name.to_string(),
                bound_at,
            },
        );
        changed = true;
    }
    changed = runtime_mark_continuation_status_verified(
        &mut runtime.continuation_statuses,
        RuntimeContinuationBindingKind::TurnState,
        turn_state,
        bound_at,
        Some(verified_route),
    ) || changed;
    if changed {
        prune_profile_bindings(
            &mut runtime.turn_state_bindings,
            TURN_STATE_PROFILE_BINDING_LIMIT,
        );
        let state_snapshot = runtime.state.clone();
        let continuations_snapshot = runtime_continuation_store_snapshot(&runtime);
        let profile_scores_snapshot = runtime.profile_health.clone();
        let usage_snapshots = runtime.profile_usage_snapshots.clone();
        let backoffs_snapshot = runtime_profile_backoffs_snapshot(&runtime);
        let paths_snapshot = runtime.paths.clone();
        drop(runtime);
        schedule_runtime_state_save(
            shared,
            state_snapshot,
            continuations_snapshot,
            profile_scores_snapshot,
            usage_snapshots,
            backoffs_snapshot,
            paths_snapshot,
            &format!("turn_state:{profile_name}"),
        );
        runtime_proxy_log(
            shared,
            format!("binding turn_state profile={profile_name} value={turn_state}"),
        );
    } else {
        drop(runtime);
    }
    Ok(())
}

fn remember_runtime_session_id(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    session_id: Option<&str>,
    verified_route: RuntimeRouteKind,
) -> Result<()> {
    let Some(session_id) = session_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let bound_at = Local::now().timestamp();
    let mut changed = false;
    let should_update = runtime
        .session_id_bindings
        .get(session_id)
        .is_none_or(|binding| binding.profile_name != profile_name);
    if should_update {
        let binding = ResponseProfileBinding {
            profile_name: profile_name.to_string(),
            bound_at,
        };
        runtime
            .session_id_bindings
            .insert(session_id.to_string(), binding.clone());
        runtime
            .state
            .session_profile_bindings
            .insert(session_id.to_string(), binding);
        changed = true;
    }
    changed = runtime_mark_continuation_status_verified(
        &mut runtime.continuation_statuses,
        RuntimeContinuationBindingKind::SessionId,
        session_id,
        bound_at,
        Some(verified_route),
    ) || changed;
    if changed {
        prune_profile_bindings(
            &mut runtime.session_id_bindings,
            SESSION_ID_PROFILE_BINDING_LIMIT,
        );
        prune_profile_bindings(
            &mut runtime.state.session_profile_bindings,
            SESSION_ID_PROFILE_BINDING_LIMIT,
        );
        let state_snapshot = runtime.state.clone();
        let continuations_snapshot = runtime_continuation_store_snapshot(&runtime);
        let profile_scores_snapshot = runtime.profile_health.clone();
        let usage_snapshots = runtime.profile_usage_snapshots.clone();
        let backoffs_snapshot = runtime_profile_backoffs_snapshot(&runtime);
        let paths_snapshot = runtime.paths.clone();
        drop(runtime);
        schedule_runtime_state_save(
            shared,
            state_snapshot,
            continuations_snapshot,
            profile_scores_snapshot,
            usage_snapshots,
            backoffs_snapshot,
            paths_snapshot,
            &format!("session_id:{profile_name}"),
        );
        runtime_proxy_log(
            shared,
            format!("binding session_id profile={profile_name} value={session_id}"),
        );
    } else {
        drop(runtime);
    }
    Ok(())
}

fn remember_runtime_compact_lineage(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    session_id: Option<&str>,
    turn_state: Option<&str>,
    verified_route: RuntimeRouteKind,
) -> Result<()> {
    let session_id = session_id.map(str::trim).filter(|value| !value.is_empty());
    let turn_state = turn_state.map(str::trim).filter(|value| !value.is_empty());
    if session_id.is_none() && turn_state.is_none() {
        return Ok(());
    }

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let bound_at = Local::now().timestamp();
    let mut changed = false;

    if let Some(session_id) = session_id {
        let key = runtime_compact_session_lineage_key(session_id);
        let should_update = runtime.session_id_bindings.get(&key).is_none_or(|binding| {
            binding.profile_name != profile_name || binding.bound_at < bound_at
        });
        if should_update {
            runtime.session_id_bindings.insert(
                key.clone(),
                ResponseProfileBinding {
                    profile_name: profile_name.to_string(),
                    bound_at,
                },
            );
            changed = true;
        }
        changed = runtime_mark_continuation_status_verified(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::SessionId,
            &key,
            bound_at,
            Some(verified_route),
        ) || changed;
    }

    if let Some(turn_state) = turn_state {
        let key = runtime_compact_turn_state_lineage_key(turn_state);
        let should_update = runtime.turn_state_bindings.get(&key).is_none_or(|binding| {
            binding.profile_name != profile_name || binding.bound_at < bound_at
        });
        if should_update {
            runtime.turn_state_bindings.insert(
                key.clone(),
                ResponseProfileBinding {
                    profile_name: profile_name.to_string(),
                    bound_at,
                },
            );
            changed = true;
        }
        changed = runtime_mark_continuation_status_verified(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::TurnState,
            &key,
            bound_at,
            Some(verified_route),
        ) || changed;
    }

    if changed {
        prune_profile_bindings(
            &mut runtime.turn_state_bindings,
            TURN_STATE_PROFILE_BINDING_LIMIT,
        );
        prune_profile_bindings(
            &mut runtime.session_id_bindings,
            SESSION_ID_PROFILE_BINDING_LIMIT,
        );
        let state_snapshot = runtime.state.clone();
        let continuations_snapshot = runtime_continuation_store_snapshot(&runtime);
        let profile_scores_snapshot = runtime.profile_health.clone();
        let usage_snapshots = runtime.profile_usage_snapshots.clone();
        let backoffs_snapshot = runtime_profile_backoffs_snapshot(&runtime);
        let paths_snapshot = runtime.paths.clone();
        drop(runtime);
        schedule_runtime_state_save(
            shared,
            state_snapshot,
            continuations_snapshot,
            profile_scores_snapshot,
            usage_snapshots,
            backoffs_snapshot,
            paths_snapshot,
            &format!("compact_lineage:{profile_name}"),
        );
    } else {
        drop(runtime);
    }
    Ok(())
}

fn release_runtime_compact_lineage(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    session_id: Option<&str>,
    turn_state: Option<&str>,
    reason: &str,
) -> Result<bool> {
    let session_id = session_id.map(str::trim).filter(|value| !value.is_empty());
    let turn_state = turn_state.map(str::trim).filter(|value| !value.is_empty());
    if session_id.is_none() && turn_state.is_none() {
        return Ok(false);
    }

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let mut changed = false;
    let now = Local::now().timestamp();

    if let Some(session_id) = session_id {
        let key = runtime_compact_session_lineage_key(session_id);
        if runtime
            .session_id_bindings
            .get(&key)
            .is_some_and(|binding| binding.profile_name == profile_name)
        {
            runtime.session_id_bindings.remove(&key);
            let _ = runtime_mark_continuation_status_dead(
                &mut runtime.continuation_statuses,
                RuntimeContinuationBindingKind::SessionId,
                &key,
                now,
            );
            changed = true;
        }
    }

    if let Some(turn_state) = turn_state {
        let key = runtime_compact_turn_state_lineage_key(turn_state);
        if runtime
            .turn_state_bindings
            .get(&key)
            .is_some_and(|binding| binding.profile_name == profile_name)
        {
            runtime.turn_state_bindings.remove(&key);
            let _ = runtime_mark_continuation_status_dead(
                &mut runtime.continuation_statuses,
                RuntimeContinuationBindingKind::TurnState,
                &key,
                now,
            );
            changed = true;
        }
    }

    if changed {
        let state_snapshot = runtime.state.clone();
        let continuations_snapshot = runtime_continuation_store_snapshot(&runtime);
        let profile_scores_snapshot = runtime.profile_health.clone();
        let usage_snapshots = runtime.profile_usage_snapshots.clone();
        let backoffs_snapshot = runtime_profile_backoffs_snapshot(&runtime);
        let paths_snapshot = runtime.paths.clone();
        drop(runtime);
        schedule_runtime_state_save(
            shared,
            state_snapshot,
            continuations_snapshot,
            profile_scores_snapshot,
            usage_snapshots,
            backoffs_snapshot,
            paths_snapshot,
            &format!("compact_lineage_release:{profile_name}"),
        );
        runtime_proxy_log(
            shared,
            format!(
                "compact_lineage_released profile={profile_name} reason={reason} session={} turn_state={}",
                session_id.unwrap_or("-"),
                turn_state.unwrap_or("-"),
            ),
        );
    } else {
        drop(runtime);
    }
    Ok(changed)
}

fn remember_runtime_response_ids(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    response_ids: &[String],
    verified_route: RuntimeRouteKind,
) -> Result<()> {
    if response_ids.is_empty() {
        return Ok(());
    }

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let bound_at = Local::now().timestamp();
    let mut changed = false;
    for response_id in response_ids {
        changed =
            clear_runtime_previous_response_negative_cache(&mut runtime, response_id, profile_name)
                || changed;
        let should_update = runtime
            .state
            .response_profile_bindings
            .get(response_id)
            .is_none_or(|binding| binding.profile_name != profile_name);
        if should_update {
            runtime.state.response_profile_bindings.insert(
                response_id.clone(),
                ResponseProfileBinding {
                    profile_name: profile_name.to_string(),
                    bound_at,
                },
            );
            changed = true;
        }
        changed = runtime_mark_continuation_status_verified(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
            response_id,
            bound_at,
            Some(verified_route),
        ) || changed;
    }
    if changed {
        prune_profile_bindings(
            &mut runtime.state.response_profile_bindings,
            RESPONSE_PROFILE_BINDING_LIMIT,
        );
        let state_snapshot = runtime.state.clone();
        let continuations_snapshot = runtime_continuation_store_snapshot(&runtime);
        let profile_scores_snapshot = runtime.profile_health.clone();
        let usage_snapshots = runtime.profile_usage_snapshots.clone();
        let backoffs_snapshot = runtime_profile_backoffs_snapshot(&runtime);
        let paths_snapshot = runtime.paths.clone();
        drop(runtime);
        schedule_runtime_state_save(
            shared,
            state_snapshot,
            continuations_snapshot,
            profile_scores_snapshot,
            usage_snapshots,
            backoffs_snapshot,
            paths_snapshot,
            &format!("response_ids:{profile_name}"),
        );
        runtime_proxy_log(
            shared,
            format!(
                "binding response_ids profile={profile_name} count={} first={:?}",
                response_ids.len(),
                response_ids.first()
            ),
        );
    } else {
        drop(runtime);
    }
    Ok(())
}

fn remember_runtime_successful_previous_response_owner(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    previous_response_id: Option<&str>,
    verified_route: RuntimeRouteKind,
) -> Result<()> {
    let Some(previous_response_id) = previous_response_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let bound_at = Local::now().timestamp();
    let mut changed = clear_runtime_previous_response_negative_cache(
        &mut runtime,
        previous_response_id,
        profile_name,
    );
    let should_update = runtime
        .state
        .response_profile_bindings
        .get(previous_response_id)
        .is_none_or(|binding| binding.profile_name != profile_name || binding.bound_at < bound_at);
    if should_update {
        runtime.state.response_profile_bindings.insert(
            previous_response_id.to_string(),
            ResponseProfileBinding {
                profile_name: profile_name.to_string(),
                bound_at,
            },
        );
        changed = true;
    }
    changed = runtime_mark_continuation_status_verified(
        &mut runtime.continuation_statuses,
        RuntimeContinuationBindingKind::Response,
        previous_response_id,
        bound_at,
        Some(verified_route),
    ) || changed;
    if changed {
        prune_profile_bindings(
            &mut runtime.state.response_profile_bindings,
            RESPONSE_PROFILE_BINDING_LIMIT,
        );
        let state_snapshot = runtime.state.clone();
        let continuations_snapshot = runtime_continuation_store_snapshot(&runtime);
        let profile_scores_snapshot = runtime.profile_health.clone();
        let usage_snapshots = runtime.profile_usage_snapshots.clone();
        let backoffs_snapshot = runtime_profile_backoffs_snapshot(&runtime);
        let paths_snapshot = runtime.paths.clone();
        drop(runtime);
        schedule_runtime_state_save(
            shared,
            state_snapshot,
            continuations_snapshot,
            profile_scores_snapshot,
            usage_snapshots,
            backoffs_snapshot,
            paths_snapshot,
            &format!("previous_response_owner:{profile_name}"),
        );
        runtime_proxy_log(
            shared,
            format!(
                "binding previous_response_owner profile={profile_name} response_id={previous_response_id}"
            ),
        );
    } else {
        drop(runtime);
    }
    Ok(())
}

fn release_runtime_quota_blocked_affinity(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    previous_response_id: Option<&str>,
    turn_state: Option<&str>,
    session_id: Option<&str>,
) -> Result<bool> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let mut changed = false;
    let now = Local::now().timestamp();

    if let Some(previous_response_id) = previous_response_id
        && runtime
            .state
            .response_profile_bindings
            .get(previous_response_id)
            .is_some_and(|binding| binding.profile_name == profile_name)
    {
        runtime
            .state
            .response_profile_bindings
            .remove(previous_response_id);
        let _ = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
            previous_response_id,
            now,
        );
        changed = true;
    }

    if let Some(turn_state) = turn_state
        && runtime
            .turn_state_bindings
            .get(turn_state)
            .is_some_and(|binding| binding.profile_name == profile_name)
    {
        runtime.turn_state_bindings.remove(turn_state);
        let _ = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::TurnState,
            turn_state,
            now,
        );
        changed = true;
    }

    if let Some(session_id) = session_id
        && runtime
            .session_id_bindings
            .get(session_id)
            .is_some_and(|binding| binding.profile_name == profile_name)
    {
        runtime.session_id_bindings.remove(session_id);
        runtime.state.session_profile_bindings.remove(session_id);
        let _ = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::SessionId,
            session_id,
            now,
        );
        changed = true;
    }

    if changed {
        let state_snapshot = runtime.state.clone();
        let continuations_snapshot = runtime_continuation_store_snapshot(&runtime);
        let profile_scores_snapshot = runtime.profile_health.clone();
        let usage_snapshots = runtime.profile_usage_snapshots.clone();
        let backoffs_snapshot = runtime_profile_backoffs_snapshot(&runtime);
        let paths_snapshot = runtime.paths.clone();
        drop(runtime);
        schedule_runtime_state_save(
            shared,
            state_snapshot,
            continuations_snapshot,
            profile_scores_snapshot,
            usage_snapshots,
            backoffs_snapshot,
            paths_snapshot,
            &format!("quota_release:{profile_name}"),
        );
        runtime_proxy_log(
            shared,
            format!(
                "quota_release_affinity profile={profile_name} previous_response_id={:?} turn_state={:?} session_id={:?}",
                previous_response_id, turn_state, session_id
            ),
        );
    } else {
        drop(runtime);
    }

    Ok(changed)
}

fn release_runtime_previous_response_affinity(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    previous_response_id: Option<&str>,
    turn_state: Option<&str>,
    session_id: Option<&str>,
    route_kind: RuntimeRouteKind,
) -> Result<bool> {
    let previous_response_failures = note_runtime_previous_response_not_found(
        shared,
        profile_name,
        previous_response_id,
        route_kind,
    )?;
    if previous_response_failures < RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD {
        runtime_proxy_log(
            shared,
            format!(
                "previous_response_release_deferred profile={profile_name} route={} previous_response_id={:?} failures={previous_response_failures}",
                runtime_route_kind_label(route_kind),
                previous_response_id,
            ),
        );
        return Ok(false);
    }
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let mut changed = false;
    let now = Local::now().timestamp();

    if let Some(previous_response_id) = previous_response_id
        && runtime
            .state
            .response_profile_bindings
            .get(previous_response_id)
            .is_some_and(|binding| binding.profile_name == profile_name)
    {
        runtime
            .state
            .response_profile_bindings
            .remove(previous_response_id);
        let _ = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
            previous_response_id,
            now,
        );
        changed = true;
    }

    if let Some(turn_state) = turn_state
        && runtime
            .turn_state_bindings
            .get(turn_state)
            .is_some_and(|binding| binding.profile_name == profile_name)
    {
        runtime.turn_state_bindings.remove(turn_state);
        let _ = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::TurnState,
            turn_state,
            now,
        );
        changed = true;
    }

    if let Some(session_id) = session_id
        && runtime
            .session_id_bindings
            .get(session_id)
            .is_some_and(|binding| binding.profile_name == profile_name)
    {
        runtime.session_id_bindings.remove(session_id);
        runtime.state.session_profile_bindings.remove(session_id);
        let _ = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::SessionId,
            session_id,
            now,
        );
        changed = true;
    }

    if changed {
        let state_snapshot = runtime.state.clone();
        let continuations_snapshot = runtime_continuation_store_snapshot(&runtime);
        let profile_scores_snapshot = runtime.profile_health.clone();
        let usage_snapshots = runtime.profile_usage_snapshots.clone();
        let backoffs_snapshot = runtime_profile_backoffs_snapshot(&runtime);
        let paths_snapshot = runtime.paths.clone();
        drop(runtime);
        schedule_runtime_state_save(
            shared,
            state_snapshot,
            continuations_snapshot,
            profile_scores_snapshot,
            usage_snapshots,
            backoffs_snapshot,
            paths_snapshot,
            &format!("previous_response_release:{profile_name}"),
        );
        runtime_proxy_log(
            shared,
            format!(
                "previous_response_release_affinity profile={profile_name} previous_response_id={:?} turn_state={:?} session_id={:?}",
                previous_response_id, turn_state, session_id
            ),
        );
    } else {
        drop(runtime);
    }

    Ok(changed)
}

fn prune_profile_bindings(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
    max_entries: usize,
) {
    if bindings.len() <= max_entries {
        return;
    }

    let excess = bindings.len() - max_entries;
    let mut oldest = bindings
        .iter()
        .map(|(response_id, binding)| (response_id.clone(), binding.bound_at))
        .collect::<Vec<_>>();
    oldest.sort_by_key(|(_, bound_at)| *bound_at);

    for (response_id, _) in oldest.into_iter().take(excess) {
        bindings.remove(&response_id);
    }
}

fn runtime_previous_response_retry_delay(retry_index: usize) -> Option<Duration> {
    RUNTIME_PREVIOUS_RESPONSE_RETRY_DELAYS_MS
        .get(retry_index)
        .copied()
        .map(Duration::from_millis)
}

fn runtime_proxy_precommit_budget_exhausted(
    started_at: Instant,
    attempts: usize,
    continuation: bool,
    pressure_mode: bool,
) -> bool {
    let (attempt_limit, budget_ms) = if continuation {
        (
            RUNTIME_PROXY_PRECOMMIT_CONTINUATION_ATTEMPT_LIMIT,
            RUNTIME_PROXY_PRECOMMIT_CONTINUATION_BUDGET_MS,
        )
    } else if pressure_mode {
        (
            RUNTIME_PROXY_PRESSURE_PRECOMMIT_ATTEMPT_LIMIT,
            RUNTIME_PROXY_PRESSURE_PRECOMMIT_BUDGET_MS,
        )
    } else {
        (
            RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT,
            RUNTIME_PROXY_PRECOMMIT_BUDGET_MS,
        )
    };

    attempts >= attempt_limit || started_at.elapsed() >= Duration::from_millis(budget_ms)
}

fn runtime_proxy_has_continuation_priority(
    previous_response_id: Option<&str>,
    pinned_profile: Option<&str>,
    request_turn_state: Option<&str>,
    turn_state_profile: Option<&str>,
    session_profile: Option<&str>,
) -> bool {
    previous_response_id.is_some()
        || pinned_profile.is_some()
        || request_turn_state.is_some()
        || turn_state_profile.is_some()
        || session_profile.is_some()
}

fn runtime_proxy_allows_direct_current_profile_fallback(
    previous_response_id: Option<&str>,
    pinned_profile: Option<&str>,
    request_turn_state: Option<&str>,
    turn_state_profile: Option<&str>,
    session_profile: Option<&str>,
    saw_inflight_saturation: bool,
    saw_upstream_failure: bool,
) -> bool {
    previous_response_id.is_none()
        && pinned_profile.is_none()
        && request_turn_state.is_none()
        && turn_state_profile.is_none()
        && session_profile.is_none()
        && !saw_inflight_saturation
        && !saw_upstream_failure
}

fn runtime_proxy_direct_current_fallback_profile(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let (profile_name, codex_home) = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        let profile_name = runtime.current_profile.clone();
        let Some(profile) = runtime.state.profiles.get(&profile_name) else {
            return Ok(None);
        };
        (profile_name, profile.codex_home.clone())
    };
    if excluded_profiles.contains(&profile_name) {
        return Ok(None);
    }
    if !read_auth_summary(&codex_home).quota_compatible {
        return Ok(None);
    }
    if runtime_profile_inflight_hard_limited_for_context(
        shared,
        &profile_name,
        runtime_route_kind_inflight_context(route_kind),
    )? {
        return Ok(None);
    }
    Ok(Some(profile_name))
}

fn runtime_proxy_local_selection_failure_message() -> &'static str {
    "Runtime proxy could not secure a healthy upstream profile before the pre-commit retry budget was exhausted. Retry the request."
}

fn runtime_quota_window_usable_for_auto_rotate(status: RuntimeQuotaWindowStatus) -> bool {
    matches!(
        status,
        RuntimeQuotaWindowStatus::Ready
            | RuntimeQuotaWindowStatus::Thin
            | RuntimeQuotaWindowStatus::Critical
    )
}

fn runtime_quota_summary_allows_soft_affinity(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
) -> bool {
    source.is_some()
        && runtime_quota_window_usable_for_auto_rotate(summary.five_hour.status)
        && runtime_quota_window_usable_for_auto_rotate(summary.weekly.status)
}

fn runtime_quota_soft_affinity_rejection_reason(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
) -> &'static str {
    if source.is_none()
        || matches!(summary.five_hour.status, RuntimeQuotaWindowStatus::Unknown)
        || matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Unknown)
    {
        "quota_windows_unavailable"
    } else if matches!(
        summary.five_hour.status,
        RuntimeQuotaWindowStatus::Exhausted
    ) || matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Exhausted)
    {
        "quota_exhausted"
    } else {
        runtime_quota_pressure_band_reason(summary.route_band)
    }
}

fn runtime_profile_quota_summary_for_route(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> Result<(RuntimeQuotaSummary, Option<RuntimeQuotaSource>)> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    Ok(runtime
        .profile_probe_cache
        .get(profile_name)
        .filter(|entry| runtime_profile_usage_cache_is_fresh(entry, now))
        .and_then(|entry| entry.result.as_ref().ok())
        .map(|usage| {
            (
                runtime_quota_summary_for_route(usage, route_kind),
                Some(RuntimeQuotaSource::LiveProbe),
            )
        })
        .or_else(|| {
            runtime
                .profile_usage_snapshots
                .get(profile_name)
                .filter(|snapshot| runtime_usage_snapshot_is_usable(snapshot, now))
                .map(|snapshot| {
                    (
                        runtime_quota_summary_from_usage_snapshot(snapshot, route_kind),
                        Some(RuntimeQuotaSource::PersistedSnapshot),
                    )
                })
        })
        .unwrap_or((
            RuntimeQuotaSummary {
                five_hour: RuntimeQuotaWindowSummary {
                    status: RuntimeQuotaWindowStatus::Unknown,
                    remaining_percent: 0,
                    reset_at: i64::MAX,
                },
                weekly: RuntimeQuotaWindowSummary {
                    status: RuntimeQuotaWindowStatus::Unknown,
                    remaining_percent: 0,
                    reset_at: i64::MAX,
                },
                route_band: RuntimeQuotaPressureBand::Healthy,
            },
            None,
        )))
}

fn select_runtime_response_candidate_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    strict_affinity_profile: Option<&str>,
    pinned_profile: Option<&str>,
    turn_state_profile: Option<&str>,
    session_profile: Option<&str>,
    discover_previous_response_owner: bool,
    previous_response_id: Option<&str>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    if let Some(profile_name) = strict_affinity_profile {
        if excluded_profiles.contains(profile_name) {
            return Ok(None);
        }
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
        let compact_followup_owner_without_probe = matches!(
            route_kind,
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
        ) && quota_source.is_none();
        if runtime_quota_summary_allows_soft_affinity(quota_summary, quota_source)
            || compact_followup_owner_without_probe
        {
            return Ok(Some(profile_name.to_string()));
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=compact_followup profile={} reason={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                profile_name,
                runtime_quota_soft_affinity_rejection_reason(quota_summary, quota_source),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(None);
    }

    if let Some(profile_name) = pinned_profile.filter(|name| !excluded_profiles.contains(*name)) {
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
        if quota_summary.route_band <= RuntimeQuotaPressureBand::Critical {
            return Ok(Some(profile_name.to_string()));
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=pinned profile={} reason={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                profile_name,
                runtime_quota_pressure_band_reason(quota_summary.route_band),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
    }

    if let Some(profile_name) = turn_state_profile.filter(|name| !excluded_profiles.contains(*name))
    {
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
        if quota_summary.route_band <= RuntimeQuotaPressureBand::Critical {
            return Ok(Some(profile_name.to_string()));
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=turn_state profile={} reason={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                profile_name,
                runtime_quota_pressure_band_reason(quota_summary.route_band),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
    }

    if discover_previous_response_owner {
        return next_runtime_previous_response_candidate(
            shared,
            excluded_profiles,
            previous_response_id,
            route_kind,
        );
    }

    if let Some(profile_name) = session_profile.filter(|name| !excluded_profiles.contains(*name)) {
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
        let compact_session_owner_without_probe =
            route_kind == RuntimeRouteKind::Compact && quota_source.is_none();
        let websocket_reuse_current_profile = route_kind == RuntimeRouteKind::Websocket
            && quota_source.is_none()
            && runtime_proxy_current_profile(shared)? == profile_name;
        if runtime_quota_summary_allows_soft_affinity(quota_summary, quota_source)
            || compact_session_owner_without_probe
            || websocket_reuse_current_profile
        {
            return Ok(Some(profile_name.to_string()));
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=session profile={} reason={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                profile_name,
                runtime_quota_soft_affinity_rejection_reason(quota_summary, quota_source),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
    }

    if let Some(profile_name) =
        runtime_proxy_optimistic_current_candidate_for_route(shared, excluded_profiles, route_kind)?
    {
        return Ok(Some(profile_name));
    }

    next_runtime_response_candidate_for_route(shared, excluded_profiles, route_kind)
}

fn next_runtime_previous_response_candidate(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    previous_response_id: Option<&str>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let (state, current_profile, profile_health) = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        (
            runtime.state.clone(),
            runtime.current_profile.clone(),
            runtime.profile_health.clone(),
        )
    };
    let now = Local::now().timestamp();

    for name in active_profile_selection_order(&state, &current_profile) {
        if excluded_profiles.contains(&name) {
            continue;
        }
        if let Some(previous_response_id) = previous_response_id
            && runtime_previous_response_negative_cache_active(
                &profile_health,
                previous_response_id,
                &name,
                route_kind,
                now,
            )
        {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_affinity route={} affinity=previous_response_discovery profile={} reason=negative_cache response_id={}",
                    runtime_route_kind_label(route_kind),
                    name,
                    previous_response_id,
                ),
            );
            continue;
        }
        let Some(profile) = state.profiles.get(&name) else {
            continue;
        };
        if !read_auth_summary(&profile.codex_home).quota_compatible {
            continue;
        }
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, &name, route_kind)?;
        if quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_affinity route={} affinity=previous_response_discovery profile={} reason={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    name,
                    runtime_quota_pressure_band_reason(quota_summary.route_band),
                    quota_source
                        .map(runtime_quota_source_label)
                        .unwrap_or("unknown"),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            continue;
        }
        return Ok(Some(name));
    }
    Ok(None)
}

fn runtime_proxy_optimistic_current_candidate_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let (
        current_profile,
        codex_home,
        in_selection_backoff,
        circuit_open_until,
        inflight_count,
        health_score,
    ) = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        let now = Local::now().timestamp();
        prune_runtime_profile_selection_backoff(&mut runtime, now);

        if excluded_profiles.contains(&runtime.current_profile) {
            return Ok(None);
        }

        let Some(profile) = runtime.state.profiles.get(&runtime.current_profile) else {
            return Ok(None);
        };
        (
            runtime.current_profile.clone(),
            profile.codex_home.clone(),
            runtime_profile_in_selection_backoff(&runtime, &runtime.current_profile, now),
            runtime_profile_route_circuit_open_until(
                &runtime,
                &runtime.current_profile,
                route_kind,
                now,
            ),
            runtime_profile_inflight_count(&runtime, &runtime.current_profile),
            runtime_profile_health_score(&runtime, &runtime.current_profile, now, route_kind),
        )
    };
    let (quota_summary, quota_source) =
        runtime_profile_quota_summary_for_route(shared, &current_profile, route_kind)?;

    if in_selection_backoff
        || circuit_open_until.is_some()
        || health_score > 0
        || inflight_count >= RUNTIME_PROFILE_INFLIGHT_SOFT_LIMIT
        || quota_summary.route_band > RuntimeQuotaPressureBand::Healthy
    {
        let reason = if in_selection_backoff {
            "selection_backoff"
        } else if circuit_open_until.is_some() {
            "route_circuit_open"
        } else if health_score > 0 {
            "profile_health"
        } else if quota_summary.route_band > RuntimeQuotaPressureBand::Healthy {
            runtime_quota_pressure_band_reason(quota_summary.route_band)
        } else {
            "profile_inflight_soft_limit"
        };
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_current route={} profile={} reason={} inflight={} health={} soft_limit={} circuit_until={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                current_profile,
                reason,
                inflight_count,
                health_score,
                RUNTIME_PROFILE_INFLIGHT_SOFT_LIMIT,
                circuit_open_until.unwrap_or_default(),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(None);
    }
    if !read_auth_summary(&codex_home).quota_compatible {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_current route={} profile={} reason=auth_not_quota_compatible",
                runtime_route_kind_label(route_kind),
                current_profile
            ),
        );
        return Ok(None);
    }

    runtime_proxy_log(
        shared,
        format!(
            "selection_keep_current route={} profile={} inflight={} health={} quota_source={} {}",
            runtime_route_kind_label(route_kind),
            current_profile,
            inflight_count,
            health_score,
            quota_source
                .map(runtime_quota_source_label)
                .unwrap_or("unknown"),
            runtime_quota_summary_log_fields(quota_summary),
        ),
    );
    if !reserve_runtime_profile_route_circuit_half_open_probe(shared, &current_profile, route_kind)?
    {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_current route={} profile={} reason=route_circuit_half_open_probe_wait inflight={} health={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                current_profile,
                inflight_count,
                health_score,
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(None);
    }
    Ok(Some(current_profile))
}

fn proxy_runtime_responses_websocket_request(
    request_id: u64,
    request: tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
) {
    if !is_runtime_responses_path(request.url()) {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket unsupported_path={}",
                request.url()
            ),
        );
        let _ = request.respond(build_runtime_proxy_text_response(
            404,
            "Runtime websocket proxy only supports Codex responses endpoints.",
        ));
        return;
    }

    let handshake_request = capture_runtime_proxy_websocket_request(&request);
    let Some(websocket_key) = runtime_proxy_websocket_key(&handshake_request) else {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket missing_sec_websocket_key path={}",
                handshake_request.path_and_query
            ),
        );
        let _ = request.respond(build_runtime_proxy_text_response(
            400,
            "Missing Sec-WebSocket-Key header for runtime auto-rotate websocket proxy.",
        ));
        return;
    };

    let response = build_runtime_proxy_websocket_upgrade_response(&websocket_key);
    let upgraded = request.upgrade("websocket", response);
    let mut local_socket = WsSocket::from_raw_socket(upgraded, WsRole::Server, None);
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=websocket upgraded path={} previous_response_id={:?} turn_state={:?}",
            handshake_request.path_and_query,
            runtime_request_previous_response_id(&handshake_request),
            runtime_request_turn_state(&handshake_request)
        ),
    );
    if let Err(err) = run_runtime_proxy_websocket_session(
        request_id,
        &mut local_socket,
        &handshake_request,
        shared,
    ) {
        runtime_proxy_log(
            shared,
            format!("request={request_id} transport=websocket session_error={err:#}"),
        );
        if !is_runtime_proxy_transport_failure(&err) {
            let _ = local_socket.close(None);
        }
    }
}

fn runtime_proxy_websocket_key(request: &RuntimeProxyRequest) -> Option<String> {
    request.headers.iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("Sec-WebSocket-Key")
            .then(|| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn build_runtime_proxy_websocket_upgrade_response(key: &str) -> TinyResponse<std::io::Empty> {
    let accept = derive_accept_key(key.as_bytes());
    TinyResponse::new_empty(TinyStatusCode(101))
        .with_header(TinyHeader::from_bytes("Upgrade", "websocket").expect("upgrade header"))
        .with_header(TinyHeader::from_bytes("Connection", "Upgrade").expect("connection header"))
        .with_header(
            TinyHeader::from_bytes("Sec-WebSocket-Accept", accept.as_bytes())
                .expect("accept header"),
        )
}

#[derive(Default)]
struct RuntimeWebsocketSessionState {
    upstream_socket: Option<RuntimeUpstreamWebSocket>,
    profile_name: Option<String>,
    turn_state: Option<String>,
    inflight_guard: Option<RuntimeProfileInFlightGuard>,
    last_terminal_at: Option<Instant>,
}

impl RuntimeWebsocketSessionState {
    fn can_reuse(&self, profile_name: &str, turn_state_override: Option<&str>) -> bool {
        self.upstream_socket.is_some()
            && self.profile_name.as_deref() == Some(profile_name)
            && turn_state_override.is_none_or(|value| self.turn_state.as_deref() == Some(value))
    }

    fn take_socket(&mut self) -> Option<RuntimeUpstreamWebSocket> {
        self.upstream_socket.take()
    }

    fn last_terminal_elapsed(&self) -> Option<Duration> {
        self.last_terminal_at.map(|timestamp| timestamp.elapsed())
    }

    fn store(
        &mut self,
        socket: RuntimeUpstreamWebSocket,
        profile_name: &str,
        turn_state: Option<String>,
        inflight_guard: Option<RuntimeProfileInFlightGuard>,
    ) {
        self.upstream_socket = Some(socket);
        self.profile_name = Some(profile_name.to_string());
        self.turn_state = turn_state;
        self.last_terminal_at = Some(Instant::now());
        if let Some(inflight_guard) = inflight_guard {
            self.inflight_guard = Some(inflight_guard);
        }
    }

    fn reset(&mut self) {
        self.upstream_socket = None;
        self.profile_name = None;
        self.turn_state = None;
        self.inflight_guard = None;
    }

    fn close(&mut self) {
        if let Some(mut socket) = self.upstream_socket.take() {
            let _ = socket.close(None);
        }
        self.profile_name = None;
        self.turn_state = None;
        self.inflight_guard = None;
    }
}

fn acquire_runtime_profile_inflight_guard(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    context: &'static str,
) -> Result<RuntimeProfileInFlightGuard> {
    let weight = runtime_profile_inflight_weight(context);
    let count = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        let count = runtime
            .profile_inflight
            .entry(profile_name.to_string())
            .or_insert(0);
        *count = count.saturating_add(weight);
        *count
    };
    runtime_proxy_log(
        shared,
        format!(
            "profile_inflight profile={profile_name} count={count} weight={weight} context={context} event=acquire"
        ),
    );
    Ok(RuntimeProfileInFlightGuard {
        shared: shared.clone(),
        profile_name: profile_name.to_string(),
        context,
        weight,
    })
}

fn run_runtime_proxy_websocket_session(
    session_id: u64,
    local_socket: &mut RuntimeLocalWebSocket,
    handshake_request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<()> {
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    loop {
        match local_socket.read() {
            Ok(WsMessage::Text(text)) => {
                let message_id = runtime_proxy_next_request_id(shared);
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={message_id} websocket_session={session_id} inbound_text previous_response_id={:?} turn_state={:?} bytes={}",
                        runtime_request_previous_response_id_from_text(text.as_ref()),
                        runtime_request_turn_state(handshake_request),
                        text.len()
                    ),
                );
                proxy_runtime_websocket_text_message(
                    session_id,
                    message_id,
                    local_socket,
                    handshake_request,
                    text.as_ref(),
                    shared,
                    &mut websocket_session,
                )?;
            }
            Ok(WsMessage::Binary(_)) => {
                runtime_proxy_log(
                    shared,
                    format!("websocket_session={session_id} inbound_binary_rejected"),
                );
                send_runtime_proxy_websocket_error(
                    local_socket,
                    400,
                    "invalid_request_error",
                    "Binary websocket messages are not supported by the runtime auto-rotate proxy.",
                )?;
            }
            Ok(WsMessage::Ping(payload)) => {
                local_socket
                    .send(WsMessage::Pong(payload))
                    .context("failed to respond to runtime websocket ping")?;
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
            Ok(WsMessage::Close(frame)) => {
                runtime_proxy_log(
                    shared,
                    format!("websocket_session={session_id} local_close"),
                );
                websocket_session.close();
                let _ = local_socket.close(frame);
                break;
            }
            Err(WsError::ConnectionClosed) | Err(WsError::AlreadyClosed) => {
                runtime_proxy_log(
                    shared,
                    format!("websocket_session={session_id} local_connection_closed"),
                );
                websocket_session.close();
                break;
            }
            Err(err) => {
                runtime_proxy_log(
                    shared,
                    format!("websocket_session={session_id} local_read_error={err}"),
                );
                websocket_session.close();
                return Err(anyhow::anyhow!(
                    "runtime websocket session ended unexpectedly: {err}"
                ));
            }
        }
    }

    Ok(())
}

fn proxy_runtime_websocket_text_message(
    session_id: u64,
    request_id: u64,
    local_socket: &mut RuntimeLocalWebSocket,
    handshake_request: &RuntimeProxyRequest,
    request_text: &str,
    shared: &RuntimeRotationProxyShared,
    websocket_session: &mut RuntimeWebsocketSessionState,
) -> Result<()> {
    let mut handshake_request = handshake_request.clone();
    let mut request_text = request_text.to_string();
    let mut previous_response_id = runtime_request_previous_response_id_from_text(&request_text);
    let mut request_turn_state = runtime_request_turn_state(&handshake_request);
    let request_session_id = runtime_request_session_id(&handshake_request)
        .or_else(|| runtime_request_session_id_from_text(&request_text));
    let mut bound_profile = previous_response_id
        .as_deref()
        .map(|response_id| {
            runtime_response_bound_profile(shared, response_id, RuntimeRouteKind::Websocket)
        })
        .transpose()?
        .flatten();
    let mut turn_state_profile = request_turn_state
        .as_deref()
        .map(|value| runtime_turn_state_bound_profile(shared, value))
        .transpose()?
        .flatten();
    let mut compact_followup_profile = if previous_response_id.is_none()
        && bound_profile.is_none()
        && turn_state_profile.is_none()
    {
        runtime_compact_followup_bound_profile(
            shared,
            request_turn_state.as_deref(),
            request_session_id.as_deref(),
        )?
    } else {
        None
    };
    if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} websocket_session={session_id} compact_followup_owner profile={profile_name} source={source}"
            ),
        );
    }
    let mut session_profile = if previous_response_id.is_none()
        && bound_profile.is_none()
        && turn_state_profile.is_none()
        && compact_followup_profile.is_none()
    {
        websocket_session.profile_name.clone().or(request_session_id
            .as_deref()
            .map(|session_id| runtime_session_bound_profile(shared, session_id))
            .transpose()?
            .flatten())
    } else {
        None
    };
    let mut pinned_profile = bound_profile.clone().or(compact_followup_profile
        .as_ref()
        .map(|(profile_name, _)| profile_name.clone()));
    let mut excluded_profiles = BTreeSet::new();
    let mut last_failure = None;
    let mut previous_response_retry_candidate: Option<String> = None;
    let mut previous_response_retry_index = 0usize;
    let mut candidate_turn_state_retry_profile: Option<String> = None;
    let mut candidate_turn_state_retry_value: Option<String> = None;
    let mut saw_inflight_saturation = false;
    let mut selection_started_at = Instant::now();
    let mut selection_attempts = 0usize;
    let mut previous_response_fresh_fallback_used = false;
    let mut saw_previous_response_not_found = false;
    let mut websocket_reuse_fresh_retry_profiles = BTreeSet::new();

    loop {
        let pressure_mode = runtime_proxy_pressure_mode_active(shared);
        if runtime_proxy_precommit_budget_exhausted(
            selection_started_at,
            selection_attempts,
            runtime_proxy_has_continuation_priority(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                session_profile.as_deref(),
            ),
            pressure_mode,
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} websocket_session={session_id} precommit_budget_exhausted attempts={selection_attempts} elapsed_ms={} pressure_mode={pressure_mode}",
                    selection_started_at.elapsed().as_millis()
                ),
            );
            if previous_response_id.is_some()
                && saw_previous_response_not_found
                && !previous_response_fresh_fallback_used
                && !runtime_request_text_requires_previous_response_affinity(&request_text)
                && let Some(fresh_request_text) =
                    runtime_request_text_without_previous_response_id(&request_text)
            {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=precommit_budget_exhausted"
                    ),
                );
                request_text = fresh_request_text;
                handshake_request = runtime_request_without_turn_state_header(&handshake_request);
                previous_response_id = None;
                request_turn_state = None;
                previous_response_fresh_fallback_used = true;
                saw_previous_response_not_found = false;
                previous_response_retry_candidate = None;
                previous_response_retry_index = 0;
                candidate_turn_state_retry_profile = None;
                candidate_turn_state_retry_value = None;
                bound_profile = None;
                pinned_profile = None;
                turn_state_profile = None;
                session_profile = None;
                websocket_reuse_fresh_retry_profiles.clear();
                excluded_profiles.clear();
                last_failure = None;
                selection_started_at = Instant::now();
                selection_attempts = 0;
                continue;
            }
            if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} compact_fresh_fallback_blocked profile={profile_name} source={source} reason=precommit_budget_exhausted"
                    ),
                );
                match last_failure {
                    Some(RuntimeUpstreamFailureResponse::Websocket(payload)) => {
                        forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                    }
                    _ if saw_inflight_saturation => {
                        send_runtime_proxy_websocket_error(
                            local_socket,
                            503,
                            "service_unavailable",
                            "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
                        )?;
                    }
                    _ => {
                        send_runtime_proxy_websocket_error(
                            local_socket,
                            503,
                            "service_unavailable",
                            runtime_proxy_local_selection_failure_message(),
                        )?;
                    }
                }
                return Ok(());
            }
            if runtime_proxy_allows_direct_current_profile_fallback(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                session_profile.as_deref(),
                saw_inflight_saturation,
                last_failure.is_some(),
            ) {
                if let Some(current_profile) = runtime_proxy_direct_current_fallback_profile(
                    shared,
                    &excluded_profiles,
                    RuntimeRouteKind::Websocket,
                )? {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} direct_current_profile_fallback profile={current_profile} reason=precommit_budget_exhausted"
                        ),
                    );
                    match attempt_runtime_websocket_request(
                        request_id,
                        local_socket,
                        &handshake_request,
                        &request_text,
                        shared,
                        websocket_session,
                        &current_profile,
                        request_turn_state.as_deref(),
                    )? {
                        RuntimeWebsocketAttempt::Delivered => return Ok(()),
                        RuntimeWebsocketAttempt::QuotaBlocked {
                            profile_name,
                            payload,
                        } => {
                            mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                            forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                            return Ok(());
                        }
                        RuntimeWebsocketAttempt::PreviousResponseNotFound {
                            profile_name,
                            payload,
                            turn_state,
                        } => {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=websocket route=websocket websocket_session={session_id} previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?} via=direct_current_profile_fallback",
                                    turn_state
                                ),
                            );
                            saw_previous_response_not_found = true;
                            if previous_response_retry_candidate.as_deref()
                                != Some(profile_name.as_str())
                            {
                                previous_response_retry_candidate = Some(profile_name.clone());
                                previous_response_retry_index = 0;
                            }
                            let has_turn_state_retry = turn_state.is_some();
                            if has_turn_state_retry {
                                candidate_turn_state_retry_profile = Some(profile_name.clone());
                                candidate_turn_state_retry_value = turn_state;
                            }
                            if has_turn_state_retry
                                && let Some(delay) = runtime_previous_response_retry_delay(
                                    previous_response_retry_index,
                                )
                            {
                                previous_response_retry_index += 1;
                                last_failure =
                                    Some(RuntimeUpstreamFailureResponse::Websocket(payload));
                                runtime_proxy_log(
                                    shared,
                                    format!(
                                        "request={request_id} websocket_session={session_id} previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry via=direct_current_profile_fallback",
                                        delay.as_millis()
                                    ),
                                );
                                continue;
                            }
                            previous_response_retry_candidate = None;
                            previous_response_retry_index = 0;
                            let released_affinity = release_runtime_previous_response_affinity(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                                request_turn_state.as_deref(),
                                request_session_id.as_deref(),
                                RuntimeRouteKind::Websocket,
                            )?;
                            let released_compact_lineage = release_runtime_compact_lineage(
                                shared,
                                &profile_name,
                                request_session_id.as_deref(),
                                request_turn_state.as_deref(),
                                "previous_response_not_found",
                            )?;
                            if released_affinity {
                                runtime_proxy_log(
                                    shared,
                                    format!(
                                        "request={request_id} websocket_session={session_id} previous_response_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                    ),
                                );
                            }
                            if bound_profile.as_deref() == Some(profile_name.as_str()) {
                                bound_profile = None;
                            }
                            if session_profile.as_deref() == Some(profile_name.as_str()) {
                                session_profile = None;
                            }
                            if candidate_turn_state_retry_profile.as_deref()
                                == Some(profile_name.as_str())
                            {
                                candidate_turn_state_retry_profile = None;
                                candidate_turn_state_retry_value = None;
                            }
                            if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                                pinned_profile = None;
                            }
                            if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                                turn_state_profile = None;
                            }
                            if compact_followup_profile
                                .as_ref()
                                .is_some_and(|(owner, _)| owner == &profile_name)
                                || released_compact_lineage
                            {
                                compact_followup_profile = None;
                            }
                            excluded_profiles.insert(profile_name);
                            last_failure = Some(RuntimeUpstreamFailureResponse::Websocket(payload));
                            continue;
                        }
                        RuntimeWebsocketAttempt::ReuseWatchdogTripped { profile_name, .. } => {
                            excluded_profiles.insert(profile_name);
                            continue;
                        }
                        RuntimeWebsocketAttempt::LocalSelectionBlocked { profile_name } => {
                            excluded_profiles.insert(profile_name);
                            continue;
                        }
                    }
                }
            }
            match last_failure {
                Some(RuntimeUpstreamFailureResponse::Websocket(payload)) => {
                    forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                }
                _ if saw_inflight_saturation => {
                    send_runtime_proxy_websocket_error(
                        local_socket,
                        503,
                        "service_unavailable",
                        "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
                    )?;
                }
                _ => {
                    send_runtime_proxy_websocket_error(
                        local_socket,
                        503,
                        "service_unavailable",
                        runtime_proxy_local_selection_failure_message(),
                    )?;
                }
            }
            return Ok(());
        }

        let Some(candidate_name) = select_runtime_response_candidate_for_route(
            shared,
            &excluded_profiles,
            compact_followup_profile
                .as_ref()
                .map(|(profile_name, _)| profile_name.as_str()),
            pinned_profile.as_deref(),
            turn_state_profile.as_deref(),
            session_profile.as_deref(),
            previous_response_id.is_some(),
            previous_response_id.as_deref(),
            RuntimeRouteKind::Websocket,
        )?
        else {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} websocket_session={session_id} candidate_exhausted last_failure={}",
                    match &last_failure {
                        Some(RuntimeUpstreamFailureResponse::Websocket(_)) => "websocket",
                        Some(RuntimeUpstreamFailureResponse::Http(_)) => "http",
                        None => "none",
                    }
                ),
            );
            if previous_response_id.is_some()
                && saw_previous_response_not_found
                && !previous_response_fresh_fallback_used
                && !runtime_request_text_requires_previous_response_affinity(&request_text)
                && let Some(fresh_request_text) =
                    runtime_request_text_without_previous_response_id(&request_text)
            {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=candidate_exhausted"
                    ),
                );
                request_text = fresh_request_text;
                handshake_request = runtime_request_without_turn_state_header(&handshake_request);
                previous_response_id = None;
                request_turn_state = None;
                previous_response_fresh_fallback_used = true;
                saw_previous_response_not_found = false;
                previous_response_retry_candidate = None;
                previous_response_retry_index = 0;
                candidate_turn_state_retry_profile = None;
                candidate_turn_state_retry_value = None;
                bound_profile = None;
                pinned_profile = None;
                turn_state_profile = None;
                session_profile = None;
                websocket_reuse_fresh_retry_profiles.clear();
                excluded_profiles.clear();
                last_failure = None;
                selection_started_at = Instant::now();
                selection_attempts = 0;
                continue;
            }
            if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} compact_fresh_fallback_blocked profile={profile_name} source={source} reason=candidate_exhausted"
                    ),
                );
                match last_failure {
                    Some(RuntimeUpstreamFailureResponse::Websocket(payload)) => {
                        forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                    }
                    _ if saw_inflight_saturation => {
                        send_runtime_proxy_websocket_error(
                            local_socket,
                            503,
                            "service_unavailable",
                            "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
                        )?;
                    }
                    _ => {
                        send_runtime_proxy_websocket_error(
                            local_socket,
                            503,
                            "service_unavailable",
                            runtime_proxy_local_selection_failure_message(),
                        )?;
                    }
                }
                return Ok(());
            }
            if runtime_proxy_allows_direct_current_profile_fallback(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                session_profile.as_deref(),
                saw_inflight_saturation,
                last_failure.is_some(),
            ) {
                if let Some(current_profile) = runtime_proxy_direct_current_fallback_profile(
                    shared,
                    &excluded_profiles,
                    RuntimeRouteKind::Websocket,
                )? {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} direct_current_profile_fallback profile={current_profile} reason=candidate_exhausted"
                        ),
                    );
                    match attempt_runtime_websocket_request(
                        request_id,
                        local_socket,
                        &handshake_request,
                        &request_text,
                        shared,
                        websocket_session,
                        &current_profile,
                        request_turn_state.as_deref(),
                    )? {
                        RuntimeWebsocketAttempt::Delivered => return Ok(()),
                        RuntimeWebsocketAttempt::QuotaBlocked {
                            profile_name,
                            payload,
                        } => {
                            mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                            forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                            return Ok(());
                        }
                        RuntimeWebsocketAttempt::PreviousResponseNotFound {
                            profile_name,
                            payload,
                            turn_state,
                        } => {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=websocket route=websocket websocket_session={session_id} previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?} via=direct_current_profile_fallback",
                                    turn_state
                                ),
                            );
                            saw_previous_response_not_found = true;
                            if previous_response_retry_candidate.as_deref()
                                != Some(profile_name.as_str())
                            {
                                previous_response_retry_candidate = Some(profile_name.clone());
                                previous_response_retry_index = 0;
                            }
                            let has_turn_state_retry = turn_state.is_some();
                            if has_turn_state_retry {
                                candidate_turn_state_retry_profile = Some(profile_name.clone());
                                candidate_turn_state_retry_value = turn_state;
                            }
                            if has_turn_state_retry
                                && let Some(delay) = runtime_previous_response_retry_delay(
                                    previous_response_retry_index,
                                )
                            {
                                previous_response_retry_index += 1;
                                last_failure =
                                    Some(RuntimeUpstreamFailureResponse::Websocket(payload));
                                runtime_proxy_log(
                                    shared,
                                    format!(
                                        "request={request_id} websocket_session={session_id} previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry via=direct_current_profile_fallback",
                                        delay.as_millis()
                                    ),
                                );
                                continue;
                            }
                            previous_response_retry_candidate = None;
                            previous_response_retry_index = 0;
                            let released_affinity = release_runtime_previous_response_affinity(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                                request_turn_state.as_deref(),
                                request_session_id.as_deref(),
                                RuntimeRouteKind::Websocket,
                            )?;
                            let released_compact_lineage = release_runtime_compact_lineage(
                                shared,
                                &profile_name,
                                request_session_id.as_deref(),
                                request_turn_state.as_deref(),
                                "previous_response_not_found",
                            )?;
                            if released_affinity {
                                runtime_proxy_log(
                                    shared,
                                    format!(
                                        "request={request_id} websocket_session={session_id} previous_response_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                    ),
                                );
                            }
                            if bound_profile.as_deref() == Some(profile_name.as_str()) {
                                bound_profile = None;
                            }
                            if session_profile.as_deref() == Some(profile_name.as_str()) {
                                session_profile = None;
                            }
                            if candidate_turn_state_retry_profile.as_deref()
                                == Some(profile_name.as_str())
                            {
                                candidate_turn_state_retry_profile = None;
                                candidate_turn_state_retry_value = None;
                            }
                            if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                                pinned_profile = None;
                            }
                            if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                                turn_state_profile = None;
                            }
                            if compact_followup_profile
                                .as_ref()
                                .is_some_and(|(owner, _)| owner == &profile_name)
                                || released_compact_lineage
                            {
                                compact_followup_profile = None;
                            }
                            excluded_profiles.insert(profile_name);
                            last_failure = Some(RuntimeUpstreamFailureResponse::Websocket(payload));
                            continue;
                        }
                        RuntimeWebsocketAttempt::ReuseWatchdogTripped { profile_name, .. } => {
                            excluded_profiles.insert(profile_name);
                            continue;
                        }
                        RuntimeWebsocketAttempt::LocalSelectionBlocked { profile_name } => {
                            excluded_profiles.insert(profile_name);
                            continue;
                        }
                    }
                }
            }
            match last_failure {
                Some(RuntimeUpstreamFailureResponse::Websocket(payload)) => {
                    forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                }
                _ if saw_inflight_saturation => {
                    send_runtime_proxy_websocket_error(
                        local_socket,
                        503,
                        "service_unavailable",
                        "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
                    )?;
                }
                _ => {
                    send_runtime_proxy_websocket_error(
                        local_socket,
                        503,
                        "service_unavailable",
                        runtime_proxy_local_selection_failure_message(),
                    )?;
                }
            }
            return Ok(());
        };
        selection_attempts = selection_attempts.saturating_add(1);
        let turn_state_override =
            if candidate_turn_state_retry_profile.as_deref() == Some(candidate_name.as_str()) {
                candidate_turn_state_retry_value.as_deref()
            } else {
                request_turn_state.as_deref()
            };
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} websocket_session={session_id} candidate={} pinned={:?} turn_state_profile={:?} turn_state_override={:?} excluded_count={}",
                candidate_name,
                pinned_profile,
                turn_state_profile,
                turn_state_override,
                excluded_profiles.len()
            ),
        );
        let session_affinity_candidate =
            session_profile.as_deref() == Some(candidate_name.as_str());
        if previous_response_id.is_none()
            && pinned_profile.is_none()
            && turn_state_profile.is_none()
            && !session_affinity_candidate
            && runtime_profile_inflight_hard_limited_for_context(
                shared,
                &candidate_name,
                "websocket_session",
            )?
        {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} websocket_session={session_id} profile_inflight_saturated profile={candidate_name} hard_limit={RUNTIME_PROFILE_INFLIGHT_HARD_LIMIT}"
                ),
            );
            excluded_profiles.insert(candidate_name);
            saw_inflight_saturation = true;
            continue;
        }

        match attempt_runtime_websocket_request(
            request_id,
            local_socket,
            &handshake_request,
            &request_text,
            shared,
            websocket_session,
            &candidate_name,
            turn_state_override,
        )? {
            RuntimeWebsocketAttempt::Delivered => return Ok(()),
            RuntimeWebsocketAttempt::QuotaBlocked {
                profile_name,
                payload,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} quota_blocked profile={profile_name}"
                    ),
                );
                mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                let released_affinity = release_runtime_quota_blocked_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                )?;
                if bound_profile.as_deref() == Some(profile_name.as_str()) {
                    bound_profile = None;
                }
                if session_profile.as_deref() == Some(profile_name.as_str()) {
                    session_profile = None;
                }
                if candidate_turn_state_retry_profile.as_deref() == Some(profile_name.as_str()) {
                    candidate_turn_state_retry_profile = None;
                    candidate_turn_state_retry_value = None;
                }
                if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                    pinned_profile = None;
                    previous_response_retry_index = 0;
                }
                if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                    turn_state_profile = None;
                }
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} quota_blocked_affinity_released profile={profile_name}"
                        ),
                    );
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some(RuntimeUpstreamFailureResponse::Websocket(payload));
            }
            RuntimeWebsocketAttempt::LocalSelectionBlocked { profile_name } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} local_selection_blocked profile={profile_name} reason=quota_exhausted_before_send"
                    ),
                );
                if bound_profile.as_deref() == Some(profile_name.as_str()) {
                    bound_profile = None;
                }
                if session_profile.as_deref() == Some(profile_name.as_str()) {
                    session_profile = None;
                }
                if candidate_turn_state_retry_profile.as_deref() == Some(profile_name.as_str()) {
                    candidate_turn_state_retry_profile = None;
                    candidate_turn_state_retry_value = None;
                }
                if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                    pinned_profile = None;
                    previous_response_retry_index = 0;
                }
                if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                    turn_state_profile = None;
                }
                excluded_profiles.insert(profile_name);
            }
            RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                profile_name,
                event,
            } => {
                let reuse_terminal_idle = websocket_session.last_terminal_elapsed();
                let retry_same_profile_with_fresh_connect = !websocket_reuse_fresh_retry_profiles
                    .contains(&profile_name)
                    && (bound_profile.as_deref() == Some(profile_name.as_str())
                        || turn_state_profile.as_deref() == Some(profile_name.as_str())
                        || (request_session_id.is_some()
                            && session_profile.as_deref() == Some(profile_name.as_str())));
                let reuse_failed_bound_previous_response = previous_response_id.is_some()
                    && !previous_response_fresh_fallback_used
                    && (bound_profile.as_deref() == Some(profile_name.as_str())
                        || pinned_profile.as_deref() == Some(profile_name.as_str()));
                let nonreplayable_previous_response_reuse = previous_response_id.is_some()
                    && !previous_response_fresh_fallback_used
                    && turn_state_override.is_none();
                let stale_previous_response_reuse = nonreplayable_previous_response_reuse
                    && turn_state_override.is_none()
                    && reuse_terminal_idle.is_some_and(|elapsed| {
                        elapsed
                            >= Duration::from_millis(
                                runtime_proxy_websocket_previous_response_reuse_stale_ms(),
                            )
                    });
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} websocket_reuse_watchdog_timeout profile={profile_name} event={event}"
                    ),
                );
                if nonreplayable_previous_response_reuse {
                    if stale_previous_response_reuse {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} websocket_session={session_id} websocket_reuse_stale_previous_response_blocked profile={profile_name} event={event} elapsed_ms={} threshold_ms={}",
                                reuse_terminal_idle
                                    .map(|elapsed| elapsed.as_millis())
                                    .unwrap_or(0),
                                runtime_proxy_websocket_previous_response_reuse_stale_ms(),
                            ),
                        );
                    } else {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} websocket_session={session_id} websocket_reuse_previous_response_blocked profile={profile_name} event={event} reason=missing_turn_state elapsed_ms={}",
                                reuse_terminal_idle
                                    .map(|elapsed| elapsed.as_millis())
                                    .unwrap_or(0),
                            ),
                        );
                    }
                    return Err(anyhow::anyhow!(
                        "runtime websocket upstream closed before response.completed for previous_response_id continuation without replayable turn_state: profile={profile_name} event={event}"
                    ));
                }
                if retry_same_profile_with_fresh_connect {
                    websocket_reuse_fresh_retry_profiles.insert(profile_name.clone());
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} websocket_reuse_owner_fresh_retry profile={profile_name} event={event}"
                        ),
                    );
                    continue;
                }
                if reuse_failed_bound_previous_response
                    && !runtime_request_text_requires_previous_response_affinity(&request_text)
                    && let Some(fresh_request_text) =
                        runtime_request_text_without_previous_response_id(&request_text)
                {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=websocket_reuse_watchdog"
                        ),
                    );
                    request_text = fresh_request_text;
                    handshake_request =
                        runtime_request_without_turn_state_header(&handshake_request);
                    previous_response_id = None;
                    request_turn_state = None;
                    previous_response_fresh_fallback_used = true;
                    saw_previous_response_not_found = false;
                    previous_response_retry_candidate = None;
                    previous_response_retry_index = 0;
                    candidate_turn_state_retry_profile = None;
                    candidate_turn_state_retry_value = None;
                    bound_profile = None;
                    session_profile = None;
                    pinned_profile = None;
                    turn_state_profile = None;
                    websocket_reuse_fresh_retry_profiles.clear();
                    excluded_profiles.clear();
                    last_failure = None;
                    selection_started_at = Instant::now();
                    selection_attempts = 0;
                    continue;
                }
                if bound_profile.as_deref() == Some(profile_name.as_str()) {
                    bound_profile = None;
                }
                if session_profile.as_deref() == Some(profile_name.as_str()) {
                    session_profile = None;
                }
                if candidate_turn_state_retry_profile.as_deref() == Some(profile_name.as_str()) {
                    candidate_turn_state_retry_profile = None;
                    candidate_turn_state_retry_value = None;
                }
                if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                    pinned_profile = None;
                    previous_response_retry_index = 0;
                }
                if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                    turn_state_profile = None;
                }
                excluded_profiles.insert(profile_name);
            }
            RuntimeWebsocketAttempt::PreviousResponseNotFound {
                profile_name,
                payload,
                turn_state,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=websocket route=websocket websocket_session={session_id} previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?}",
                        turn_state
                    ),
                );
                saw_previous_response_not_found = true;
                if previous_response_retry_candidate.as_deref() != Some(profile_name.as_str()) {
                    previous_response_retry_candidate = Some(profile_name.clone());
                    previous_response_retry_index = 0;
                }
                let has_turn_state_retry = turn_state.is_some();
                if has_turn_state_retry {
                    candidate_turn_state_retry_profile = Some(profile_name.clone());
                    candidate_turn_state_retry_value = turn_state;
                }
                if has_turn_state_retry
                    && let Some(delay) =
                        runtime_previous_response_retry_delay(previous_response_retry_index)
                {
                    previous_response_retry_index += 1;
                    last_failure = Some(RuntimeUpstreamFailureResponse::Websocket(payload));
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry",
                            delay.as_millis()
                        ),
                    );
                    continue;
                }
                previous_response_retry_candidate = None;
                previous_response_retry_index = 0;
                let released_affinity = release_runtime_previous_response_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                    RuntimeRouteKind::Websocket,
                )?;
                let released_compact_lineage = release_runtime_compact_lineage(
                    shared,
                    &profile_name,
                    request_session_id.as_deref(),
                    request_turn_state.as_deref(),
                    "previous_response_not_found",
                )?;
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} previous_response_affinity_released profile={profile_name}"
                        ),
                    );
                }
                if bound_profile.as_deref() == Some(profile_name.as_str()) {
                    bound_profile = None;
                }
                if session_profile.as_deref() == Some(profile_name.as_str()) {
                    session_profile = None;
                }
                if candidate_turn_state_retry_profile.as_deref() == Some(profile_name.as_str()) {
                    candidate_turn_state_retry_profile = None;
                    candidate_turn_state_retry_value = None;
                }
                if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                    pinned_profile = None;
                }
                if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                    turn_state_profile = None;
                }
                if compact_followup_profile
                    .as_ref()
                    .is_some_and(|(owner, _)| owner == &profile_name)
                    || released_compact_lineage
                {
                    compact_followup_profile = None;
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some(RuntimeUpstreamFailureResponse::Websocket(payload));
            }
        }
    }
}

fn attempt_runtime_websocket_request(
    request_id: u64,
    local_socket: &mut RuntimeLocalWebSocket,
    handshake_request: &RuntimeProxyRequest,
    request_text: &str,
    shared: &RuntimeRotationProxyShared,
    websocket_session: &mut RuntimeWebsocketSessionState,
    profile_name: &str,
    turn_state_override: Option<&str>,
) -> Result<RuntimeWebsocketAttempt> {
    let request_previous_response_id = runtime_request_previous_response_id(handshake_request);
    let request_session_id = runtime_request_session_id(handshake_request);
    let request_turn_state = runtime_request_turn_state(handshake_request);
    let (quota_summary, quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, RuntimeRouteKind::Websocket)?;
    if quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted {
        websocket_session.close();
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket websocket_reuse_skip_quota_exhausted profile={profile_name} quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(RuntimeWebsocketAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
        });
    }
    let reuse_existing_session = websocket_session.can_reuse(profile_name, turn_state_override);
    let reuse_started_at = reuse_existing_session.then(Instant::now);
    let precommit_started_at = Instant::now();
    let (mut upstream_socket, mut upstream_turn_state, mut inflight_guard) =
        if reuse_existing_session {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=websocket websocket_reuse_start profile={profile_name} turn_state_override={:?}",
                    turn_state_override
                ),
            );
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=websocket upstream_session=reuse profile={profile_name} turn_state_override={:?}",
                    turn_state_override
                ),
            );
            (
                websocket_session
                    .take_socket()
                    .expect("runtime websocket session should keep its upstream socket"),
                websocket_session.turn_state.clone(),
                None,
            )
        } else {
            websocket_session.close();
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=websocket upstream_session=connect profile={profile_name} turn_state_override={:?}",
                    turn_state_override
                ),
            );
            match connect_runtime_proxy_upstream_websocket(
                request_id,
                handshake_request,
                shared,
                profile_name,
                turn_state_override,
            )? {
                RuntimeWebsocketConnectResult::Connected { socket, turn_state } => (
                    socket,
                    turn_state,
                    Some(acquire_runtime_profile_inflight_guard(
                        shared,
                        profile_name,
                        "websocket_session",
                    )?),
                ),
                RuntimeWebsocketConnectResult::QuotaBlocked(payload) => {
                    return Ok(RuntimeWebsocketAttempt::QuotaBlocked {
                        profile_name: profile_name.to_string(),
                        payload,
                    });
                }
            }
        };
    runtime_set_upstream_websocket_io_timeout(
        &mut upstream_socket,
        Some(Duration::from_millis(
            runtime_proxy_websocket_precommit_progress_timeout_ms(),
        )),
    )
    .context("failed to configure runtime websocket pre-commit timeout")?;

    if let Err(err) = upstream_socket.send(WsMessage::Text(request_text.to_string().into())) {
        let _ = upstream_socket.close(None);
        websocket_session.reset();
        let transport_error =
            anyhow::anyhow!("failed to send runtime websocket request upstream: {err}");
        note_runtime_profile_transport_failure(
            shared,
            profile_name,
            RuntimeRouteKind::Websocket,
            "websocket_upstream_send",
            &transport_error,
        );
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket upstream_send_error profile={profile_name} error={err}"
            ),
        );
        if reuse_existing_session {
            return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                profile_name: profile_name.to_string(),
                event: "upstream_send_error",
            });
        }
        return Err(transport_error);
    }

    let mut committed = false;
    let mut first_upstream_frame_seen = false;
    let mut buffered_precommit_text_frames = Vec::new();
    loop {
        match upstream_socket.read() {
            Ok(WsMessage::Text(text)) => {
                let text = text.to_string();
                if !first_upstream_frame_seen {
                    first_upstream_frame_seen = true;
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(runtime_proxy_stream_idle_timeout_ms())),
                    )
                    .context("failed to restore runtime websocket idle timeout")?;
                }
                if let Some(turn_state) = extract_runtime_turn_state_from_payload(text.as_str()) {
                    remember_runtime_turn_state(
                        shared,
                        profile_name,
                        Some(turn_state.as_str()),
                        RuntimeRouteKind::Websocket,
                    )?;
                    upstream_turn_state = Some(turn_state);
                }
                match runtime_proxy_websocket_retry_action(text.as_str()) {
                    Some(RuntimeWebsocketAttempt::QuotaBlocked { .. }) if !committed => {
                        let _ = upstream_socket.close(None);
                        websocket_session.reset();
                        return Ok(RuntimeWebsocketAttempt::QuotaBlocked {
                            profile_name: profile_name.to_string(),
                            payload: RuntimeWebsocketErrorPayload::Text(text),
                        });
                    }
                    Some(RuntimeWebsocketAttempt::PreviousResponseNotFound { .. })
                        if !committed =>
                    {
                        let _ = upstream_socket.close(None);
                        websocket_session.reset();
                        return Ok(RuntimeWebsocketAttempt::PreviousResponseNotFound {
                            profile_name: profile_name.to_string(),
                            payload: RuntimeWebsocketErrorPayload::Text(text),
                            turn_state: upstream_turn_state.clone(),
                        });
                    }
                    _ => {}
                }
                if reuse_existing_session
                    && !committed
                    && runtime_websocket_precommit_hold_frame(text.as_str())
                {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=websocket precommit_hold profile={profile_name} event_type={}",
                            runtime_response_event_type(text.as_str())
                                .unwrap_or_else(|| "-".to_string())
                        ),
                    );
                    buffered_precommit_text_frames.push(text);
                    continue;
                }

                if !committed {
                    remember_runtime_session_id(
                        shared,
                        profile_name,
                        request_session_id.as_deref(),
                        RuntimeRouteKind::Websocket,
                    )?;
                    remember_runtime_turn_state(
                        shared,
                        profile_name,
                        upstream_turn_state.as_deref(),
                        RuntimeRouteKind::Websocket,
                    )?;
                    commit_runtime_proxy_profile_selection_with_notice(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Websocket,
                    )?;
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=websocket committed profile={profile_name}"
                        ),
                    );
                    committed = true;
                    forward_runtime_proxy_buffered_websocket_text_frames(
                        local_socket,
                        &mut buffered_precommit_text_frames,
                        shared,
                        profile_name,
                        request_previous_response_id.as_deref(),
                        request_session_id.as_deref(),
                        request_turn_state.as_deref(),
                    )?;
                }
                remember_runtime_websocket_response_ids(
                    shared,
                    profile_name,
                    request_previous_response_id.as_deref(),
                    request_session_id.as_deref(),
                    request_turn_state.as_deref(),
                    text.as_str(),
                )?;
                local_socket
                    .send(WsMessage::Text(text.clone().into()))
                    .with_context(|| {
                        websocket_session.reset();
                        "failed to forward runtime websocket text frame"
                    })?;
                if is_runtime_terminal_event(text.as_str()) {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=websocket terminal_event profile={profile_name}"
                        ),
                    );
                    websocket_session.store(
                        upstream_socket,
                        profile_name,
                        upstream_turn_state,
                        inflight_guard.take(),
                    );
                    return Ok(RuntimeWebsocketAttempt::Delivered);
                }
            }
            Ok(WsMessage::Binary(payload)) => {
                if !first_upstream_frame_seen {
                    first_upstream_frame_seen = true;
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(runtime_proxy_stream_idle_timeout_ms())),
                    )
                    .context("failed to restore runtime websocket idle timeout")?;
                }
                if !committed {
                    remember_runtime_session_id(
                        shared,
                        profile_name,
                        request_session_id.as_deref(),
                        RuntimeRouteKind::Websocket,
                    )?;
                    remember_runtime_turn_state(
                        shared,
                        profile_name,
                        upstream_turn_state.as_deref(),
                        RuntimeRouteKind::Websocket,
                    )?;
                    commit_runtime_proxy_profile_selection_with_notice(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Websocket,
                    )?;
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=websocket committed_binary profile={profile_name}"
                        ),
                    );
                    committed = true;
                    forward_runtime_proxy_buffered_websocket_text_frames(
                        local_socket,
                        &mut buffered_precommit_text_frames,
                        shared,
                        profile_name,
                        request_previous_response_id.as_deref(),
                        request_session_id.as_deref(),
                        request_turn_state.as_deref(),
                    )?;
                }
                local_socket
                    .send(WsMessage::Binary(payload))
                    .with_context(|| {
                        websocket_session.reset();
                        "failed to forward runtime websocket binary frame"
                    })?;
            }
            Ok(WsMessage::Ping(payload)) => {
                if !first_upstream_frame_seen {
                    first_upstream_frame_seen = true;
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(runtime_proxy_stream_idle_timeout_ms())),
                    )
                    .context("failed to restore runtime websocket idle timeout")?;
                }
                upstream_socket
                    .send(WsMessage::Pong(payload))
                    .context("failed to respond to upstream websocket ping")?;
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {
                if !first_upstream_frame_seen {
                    first_upstream_frame_seen = true;
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(runtime_proxy_stream_idle_timeout_ms())),
                    )
                    .context("failed to restore runtime websocket idle timeout")?;
                }
            }
            Ok(WsMessage::Close(frame)) => {
                websocket_session.reset();
                if let Some(started_at) = reuse_started_at {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "websocket_reuse_watchdog profile={profile_name} event=upstream_close_before_terminal elapsed_ms={} committed={committed}",
                            started_at.elapsed().as_millis()
                        ),
                    );
                }
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=websocket upstream_close_before_completed profile={profile_name}"
                    ),
                );
                let _ = frame;
                let transport_error =
                    anyhow::anyhow!("runtime websocket upstream closed before response.completed");
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Websocket,
                    "websocket_upstream_close",
                    &transport_error,
                );
                if reuse_existing_session && !committed {
                    return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                        profile_name: profile_name.to_string(),
                        event: "upstream_close_before_commit",
                    });
                }
                return Err(transport_error);
            }
            Err(WsError::ConnectionClosed) | Err(WsError::AlreadyClosed) => {
                websocket_session.reset();
                if let Some(started_at) = reuse_started_at {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "websocket_reuse_watchdog profile={profile_name} event=connection_closed elapsed_ms={} committed={committed}",
                            started_at.elapsed().as_millis()
                        ),
                    );
                }
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=websocket upstream_connection_closed profile={profile_name}"
                    ),
                );
                let transport_error =
                    anyhow::anyhow!("runtime websocket upstream closed before response.completed");
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Websocket,
                    "websocket_upstream_connection_closed",
                    &transport_error,
                );
                if reuse_existing_session && !committed {
                    return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                        profile_name: profile_name.to_string(),
                        event: "connection_closed_before_commit",
                    });
                }
                return Err(transport_error);
            }
            Err(err) => {
                websocket_session.reset();
                if !committed && !first_upstream_frame_seen && runtime_websocket_timeout_error(&err)
                {
                    let elapsed_ms = precommit_started_at.elapsed().as_millis();
                    runtime_proxy_log(
                        shared,
                        format!(
                            "websocket_precommit_frame_timeout profile={profile_name} event=no_first_upstream_frame_before_deadline elapsed_ms={elapsed_ms} reuse={reuse_existing_session}"
                        ),
                    );
                    let transport_error = anyhow::anyhow!(
                        "runtime websocket upstream produced no first frame before the pre-commit deadline: {err}"
                    );
                    note_runtime_profile_transport_failure(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Websocket,
                        "websocket_first_frame_timeout",
                        &transport_error,
                    );
                    if reuse_existing_session {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "websocket_reuse_watchdog profile={profile_name} event=no_first_upstream_frame_before_deadline elapsed_ms={elapsed_ms} committed={committed}"
                            ),
                        );
                        return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                            profile_name: profile_name.to_string(),
                            event: "no_first_upstream_frame_before_deadline",
                        });
                    }
                    return Err(transport_error);
                }
                if let Some(started_at) = reuse_started_at {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "websocket_reuse_watchdog profile={profile_name} event=read_error elapsed_ms={} committed={committed}",
                            started_at.elapsed().as_millis()
                        ),
                    );
                }
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=websocket upstream_read_error profile={profile_name} error={err}"
                    ),
                );
                let transport_error = anyhow::anyhow!(
                    "runtime websocket upstream failed before response.completed: {err}"
                );
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Websocket,
                    "websocket_upstream_read",
                    &transport_error,
                );
                if reuse_existing_session && !committed {
                    return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                        profile_name: profile_name.to_string(),
                        event: "upstream_read_error",
                    });
                }
                return Err(transport_error);
            }
        }
    }
}

fn connect_runtime_proxy_upstream_websocket(
    request_id: u64,
    handshake_request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
) -> Result<RuntimeWebsocketConnectResult> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .clone();
    let profile = runtime
        .state
        .profiles
        .get(profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    let auth = read_usage_auth(&profile.codex_home)?;
    let upstream_url = runtime_proxy_upstream_websocket_url(
        &runtime.upstream_base_url,
        &handshake_request.path_and_query,
    )?;
    let mut request = upstream_url
        .as_str()
        .into_client_request()
        .with_context(|| format!("failed to build runtime websocket request for {upstream_url}"))?;

    for (name, value) in &handshake_request.headers {
        if turn_state_override.is_some() && name.eq_ignore_ascii_case("x-codex-turn-state") {
            continue;
        }
        if should_skip_runtime_request_header(name) {
            continue;
        }
        let Ok(header_name) = WsHeaderName::from_bytes(name.as_bytes()) else {
            continue;
        };
        let Ok(header_value) = WsHeaderValue::from_str(value) else {
            continue;
        };
        request.headers_mut().insert(header_name, header_value);
    }
    if let Some(turn_state) = turn_state_override {
        request.headers_mut().insert(
            WsHeaderName::from_static("x-codex-turn-state"),
            WsHeaderValue::from_str(turn_state)
                .context("failed to encode websocket turn-state header")?,
        );
    }

    request.headers_mut().insert(
        WsHeaderName::from_static("authorization"),
        WsHeaderValue::from_str(&format!("Bearer {}", auth.access_token))
            .context("failed to encode websocket authorization header")?,
    );
    let user_agent =
        runtime_proxy_effective_user_agent(&handshake_request.headers).unwrap_or("codex-cli");
    request.headers_mut().insert(
        WsHeaderName::from_static("user-agent"),
        WsHeaderValue::from_str(user_agent).context("failed to encode websocket user-agent")?,
    );
    if let Some(account_id) = auth.account_id.as_deref() {
        request.headers_mut().insert(
            WsHeaderName::from_static("chatgpt-account-id"),
            WsHeaderValue::from_str(account_id)
                .context("failed to encode websocket account header")?,
        );
    }

    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=websocket upstream_connect_start profile={profile_name} url={upstream_url} turn_state_override={:?}",
            turn_state_override
        ),
    );
    if runtime_take_fault_injection("PRODEX_RUNTIME_FAULT_UPSTREAM_CONNECT_ERROR_ONCE") {
        let transport_error = anyhow::anyhow!("injected runtime websocket connect failure");
        note_runtime_profile_transport_failure(
            shared,
            profile_name,
            RuntimeRouteKind::Websocket,
            "websocket_connect",
            &transport_error,
        );
        return Err(transport_error);
    }
    let started_at = Instant::now();
    match connect_runtime_proxy_upstream_websocket_with_timeout(request) {
        Ok((socket, response)) => Ok(RuntimeWebsocketConnectResult::Connected {
            socket,
            turn_state: {
                let turn_state = runtime_proxy_tungstenite_header_value(
                    response.headers(),
                    "x-codex-turn-state",
                );
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=websocket upstream_connect_ok profile={profile_name} status={} turn_state={:?}",
                        response.status().as_u16(),
                        turn_state
                    ),
                );
                note_runtime_profile_latency_observation(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Websocket,
                    "connect",
                    started_at.elapsed().as_millis() as u64,
                );
                turn_state
            },
        }),
        Err(WsError::Http(response)) => {
            let status = response.status().as_u16();
            let body = response.body().clone().unwrap_or_default();
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=websocket upstream_connect_http profile={profile_name} status={status} body_bytes={}",
                    body.len()
                ),
            );
            if matches!(status, 403 | 429) && extract_runtime_proxy_quota_message(&body).is_some() {
                return Ok(RuntimeWebsocketConnectResult::QuotaBlocked(
                    runtime_websocket_error_payload_from_http_body(&body),
                ));
            }
            bail!("runtime websocket upstream rejected the handshake with HTTP {status}");
        }
        Err(err) => {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=websocket upstream_connect_error profile={profile_name} error={err}"
                ),
            );
            let transport_error =
                anyhow::anyhow!("failed to connect runtime websocket upstream: {err}");
            note_runtime_profile_transport_failure(
                shared,
                profile_name,
                RuntimeRouteKind::Websocket,
                "websocket_connect",
                &transport_error,
            );
            Err(transport_error)
        }
    }
}

fn runtime_websocket_error_payload_from_http_body(body: &[u8]) -> RuntimeWebsocketErrorPayload {
    if body.is_empty() {
        return RuntimeWebsocketErrorPayload::Empty;
    }

    match std::str::from_utf8(body) {
        Ok(text) => RuntimeWebsocketErrorPayload::Text(text.to_string()),
        Err(_) => RuntimeWebsocketErrorPayload::Binary(body.to_vec()),
    }
}

fn connect_runtime_proxy_upstream_websocket_with_timeout(
    request: tungstenite::http::Request<()>,
) -> std::result::Result<
    (
        RuntimeUpstreamWebSocket,
        tungstenite::handshake::client::Response,
    ),
    WsError,
> {
    let stream = connect_runtime_proxy_upstream_tcp_stream(request.uri())?;
    match client_tls_with_config(request, stream, None, None) {
        Ok((socket, response)) => Ok((socket, response)),
        Err(WsHandshakeError::Failure(err)) => Err(err),
        Err(WsHandshakeError::Interrupted(_)) => {
            unreachable!("blocking upstream websocket handshake should not interrupt")
        }
    }
}

fn connect_runtime_proxy_upstream_tcp_stream(
    uri: &tungstenite::http::Uri,
) -> std::result::Result<TcpStream, WsError> {
    let host = uri.host().ok_or(WsError::Url(WsUrlError::NoHostName))?;
    let host = if host.starts_with('[') && host.ends_with(']') {
        &host[1..host.len() - 1]
    } else {
        host
    };
    let port = uri.port_u16().unwrap_or(match uri.scheme_str() {
        Some("wss") => 443,
        _ => 80,
    });
    let connect_timeout = Duration::from_millis(runtime_proxy_websocket_connect_timeout_ms());
    let io_timeout = Duration::from_millis(runtime_proxy_websocket_precommit_progress_timeout_ms());
    let addrs = (host, port).to_socket_addrs().map_err(WsError::Io)?;

    for addr in addrs {
        if let Ok(stream) = TcpStream::connect_timeout(&addr, connect_timeout) {
            stream.set_nodelay(true).map_err(WsError::Io)?;
            stream
                .set_read_timeout(Some(io_timeout))
                .map_err(WsError::Io)?;
            stream
                .set_write_timeout(Some(io_timeout))
                .map_err(WsError::Io)?;
            return Ok(stream);
        }
    }

    Err(WsError::Url(WsUrlError::UnableToConnect(uri.to_string())))
}

fn send_runtime_proxy_websocket_error(
    local_socket: &mut RuntimeLocalWebSocket,
    status: u16,
    code: &str,
    message: &str,
) -> Result<()> {
    let payload = serde_json::json!({
        "type": "error",
        "status": status,
        "error": {
            "code": code,
            "message": message,
        }
    })
    .to_string();
    local_socket
        .send(WsMessage::Text(payload.into()))
        .context("failed to send runtime websocket error frame")
}

fn forward_runtime_proxy_websocket_error(
    local_socket: &mut RuntimeLocalWebSocket,
    payload: &RuntimeWebsocketErrorPayload,
) -> Result<()> {
    match payload {
        RuntimeWebsocketErrorPayload::Text(text) => local_socket
            .send(WsMessage::Text(text.clone().into()))
            .context("failed to forward runtime websocket text error frame"),
        RuntimeWebsocketErrorPayload::Binary(bytes) => local_socket
            .send(WsMessage::Binary(bytes.clone().into()))
            .context("failed to forward runtime websocket binary error frame"),
        RuntimeWebsocketErrorPayload::Empty => Ok(()),
    }
}

fn remember_runtime_websocket_response_ids(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    request_previous_response_id: Option<&str>,
    request_session_id: Option<&str>,
    request_turn_state: Option<&str>,
    payload: &str,
) -> Result<()> {
    let response_ids = extract_runtime_response_ids_from_payload(payload);
    remember_runtime_successful_previous_response_owner(
        shared,
        profile_name,
        request_previous_response_id,
        RuntimeRouteKind::Websocket,
    )?;
    remember_runtime_response_ids(
        shared,
        profile_name,
        &response_ids,
        RuntimeRouteKind::Websocket,
    )?;
    if !response_ids.is_empty() {
        let _ = release_runtime_compact_lineage(
            shared,
            profile_name,
            request_session_id,
            request_turn_state,
            "response_committed",
        );
    }
    Ok(())
}

fn forward_runtime_proxy_buffered_websocket_text_frames(
    local_socket: &mut RuntimeLocalWebSocket,
    buffered_frames: &mut Vec<String>,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    request_previous_response_id: Option<&str>,
    request_session_id: Option<&str>,
    request_turn_state: Option<&str>,
) -> Result<()> {
    for frame in buffered_frames.drain(..) {
        remember_runtime_websocket_response_ids(
            shared,
            profile_name,
            request_previous_response_id,
            request_session_id,
            request_turn_state,
            frame.as_str(),
        )?;
        local_socket
            .send(WsMessage::Text(frame.into()))
            .context("failed to forward buffered runtime websocket text frame")?;
    }
    Ok(())
}

fn runtime_proxy_websocket_retry_action(payload: &str) -> Option<RuntimeWebsocketAttempt> {
    let value = serde_json::from_str::<serde_json::Value>(payload).ok()?;
    if let Some(message) = extract_runtime_proxy_previous_response_message_from_value(&value) {
        let _ = message;
        return Some(RuntimeWebsocketAttempt::PreviousResponseNotFound {
            profile_name: String::new(),
            payload: RuntimeWebsocketErrorPayload::Text(payload.to_string()),
            turn_state: None,
        });
    }
    if let Some(message) = extract_runtime_proxy_quota_message_from_value(&value) {
        let _ = message;
        return Some(RuntimeWebsocketAttempt::QuotaBlocked {
            profile_name: String::new(),
            payload: RuntimeWebsocketErrorPayload::Text(payload.to_string()),
        });
    }

    None
}

fn runtime_response_event_type(payload: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .and_then(|value| {
            value
                .get("type")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
}

fn runtime_websocket_precommit_hold_frame(payload: &str) -> bool {
    runtime_response_event_type(payload).is_some_and(|kind| {
        matches!(
            kind.as_str(),
            "response.created" | "response.in_progress" | "response.queued"
        )
    })
}

fn is_runtime_terminal_event(payload: &str) -> bool {
    runtime_response_event_type(payload)
        .is_some_and(|kind| matches!(kind.as_str(), "response.completed" | "response.failed"))
}

fn proxy_runtime_standard_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<tiny_http::ResponseBox> {
    let request_session_id = runtime_request_session_id(request);
    let mut session_profile = request_session_id
        .as_deref()
        .map(|session_id| runtime_session_bound_profile(shared, session_id))
        .transpose()?
        .flatten();
    if !is_runtime_compact_path(&request.path_and_query) {
        let current_profile = runtime_proxy_current_profile(shared)?;
        let preferred_profile = session_profile
            .clone()
            .unwrap_or_else(|| current_profile.clone());
        let (quota_summary, quota_source) = runtime_profile_quota_summary_for_route(
            shared,
            &preferred_profile,
            RuntimeRouteKind::Standard,
        )?;
        let preferred_is_session = session_profile.as_deref() == Some(preferred_profile.as_str());
        let preferred_profile_usable = if preferred_is_session {
            runtime_quota_summary_allows_soft_affinity(quota_summary, quota_source)
        } else {
            quota_summary.route_band != RuntimeQuotaPressureBand::Exhausted
        };
        if preferred_profile_usable {
            return proxy_runtime_standard_request_for_profile(
                request_id,
                request,
                shared,
                &preferred_profile,
            );
        }
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http {} profile={} reason={} quota_source={} {}",
                if preferred_is_session {
                    format!(
                        "selection_skip_affinity route={} affinity=session",
                        runtime_route_kind_label(RuntimeRouteKind::Standard)
                    )
                } else {
                    format!(
                        "selection_skip_current route={}",
                        runtime_route_kind_label(RuntimeRouteKind::Standard)
                    )
                },
                preferred_profile,
                if preferred_is_session {
                    runtime_quota_soft_affinity_rejection_reason(quota_summary, quota_source)
                } else {
                    runtime_quota_pressure_band_reason(quota_summary.route_band)
                },
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        let mut excluded_profiles = BTreeSet::from([preferred_profile]);
        if let Some(candidate_name) = select_runtime_response_candidate_for_route(
            shared,
            &excluded_profiles,
            None,
            None,
            None,
            None,
            false,
            None,
            RuntimeRouteKind::Standard,
        )? {
            return proxy_runtime_standard_request_for_profile(
                request_id,
                request,
                shared,
                &candidate_name,
            );
        }
        excluded_profiles.clear();
        return Ok(build_runtime_proxy_text_response(
            503,
            runtime_proxy_local_selection_failure_message(),
        ));
    }

    let current_profile = runtime_proxy_current_profile(shared)?;
    let compact_owner_profile = session_profile
        .clone()
        .unwrap_or_else(|| current_profile.clone());
    let pressure_mode = runtime_proxy_pressure_mode_active(shared);
    if runtime_proxy_should_shed_fresh_compact_request(pressure_mode, session_profile.as_deref()) {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_pressure_shed reason=fresh_request pressure_mode={pressure_mode}"
            ),
        );
        return Ok(build_runtime_proxy_json_error_response(
            503,
            "service_unavailable",
            "Fresh compact requests are temporarily deferred while the runtime proxy is under pressure. Retry the request.",
        ));
    }
    let mut excluded_profiles = BTreeSet::new();
    let mut conservative_overload_retried_profiles = BTreeSet::new();
    let mut last_failure = None;
    let mut saw_inflight_saturation = false;
    let selection_started_at = Instant::now();
    let mut selection_attempts = 0usize;

    loop {
        if runtime_proxy_precommit_budget_exhausted(
            selection_started_at,
            selection_attempts,
            session_profile.is_some(),
            pressure_mode,
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http compact_precommit_budget_exhausted attempts={selection_attempts} elapsed_ms={} pressure_mode={pressure_mode}",
                    selection_started_at.elapsed().as_millis()
                ),
            );
            return match last_failure {
                Some(response) => Ok(response),
                None if saw_inflight_saturation => Ok(build_runtime_proxy_json_error_response(
                    503,
                    "service_unavailable",
                    "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
                )),
                None if session_profile.is_some() => Ok(build_runtime_proxy_json_error_response(
                    503,
                    "service_unavailable",
                    runtime_proxy_local_selection_failure_message(),
                )),
                None => match attempt_runtime_standard_request(
                    request_id,
                    request,
                    shared,
                    &compact_owner_profile,
                )? {
                    RuntimeStandardAttempt::Success {
                        profile_name,
                        response,
                    } => {
                        commit_runtime_proxy_profile_selection_with_notice(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Compact,
                        )?;
                        Ok(response)
                    }
                    RuntimeStandardAttempt::RetryableFailure { response, .. } => Ok(response),
                    RuntimeStandardAttempt::LocalSelectionBlocked { .. } => {
                        Ok(build_runtime_proxy_json_error_response(
                            503,
                            "service_unavailable",
                            runtime_proxy_local_selection_failure_message(),
                        ))
                    }
                },
            };
        }
        selection_attempts = selection_attempts.saturating_add(1);

        let Some(candidate_name) = select_runtime_response_candidate_for_route(
            shared,
            &excluded_profiles,
            None,
            None,
            None,
            session_profile.as_deref(),
            false,
            None,
            RuntimeRouteKind::Compact,
        )?
        else {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http compact_candidate_exhausted last_failure={}",
                    if last_failure.is_some() {
                        "http"
                    } else {
                        "none"
                    }
                ),
            );
            return match last_failure {
                Some(response) => Ok(response),
                None if saw_inflight_saturation => Ok(build_runtime_proxy_json_error_response(
                    503,
                    "service_unavailable",
                    "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
                )),
                None if session_profile.is_some() => Ok(build_runtime_proxy_json_error_response(
                    503,
                    "service_unavailable",
                    runtime_proxy_local_selection_failure_message(),
                )),
                None => match attempt_runtime_standard_request(
                    request_id,
                    request,
                    shared,
                    &compact_owner_profile,
                )? {
                    RuntimeStandardAttempt::Success {
                        profile_name,
                        response,
                    } => {
                        commit_runtime_proxy_profile_selection_with_notice(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Compact,
                        )?;
                        Ok(response)
                    }
                    RuntimeStandardAttempt::RetryableFailure { response, .. } => Ok(response),
                    RuntimeStandardAttempt::LocalSelectionBlocked { .. } => {
                        Ok(build_runtime_proxy_json_error_response(
                            503,
                            "service_unavailable",
                            runtime_proxy_local_selection_failure_message(),
                        ))
                    }
                },
            };
        };

        if excluded_profiles.contains(&candidate_name) {
            continue;
        }

        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_candidate={} excluded_count={}",
                candidate_name,
                excluded_profiles.len()
            ),
        );
        if runtime_profile_inflight_hard_limited_for_context(
            shared,
            &candidate_name,
            "compact_http",
        )? {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http profile_inflight_saturated profile={candidate_name} hard_limit={RUNTIME_PROFILE_INFLIGHT_HARD_LIMIT}",
                ),
            );
            excluded_profiles.insert(candidate_name);
            saw_inflight_saturation = true;
            continue;
        }

        match attempt_runtime_standard_request(request_id, request, shared, &candidate_name)? {
            RuntimeStandardAttempt::Success {
                profile_name,
                response,
            } => {
                commit_runtime_proxy_profile_selection_with_notice(
                    shared,
                    &profile_name,
                    RuntimeRouteKind::Compact,
                )?;
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http compact_committed profile={profile_name}"
                    ),
                );
                return Ok(response);
            }
            RuntimeStandardAttempt::RetryableFailure {
                profile_name,
                response,
                overload,
            } => {
                let mut released_affinity = false;
                let mut released_compact_lineage = false;
                if !overload {
                    released_affinity = release_runtime_quota_blocked_affinity(
                        shared,
                        &profile_name,
                        None,
                        None,
                        request_session_id.as_deref(),
                    )?;
                    released_compact_lineage = release_runtime_compact_lineage(
                        shared,
                        &profile_name,
                        request_session_id.as_deref(),
                        None,
                        "quota_blocked",
                    )?;
                    if session_profile.as_deref() == Some(profile_name.as_str()) {
                        session_profile = None;
                    }
                }
                let should_retry_same_profile = overload
                    && !conservative_overload_retried_profiles.contains(&profile_name)
                    && (session_profile.as_deref() == Some(profile_name.as_str())
                        || current_profile == profile_name);
                if should_retry_same_profile {
                    conservative_overload_retried_profiles.insert(profile_name.clone());
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http compact_overload_conservative_retry profile={profile_name} delay_ms={RUNTIME_PROXY_COMPACT_OWNER_RETRY_DELAY_MS} reason=non_blocking_retry"
                        ),
                    );
                    last_failure = Some(response);
                    continue;
                }
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http compact_retryable_failure profile={profile_name}"
                    ),
                );
                mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} route=compact"
                        ),
                    );
                }
                if released_compact_lineage {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http compact_lineage_released profile={profile_name} reason=quota_blocked"
                        ),
                    );
                }
                if overload {
                    let _ = bump_runtime_profile_health_score(
                        shared,
                        &profile_name,
                        RuntimeRouteKind::Compact,
                        RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
                        "compact_overload",
                    );
                    let _ = bump_runtime_profile_bad_pairing_score(
                        shared,
                        &profile_name,
                        RuntimeRouteKind::Compact,
                        RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
                        "compact_overload",
                    );
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some(response);
            }
            RuntimeStandardAttempt::LocalSelectionBlocked { profile_name } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http local_selection_blocked profile={profile_name} route=compact reason=quota_exhausted_before_send"
                    ),
                );
                excluded_profiles.insert(profile_name);
            }
        }
    }
}

fn proxy_runtime_standard_request_for_profile(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<tiny_http::ResponseBox> {
    let (quota_summary, quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, RuntimeRouteKind::Standard)?;
    if quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http standard_pre_send_skip profile={profile_name} route=standard quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(build_runtime_proxy_text_response(
            503,
            runtime_proxy_local_selection_failure_message(),
        ));
    }
    let _inflight_guard =
        acquire_runtime_profile_inflight_guard(shared, profile_name, "standard_http")?;
    let response =
        send_runtime_proxy_upstream_request(request_id, request, shared, profile_name, None)
            .map_err(|err| {
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Standard,
                    "standard_upstream_request",
                    &err,
                );
                err
            })?;
    remember_runtime_session_id(
        shared,
        profile_name,
        runtime_request_session_id(request).as_deref(),
        RuntimeRouteKind::Standard,
    )?;
    if request.path_and_query.ends_with("/backend-api/wham/usage") {
        let parts = buffer_runtime_proxy_async_response_parts(shared, response, Vec::new())
            .map_err(|err| {
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Standard,
                    "standard_buffer_usage_response",
                    &err,
                );
                err
            })?;
        if let Ok(usage) = serde_json::from_slice::<UsageResponse>(&parts.body) {
            update_runtime_profile_probe_cache_with_usage(shared, profile_name, usage)?;
        }
        return Ok(build_runtime_proxy_response_from_parts(parts));
    }
    forward_runtime_proxy_response(shared, response, Vec::new()).map_err(|err| {
        note_runtime_profile_transport_failure(
            shared,
            profile_name,
            RuntimeRouteKind::Standard,
            "standard_forward_response",
            &err,
        );
        err
    })
}

fn attempt_runtime_standard_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<RuntimeStandardAttempt> {
    let request_session_id = runtime_request_session_id(request);
    let (quota_summary, quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, RuntimeRouteKind::Compact)?;
    if quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http standard_pre_send_skip profile={profile_name} route=compact quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(RuntimeStandardAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
        });
    }
    let _inflight_guard =
        acquire_runtime_profile_inflight_guard(shared, profile_name, "compact_http")?;
    let response =
        send_runtime_proxy_upstream_request(request_id, request, shared, profile_name, None)
            .map_err(|err| {
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Compact,
                    "compact_upstream_request",
                    &err,
                );
                err
            })?;
    if !is_runtime_compact_path(&request.path_and_query) || response.status().is_success() {
        let response_turn_state = is_runtime_compact_path(&request.path_and_query)
            .then(|| runtime_proxy_header_value(response.headers(), "x-codex-turn-state"))
            .flatten();
        let response =
            forward_runtime_proxy_response(shared, response, Vec::new()).map_err(|err| {
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Compact,
                    "compact_forward_response",
                    &err,
                );
                err
            })?;
        remember_runtime_session_id(
            shared,
            profile_name,
            request_session_id.as_deref(),
            if is_runtime_compact_path(&request.path_and_query) {
                RuntimeRouteKind::Compact
            } else {
                RuntimeRouteKind::Standard
            },
        )?;
        if is_runtime_compact_path(&request.path_and_query) {
            remember_runtime_compact_lineage(
                shared,
                profile_name,
                request_session_id.as_deref(),
                response_turn_state.as_deref(),
                RuntimeRouteKind::Compact,
            )?;
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http compact_committed_owner profile={profile_name} session={} turn_state={}",
                    request_session_id.as_deref().unwrap_or("-"),
                    response_turn_state.as_deref().unwrap_or("-"),
                ),
            );
        }
        return Ok(RuntimeStandardAttempt::Success {
            profile_name: profile_name.to_string(),
            response,
        });
    }

    let status = response.status().as_u16();
    let parts =
        buffer_runtime_proxy_async_response_parts(shared, response, Vec::new()).map_err(|err| {
            note_runtime_profile_transport_failure(
                shared,
                profile_name,
                RuntimeRouteKind::Compact,
                "compact_buffer_response",
                &err,
            );
            err
        })?;
    let retryable_quota =
        matches!(status, 403 | 429) && extract_runtime_proxy_quota_message(&parts.body).is_some();
    let retryable_overload = extract_runtime_proxy_overload_message(status, &parts.body).is_some();
    if matches!(status, 403 | 429) && !retryable_quota {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_quota_unclassified profile={profile_name} status={status} body_snippet={}",
                runtime_proxy_body_snippet(&parts.body, 240),
            ),
        );
    }
    let response = build_runtime_proxy_response_from_parts(parts);

    if retryable_quota || retryable_overload {
        return Ok(RuntimeStandardAttempt::RetryableFailure {
            profile_name: profile_name.to_string(),
            response,
            overload: retryable_overload,
        });
    }

    Ok(RuntimeStandardAttempt::Success {
        profile_name: profile_name.to_string(),
        response,
    })
}

fn proxy_runtime_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<RuntimeResponsesReply> {
    let mut request = request.clone();
    let mut previous_response_id = runtime_request_previous_response_id(&request);
    let mut request_turn_state = runtime_request_turn_state(&request);
    let request_session_id = runtime_request_session_id(&request);
    let mut bound_profile = previous_response_id
        .as_deref()
        .map(|response_id| {
            runtime_response_bound_profile(shared, response_id, RuntimeRouteKind::Responses)
        })
        .transpose()?
        .flatten();
    let mut turn_state_profile = request_turn_state
        .as_deref()
        .map(|value| runtime_turn_state_bound_profile(shared, value))
        .transpose()?
        .flatten();
    let mut compact_followup_profile = if previous_response_id.is_none()
        && bound_profile.is_none()
        && turn_state_profile.is_none()
    {
        runtime_compact_followup_bound_profile(
            shared,
            request_turn_state.as_deref(),
            request_session_id.as_deref(),
        )?
    } else {
        None
    };
    if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_followup_owner profile={profile_name} source={source}"
            ),
        );
    }
    let mut session_profile = if previous_response_id.is_none()
        && bound_profile.is_none()
        && turn_state_profile.is_none()
        && compact_followup_profile.is_none()
    {
        request_session_id
            .as_deref()
            .map(|session_id| runtime_session_bound_profile(shared, session_id))
            .transpose()?
            .flatten()
    } else {
        None
    };
    let mut pinned_profile = bound_profile.clone().or(compact_followup_profile
        .as_ref()
        .map(|(profile_name, _)| profile_name.clone()));
    let mut excluded_profiles = BTreeSet::new();
    let mut last_failure = None;
    let mut previous_response_retry_candidate: Option<String> = None;
    let mut previous_response_retry_index = 0usize;
    let mut candidate_turn_state_retry_profile: Option<String> = None;
    let mut candidate_turn_state_retry_value: Option<String> = None;
    let mut saw_inflight_saturation = false;
    let mut selection_started_at = Instant::now();
    let mut selection_attempts = 0usize;
    let mut previous_response_fresh_fallback_used = false;
    let mut saw_previous_response_not_found = false;

    loop {
        let pressure_mode = runtime_proxy_pressure_mode_active(shared);
        if runtime_proxy_precommit_budget_exhausted(
            selection_started_at,
            selection_attempts,
            runtime_proxy_has_continuation_priority(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                session_profile.as_deref(),
            ),
            pressure_mode,
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http precommit_budget_exhausted attempts={selection_attempts} elapsed_ms={} pressure_mode={pressure_mode}",
                    selection_started_at.elapsed().as_millis()
                ),
            );
            if previous_response_id.is_some()
                && saw_previous_response_not_found
                && !previous_response_fresh_fallback_used
                && !runtime_request_requires_previous_response_affinity(&request)
                && let Some(fresh_request) =
                    runtime_request_without_previous_response_affinity(&request)
            {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http previous_response_fresh_fallback reason=precommit_budget_exhausted"
                    ),
                );
                request = fresh_request;
                previous_response_id = None;
                request_turn_state = None;
                previous_response_fresh_fallback_used = true;
                saw_previous_response_not_found = false;
                previous_response_retry_candidate = None;
                previous_response_retry_index = 0;
                candidate_turn_state_retry_profile = None;
                candidate_turn_state_retry_value = None;
                bound_profile = None;
                pinned_profile = None;
                turn_state_profile = None;
                session_profile = None;
                excluded_profiles.clear();
                last_failure = None;
                selection_started_at = Instant::now();
                selection_attempts = 0;
                continue;
            }
            if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http compact_fresh_fallback_blocked profile={profile_name} source={source} reason=precommit_budget_exhausted"
                    ),
                );
                return Ok(match last_failure {
                    Some(RuntimeUpstreamFailureResponse::Http(response)) => response,
                    _ if saw_inflight_saturation => {
                        RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_response(
                            503,
                            "service_unavailable",
                            "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
                        ))
                    }
                    _ => RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_response(
                        503,
                        "service_unavailable",
                        runtime_proxy_local_selection_failure_message(),
                    )),
                });
            }
            if runtime_proxy_allows_direct_current_profile_fallback(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                session_profile.as_deref(),
                saw_inflight_saturation,
                last_failure.is_some(),
            ) {
                if let Some(current_profile) = runtime_proxy_direct_current_fallback_profile(
                    shared,
                    &excluded_profiles,
                    RuntimeRouteKind::Responses,
                )? {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http direct_current_profile_fallback profile={current_profile} reason=precommit_budget_exhausted"
                        ),
                    );
                    match attempt_runtime_responses_request(
                        request_id,
                        &request,
                        shared,
                        &current_profile,
                        request_turn_state.as_deref(),
                    )? {
                        RuntimeResponsesAttempt::Success {
                            profile_name,
                            response,
                        } => {
                            if saw_previous_response_not_found {
                                remember_runtime_successful_previous_response_owner(
                                    shared,
                                    &profile_name,
                                    previous_response_id.as_deref(),
                                    RuntimeRouteKind::Responses,
                                )?;
                            }
                            commit_runtime_proxy_profile_selection_with_notice(
                                shared,
                                &profile_name,
                                RuntimeRouteKind::Responses,
                            )?;
                            let _ = release_runtime_compact_lineage(
                                shared,
                                &profile_name,
                                request_session_id.as_deref(),
                                request_turn_state.as_deref(),
                                "response_committed_post_commit",
                            );
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http committed profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                            return Ok(response);
                        }
                        RuntimeResponsesAttempt::QuotaBlocked {
                            profile_name,
                            response,
                        } => {
                            mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                            return Ok(response);
                        }
                        RuntimeResponsesAttempt::PreviousResponseNotFound {
                            profile_name,
                            response,
                            turn_state,
                        } => {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http route=responses previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?} via=direct_current_profile_fallback",
                                    turn_state
                                ),
                            );
                            saw_previous_response_not_found = true;
                            if previous_response_retry_candidate.as_deref()
                                != Some(profile_name.as_str())
                            {
                                previous_response_retry_candidate = Some(profile_name.clone());
                                previous_response_retry_index = 0;
                            }
                            let has_turn_state_retry = turn_state.is_some();
                            if has_turn_state_retry {
                                candidate_turn_state_retry_profile = Some(profile_name.clone());
                                candidate_turn_state_retry_value = turn_state;
                            }
                            if has_turn_state_retry
                                && let Some(delay) = runtime_previous_response_retry_delay(
                                    previous_response_retry_index,
                                )
                            {
                                previous_response_retry_index += 1;
                                last_failure = Some(RuntimeUpstreamFailureResponse::Http(response));
                                runtime_proxy_log(
                                    shared,
                                    format!(
                                        "request={request_id} transport=http previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry via=direct_current_profile_fallback",
                                        delay.as_millis()
                                    ),
                                );
                                continue;
                            }
                            previous_response_retry_candidate = None;
                            previous_response_retry_index = 0;
                            let released_affinity = release_runtime_previous_response_affinity(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                                request_turn_state.as_deref(),
                                request_session_id.as_deref(),
                                RuntimeRouteKind::Responses,
                            )?;
                            let released_compact_lineage = release_runtime_compact_lineage(
                                shared,
                                &profile_name,
                                request_session_id.as_deref(),
                                request_turn_state.as_deref(),
                                "previous_response_not_found",
                            )?;
                            if released_affinity {
                                runtime_proxy_log(
                                    shared,
                                    format!(
                                        "request={request_id} transport=http previous_response_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                    ),
                                );
                            }
                            if bound_profile.as_deref() == Some(profile_name.as_str()) {
                                bound_profile = None;
                            }
                            if session_profile.as_deref() == Some(profile_name.as_str()) {
                                session_profile = None;
                            }
                            if candidate_turn_state_retry_profile.as_deref()
                                == Some(profile_name.as_str())
                            {
                                candidate_turn_state_retry_profile = None;
                                candidate_turn_state_retry_value = None;
                            }
                            if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                                pinned_profile = None;
                            }
                            if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                                turn_state_profile = None;
                            }
                            if compact_followup_profile
                                .as_ref()
                                .is_some_and(|(owner, _)| owner == &profile_name)
                                || released_compact_lineage
                            {
                                compact_followup_profile = None;
                            }
                            excluded_profiles.insert(profile_name);
                            last_failure = Some(RuntimeUpstreamFailureResponse::Http(response));
                            continue;
                        }
                        RuntimeResponsesAttempt::LocalSelectionBlocked { profile_name } => {
                            excluded_profiles.insert(profile_name);
                            continue;
                        }
                    }
                }
            }
            return Ok(match last_failure {
                Some(RuntimeUpstreamFailureResponse::Http(response)) => response,
                _ if saw_inflight_saturation => {
                    RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_response(
                        503,
                        "service_unavailable",
                        "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
                    ))
                }
                _ => RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_response(
                    503,
                    "service_unavailable",
                    runtime_proxy_local_selection_failure_message(),
                )),
            });
        }

        let Some(candidate_name) = select_runtime_response_candidate_for_route(
            shared,
            &excluded_profiles,
            compact_followup_profile
                .as_ref()
                .map(|(profile_name, _)| profile_name.as_str()),
            pinned_profile.as_deref(),
            turn_state_profile.as_deref(),
            session_profile.as_deref(),
            previous_response_id.is_some(),
            previous_response_id.as_deref(),
            RuntimeRouteKind::Responses,
        )?
        else {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http candidate_exhausted last_failure={}",
                    match &last_failure {
                        Some(RuntimeUpstreamFailureResponse::Http(_)) => "http",
                        Some(RuntimeUpstreamFailureResponse::Websocket(_)) => "websocket",
                        None => "none",
                    }
                ),
            );
            if previous_response_id.is_some()
                && saw_previous_response_not_found
                && !previous_response_fresh_fallback_used
                && !runtime_request_requires_previous_response_affinity(&request)
                && let Some(fresh_request) =
                    runtime_request_without_previous_response_affinity(&request)
            {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http previous_response_fresh_fallback reason=candidate_exhausted"
                    ),
                );
                request = fresh_request;
                previous_response_id = None;
                request_turn_state = None;
                previous_response_fresh_fallback_used = true;
                saw_previous_response_not_found = false;
                previous_response_retry_candidate = None;
                previous_response_retry_index = 0;
                candidate_turn_state_retry_profile = None;
                candidate_turn_state_retry_value = None;
                bound_profile = None;
                pinned_profile = None;
                turn_state_profile = None;
                session_profile = None;
                excluded_profiles.clear();
                last_failure = None;
                selection_started_at = Instant::now();
                selection_attempts = 0;
                continue;
            }
            if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http compact_fresh_fallback_blocked profile={profile_name} source={source} reason=candidate_exhausted"
                    ),
                );
                return Ok(match last_failure {
                    Some(RuntimeUpstreamFailureResponse::Http(response)) => response,
                    _ if saw_inflight_saturation => {
                        RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_response(
                            503,
                            "service_unavailable",
                            "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
                        ))
                    }
                    _ => RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_response(
                        503,
                        "service_unavailable",
                        runtime_proxy_local_selection_failure_message(),
                    )),
                });
            }
            if runtime_proxy_allows_direct_current_profile_fallback(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                session_profile.as_deref(),
                saw_inflight_saturation,
                last_failure.is_some(),
            ) {
                if let Some(current_profile) = runtime_proxy_direct_current_fallback_profile(
                    shared,
                    &excluded_profiles,
                    RuntimeRouteKind::Responses,
                )? {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http direct_current_profile_fallback profile={current_profile} reason=candidate_exhausted"
                        ),
                    );
                    match attempt_runtime_responses_request(
                        request_id,
                        &request,
                        shared,
                        &current_profile,
                        request_turn_state.as_deref(),
                    )? {
                        RuntimeResponsesAttempt::Success {
                            profile_name,
                            response,
                        } => {
                            if saw_previous_response_not_found {
                                remember_runtime_successful_previous_response_owner(
                                    shared,
                                    &profile_name,
                                    previous_response_id.as_deref(),
                                    RuntimeRouteKind::Responses,
                                )?;
                            }
                            commit_runtime_proxy_profile_selection_with_notice(
                                shared,
                                &profile_name,
                                RuntimeRouteKind::Responses,
                            )?;
                            let _ = release_runtime_compact_lineage(
                                shared,
                                &profile_name,
                                request_session_id.as_deref(),
                                request_turn_state.as_deref(),
                                "response_committed_post_commit",
                            );
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http committed profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                            return Ok(response);
                        }
                        RuntimeResponsesAttempt::QuotaBlocked {
                            profile_name,
                            response,
                        } => {
                            mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                            return Ok(response);
                        }
                        RuntimeResponsesAttempt::PreviousResponseNotFound {
                            profile_name,
                            response,
                            turn_state,
                        } => {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http route=responses previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?} via=direct_current_profile_fallback",
                                    turn_state
                                ),
                            );
                            saw_previous_response_not_found = true;
                            if previous_response_retry_candidate.as_deref()
                                != Some(profile_name.as_str())
                            {
                                previous_response_retry_candidate = Some(profile_name.clone());
                                previous_response_retry_index = 0;
                            }
                            let has_turn_state_retry = turn_state.is_some();
                            if has_turn_state_retry {
                                candidate_turn_state_retry_profile = Some(profile_name.clone());
                                candidate_turn_state_retry_value = turn_state;
                            }
                            if has_turn_state_retry
                                && let Some(delay) = runtime_previous_response_retry_delay(
                                    previous_response_retry_index,
                                )
                            {
                                previous_response_retry_index += 1;
                                last_failure = Some(RuntimeUpstreamFailureResponse::Http(response));
                                runtime_proxy_log(
                                    shared,
                                    format!(
                                        "request={request_id} transport=http previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry via=direct_current_profile_fallback",
                                        delay.as_millis()
                                    ),
                                );
                                continue;
                            }
                            previous_response_retry_candidate = None;
                            previous_response_retry_index = 0;
                            let released_affinity = release_runtime_previous_response_affinity(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                                request_turn_state.as_deref(),
                                request_session_id.as_deref(),
                                RuntimeRouteKind::Responses,
                            )?;
                            let released_compact_lineage = release_runtime_compact_lineage(
                                shared,
                                &profile_name,
                                request_session_id.as_deref(),
                                request_turn_state.as_deref(),
                                "previous_response_not_found",
                            )?;
                            if released_affinity {
                                runtime_proxy_log(
                                    shared,
                                    format!(
                                        "request={request_id} transport=http previous_response_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                    ),
                                );
                            }
                            if bound_profile.as_deref() == Some(profile_name.as_str()) {
                                bound_profile = None;
                            }
                            if session_profile.as_deref() == Some(profile_name.as_str()) {
                                session_profile = None;
                            }
                            if candidate_turn_state_retry_profile.as_deref()
                                == Some(profile_name.as_str())
                            {
                                candidate_turn_state_retry_profile = None;
                                candidate_turn_state_retry_value = None;
                            }
                            if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                                pinned_profile = None;
                            }
                            if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                                turn_state_profile = None;
                            }
                            if compact_followup_profile
                                .as_ref()
                                .is_some_and(|(owner, _)| owner == &profile_name)
                                || released_compact_lineage
                            {
                                compact_followup_profile = None;
                            }
                            excluded_profiles.insert(profile_name);
                            last_failure = Some(RuntimeUpstreamFailureResponse::Http(response));
                            continue;
                        }
                        RuntimeResponsesAttempt::LocalSelectionBlocked { profile_name } => {
                            excluded_profiles.insert(profile_name);
                            continue;
                        }
                    }
                }
            }
            return Ok(match last_failure {
                Some(RuntimeUpstreamFailureResponse::Http(response)) => response,
                _ if saw_inflight_saturation => {
                    RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_response(
                        503,
                        "service_unavailable",
                        "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
                    ))
                }
                _ => RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_response(
                    503,
                    "service_unavailable",
                    runtime_proxy_local_selection_failure_message(),
                )),
            });
        };
        selection_attempts = selection_attempts.saturating_add(1);
        let turn_state_override =
            if candidate_turn_state_retry_profile.as_deref() == Some(candidate_name.as_str()) {
                candidate_turn_state_retry_value.as_deref()
            } else {
                request_turn_state.as_deref()
            };
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http candidate={} pinned={:?} turn_state_profile={:?} turn_state_override={:?} excluded_count={}",
                candidate_name,
                pinned_profile,
                turn_state_profile,
                turn_state_override,
                excluded_profiles.len()
            ),
        );
        if previous_response_id.is_none()
            && pinned_profile.is_none()
            && turn_state_profile.is_none()
            && runtime_profile_inflight_hard_limited_for_context(
                shared,
                &candidate_name,
                "responses_http",
            )?
        {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http profile_inflight_saturated profile={candidate_name} hard_limit={RUNTIME_PROFILE_INFLIGHT_HARD_LIMIT}"
                ),
            );
            excluded_profiles.insert(candidate_name);
            saw_inflight_saturation = true;
            continue;
        }

        match attempt_runtime_responses_request(
            request_id,
            &request,
            shared,
            &candidate_name,
            turn_state_override,
        )? {
            RuntimeResponsesAttempt::Success {
                profile_name,
                response,
            } => {
                if saw_previous_response_not_found {
                    remember_runtime_successful_previous_response_owner(
                        shared,
                        &profile_name,
                        previous_response_id.as_deref(),
                        RuntimeRouteKind::Responses,
                    )?;
                }
                commit_runtime_proxy_profile_selection_with_notice(
                    shared,
                    &profile_name,
                    RuntimeRouteKind::Responses,
                )?;
                let _ = release_runtime_compact_lineage(
                    shared,
                    &profile_name,
                    request_session_id.as_deref(),
                    request_turn_state.as_deref(),
                    "response_committed_post_commit",
                );
                runtime_proxy_log(
                    shared,
                    format!("request={request_id} transport=http committed profile={profile_name}"),
                );
                return Ok(response);
            }
            RuntimeResponsesAttempt::QuotaBlocked {
                profile_name,
                response,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http quota_blocked profile={profile_name}"
                    ),
                );
                mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                let released_affinity = release_runtime_quota_blocked_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                )?;
                if bound_profile.as_deref() == Some(profile_name.as_str()) {
                    bound_profile = None;
                }
                if session_profile.as_deref() == Some(profile_name.as_str()) {
                    session_profile = None;
                }
                if candidate_turn_state_retry_profile.as_deref() == Some(profile_name.as_str()) {
                    candidate_turn_state_retry_profile = None;
                    candidate_turn_state_retry_value = None;
                }
                if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                    pinned_profile = None;
                    previous_response_retry_index = 0;
                }
                if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                    turn_state_profile = None;
                }
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name}"
                        ),
                    );
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some(RuntimeUpstreamFailureResponse::Http(response));
            }
            RuntimeResponsesAttempt::LocalSelectionBlocked { profile_name } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http local_selection_blocked profile={profile_name} route=responses reason=quota_exhausted_before_send"
                    ),
                );
                if bound_profile.as_deref() == Some(profile_name.as_str()) {
                    bound_profile = None;
                }
                if session_profile.as_deref() == Some(profile_name.as_str()) {
                    session_profile = None;
                }
                if candidate_turn_state_retry_profile.as_deref() == Some(profile_name.as_str()) {
                    candidate_turn_state_retry_profile = None;
                    candidate_turn_state_retry_value = None;
                }
                if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                    pinned_profile = None;
                    previous_response_retry_index = 0;
                }
                if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                    turn_state_profile = None;
                }
                excluded_profiles.insert(profile_name);
            }
            RuntimeResponsesAttempt::PreviousResponseNotFound {
                profile_name,
                response,
                turn_state,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http route=responses previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?}",
                        turn_state
                    ),
                );
                saw_previous_response_not_found = true;
                if previous_response_retry_candidate.as_deref() != Some(profile_name.as_str()) {
                    previous_response_retry_candidate = Some(profile_name.clone());
                    previous_response_retry_index = 0;
                }
                let has_turn_state_retry = turn_state.is_some();
                if has_turn_state_retry {
                    candidate_turn_state_retry_profile = Some(profile_name.clone());
                    candidate_turn_state_retry_value = turn_state;
                }
                if has_turn_state_retry
                    && let Some(delay) =
                        runtime_previous_response_retry_delay(previous_response_retry_index)
                {
                    previous_response_retry_index += 1;
                    last_failure = Some(RuntimeUpstreamFailureResponse::Http(response));
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry",
                            delay.as_millis()
                        ),
                    );
                    continue;
                }
                previous_response_retry_candidate = None;
                previous_response_retry_index = 0;
                let released_affinity = release_runtime_previous_response_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                    RuntimeRouteKind::Responses,
                )?;
                let released_compact_lineage = release_runtime_compact_lineage(
                    shared,
                    &profile_name,
                    request_session_id.as_deref(),
                    request_turn_state.as_deref(),
                    "previous_response_not_found",
                )?;
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http previous_response_affinity_released profile={profile_name}"
                        ),
                    );
                }
                if bound_profile.as_deref() == Some(profile_name.as_str()) {
                    bound_profile = None;
                }
                if session_profile.as_deref() == Some(profile_name.as_str()) {
                    session_profile = None;
                }
                if candidate_turn_state_retry_profile.as_deref() == Some(profile_name.as_str()) {
                    candidate_turn_state_retry_profile = None;
                    candidate_turn_state_retry_value = None;
                }
                if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                    pinned_profile = None;
                }
                if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                    turn_state_profile = None;
                }
                if compact_followup_profile
                    .as_ref()
                    .is_some_and(|(owner, _)| owner == &profile_name)
                    || released_compact_lineage
                {
                    compact_followup_profile = None;
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some(RuntimeUpstreamFailureResponse::Http(response));
            }
        }
    }
}

fn attempt_runtime_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
) -> Result<RuntimeResponsesAttempt> {
    let request_session_id = runtime_request_session_id(request);
    let (quota_summary, quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, RuntimeRouteKind::Responses)?;
    if quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http responses_pre_send_skip profile={profile_name} route=responses quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(RuntimeResponsesAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
        });
    }
    let inflight_guard =
        acquire_runtime_profile_inflight_guard(shared, profile_name, "responses_http")?;
    let response = send_runtime_proxy_upstream_responses_request(
        request_id,
        request,
        shared,
        profile_name,
        turn_state_override,
    )
    .map_err(|err| {
        note_runtime_profile_transport_failure(
            shared,
            profile_name,
            RuntimeRouteKind::Responses,
            "responses_upstream_request",
            &err,
        );
        err
    })?;
    let response_turn_state = runtime_proxy_header_value(response.headers(), "x-codex-turn-state");
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let parts = buffer_runtime_proxy_async_response_parts(shared, response, Vec::new())
            .map_err(|err| {
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Responses,
                    "responses_buffer_response",
                    &err,
                );
                err
            })?;
        let retryable_quota = matches!(status, 403 | 429)
            && extract_runtime_proxy_quota_message(&parts.body).is_some();
        let retryable_previous =
            status == 400 && extract_runtime_proxy_previous_response_message(&parts.body).is_some();
        let response =
            RuntimeResponsesReply::Buffered(build_runtime_proxy_response_from_parts(parts));

        if retryable_quota {
            return Ok(RuntimeResponsesAttempt::QuotaBlocked {
                profile_name: profile_name.to_string(),
                response,
            });
        }
        if retryable_previous {
            return Ok(RuntimeResponsesAttempt::PreviousResponseNotFound {
                profile_name: profile_name.to_string(),
                response,
                turn_state: response_turn_state,
            });
        }

        return Ok(RuntimeResponsesAttempt::Success {
            profile_name: profile_name.to_string(),
            response,
        });
    }
    prepare_runtime_proxy_responses_success(
        request_id,
        runtime_request_previous_response_id(request).as_deref(),
        request_session_id.as_deref(),
        runtime_request_turn_state(request).as_deref(),
        response,
        shared,
        profile_name,
        inflight_guard,
    )
    .map_err(|err| {
        note_runtime_profile_transport_failure(
            shared,
            profile_name,
            RuntimeRouteKind::Responses,
            "responses_prepare_success",
            &err,
        );
        err
    })
}

fn next_runtime_response_candidate_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let now = Local::now().timestamp();
    let (
        state,
        current_profile,
        include_code_review,
        upstream_base_url,
        cached_reports,
        cached_usage_snapshots,
        retry_backoff_until,
        transport_backoff_until,
        route_circuit_open_until,
        profile_inflight,
        profile_health,
    ) = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        (
            runtime.state.clone(),
            runtime.current_profile.clone(),
            runtime.include_code_review,
            runtime.upstream_base_url.clone(),
            runtime.profile_probe_cache.clone(),
            runtime.profile_usage_snapshots.clone(),
            runtime.profile_retry_backoff_until.clone(),
            runtime.profile_transport_backoff_until.clone(),
            runtime.profile_route_circuit_open_until.clone(),
            runtime.profile_inflight.clone(),
            runtime.profile_health.clone(),
        )
    };

    let mut reports = Vec::new();
    let mut cold_start_probe_jobs = Vec::new();
    for (order_index, name) in active_profile_selection_order(&state, &current_profile)
        .into_iter()
        .enumerate()
    {
        if excluded_profiles.contains(&name) {
            continue;
        }
        let Some(profile) = state.profiles.get(&name) else {
            continue;
        };
        if let Some(entry) = cached_reports.get(&name) {
            reports.push(RunProfileProbeReport {
                name: name.clone(),
                order_index,
                auth: entry.auth.clone(),
                result: entry.result.clone(),
            });
            if runtime_profile_probe_cache_freshness(entry, now)
                != RuntimeProbeCacheFreshness::Fresh
            {
                schedule_runtime_probe_refresh(shared, &name, &profile.codex_home);
            }
        } else {
            let auth = read_auth_summary(&profile.codex_home);
            reports.push(RunProfileProbeReport {
                name: name.clone(),
                order_index,
                auth,
                result: Err("runtime quota snapshot unavailable".to_string()),
            });
            cold_start_probe_jobs.push(RunProfileProbeJob {
                name,
                order_index,
                codex_home: profile.codex_home.clone(),
            });
        }
    }

    cold_start_probe_jobs.sort_by_key(|job| {
        let quota_summary = cached_usage_snapshots
            .get(&job.name)
            .filter(|snapshot| runtime_usage_snapshot_is_usable(snapshot, now))
            .map(|snapshot| runtime_quota_summary_from_usage_snapshot(snapshot, route_kind))
            .unwrap_or(RuntimeQuotaSummary {
                five_hour: RuntimeQuotaWindowSummary {
                    status: RuntimeQuotaWindowStatus::Unknown,
                    remaining_percent: 0,
                    reset_at: i64::MAX,
                },
                weekly: RuntimeQuotaWindowSummary {
                    status: RuntimeQuotaWindowStatus::Unknown,
                    remaining_percent: 0,
                    reset_at: i64::MAX,
                },
                route_band: RuntimeQuotaPressureBand::Unknown,
            });
        (
            runtime_quota_pressure_sort_key_for_route_from_summary(quota_summary),
            job.order_index,
        )
    });

    reports.sort_by_key(|report| report.order_index);
    let mut candidates = ready_profile_candidates(
        &reports,
        include_code_review,
        Some(current_profile.as_str()),
        &state,
        Some(&cached_usage_snapshots),
    );

    if candidates.is_empty() && !cold_start_probe_jobs.is_empty() {
        let base_url = Some(upstream_base_url.clone());
        let sync_jobs = cold_start_probe_jobs
            .iter()
            .take(RUNTIME_PROFILE_SYNC_PROBE_FALLBACK_LIMIT)
            .map(|job| RunProfileProbeJob {
                name: job.name.clone(),
                order_index: job.order_index,
                codex_home: job.codex_home.clone(),
            })
            .collect::<Vec<_>>();
        let probed_names = sync_jobs
            .iter()
            .map(|job| job.name.clone())
            .collect::<BTreeSet<_>>();
        let fresh_reports = map_parallel(sync_jobs, |job| {
            let auth = read_auth_summary(&job.codex_home);
            let result = if auth.quota_compatible {
                fetch_usage(&job.codex_home, base_url.as_deref()).map_err(|err| err.to_string())
            } else {
                Err("auth mode is not quota-compatible".to_string())
            };

            RunProfileProbeReport {
                name: job.name,
                order_index: job.order_index,
                auth,
                result,
            }
        });

        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        let mut usage_snapshot_changed = false;
        for report in &fresh_reports {
            runtime.profile_probe_cache.insert(
                report.name.clone(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: report.auth.clone(),
                    result: report.result.clone(),
                },
            );
            if let Ok(usage) = report.result.as_ref() {
                runtime.profile_usage_snapshots.insert(
                    report.name.clone(),
                    runtime_profile_usage_snapshot_from_usage(usage),
                );
                usage_snapshot_changed = true;
            }
        }
        let state_snapshot = runtime.state.clone();
        let continuations_snapshot = runtime_continuation_store_snapshot(&runtime);
        let profile_scores_snapshot = runtime.profile_health.clone();
        let usage_snapshots = runtime.profile_usage_snapshots.clone();
        let backoffs_snapshot = runtime_profile_backoffs_snapshot(&runtime);
        let paths_snapshot = runtime.paths.clone();
        drop(runtime);
        if usage_snapshot_changed {
            schedule_runtime_state_save(
                shared,
                state_snapshot,
                continuations_snapshot,
                profile_scores_snapshot,
                usage_snapshots,
                backoffs_snapshot,
                paths_snapshot,
                "usage_snapshot:sync_probe",
            );
        }

        for fresh_report in fresh_reports {
            if let Some(existing) = reports
                .iter_mut()
                .find(|report| report.name == fresh_report.name)
            {
                *existing = fresh_report;
            }
        }
        reports.sort_by_key(|report| report.order_index);
        candidates = ready_profile_candidates(
            &reports,
            include_code_review,
            Some(current_profile.as_str()),
            &state,
            Some(&cached_usage_snapshots),
        );
        for job in cold_start_probe_jobs
            .into_iter()
            .filter(|job| !probed_names.contains(&job.name))
        {
            schedule_runtime_probe_refresh(shared, &job.name, &job.codex_home);
        }
    } else {
        for job in cold_start_probe_jobs {
            schedule_runtime_probe_refresh(shared, &job.name, &job.codex_home);
        }
    }
    let available_candidates = candidates
        .into_iter()
        .enumerate()
        .filter(|(_, candidate)| !excluded_profiles.contains(&candidate.name))
        .collect::<Vec<_>>();

    let mut ready_candidates = available_candidates
        .iter()
        .filter(|(_, candidate)| {
            !runtime_profile_name_in_selection_backoff(
                &candidate.name,
                &retry_backoff_until,
                &transport_backoff_until,
                &route_circuit_open_until,
                route_kind,
                now,
            )
        })
        .collect::<Vec<_>>();
    ready_candidates.sort_by_key(|(index, candidate)| {
        (
            runtime_quota_pressure_sort_key_for_route(&candidate.usage, route_kind),
            runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
            runtime_profile_health_sort_key(&candidate.name, &profile_health, now, route_kind),
            *index,
            runtime_profile_selection_jitter(shared, &candidate.name, route_kind),
        )
    });
    for (index, candidate) in ready_candidates {
        if !reserve_runtime_profile_route_circuit_half_open_probe(
            shared,
            &candidate.name,
            route_kind,
        )? {
            let quota_summary = runtime_quota_summary_for_route(&candidate.usage, route_kind);
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason=route_circuit_half_open_probe_wait inflight={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
                    runtime_profile_health_sort_key(
                        &candidate.name,
                        &profile_health,
                        now,
                        route_kind
                    ),
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            continue;
        }
        let quota_summary = runtime_quota_summary_for_route(&candidate.usage, route_kind);
        runtime_proxy_log(
            shared,
            format!(
                "selection_pick route={} profile={} mode=ready inflight={} health={} order={} {}",
                runtime_route_kind_label(route_kind),
                candidate.name,
                runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
                runtime_profile_health_sort_key(&candidate.name, &profile_health, now, route_kind),
                index,
                format!(
                    "quota_source={} {}",
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(quota_summary)
                ),
            ),
        );
        return Ok(Some(candidate.name.clone()));
    }

    let mut fallback_candidates = available_candidates.into_iter().collect::<Vec<_>>();
    fallback_candidates.sort_by_key(|(index, candidate)| {
        (
            runtime_profile_backoff_sort_key(
                &candidate.name,
                &retry_backoff_until,
                &transport_backoff_until,
                &route_circuit_open_until,
                route_kind,
                now,
            ),
            runtime_quota_pressure_sort_key_for_route(&candidate.usage, route_kind),
            runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
            runtime_profile_health_sort_key(&candidate.name, &profile_health, now, route_kind),
            *index,
            runtime_profile_selection_jitter(shared, &candidate.name, route_kind),
        )
    });
    let mut fallback = None;
    for (index, candidate) in fallback_candidates {
        if !reserve_runtime_profile_route_circuit_half_open_probe(
            shared,
            &candidate.name,
            route_kind,
        )? {
            let quota_summary = runtime_quota_summary_for_route(&candidate.usage, route_kind);
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason=route_circuit_half_open_probe_wait inflight={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
                    runtime_profile_health_sort_key(
                        &candidate.name,
                        &profile_health,
                        now,
                        route_kind
                    ),
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            continue;
        }
        let quota_summary = runtime_quota_summary_for_route(&candidate.usage, route_kind);
        runtime_proxy_log(
            shared,
            format!(
                "selection_pick route={} profile={} mode=backoff inflight={} health={} backoff={:?} order={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                candidate.name,
                runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
                runtime_profile_health_sort_key(&candidate.name, &profile_health, now, route_kind),
                runtime_profile_backoff_sort_key(
                    &candidate.name,
                    &retry_backoff_until,
                    &transport_backoff_until,
                    &route_circuit_open_until,
                    route_kind,
                    now,
                ),
                index,
                runtime_quota_source_label(candidate.quota_source),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        fallback = Some(candidate.name);
        break;
    }

    if fallback.is_none() {
        runtime_proxy_log(
            shared,
            format!(
                "selection_pick route={} profile=none mode=exhausted excluded_count={}",
                runtime_route_kind_label(route_kind),
                excluded_profiles.len()
            ),
        );
    }

    Ok(fallback)
}

fn runtime_profile_usage_cache_is_fresh(entry: &RuntimeProfileProbeCacheEntry, now: i64) -> bool {
    now.saturating_sub(entry.checked_at) <= RUNTIME_PROFILE_USAGE_CACHE_FRESH_SECONDS
}

fn runtime_profile_probe_cache_freshness(
    entry: &RuntimeProfileProbeCacheEntry,
    now: i64,
) -> RuntimeProbeCacheFreshness {
    let age = now.saturating_sub(entry.checked_at);
    if age <= RUNTIME_PROFILE_USAGE_CACHE_FRESH_SECONDS {
        RuntimeProbeCacheFreshness::Fresh
    } else if age <= RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS {
        RuntimeProbeCacheFreshness::StaleUsable
    } else {
        RuntimeProbeCacheFreshness::Expired
    }
}

fn update_runtime_profile_probe_cache_with_usage(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    usage: UsageResponse,
) -> Result<()> {
    let auth = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .state
        .profiles
        .get(profile_name)
        .map(|profile| read_auth_summary(&profile.codex_home))
        .unwrap_or(AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        });
    apply_runtime_profile_probe_result(shared, profile_name, auth, Ok(usage))
}

fn runtime_proxy_current_profile(shared: &RuntimeRotationProxyShared) -> Result<String> {
    Ok(shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .current_profile
        .clone())
}

fn runtime_profile_in_retry_backoff(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    now: i64,
) -> bool {
    runtime
        .profile_retry_backoff_until
        .get(profile_name)
        .copied()
        .is_some_and(|until| until > now)
}

fn runtime_profile_in_transport_backoff(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    now: i64,
) -> bool {
    runtime
        .profile_transport_backoff_until
        .get(profile_name)
        .copied()
        .is_some_and(|until| until > now)
}

fn runtime_profile_inflight_count(runtime: &RuntimeRotationState, profile_name: &str) -> usize {
    runtime
        .profile_inflight
        .get(profile_name)
        .copied()
        .unwrap_or(0)
}

fn runtime_profile_inflight_hard_limit_context(context: &str) -> usize {
    runtime_profile_inflight_weight(context)
}

fn runtime_profile_inflight_hard_limited_for_context(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    context: &str,
) -> Result<bool> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    Ok(runtime_profile_inflight_count(&runtime, profile_name)
        .saturating_add(runtime_profile_inflight_hard_limit_context(context))
        > RUNTIME_PROFILE_INFLIGHT_HARD_LIMIT)
}

fn runtime_profile_in_selection_backoff(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    now: i64,
) -> bool {
    runtime_profile_in_retry_backoff(runtime, profile_name, now)
        || runtime_profile_in_transport_backoff(runtime, profile_name, now)
}

fn runtime_profile_health_score(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_profile_global_health_score(runtime, profile_name, now)
        .saturating_add(runtime_profile_route_health_score(
            runtime,
            profile_name,
            now,
            route_kind,
        ))
        .saturating_add(runtime_profile_route_coupling_score(
            runtime,
            profile_name,
            now,
            route_kind,
        ))
}

fn runtime_route_coupled_kinds(route_kind: RuntimeRouteKind) -> &'static [RuntimeRouteKind] {
    match route_kind {
        RuntimeRouteKind::Responses => &[RuntimeRouteKind::Websocket],
        RuntimeRouteKind::Websocket => &[RuntimeRouteKind::Responses],
        RuntimeRouteKind::Compact => &[RuntimeRouteKind::Standard],
        RuntimeRouteKind::Standard => &[RuntimeRouteKind::Compact],
    }
}

fn runtime_profile_route_coupling_score(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_profile_route_coupling_score_from_map(
        &runtime.profile_health,
        profile_name,
        now,
        route_kind,
    )
}

fn runtime_profile_route_coupling_score_from_map(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_route_coupled_kinds(route_kind)
        .iter()
        .copied()
        .map(|coupled_kind| {
            let route_score = runtime_profile_effective_health_score_from_map(
                profile_health,
                &runtime_profile_route_health_key(profile_name, coupled_kind),
                now,
            );
            let bad_pairing_score = runtime_profile_effective_score_from_map(
                profile_health,
                &runtime_profile_route_bad_pairing_key(profile_name, coupled_kind),
                now,
                RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
            );
            route_score
                .saturating_add(bad_pairing_score)
                .saturating_div(2)
        })
        .fold(0, u32::saturating_add)
}

fn runtime_profile_selection_jitter(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    shared
        .request_sequence
        .load(Ordering::Relaxed)
        .hash(&mut hasher);
    profile_name.hash(&mut hasher);
    runtime_route_kind_label(route_kind).hash(&mut hasher);
    hasher.finish()
}

fn prune_runtime_profile_retry_backoff(runtime: &mut RuntimeRotationState, now: i64) {
    runtime
        .profile_retry_backoff_until
        .retain(|_, until| *until > now);
}

fn prune_runtime_profile_transport_backoff(runtime: &mut RuntimeRotationState, now: i64) {
    runtime
        .profile_transport_backoff_until
        .retain(|_, until| *until > now);
}

fn prune_runtime_profile_route_circuits(runtime: &mut RuntimeRotationState, now: i64) {
    runtime
        .profile_route_circuit_open_until
        .retain(|key, until| {
            if *until > now {
                return true;
            }
            let health_key = runtime_profile_route_circuit_health_key(key);
            runtime_profile_effective_health_score_from_map(
                &runtime.profile_health,
                &health_key,
                now,
            ) > 0
        });
}

fn prune_runtime_profile_selection_backoff(runtime: &mut RuntimeRotationState, now: i64) {
    prune_runtime_profile_retry_backoff(runtime, now);
    prune_runtime_profile_transport_backoff(runtime, now);
    prune_runtime_profile_route_circuits(runtime, now);
}

fn runtime_profile_name_in_selection_backoff(
    profile_name: &str,
    retry_backoff_until: &BTreeMap<String, i64>,
    transport_backoff_until: &BTreeMap<String, i64>,
    route_circuit_open_until: &BTreeMap<String, i64>,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    retry_backoff_until
        .get(profile_name)
        .copied()
        .is_some_and(|until| until > now)
        || transport_backoff_until
            .get(profile_name)
            .copied()
            .is_some_and(|until| until > now)
        || route_circuit_open_until
            .get(&runtime_profile_route_circuit_key(profile_name, route_kind))
            .copied()
            .is_some_and(|until| until > now)
}

fn runtime_profile_backoff_sort_key(
    profile_name: &str,
    retry_backoff_until: &BTreeMap<String, i64>,
    transport_backoff_until: &BTreeMap<String, i64>,
    route_circuit_open_until: &BTreeMap<String, i64>,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> (usize, i64, i64, i64) {
    let retry_until = retry_backoff_until
        .get(profile_name)
        .copied()
        .filter(|until| *until > now);
    let transport_until = transport_backoff_until
        .get(profile_name)
        .copied()
        .filter(|until| *until > now);
    let circuit_until = route_circuit_open_until
        .get(&runtime_profile_route_circuit_key(profile_name, route_kind))
        .copied()
        .filter(|until| *until > now);

    match (circuit_until, transport_until, retry_until) {
        (None, None, None) => (0, 0, 0, 0),
        (Some(circuit_until), None, None) => (1, circuit_until, 0, 0),
        (None, Some(transport_until), None) => (2, transport_until, 0, 0),
        (None, None, Some(retry_until)) => (3, retry_until, 0, 0),
        (Some(circuit_until), Some(transport_until), None) => (
            4,
            circuit_until.min(transport_until),
            circuit_until.max(transport_until),
            0,
        ),
        (Some(circuit_until), None, Some(retry_until)) => (
            5,
            circuit_until.min(retry_until),
            circuit_until.max(retry_until),
            0,
        ),
        (None, Some(transport_until), Some(retry_until)) => (
            6,
            transport_until.min(retry_until),
            transport_until.max(retry_until),
            0,
        ),
        (Some(circuit_until), Some(transport_until), Some(retry_until)) => (
            7,
            circuit_until.min(transport_until.min(retry_until)),
            circuit_until.max(transport_until.max(retry_until)),
            retry_until,
        ),
    }
}

fn runtime_profile_health_sort_key(
    profile_name: &str,
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_profile_effective_health_score_from_map(profile_health, profile_name, now)
        .saturating_add(runtime_profile_effective_health_score_from_map(
            profile_health,
            &runtime_profile_route_health_key(profile_name, route_kind),
            now,
        ))
        .saturating_add(runtime_profile_effective_score_from_map(
            profile_health,
            &runtime_profile_route_bad_pairing_key(profile_name, route_kind),
            now,
            RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
        ))
        .saturating_add(runtime_profile_route_coupling_score_from_map(
            profile_health,
            profile_name,
            now,
            route_kind,
        ))
        .saturating_add(runtime_profile_route_performance_score(
            profile_health,
            profile_name,
            now,
            route_kind,
        ))
}

fn runtime_profile_inflight_sort_key(
    profile_name: &str,
    profile_inflight: &BTreeMap<String, usize>,
) -> usize {
    profile_inflight.get(profile_name).copied().unwrap_or(0)
}

fn runtime_profile_inflight_weight(context: &str) -> usize {
    match context {
        "websocket_session" | "responses_http" => 2,
        _ => 1,
    }
}

fn runtime_route_kind_inflight_context(route_kind: RuntimeRouteKind) -> &'static str {
    match route_kind {
        RuntimeRouteKind::Responses => "responses_http",
        RuntimeRouteKind::Compact => "compact_http",
        RuntimeRouteKind::Websocket => "websocket_session",
        RuntimeRouteKind::Standard => "standard_http",
    }
}

fn runtime_quota_pressure_band_reason(band: RuntimeQuotaPressureBand) -> &'static str {
    match band {
        RuntimeQuotaPressureBand::Healthy => "quota_healthy",
        RuntimeQuotaPressureBand::Thin => "quota_thin",
        RuntimeQuotaPressureBand::Critical => "quota_critical",
        RuntimeQuotaPressureBand::Exhausted => "quota_exhausted",
        RuntimeQuotaPressureBand::Unknown => "quota_unknown",
    }
}

fn runtime_quota_window_status_reason(status: RuntimeQuotaWindowStatus) -> &'static str {
    match status {
        RuntimeQuotaWindowStatus::Ready => "ready",
        RuntimeQuotaWindowStatus::Thin => "thin",
        RuntimeQuotaWindowStatus::Critical => "critical",
        RuntimeQuotaWindowStatus::Exhausted => "exhausted",
        RuntimeQuotaWindowStatus::Unknown => "unknown",
    }
}

fn runtime_quota_window_summary(usage: &UsageResponse, label: &str) -> RuntimeQuotaWindowSummary {
    let Some(window) = required_main_window_snapshot(usage, label) else {
        return RuntimeQuotaWindowSummary {
            status: RuntimeQuotaWindowStatus::Unknown,
            remaining_percent: 0,
            reset_at: i64::MAX,
        };
    };
    let status = if window.remaining_percent == 0 {
        RuntimeQuotaWindowStatus::Exhausted
    } else if window.remaining_percent <= 5 {
        RuntimeQuotaWindowStatus::Critical
    } else if window.remaining_percent <= 15 {
        RuntimeQuotaWindowStatus::Thin
    } else {
        RuntimeQuotaWindowStatus::Ready
    };
    RuntimeQuotaWindowSummary {
        status,
        remaining_percent: window.remaining_percent,
        reset_at: window.reset_at,
    }
}

fn runtime_quota_summary_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaSummary {
    RuntimeQuotaSummary {
        five_hour: runtime_quota_window_summary(usage, "5h"),
        weekly: runtime_quota_window_summary(usage, "weekly"),
        route_band: runtime_quota_pressure_band_for_route(usage, route_kind),
    }
}

fn runtime_profile_usage_snapshot_from_usage(usage: &UsageResponse) -> RuntimeProfileUsageSnapshot {
    let five_hour = runtime_quota_window_summary(usage, "5h");
    let weekly = runtime_quota_window_summary(usage, "weekly");
    RuntimeProfileUsageSnapshot {
        checked_at: Local::now().timestamp(),
        five_hour_status: five_hour.status,
        five_hour_remaining_percent: five_hour.remaining_percent,
        five_hour_reset_at: five_hour.reset_at,
        weekly_status: weekly.status,
        weekly_remaining_percent: weekly.remaining_percent,
        weekly_reset_at: weekly.reset_at,
    }
}

fn runtime_quota_summary_from_usage_snapshot(
    snapshot: &RuntimeProfileUsageSnapshot,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaSummary {
    let five_hour = RuntimeQuotaWindowSummary {
        status: snapshot.five_hour_status,
        remaining_percent: snapshot.five_hour_remaining_percent,
        reset_at: snapshot.five_hour_reset_at,
    };
    let weekly = RuntimeQuotaWindowSummary {
        status: snapshot.weekly_status,
        remaining_percent: snapshot.weekly_remaining_percent,
        reset_at: snapshot.weekly_reset_at,
    };
    let route_band = [
        five_hour.status,
        weekly.status,
        match route_kind {
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => weekly.status,
            RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => five_hour.status,
        },
    ]
    .into_iter()
    .fold(RuntimeQuotaPressureBand::Healthy, |band, status| {
        band.max(match status {
            RuntimeQuotaWindowStatus::Ready => RuntimeQuotaPressureBand::Healthy,
            RuntimeQuotaWindowStatus::Thin => RuntimeQuotaPressureBand::Thin,
            RuntimeQuotaWindowStatus::Critical => RuntimeQuotaPressureBand::Critical,
            RuntimeQuotaWindowStatus::Exhausted => RuntimeQuotaPressureBand::Exhausted,
            RuntimeQuotaWindowStatus::Unknown => RuntimeQuotaPressureBand::Unknown,
        })
    });
    RuntimeQuotaSummary {
        five_hour,
        weekly,
        route_band,
    }
}

fn usage_from_runtime_usage_snapshot(snapshot: &RuntimeProfileUsageSnapshot) -> UsageResponse {
    UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some((100 - snapshot.five_hour_remaining_percent).clamp(0, 100)),
                reset_at: (snapshot.five_hour_reset_at != i64::MAX)
                    .then_some(snapshot.five_hour_reset_at),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some((100 - snapshot.weekly_remaining_percent).clamp(0, 100)),
                reset_at: (snapshot.weekly_reset_at != i64::MAX)
                    .then_some(snapshot.weekly_reset_at),
                limit_window_seconds: Some(604_800),
            }),
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    }
}

fn runtime_profile_backoffs_snapshot(runtime: &RuntimeRotationState) -> RuntimeProfileBackoffs {
    RuntimeProfileBackoffs {
        retry_backoff_until: runtime.profile_retry_backoff_until.clone(),
        transport_backoff_until: runtime.profile_transport_backoff_until.clone(),
        route_circuit_open_until: runtime.profile_route_circuit_open_until.clone(),
    }
}

fn runtime_continuation_store_snapshot(runtime: &RuntimeRotationState) -> RuntimeContinuationStore {
    RuntimeContinuationStore {
        response_profile_bindings: runtime.state.response_profile_bindings.clone(),
        session_profile_bindings: runtime.state.session_profile_bindings.clone(),
        turn_state_bindings: runtime.turn_state_bindings.clone(),
        session_id_bindings: runtime.session_id_bindings.clone(),
        statuses: runtime.continuation_statuses.clone(),
    }
}

const RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD: u32 = 4;
const RUNTIME_PROFILE_CIRCUIT_OPEN_SECONDS: i64 = 20;
const RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS: i64 = 5;

fn runtime_profile_route_circuit_key(profile_name: &str, route_kind: RuntimeRouteKind) -> String {
    format!(
        "__route_circuit__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

fn runtime_profile_route_circuit_profile_name(key: &str) -> &str {
    key.rsplit(':').next().unwrap_or(key)
}

fn runtime_profile_route_circuit_health_key(key: &str) -> String {
    key.replacen("__route_circuit__", "__route_health__", 1)
}

fn runtime_profile_route_circuit_open_until(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> Option<i64> {
    runtime
        .profile_route_circuit_open_until
        .get(&runtime_profile_route_circuit_key(profile_name, route_kind))
        .copied()
        .filter(|until| *until > now)
}

fn reserve_runtime_profile_route_circuit_half_open_probe(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> Result<bool> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let key = runtime_profile_route_circuit_key(profile_name, route_kind);
    let Some(until) = runtime.profile_route_circuit_open_until.get(&key).copied() else {
        return Ok(true);
    };
    if until > now {
        return Ok(false);
    }
    let health_key = runtime_profile_route_circuit_health_key(&key);
    let health_score =
        runtime_profile_effective_health_score_from_map(&runtime.profile_health, &health_key, now);
    if health_score == 0 {
        runtime.profile_route_circuit_open_until.remove(&key);
        return Ok(true);
    }

    let reserve_until = now.saturating_add(RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS);
    runtime
        .profile_route_circuit_open_until
        .insert(key, reserve_until);
    let state_snapshot = runtime.state.clone();
    let continuations_snapshot = runtime_continuation_store_snapshot(&runtime);
    let profile_scores_snapshot = runtime.profile_health.clone();
    let usage_snapshots = runtime.profile_usage_snapshots.clone();
    let backoffs_snapshot = runtime_profile_backoffs_snapshot(&runtime);
    let paths_snapshot = runtime.paths.clone();
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "profile_circuit_half_open_probe profile={profile_name} route={} until={reserve_until} health={health_score}",
            runtime_route_kind_label(route_kind)
        ),
    );
    schedule_runtime_state_save(
        shared,
        state_snapshot,
        continuations_snapshot,
        profile_scores_snapshot,
        usage_snapshots,
        backoffs_snapshot,
        paths_snapshot,
        &format!(
            "profile_circuit_half_open_probe:{profile_name}:{}",
            runtime_route_kind_label(route_kind)
        ),
    );
    Ok(true)
}

fn clear_runtime_profile_circuit_for_route(
    runtime: &mut RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> bool {
    runtime
        .profile_route_circuit_open_until
        .remove(&runtime_profile_route_circuit_key(profile_name, route_kind))
        .is_some()
}

fn runtime_quota_source_label(source: RuntimeQuotaSource) -> &'static str {
    match source {
        RuntimeQuotaSource::LiveProbe => "probe_cache",
        RuntimeQuotaSource::PersistedSnapshot => "persisted_snapshot",
    }
}

fn runtime_usage_snapshot_is_usable(snapshot: &RuntimeProfileUsageSnapshot, now: i64) -> bool {
    now.saturating_sub(snapshot.checked_at) <= RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS
}

fn runtime_quota_summary_log_fields(summary: RuntimeQuotaSummary) -> String {
    format!(
        "quota_band={} five_hour_status={} five_hour_remaining={} five_hour_reset_at={} weekly_status={} weekly_remaining={} weekly_reset_at={}",
        runtime_quota_pressure_band_reason(summary.route_band),
        runtime_quota_window_status_reason(summary.five_hour.status),
        summary.five_hour.remaining_percent,
        summary.five_hour.reset_at,
        runtime_quota_window_status_reason(summary.weekly.status),
        summary.weekly.remaining_percent,
        summary.weekly.reset_at,
    )
}

fn runtime_quota_pressure_sort_key_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> (
    RuntimeQuotaPressureBand,
    i64,
    i64,
    i64,
    Reverse<i64>,
    Reverse<i64>,
    Reverse<i64>,
    i64,
    i64,
) {
    let score = ready_profile_score_for_route(usage, route_kind);
    (
        runtime_quota_pressure_band_for_route(usage, route_kind),
        score.total_pressure,
        score.weekly_pressure,
        score.five_hour_pressure,
        Reverse(score.reserve_floor),
        Reverse(score.weekly_remaining),
        Reverse(score.five_hour_remaining),
        score.weekly_reset_at,
        score.five_hour_reset_at,
    )
}

fn runtime_quota_pressure_sort_key_for_route_from_summary(
    summary: RuntimeQuotaSummary,
) -> (
    RuntimeQuotaPressureBand,
    i64,
    i64,
    i64,
    Reverse<i64>,
    Reverse<i64>,
    Reverse<i64>,
    i64,
    i64,
) {
    (
        summary.route_band,
        match summary.route_band {
            RuntimeQuotaPressureBand::Healthy => 0,
            RuntimeQuotaPressureBand::Thin => 1,
            RuntimeQuotaPressureBand::Critical => 2,
            RuntimeQuotaPressureBand::Exhausted => 3,
            RuntimeQuotaPressureBand::Unknown => 4,
        },
        match summary.weekly.status {
            RuntimeQuotaWindowStatus::Ready => 0,
            RuntimeQuotaWindowStatus::Thin => 1,
            RuntimeQuotaWindowStatus::Critical => 2,
            RuntimeQuotaWindowStatus::Exhausted => 3,
            RuntimeQuotaWindowStatus::Unknown => 4,
        },
        match summary.five_hour.status {
            RuntimeQuotaWindowStatus::Ready => 0,
            RuntimeQuotaWindowStatus::Thin => 1,
            RuntimeQuotaWindowStatus::Critical => 2,
            RuntimeQuotaWindowStatus::Exhausted => 3,
            RuntimeQuotaWindowStatus::Unknown => 4,
        },
        Reverse(
            summary
                .weekly
                .remaining_percent
                .min(summary.five_hour.remaining_percent),
        ),
        Reverse(summary.weekly.remaining_percent),
        Reverse(summary.five_hour.remaining_percent),
        summary.weekly.reset_at,
        summary.five_hour.reset_at,
    )
}

fn runtime_quota_pressure_band_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaPressureBand {
    let Some(weekly) = required_main_window_snapshot(usage, "weekly") else {
        return RuntimeQuotaPressureBand::Unknown;
    };
    let Some(five_hour) = required_main_window_snapshot(usage, "5h") else {
        return RuntimeQuotaPressureBand::Unknown;
    };

    let weekly_remaining = weekly.remaining_percent;
    let five_hour_remaining = five_hour.remaining_percent;
    if weekly_remaining == 0 || five_hour_remaining == 0 {
        return RuntimeQuotaPressureBand::Exhausted;
    }

    let (thin_weekly, thin_five_hour, critical_weekly, critical_five_hour) = match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => (20, 10, 10, 5),
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => (10, 5, 5, 3),
    };

    if weekly_remaining <= critical_weekly || five_hour_remaining <= critical_five_hour {
        RuntimeQuotaPressureBand::Critical
    } else if weekly_remaining <= thin_weekly || five_hour_remaining <= thin_five_hour {
        RuntimeQuotaPressureBand::Thin
    } else {
        RuntimeQuotaPressureBand::Healthy
    }
}

fn runtime_profile_effective_health_score(entry: &RuntimeProfileHealth, now: i64) -> u32 {
    runtime_profile_effective_score(entry, now, RUNTIME_PROFILE_HEALTH_DECAY_SECONDS)
}

fn runtime_profile_effective_score(
    entry: &RuntimeProfileHealth,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    let decay = now
        .saturating_sub(entry.updated_at)
        .saturating_div(decay_seconds.max(1))
        .clamp(0, i64::from(u32::MAX)) as u32;
    entry.score.saturating_sub(decay)
}

fn runtime_profile_effective_health_score_from_map(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    key: &str,
    now: i64,
) -> u32 {
    profile_health
        .get(key)
        .map(|entry| runtime_profile_effective_health_score(entry, now))
        .unwrap_or(0)
}

fn runtime_profile_effective_score_from_map(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    key: &str,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    profile_health
        .get(key)
        .map(|entry| runtime_profile_effective_score(entry, now, decay_seconds))
        .unwrap_or(0)
}

fn runtime_profile_global_health_score(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    now: i64,
) -> u32 {
    runtime_profile_effective_health_score_from_map(&runtime.profile_health, profile_name, now)
}

fn runtime_profile_route_health_key(profile_name: &str, route_kind: RuntimeRouteKind) -> String {
    format!(
        "__route_health__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

fn runtime_profile_route_bad_pairing_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_bad_pairing__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

fn runtime_profile_route_success_streak_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_success__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

fn runtime_profile_route_performance_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_performance__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

fn runtime_profile_route_health_score(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_profile_effective_health_score_from_map(
        &runtime.profile_health,
        &runtime_profile_route_health_key(profile_name, route_kind),
        now,
    )
}

fn runtime_profile_route_performance_score(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    let route_score = runtime_profile_effective_score_from_map(
        profile_health,
        &runtime_profile_route_performance_key(profile_name, route_kind),
        now,
        RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
    );
    let coupled_score = runtime_route_coupled_kinds(route_kind)
        .iter()
        .copied()
        .map(|coupled_kind| {
            runtime_profile_effective_score_from_map(
                profile_health,
                &runtime_profile_route_performance_key(profile_name, coupled_kind),
                now,
                RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
            )
            .saturating_div(2)
        })
        .fold(0, u32::saturating_add);
    route_score.saturating_add(coupled_score)
}

fn runtime_profile_latency_penalty(
    elapsed_ms: u64,
    route_kind: RuntimeRouteKind,
    stage: &str,
) -> u32 {
    let (good_ms, warn_ms, poor_ms, severe_ms) = match (route_kind, stage) {
        (RuntimeRouteKind::Responses, "ttfb") | (RuntimeRouteKind::Websocket, "connect") => {
            (120, 300, 700, 1_500)
        }
        (RuntimeRouteKind::Compact, _) | (RuntimeRouteKind::Standard, _) => (80, 180, 400, 900),
        _ => (100, 250, 600, 1_200),
    };
    match elapsed_ms {
        elapsed if elapsed <= good_ms => 0,
        elapsed if elapsed <= warn_ms => 2,
        elapsed if elapsed <= poor_ms => 4,
        elapsed if elapsed <= severe_ms => 7,
        _ => RUNTIME_PROFILE_LATENCY_PENALTY_MAX,
    }
}

fn update_runtime_profile_route_performance(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    next_score: u32,
    reason: &str,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let key = runtime_profile_route_performance_key(profile_name, route_kind);
    if next_score == 0 {
        runtime.profile_health.remove(&key);
    } else {
        runtime.profile_health.insert(
            key,
            RuntimeProfileHealth {
                score: next_score.min(RUNTIME_PROFILE_LATENCY_PENALTY_MAX),
                updated_at: now,
            },
        );
    }
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "profile_latency profile={profile_name} route={} score={} reason={reason}",
            runtime_route_kind_label(route_kind),
            next_score.min(RUNTIME_PROFILE_LATENCY_PENALTY_MAX),
        ),
    );
    Ok(())
}

fn note_runtime_profile_latency_observation(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    stage: &str,
    elapsed_ms: u64,
) {
    let current_score = shared
        .runtime
        .lock()
        .ok()
        .map(|runtime| {
            let now = Local::now().timestamp();
            runtime_profile_effective_score_from_map(
                &runtime.profile_health,
                &runtime_profile_route_performance_key(profile_name, route_kind),
                now,
                RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
            )
        })
        .unwrap_or(0);
    let observed = runtime_profile_latency_penalty(elapsed_ms, route_kind, stage);
    let next_score = if observed == 0 {
        current_score.saturating_sub(2)
    } else {
        (((current_score as u64) * 2) + (observed as u64)).div_ceil(3) as u32
    };
    let _ = update_runtime_profile_route_performance(
        shared,
        profile_name,
        route_kind,
        next_score,
        &format!("{stage}_{elapsed_ms}ms"),
    );
}

fn note_runtime_profile_latency_failure(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    stage: &str,
) {
    let current_score = shared
        .runtime
        .lock()
        .ok()
        .map(|runtime| {
            let now = Local::now().timestamp();
            runtime_profile_effective_score_from_map(
                &runtime.profile_health,
                &runtime_profile_route_performance_key(profile_name, route_kind),
                now,
                RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
            )
        })
        .unwrap_or(0);
    let next_score = current_score
        .saturating_add(RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY)
        .min(RUNTIME_PROFILE_LATENCY_PENALTY_MAX);
    let _ = update_runtime_profile_route_performance(
        shared,
        profile_name,
        route_kind,
        next_score,
        stage,
    );
}

fn runtime_route_kind_label(route_kind: RuntimeRouteKind) -> &'static str {
    match route_kind {
        RuntimeRouteKind::Responses => "responses",
        RuntimeRouteKind::Compact => "compact",
        RuntimeRouteKind::Websocket => "websocket",
        RuntimeRouteKind::Standard => "standard",
    }
}

fn runtime_profile_transport_health_penalty(context: &str) -> u32 {
    if context.contains("connect") || context.contains("handshake") || context.contains("timeout") {
        RUNTIME_PROFILE_CONNECT_FAILURE_HEALTH_PENALTY
    } else if context.contains("stream_read") || context.contains("upstream_read") {
        RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY
    } else if context.contains("forward_response")
        || context.contains("buffer_response")
        || context.contains("prepare_success")
        || context.contains("upstream_send")
    {
        RUNTIME_PROFILE_FORWARD_FAILURE_HEALTH_PENALTY
    } else {
        RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY
    }
}

fn reset_runtime_profile_success_streak(
    runtime: &mut RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) {
    runtime
        .profile_health
        .remove(&runtime_profile_route_success_streak_key(
            profile_name,
            route_kind,
        ));
}

fn bump_runtime_profile_bad_pairing_score(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    delta: u32,
    reason: &str,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let key = runtime_profile_route_bad_pairing_key(profile_name, route_kind);
    let next_score = runtime_profile_effective_score_from_map(
        &runtime.profile_health,
        &key,
        now,
        RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
    )
    .saturating_add(delta)
    .min(RUNTIME_PROFILE_HEALTH_MAX_SCORE);
    reset_runtime_profile_success_streak(&mut runtime, profile_name, route_kind);
    runtime.profile_health.insert(
        key,
        RuntimeProfileHealth {
            score: next_score,
            updated_at: now,
        },
    );
    let state_snapshot = runtime.state.clone();
    let continuations_snapshot = runtime_continuation_store_snapshot(&runtime);
    let profile_scores_snapshot = runtime.profile_health.clone();
    let usage_snapshots = runtime.profile_usage_snapshots.clone();
    let backoffs_snapshot = runtime_profile_backoffs_snapshot(&runtime);
    let paths_snapshot = runtime.paths.clone();
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "profile_bad_pairing profile={profile_name} route={} score={next_score} delta={delta} reason={reason}",
            runtime_route_kind_label(route_kind)
        ),
    );
    schedule_runtime_state_save(
        shared,
        state_snapshot,
        continuations_snapshot,
        profile_scores_snapshot,
        usage_snapshots,
        backoffs_snapshot,
        paths_snapshot,
        &format!(
            "profile_bad_pairing:{profile_name}:{}",
            runtime_route_kind_label(route_kind)
        ),
    );
    Ok(())
}

fn bump_runtime_profile_health_score(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    delta: u32,
    reason: &str,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let key = runtime_profile_route_health_key(profile_name, route_kind);
    let next_score = runtime
        .profile_health
        .get(&key)
        .map(|entry| runtime_profile_effective_health_score(entry, now))
        .unwrap_or(0)
        .saturating_add(delta)
        .min(RUNTIME_PROFILE_HEALTH_MAX_SCORE);
    reset_runtime_profile_success_streak(&mut runtime, profile_name, route_kind);
    runtime.profile_health.insert(
        key,
        RuntimeProfileHealth {
            score: next_score,
            updated_at: now,
        },
    );
    let circuit_until = if next_score >= RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD {
        let until = now.saturating_add(RUNTIME_PROFILE_CIRCUIT_OPEN_SECONDS);
        runtime
            .profile_route_circuit_open_until
            .entry(runtime_profile_route_circuit_key(profile_name, route_kind))
            .and_modify(|current| *current = (*current).max(until))
            .or_insert(until);
        Some(until)
    } else {
        None
    };
    let state_snapshot = runtime.state.clone();
    let continuations_snapshot = runtime_continuation_store_snapshot(&runtime);
    let profile_scores_snapshot = runtime.profile_health.clone();
    let usage_snapshots = runtime.profile_usage_snapshots.clone();
    let backoffs_snapshot = runtime_profile_backoffs_snapshot(&runtime);
    let paths_snapshot = runtime.paths.clone();
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "profile_health profile={profile_name} route={} score={next_score} delta={delta} reason={reason}",
            runtime_route_kind_label(route_kind)
        ),
    );
    if let Some(until) = circuit_until {
        runtime_proxy_log(
            shared,
            format!(
                "profile_circuit_open profile={profile_name} route={} until={until} reason={reason} score={next_score}",
                runtime_route_kind_label(route_kind)
            ),
        );
    }
    schedule_runtime_state_save(
        shared,
        state_snapshot,
        continuations_snapshot,
        profile_scores_snapshot,
        usage_snapshots,
        backoffs_snapshot,
        paths_snapshot,
        &format!(
            "profile_health:{profile_name}:{}",
            runtime_route_kind_label(route_kind)
        ),
    );
    Ok(())
}

fn recover_runtime_profile_health_for_route(
    runtime: &mut RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) {
    let key = runtime_profile_route_health_key(profile_name, route_kind);
    let streak_key = runtime_profile_route_success_streak_key(profile_name, route_kind);
    let Some(current_score) = runtime
        .profile_health
        .get(&key)
        .map(|entry| runtime_profile_effective_health_score(entry, now))
    else {
        runtime.profile_health.remove(&streak_key);
        return;
    };

    let next_streak = runtime_profile_effective_score_from_map(
        &runtime.profile_health,
        &streak_key,
        now,
        RUNTIME_PROFILE_SUCCESS_STREAK_DECAY_SECONDS,
    )
    .saturating_add(1)
    .min(RUNTIME_PROFILE_SUCCESS_STREAK_MAX);
    let recovery = RUNTIME_PROFILE_HEALTH_SUCCESS_RECOVERY_SCORE
        .saturating_add(next_streak.saturating_sub(1).min(1));
    let next_score = current_score.saturating_sub(recovery);
    if next_score == 0 {
        runtime.profile_health.remove(&key);
        runtime.profile_health.remove(&streak_key);
    } else {
        runtime.profile_health.insert(
            key,
            RuntimeProfileHealth {
                score: next_score,
                updated_at: now,
            },
        );
        runtime.profile_health.insert(
            streak_key,
            RuntimeProfileHealth {
                score: next_streak,
                updated_at: now,
            },
        );
    }
}

fn mark_runtime_profile_retry_backoff(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    prune_runtime_profile_selection_backoff(&mut runtime, now);
    runtime.profile_probe_cache.remove(profile_name);
    let until = now.saturating_add(RUNTIME_PROFILE_RETRY_BACKOFF_SECONDS);
    runtime
        .profile_retry_backoff_until
        .insert(profile_name.to_string(), until);
    let state_snapshot = runtime.state.clone();
    let continuations_snapshot = runtime_continuation_store_snapshot(&runtime);
    let profile_scores_snapshot = runtime.profile_health.clone();
    let usage_snapshots = runtime.profile_usage_snapshots.clone();
    let backoffs_snapshot = runtime_profile_backoffs_snapshot(&runtime);
    let paths_snapshot = runtime.paths.clone();
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!("profile_retry_backoff profile={profile_name} until={until}"),
    );
    schedule_runtime_state_save(
        shared,
        state_snapshot,
        continuations_snapshot,
        profile_scores_snapshot,
        usage_snapshots,
        backoffs_snapshot,
        paths_snapshot,
        &format!("profile_retry_backoff:{profile_name}"),
    );
    Ok(())
}

fn mark_runtime_profile_transport_backoff(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    context: &str,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    prune_runtime_profile_selection_backoff(&mut runtime, now);
    runtime.profile_probe_cache.remove(profile_name);
    let existing_remaining = runtime
        .profile_transport_backoff_until
        .get(profile_name)
        .copied()
        .unwrap_or(now)
        .saturating_sub(now);
    let next_backoff_seconds = if existing_remaining > 0 {
        existing_remaining.saturating_mul(2).clamp(
            RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS,
            RUNTIME_PROFILE_TRANSPORT_BACKOFF_MAX_SECONDS,
        )
    } else {
        RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS
    };
    let until = now.saturating_add(next_backoff_seconds);
    runtime
        .profile_transport_backoff_until
        .entry(profile_name.to_string())
        .and_modify(|current| *current = (*current).max(until))
        .or_insert(until);
    let state_snapshot = runtime.state.clone();
    let continuations_snapshot = runtime_continuation_store_snapshot(&runtime);
    let profile_scores_snapshot = runtime.profile_health.clone();
    let usage_snapshots = runtime.profile_usage_snapshots.clone();
    let backoffs_snapshot = runtime_profile_backoffs_snapshot(&runtime);
    let paths_snapshot = runtime.paths.clone();
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "profile_transport_backoff profile={profile_name} until={until} seconds={next_backoff_seconds} context={context}"
        ),
    );
    schedule_runtime_state_save(
        shared,
        state_snapshot,
        continuations_snapshot,
        profile_scores_snapshot,
        usage_snapshots,
        backoffs_snapshot,
        paths_snapshot,
        &format!("profile_transport_backoff:{profile_name}"),
    );
    Ok(())
}

fn note_runtime_profile_transport_failure(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    context: &str,
    err: &anyhow::Error,
) {
    if !is_runtime_proxy_transport_failure(err) {
        return;
    }
    let _ = bump_runtime_profile_health_score(
        shared,
        profile_name,
        route_kind,
        runtime_profile_transport_health_penalty(context),
        context,
    );
    let _ = bump_runtime_profile_bad_pairing_score(
        shared,
        profile_name,
        route_kind,
        RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
        context,
    );
    note_runtime_profile_latency_failure(shared, profile_name, route_kind, context);
    let _ = mark_runtime_profile_transport_backoff(shared, profile_name, context);
}

fn commit_runtime_proxy_profile_selection(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> Result<bool> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let switch_runtime_profile = runtime.current_profile != profile_name;
    let switch_global_profile = !matches!(route_kind, RuntimeRouteKind::Compact);
    let switched = switch_runtime_profile;
    let now = Local::now().timestamp();
    let cleared_retry_backoff = runtime
        .profile_retry_backoff_until
        .remove(profile_name)
        .is_some();
    let cleared_transport_backoff = runtime
        .profile_transport_backoff_until
        .remove(profile_name)
        .is_some();
    let cleared_route_circuit =
        clear_runtime_profile_circuit_for_route(&mut runtime, profile_name, route_kind);
    let cleared_health =
        clear_runtime_profile_health_for_route(&mut runtime, profile_name, route_kind, now);
    if switch_runtime_profile {
        runtime.current_profile = profile_name.to_string();
    }
    let state_changed =
        switch_global_profile && runtime.state.active_profile.as_deref() != Some(profile_name);
    if switch_global_profile {
        runtime.state.active_profile = Some(profile_name.to_string());
        record_run_selection(&mut runtime.state, profile_name);
    }
    let state_snapshot = runtime.state.clone();
    let continuations_snapshot = runtime_continuation_store_snapshot(&runtime);
    let profile_scores_snapshot = runtime.profile_health.clone();
    let usage_snapshots = runtime.profile_usage_snapshots.clone();
    let backoffs_snapshot = runtime_profile_backoffs_snapshot(&runtime);
    let paths_snapshot = runtime.paths.clone();
    drop(runtime);
    if switch_runtime_profile {
        update_runtime_broker_current_profile(&shared.log_path, profile_name);
    }
    let should_persist = switched
        || state_changed
        || cleared_retry_backoff
        || cleared_transport_backoff
        || cleared_route_circuit
        || cleared_health;
    if should_persist {
        schedule_runtime_state_save(
            shared,
            state_snapshot,
            continuations_snapshot,
            profile_scores_snapshot,
            usage_snapshots,
            backoffs_snapshot,
            paths_snapshot,
            &format!("profile_commit:{profile_name}"),
        );
    }
    runtime_proxy_log(
        shared,
        format!(
            "profile_commit profile={profile_name} route={} switched={switched} persisted={should_persist} cleared_route_circuit={cleared_route_circuit}",
            runtime_route_kind_label(route_kind),
        ),
    );
    Ok(switched)
}

fn clear_runtime_profile_health_for_route(
    runtime: &mut RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    let mut changed = runtime.profile_health.remove(profile_name).is_some();
    let previous_route_score =
        runtime_profile_route_health_score(runtime, profile_name, now, route_kind);
    let previous_bad_pairing = runtime_profile_effective_score_from_map(
        &runtime.profile_health,
        &runtime_profile_route_bad_pairing_key(profile_name, route_kind),
        now,
        RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
    );
    recover_runtime_profile_health_for_route(runtime, profile_name, route_kind, now);
    changed = changed || previous_route_score > 0;
    changed = changed || previous_bad_pairing > 0;
    changed = runtime
        .profile_health
        .remove(&runtime_profile_route_bad_pairing_key(
            profile_name,
            route_kind,
        ))
        .is_some()
        || changed;
    changed
}

fn commit_runtime_proxy_profile_selection_with_notice(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> Result<()> {
    let _ = commit_runtime_proxy_profile_selection(shared, profile_name, route_kind)?;
    Ok(())
}

fn send_runtime_proxy_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
) -> Result<reqwest::Response> {
    let started_at = Instant::now();
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .clone();
    let profile = runtime
        .state
        .profiles
        .get(profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    let auth = read_usage_auth(&profile.codex_home)?;
    let upstream_url =
        runtime_proxy_upstream_url(&runtime.upstream_base_url, &request.path_and_query);
    let method = reqwest::Method::from_bytes(request.method.as_bytes()).with_context(|| {
        format!(
            "failed to proxy unsupported HTTP method '{}' for runtime auto-rotate",
            request.method
        )
    })?;

    let mut upstream_request = shared.async_client.request(method, &upstream_url);
    for (name, value) in &request.headers {
        if turn_state_override.is_some() && name.eq_ignore_ascii_case("x-codex-turn-state") {
            continue;
        }
        if should_skip_runtime_request_header(name) {
            continue;
        }
        upstream_request = upstream_request.header(name.as_str(), value.as_str());
    }
    if let Some(turn_state) = turn_state_override {
        upstream_request = upstream_request.header("x-codex-turn-state", turn_state);
    }

    upstream_request = upstream_request
        .header("Authorization", format!("Bearer {}", auth.access_token))
        .body(request.body.clone());

    if let Some(user_agent) = runtime_proxy_effective_user_agent(&request.headers) {
        upstream_request = upstream_request.header("User-Agent", user_agent);
    } else {
        upstream_request = upstream_request.header("User-Agent", "codex-cli");
    }

    if let Some(account_id) = auth.account_id.as_deref() {
        upstream_request = upstream_request.header("ChatGPT-Account-Id", account_id);
    }

    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http upstream_start profile={profile_name} method={} url={} turn_state_override={:?} previous_response_id={:?}",
            request.method,
            upstream_url,
            turn_state_override,
            runtime_request_previous_response_id(request)
        ),
    );
    if runtime_take_fault_injection("PRODEX_RUNTIME_FAULT_UPSTREAM_CONNECT_ERROR_ONCE") {
        bail!("injected runtime upstream connect failure");
    }
    let response = shared
        .async_runtime
        .block_on(async move { upstream_request.send().await })
        .with_context(|| {
            format!(
                "failed to proxy runtime request for profile '{}' to {}",
                profile_name, upstream_url
            )
        })?;
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http upstream_response profile={profile_name} status={} content_type={:?} turn_state={:?}",
            response.status().as_u16(),
            response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            runtime_proxy_header_value(response.headers(), "x-codex-turn-state")
        ),
    );
    note_runtime_profile_latency_observation(
        shared,
        profile_name,
        runtime_proxy_request_lane(&request.path_and_query, false),
        "connect",
        started_at.elapsed().as_millis() as u64,
    );
    Ok(response)
}

fn send_runtime_proxy_upstream_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
) -> Result<reqwest::Response> {
    let started_at = Instant::now();
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .clone();
    let profile = runtime
        .state
        .profiles
        .get(profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    let auth = read_usage_auth(&profile.codex_home)?;
    let upstream_url =
        runtime_proxy_upstream_url(&runtime.upstream_base_url, &request.path_and_query);
    let method = reqwest::Method::from_bytes(request.method.as_bytes()).with_context(|| {
        format!(
            "failed to proxy unsupported HTTP method '{}' for runtime auto-rotate",
            request.method
        )
    })?;

    let mut upstream_request = shared.async_client.request(method, &upstream_url);
    for (name, value) in &request.headers {
        if turn_state_override.is_some() && name.eq_ignore_ascii_case("x-codex-turn-state") {
            continue;
        }
        if should_skip_runtime_request_header(name) {
            continue;
        }
        upstream_request = upstream_request.header(name.as_str(), value.as_str());
    }
    if let Some(turn_state) = turn_state_override {
        upstream_request = upstream_request.header("x-codex-turn-state", turn_state);
    }

    upstream_request = upstream_request
        .header("Authorization", format!("Bearer {}", auth.access_token))
        .body(request.body.clone());

    if let Some(user_agent) = runtime_proxy_effective_user_agent(&request.headers) {
        upstream_request = upstream_request.header("User-Agent", user_agent);
    } else {
        upstream_request = upstream_request.header("User-Agent", "codex-cli");
    }

    if let Some(account_id) = auth.account_id.as_deref() {
        upstream_request = upstream_request.header("ChatGPT-Account-Id", account_id);
    }

    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http upstream_async_start profile={profile_name} method={} url={} turn_state_override={:?} previous_response_id={:?}",
            request.method,
            upstream_url,
            turn_state_override,
            runtime_request_previous_response_id(request)
        ),
    );
    if runtime_take_fault_injection("PRODEX_RUNTIME_FAULT_UPSTREAM_CONNECT_ERROR_ONCE") {
        bail!("injected runtime upstream connect failure");
    }
    let response = shared
        .async_runtime
        .block_on(async move { upstream_request.send().await })
        .with_context(|| {
            format!(
                "failed to proxy runtime request for profile '{}' to {}",
                profile_name, upstream_url
            )
        })?;
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http upstream_async_response profile={profile_name} status={} content_type={:?} turn_state={:?}",
            response.status().as_u16(),
            response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            runtime_proxy_header_value(response.headers(), "x-codex-turn-state")
        ),
    );
    note_runtime_profile_latency_observation(
        shared,
        profile_name,
        RuntimeRouteKind::Responses,
        "connect",
        started_at.elapsed().as_millis() as u64,
    );
    Ok(response)
}

fn runtime_proxy_upstream_url(base_url: &str, path_and_query: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    if base_url.contains("/backend-api")
        && let Some(suffix) = path_and_query.strip_prefix("/backend-api")
    {
        return format!("{base_url}{suffix}");
    }
    if path_and_query.starts_with('/') {
        return format!("{base_url}{path_and_query}");
    }
    format!("{base_url}/{path_and_query}")
}

fn runtime_proxy_upstream_websocket_url(base_url: &str, path_and_query: &str) -> Result<String> {
    let upstream_url = runtime_proxy_upstream_url(base_url, path_and_query);
    let mut url = reqwest::Url::parse(&upstream_url)
        .with_context(|| format!("failed to parse upstream websocket URL {}", upstream_url))?;
    match url.scheme() {
        "http" => {
            url.set_scheme("ws").map_err(|_| {
                anyhow::anyhow!("failed to set websocket scheme for {upstream_url}")
            })?;
        }
        "https" => {
            url.set_scheme("wss").map_err(|_| {
                anyhow::anyhow!("failed to set websocket scheme for {upstream_url}")
            })?;
        }
        "ws" | "wss" => {}
        scheme => bail!(
            "unsupported upstream websocket scheme '{scheme}' in {}",
            upstream_url
        ),
    }
    Ok(url.to_string())
}

fn should_skip_runtime_request_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "authorization"
            | "chatgpt-account-id"
            | "connection"
            | "content-length"
            | "host"
            | "transfer-encoding"
            | "upgrade"
    ) || lower.starts_with("sec-websocket-")
}

fn runtime_proxy_effective_user_agent(headers: &[(String, String)]) -> Option<&str> {
    headers.iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("user-agent")
            .then_some(value.as_str())
            .filter(|value| !value.is_empty())
    })
}

fn should_skip_runtime_response_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "content-encoding"
            | "content-length"
            | "date"
            | "server"
            | "transfer-encoding"
    )
}

fn forward_runtime_proxy_response(
    shared: &RuntimeRotationProxyShared,
    response: reqwest::Response,
    prelude: Vec<u8>,
) -> Result<tiny_http::ResponseBox> {
    let parts = buffer_runtime_proxy_async_response_parts(shared, response, prelude)?;
    Ok(build_runtime_proxy_response_from_parts(parts))
}

fn prepare_runtime_proxy_responses_success(
    request_id: u64,
    request_previous_response_id: Option<&str>,
    request_session_id: Option<&str>,
    request_turn_state: Option<&str>,
    response: reqwest::Response,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    inflight_guard: RuntimeProfileInFlightGuard,
) -> Result<RuntimeResponsesAttempt> {
    let turn_state = runtime_proxy_header_value(response.headers(), "x-codex-turn-state");
    remember_runtime_successful_previous_response_owner(
        shared,
        profile_name,
        request_previous_response_id,
        RuntimeRouteKind::Responses,
    )?;
    remember_runtime_session_id(
        shared,
        profile_name,
        request_session_id,
        RuntimeRouteKind::Responses,
    )?;
    remember_runtime_turn_state(
        shared,
        profile_name,
        turn_state.as_deref(),
        RuntimeRouteKind::Responses,
    )?;
    let is_sse = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("text/event-stream"));
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http prepare_success profile={profile_name} sse={is_sse} turn_state={:?}",
            turn_state
        ),
    );
    if !is_sse {
        let parts = buffer_runtime_proxy_async_response_parts(shared, response, Vec::new())?;
        let response_ids = extract_runtime_response_ids_from_body_bytes(&parts.body);
        if !response_ids.is_empty() {
            remember_runtime_response_ids(
                shared,
                profile_name,
                &response_ids,
                RuntimeRouteKind::Responses,
            )?;
            let _ = release_runtime_compact_lineage(
                shared,
                profile_name,
                request_session_id,
                request_turn_state,
                "response_committed",
            );
        }
        return Ok(RuntimeResponsesAttempt::Success {
            profile_name: profile_name.to_string(),
            response: RuntimeResponsesReply::Buffered(build_runtime_proxy_response_from_parts(
                parts,
            )),
        });
    }

    let status = response.status().as_u16();
    let mut headers = Vec::new();
    for (name, value) in response.headers() {
        if should_skip_runtime_response_header(name.as_str()) {
            continue;
        }
        if let Ok(value) = value.to_str() {
            headers.push((name.to_string(), value.to_string()));
        }
    }

    let mut prefetch = RuntimePrefetchStream::spawn(
        response,
        Arc::clone(&shared.async_runtime),
        shared.log_path.clone(),
        request_id,
    );
    let lookahead = inspect_runtime_sse_lookahead(&mut prefetch, &shared.log_path, request_id)?;

    let (prelude, response_ids) = match lookahead {
        RuntimeSseInspection::Commit {
            prelude,
            response_ids,
        } => {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http sse_commit profile={profile_name} prelude_bytes={} response_ids={}",
                    prelude.len(),
                    response_ids.len()
                ),
            );
            (prelude, response_ids)
        }
        RuntimeSseInspection::QuotaBlocked(prelude) => {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http sse_quota_blocked profile={profile_name} prelude_bytes={}",
                    prelude.len()
                ),
            );
            return Ok(RuntimeResponsesAttempt::QuotaBlocked {
                profile_name: profile_name.to_string(),
                response: RuntimeResponsesReply::Streaming(RuntimeStreamingResponse {
                    status,
                    headers: headers.clone(),
                    body: Box::new(prefetch.into_reader(prelude)),
                    request_id,
                    profile_name: profile_name.to_string(),
                    log_path: shared.log_path.clone(),
                    shared: shared.clone(),
                    _inflight_guard: Some(inflight_guard),
                }),
            });
        }
        RuntimeSseInspection::PreviousResponseNotFound(prelude) => {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http route=responses previous_response_not_found profile={profile_name} stage=sse_prelude prelude_bytes={}",
                    prelude.len()
                ),
            );
            return Ok(RuntimeResponsesAttempt::PreviousResponseNotFound {
                profile_name: profile_name.to_string(),
                response: RuntimeResponsesReply::Streaming(RuntimeStreamingResponse {
                    status,
                    headers: headers.clone(),
                    body: Box::new(prefetch.into_reader(prelude)),
                    request_id,
                    profile_name: profile_name.to_string(),
                    log_path: shared.log_path.clone(),
                    shared: shared.clone(),
                    _inflight_guard: Some(inflight_guard),
                }),
                turn_state,
            });
        }
    };
    remember_runtime_response_ids(
        shared,
        profile_name,
        &response_ids,
        RuntimeRouteKind::Responses,
    )?;
    if !response_ids.is_empty() {
        let _ = release_runtime_compact_lineage(
            shared,
            profile_name,
            request_session_id,
            request_turn_state,
            "response_committed",
        );
    }

    Ok(RuntimeResponsesAttempt::Success {
        profile_name: profile_name.to_string(),
        response: RuntimeResponsesReply::Streaming(RuntimeStreamingResponse {
            status,
            headers,
            body: Box::new(RuntimeSseTapReader::new(
                prefetch.into_reader(prelude.clone()),
                shared.clone(),
                profile_name.to_string(),
                &prelude,
                &response_ids,
            )),
            request_id,
            profile_name: profile_name.to_string(),
            log_path: shared.log_path.clone(),
            shared: shared.clone(),
            _inflight_guard: Some(inflight_guard),
        }),
    })
}

impl RuntimeSseTapState {
    fn observe(&mut self, shared: &RuntimeRotationProxyShared, profile_name: &str, chunk: &[u8]) {
        for byte in chunk {
            self.line.push(*byte);
            if *byte != b'\n' {
                continue;
            }

            let line_text = String::from_utf8_lossy(&self.line);
            let trimmed = line_text.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                self.remember_response_ids(shared, profile_name, RuntimeRouteKind::Responses);
                self.data_lines.clear();
                self.line.clear();
                continue;
            }

            if let Some(payload) = trimmed.strip_prefix("data:") {
                self.data_lines.push(payload.trim_start().to_string());
            }
            self.line.clear();
        }
    }

    fn finish(&mut self, shared: &RuntimeRotationProxyShared, profile_name: &str) {
        self.remember_response_ids(shared, profile_name, RuntimeRouteKind::Responses);
    }

    fn remember_response_ids(
        &mut self,
        shared: &RuntimeRotationProxyShared,
        profile_name: &str,
        verified_route: RuntimeRouteKind,
    ) {
        let fresh_ids = extract_runtime_response_ids_from_sse(&self.data_lines)
            .into_iter()
            .filter(|response_id| self.remembered_response_ids.insert(response_id.clone()))
            .collect::<Vec<_>>();
        if fresh_ids.is_empty() {
            return;
        }
        let _ = remember_runtime_response_ids(shared, profile_name, &fresh_ids, verified_route);
    }
}

struct RuntimeSseTapReader {
    inner: Box<dyn Read + Send>,
    shared: RuntimeRotationProxyShared,
    profile_name: String,
    state: RuntimeSseTapState,
}

impl Read for RuntimePrefetchReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.finished {
            return Ok(0);
        }

        loop {
            let read = self.pending.read(buf)?;
            if read > 0 {
                return Ok(read);
            }

            let next = if let Some(chunk) = self.backlog.pop_front() {
                Some(chunk)
            } else {
                match self
                    .receiver
                    .recv_timeout(Duration::from_millis(runtime_proxy_stream_idle_timeout_ms()))
                {
                    Ok(chunk) => Some(chunk),
                    Err(RecvTimeoutError::Timeout) => {
                        self.finished = true;
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "runtime upstream stream idle timed out",
                        ));
                    }
                    Err(RecvTimeoutError::Disconnected) => None,
                }
            };

            match next {
                Some(RuntimePrefetchChunk::Data(chunk)) => {
                    self.pending = Cursor::new(chunk);
                }
                Some(RuntimePrefetchChunk::End) | None => {
                    self.finished = true;
                    return Ok(0);
                }
                Some(RuntimePrefetchChunk::Error(kind, message)) => {
                    self.finished = true;
                    return Err(io::Error::new(kind, message));
                }
            }
        }
    }
}

impl RuntimeSseTapReader {
    fn new(
        inner: impl Read + Send + 'static,
        shared: RuntimeRotationProxyShared,
        profile_name: String,
        prelude: &[u8],
        remembered_response_ids: &[String],
    ) -> Self {
        let mut state = RuntimeSseTapState {
            remembered_response_ids: remembered_response_ids.iter().cloned().collect(),
            ..RuntimeSseTapState::default()
        };
        state.observe(&shared, &profile_name, prelude);
        Self {
            inner: Box::new(inner),
            shared,
            profile_name,
            state,
        }
    }
}

impl Read for RuntimeSseTapReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = match self.inner.read(buf) {
            Ok(read) => read,
            Err(err) => {
                let transport_error =
                    anyhow::Error::new(io::Error::new(err.kind(), err.to_string()));
                note_runtime_profile_transport_failure(
                    &self.shared,
                    &self.profile_name,
                    RuntimeRouteKind::Responses,
                    "sse_read",
                    &transport_error,
                );
                return Err(err);
            }
        };
        if read == 0 {
            self.state.finish(&self.shared, &self.profile_name);
            return Ok(0);
        }
        self.state
            .observe(&self.shared, &self.profile_name, &buf[..read]);
        Ok(read)
    }
}

fn write_runtime_streaming_response(
    writer: Box<dyn Write + Send + 'static>,
    mut response: RuntimeStreamingResponse,
) -> io::Result<()> {
    let mut writer = writer;
    let started_at = Instant::now();
    let log_writer_error = |stage: &str,
                            chunk_count: usize,
                            total_bytes: usize,
                            err: &io::Error| {
        runtime_proxy_log_to_path(
            &response.log_path,
            &format!(
                "local_writer_error request={} transport=http profile={} stage={} chunks={} bytes={} elapsed_ms={} error={}",
                response.request_id,
                response.profile_name,
                stage,
                chunk_count,
                total_bytes,
                started_at.elapsed().as_millis(),
                err
            ),
        );
    };
    runtime_proxy_log_to_path(
        &response.log_path,
        &format!(
            "request={} transport=http stream_start profile={} status={}",
            response.request_id, response.profile_name, response.status
        ),
    );
    let status = reqwest::StatusCode::from_u16(response.status)
        .ok()
        .and_then(|status| status.canonical_reason().map(str::to_string))
        .unwrap_or_else(|| "OK".to_string());
    write!(
        writer,
        "HTTP/1.1 {} {}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n",
        response.status, status
    )
    .map_err(|err| {
        log_writer_error("headers_start", 0, 0, &err);
        err
    })?;
    for (name, value) in response.headers {
        write!(writer, "{name}: {value}\r\n").map_err(|err| {
            log_writer_error("header_line", 0, 0, &err);
            err
        })?;
    }
    writer.write_all(b"\r\n").map_err(|err| {
        log_writer_error("headers_end", 0, 0, &err);
        err
    })?;
    writer.flush().map_err(|err| {
        log_writer_error("headers_flush", 0, 0, &err);
        err
    })?;

    let mut buffer = [0_u8; 8192];
    let mut total_bytes = 0usize;
    let mut chunk_count = 0usize;
    loop {
        let read = match response.body.read(&mut buffer) {
            Ok(read) => read,
            Err(err) => {
                runtime_proxy_log_to_path(
                    &response.log_path,
                    &format!(
                        "request={} transport=http stream_read_error profile={} chunks={} bytes={} elapsed_ms={} error={}",
                        response.request_id,
                        response.profile_name,
                        chunk_count,
                        total_bytes,
                        started_at.elapsed().as_millis(),
                        err
                    ),
                );
                note_runtime_profile_latency_failure(
                    &response.shared,
                    &response.profile_name,
                    RuntimeRouteKind::Responses,
                    "stream_read_error",
                );
                return Err(err);
            }
        };
        if read == 0 {
            break;
        }
        if chunk_count == 0
            && runtime_take_fault_injection("PRODEX_RUNTIME_FAULT_STREAM_READ_ERROR_ONCE")
        {
            let err = io::Error::new(
                io::ErrorKind::ConnectionReset,
                "injected runtime stream read failure",
            );
            runtime_proxy_log_to_path(
                &response.log_path,
                &format!(
                    "request={} transport=http stream_read_error profile={} chunks={} bytes={} elapsed_ms={} error={}",
                    response.request_id,
                    response.profile_name,
                    chunk_count,
                    total_bytes,
                    started_at.elapsed().as_millis(),
                    err
                ),
            );
            note_runtime_profile_latency_failure(
                &response.shared,
                &response.profile_name,
                RuntimeRouteKind::Responses,
                "stream_read_error",
            );
            return Err(err);
        }
        chunk_count += 1;
        total_bytes += read;
        if chunk_count == 1 {
            runtime_proxy_log_to_path(
                &response.log_path,
                &format!(
                    "request={} transport=http first_local_chunk profile={} bytes={} elapsed_ms={}",
                    response.request_id,
                    response.profile_name,
                    read,
                    started_at.elapsed().as_millis()
                ),
            );
            note_runtime_profile_latency_observation(
                &response.shared,
                &response.profile_name,
                RuntimeRouteKind::Responses,
                "ttfb",
                started_at.elapsed().as_millis() as u64,
            );
        }
        write!(writer, "{:X}\r\n", read).map_err(|err| {
            log_writer_error("chunk_size", chunk_count, total_bytes, &err);
            err
        })?;
        writer.write_all(&buffer[..read]).map_err(|err| {
            log_writer_error("chunk_body", chunk_count, total_bytes, &err);
            err
        })?;
        writer.write_all(b"\r\n").map_err(|err| {
            log_writer_error("chunk_suffix", chunk_count, total_bytes, &err);
            err
        })?;
        writer.flush().map_err(|err| {
            log_writer_error("chunk_flush", chunk_count, total_bytes, &err);
            err
        })?;
    }
    writer.write_all(b"0\r\n\r\n").map_err(|err| {
        log_writer_error("trailer", chunk_count, total_bytes, &err);
        err
    })?;
    writer.flush().map_err(|err| {
        log_writer_error("trailer_flush", chunk_count, total_bytes, &err);
        err
    })?;
    runtime_proxy_log_to_path(
        &response.log_path,
        &format!(
            "request={} transport=http stream_complete profile={} chunks={} bytes={} elapsed_ms={}",
            response.request_id,
            response.profile_name,
            chunk_count,
            total_bytes,
            started_at.elapsed().as_millis()
        ),
    );
    note_runtime_profile_latency_observation(
        &response.shared,
        &response.profile_name,
        RuntimeRouteKind::Responses,
        "stream_complete",
        started_at.elapsed().as_millis() as u64,
    );
    Ok(())
}

fn build_runtime_proxy_text_response(status: u16, message: &str) -> tiny_http::ResponseBox {
    let mut response = TinyResponse::from_string(message.to_string()).with_status_code(status);
    if let Ok(header) = TinyHeader::from_bytes("Content-Type", "text/plain; charset=utf-8") {
        response = response.with_header(header);
    }
    response.boxed()
}

fn runtime_proxy_header_value(headers: &reqwest::header::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn runtime_proxy_tungstenite_header_value(
    headers: &tungstenite::http::HeaderMap,
    name: &str,
) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn is_runtime_proxy_transport_failure(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        let message = cause.to_string().to_ascii_lowercase();
        message.contains("timed out")
            || message.contains("timeout")
            || message.contains("connection reset")
            || message.contains("broken pipe")
            || message.contains("unexpected eof")
            || message.contains("connection aborted")
            || message.contains("stream closed before response.completed")
            || message.contains("closed before response.completed")
    })
}

fn build_runtime_proxy_json_error_response(
    status: u16,
    code: &str,
    message: &str,
) -> tiny_http::ResponseBox {
    let body = serde_json::json!({
        "error": {
            "code": code,
            "message": message,
        }
    })
    .to_string();
    let mut response = TinyResponse::from_string(body).with_status_code(status);
    if let Ok(header) = TinyHeader::from_bytes("Content-Type", "application/json") {
        response = response.with_header(header);
    }
    response.boxed()
}

struct RuntimeBufferedResponseParts {
    status: u16,
    headers: Vec<(String, Vec<u8>)>,
    body: Vec<u8>,
}

fn is_runtime_responses_path(path_and_query: &str) -> bool {
    path_without_query(path_and_query).ends_with("/codex/responses")
}

fn is_runtime_compact_path(path_and_query: &str) -> bool {
    path_without_query(path_and_query).ends_with("/responses/compact")
}

fn path_without_query(path_and_query: &str) -> &str {
    path_and_query
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(path_and_query)
}

impl RuntimePrefetchStream {
    fn spawn(
        response: reqwest::Response,
        async_runtime: Arc<TokioRuntime>,
        log_path: PathBuf,
        request_id: u64,
    ) -> Self {
        let (sender, receiver) = mpsc::channel::<RuntimePrefetchChunk>();
        async_runtime.spawn(async move {
            runtime_prefetch_response_chunks(response, sender, log_path, request_id).await;
        });
        Self {
            receiver,
            backlog: VecDeque::new(),
        }
    }

    fn recv_timeout(
        &mut self,
        timeout: Duration,
    ) -> std::result::Result<RuntimePrefetchChunk, RecvTimeoutError> {
        if let Some(chunk) = self.backlog.pop_front() {
            return Ok(chunk);
        }
        self.receiver.recv_timeout(timeout)
    }

    fn push_backlog(&mut self, chunk: RuntimePrefetchChunk) {
        self.backlog.push_back(chunk);
    }

    fn into_reader(self, prelude: Vec<u8>) -> RuntimePrefetchReader {
        RuntimePrefetchReader {
            receiver: self.receiver,
            backlog: self.backlog,
            pending: Cursor::new(prelude),
            finished: false,
        }
    }
}

async fn runtime_prefetch_response_chunks(
    mut response: reqwest::Response,
    sender: Sender<RuntimePrefetchChunk>,
    log_path: PathBuf,
    request_id: u64,
) {
    let mut saw_data = false;
    loop {
        match response.chunk().await {
            Ok(None) => {
                runtime_proxy_log_to_path(
                    &log_path,
                    &format!(
                        "request={request_id} transport=http upstream_stream_end saw_data={saw_data}"
                    ),
                );
                let _ = sender.send(RuntimePrefetchChunk::End);
                break;
            }
            Ok(Some(chunk)) => {
                if !saw_data {
                    saw_data = true;
                    runtime_proxy_log_to_path(
                        &log_path,
                        &format!(
                            "request={request_id} transport=http first_upstream_chunk bytes={}",
                            chunk.len()
                        ),
                    );
                }
                if sender
                    .send(RuntimePrefetchChunk::Data(chunk.to_vec()))
                    .is_err()
                {
                    runtime_proxy_log_to_path(
                        &log_path,
                        &format!(
                            "request={request_id} transport=http prefetch_receiver_disconnected"
                        ),
                    );
                    break;
                }
            }
            Err(err) => {
                let kind = runtime_reqwest_error_kind(&err);
                runtime_proxy_log_to_path(
                    &log_path,
                    &format!(
                        "request={request_id} transport=http upstream_stream_error kind={kind:?} error={err}"
                    ),
                );
                let _ = sender.send(RuntimePrefetchChunk::Error(kind, err.to_string()));
                break;
            }
        }
    }
}

fn inspect_runtime_sse_lookahead(
    prefetch: &mut RuntimePrefetchStream,
    log_path: &Path,
    request_id: u64,
) -> Result<RuntimeSseInspection> {
    let deadline = Instant::now() + Duration::from_millis(runtime_proxy_sse_lookahead_timeout_ms());
    let mut buffered = Vec::new();

    loop {
        if buffered.len() >= RUNTIME_PROXY_SSE_LOOKAHEAD_BYTES {
            break;
        }
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        let remaining = deadline.saturating_duration_since(now);
        match prefetch.recv_timeout(remaining) {
            Ok(RuntimePrefetchChunk::Data(chunk)) => {
                buffered.extend_from_slice(&chunk);
                match inspect_runtime_sse_buffer(buffered.clone())? {
                    RuntimeSseInspection::Commit { response_ids, .. }
                        if !response_ids.is_empty() =>
                    {
                        runtime_proxy_log_to_path(
                            log_path,
                            &format!(
                                "request={request_id} transport=http lookahead_commit_with_response_ids bytes={} response_ids={}",
                                buffered.len(),
                                response_ids.len()
                            ),
                        );
                        return Ok(RuntimeSseInspection::Commit {
                            prelude: buffered,
                            response_ids,
                        });
                    }
                    RuntimeSseInspection::Commit { .. } => {}
                    other => {
                        runtime_proxy_log_to_path(
                            log_path,
                            &format!(
                                "request={request_id} transport=http lookahead_retryable_signal bytes={}",
                                buffered.len()
                            ),
                        );
                        return Ok(other);
                    }
                }
            }
            Ok(RuntimePrefetchChunk::End) => break,
            Ok(RuntimePrefetchChunk::Error(kind, message)) => {
                if buffered.is_empty() {
                    runtime_proxy_log_to_path(
                        log_path,
                        &format!(
                            "request={request_id} transport=http lookahead_error_before_bytes kind={kind:?} error={message}"
                        ),
                    );
                    return Err(anyhow::Error::new(io::Error::new(kind, message))
                        .context("failed to inspect runtime auto-rotate SSE stream"));
                }
                prefetch.push_backlog(RuntimePrefetchChunk::Error(kind, message));
                break;
            }
            Err(RecvTimeoutError::Timeout) => {
                runtime_proxy_log_to_path(
                    log_path,
                    &format!(
                        "request={request_id} transport=http lookahead_timeout bytes={}",
                        buffered.len()
                    ),
                );
                break;
            }
            Err(RecvTimeoutError::Disconnected) => {
                runtime_proxy_log_to_path(
                    log_path,
                    &format!(
                        "request={request_id} transport=http lookahead_channel_disconnected bytes={}",
                        buffered.len()
                    ),
                );
                break;
            }
        }
    }

    inspect_runtime_sse_buffer(buffered)
}

fn inspect_runtime_sse_buffer(buffered: Vec<u8>) -> Result<RuntimeSseInspection> {
    let mut line = Vec::new();
    let mut data_lines = Vec::new();
    let mut response_ids = BTreeSet::new();

    for byte in &buffered {
        line.push(*byte);
        if *byte != b'\n' {
            continue;
        }

        let line_text = String::from_utf8_lossy(&line);
        let trimmed = line_text.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            if let Some(message) = extract_runtime_proxy_quota_message_from_sse(&data_lines) {
                let _ = message;
                return Ok(RuntimeSseInspection::QuotaBlocked(buffered));
            }
            if let Some(message) =
                extract_runtime_proxy_previous_response_message_from_sse(&data_lines)
            {
                let _ = message;
                return Ok(RuntimeSseInspection::PreviousResponseNotFound(buffered));
            }
            response_ids.extend(extract_runtime_response_ids_from_sse(&data_lines));
            data_lines.clear();
            line.clear();
            continue;
        }

        if let Some(payload) = trimmed.strip_prefix("data:") {
            data_lines.push(payload.trim_start().to_string());
        }
        line.clear();
    }

    Ok(RuntimeSseInspection::Commit {
        prelude: buffered,
        response_ids: response_ids.into_iter().collect(),
    })
}

fn buffer_runtime_proxy_async_response_parts(
    shared: &RuntimeRotationProxyShared,
    response: reqwest::Response,
    mut prelude: Vec<u8>,
) -> Result<RuntimeBufferedResponseParts> {
    let status = response.status().as_u16();
    let mut headers = Vec::new();
    for (name, value) in response.headers() {
        if should_skip_runtime_response_header(name.as_str()) {
            continue;
        }
        headers.push((name.as_str().to_string(), value.as_bytes().to_vec()));
    }
    let body = shared
        .async_runtime
        .block_on(async move { response.bytes().await })
        .context("failed to read upstream runtime response body")?;
    prelude.extend_from_slice(&body);
    Ok(RuntimeBufferedResponseParts {
        status,
        headers,
        body: prelude,
    })
}

fn runtime_reqwest_error_kind(err: &reqwest::Error) -> io::ErrorKind {
    let message = err.to_string().to_ascii_lowercase();
    if err.is_timeout() || message.contains("timed out") || message.contains("timeout") {
        io::ErrorKind::TimedOut
    } else if message.contains("connection reset") {
        io::ErrorKind::ConnectionReset
    } else if message.contains("broken pipe") {
        io::ErrorKind::BrokenPipe
    } else if message.contains("connection aborted") {
        io::ErrorKind::ConnectionAborted
    } else if message.contains("unexpected eof") {
        io::ErrorKind::UnexpectedEof
    } else {
        io::ErrorKind::Other
    }
}

fn build_runtime_proxy_response_from_parts(
    parts: RuntimeBufferedResponseParts,
) -> tiny_http::ResponseBox {
    let status = TinyStatusCode(parts.status);
    let headers = parts
        .headers
        .into_iter()
        .filter_map(|(name, value)| TinyHeader::from_bytes(name.as_bytes(), value).ok())
        .collect::<Vec<_>>();
    let body_len = parts.body.len();
    TinyResponse::new(
        status,
        headers,
        Box::new(Cursor::new(parts.body)),
        Some(body_len),
        None,
    )
    .boxed()
}

fn extract_runtime_proxy_quota_message_from_sse(data_lines: &[String]) -> Option<String> {
    if data_lines.is_empty() {
        return None;
    }

    let payload = data_lines.join("\n");
    let value = serde_json::from_str::<serde_json::Value>(&payload).ok()?;
    extract_runtime_proxy_quota_message_from_value(&value)
}

fn extract_runtime_proxy_previous_response_message_from_sse(
    data_lines: &[String],
) -> Option<String> {
    if data_lines.is_empty() {
        return None;
    }

    let payload = data_lines.join("\n");
    let value = serde_json::from_str::<serde_json::Value>(&payload).ok()?;
    extract_runtime_proxy_previous_response_message_from_value(&value)
}

fn extract_runtime_response_ids_from_sse(data_lines: &[String]) -> Vec<String> {
    if data_lines.is_empty() {
        return Vec::new();
    }

    let payload = data_lines.join("\n");
    extract_runtime_response_ids_from_payload(&payload)
}

fn extract_runtime_proxy_quota_message(body: &[u8]) -> Option<String> {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body)
        && let Some(message) = extract_runtime_proxy_quota_message_from_value(&value)
    {
        return Some(message);
    }

    extract_runtime_proxy_quota_message_from_text(&String::from_utf8_lossy(body))
}

fn extract_runtime_proxy_previous_response_message(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| extract_runtime_proxy_previous_response_message_from_value(&value))
}

fn extract_runtime_proxy_overload_message(status: u16, body: &[u8]) -> Option<String> {
    if status == 503
        && let Ok(value) = serde_json::from_slice::<serde_json::Value>(body)
        && let Some(error) = value.get("error")
        && matches!(
            error.get("code").and_then(serde_json::Value::as_str),
            Some("server_is_overloaded" | "slow_down")
        )
    {
        return Some(
            error
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("Upstream Codex backend is currently overloaded.")
                .to_string(),
        );
    }

    (status == 500).then(|| {
        let body_text = String::from_utf8_lossy(body).trim().to_string();
        if body_text.is_empty() {
            "Upstream Codex backend is currently experiencing high demand.".to_string()
        } else {
            body_text
        }
    })
}

fn extract_runtime_proxy_quota_message_from_value(value: &serde_json::Value) -> Option<String> {
    if let Some(message) = extract_runtime_proxy_quota_message_candidate(value) {
        return Some(message);
    }

    match value {
        serde_json::Value::Array(values) => values
            .iter()
            .find_map(extract_runtime_proxy_quota_message_from_value),
        serde_json::Value::Object(map) => map
            .values()
            .find_map(extract_runtime_proxy_quota_message_from_value),
        _ => None,
    }
}

fn extract_runtime_proxy_quota_message_candidate(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(message) => {
            runtime_proxy_usage_limit_message(message).then(|| message.to_string())
        }
        serde_json::Value::Object(map) => {
            let message = map
                .get("message")
                .and_then(serde_json::Value::as_str)
                .or_else(|| map.get("detail").and_then(serde_json::Value::as_str))
                .or_else(|| map.get("error").and_then(serde_json::Value::as_str));
            let code = map.get("code").and_then(serde_json::Value::as_str);
            let error_type = map.get("type").and_then(serde_json::Value::as_str);
            let code_matches = matches!(code, Some("insufficient_quota" | "rate_limit_exceeded"));
            let type_matches = matches!(error_type, Some("usage_limit_reached"));
            let message_matches = message.is_some_and(runtime_proxy_usage_limit_message);
            if !(code_matches || type_matches || message_matches) {
                return None;
            }

            Some(
                message
                    .unwrap_or("Upstream Codex account quota was exhausted.")
                    .to_string(),
            )
        }
        _ => None,
    }
}

fn extract_runtime_proxy_quota_message_from_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();
    if runtime_proxy_usage_limit_message(trimmed)
        || lower.contains("usage_limit_reached")
        || lower.contains("insufficient_quota")
        || lower.contains("rate_limit_exceeded")
    {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn runtime_proxy_usage_limit_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("you've hit your usage limit")
        || lower.contains("you have hit your usage limit")
        || lower.contains("the usage limit has been reached")
        || lower.contains("usage limit has been reached")
        || lower.contains("usage limit")
            && (lower.contains("try again at")
                || lower.contains("request to your admin")
                || lower.contains("more access now"))
}

fn runtime_proxy_body_snippet(body: &[u8], max_chars: usize) -> String {
    let normalized = String::from_utf8_lossy(body)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.is_empty() {
        return "-".to_string();
    }

    let snippet = normalized.chars().take(max_chars).collect::<String>();
    if normalized.chars().count() > max_chars {
        format!("{snippet}...")
    } else {
        snippet
    }
}

fn extract_runtime_proxy_previous_response_message_from_value(
    value: &serde_json::Value,
) -> Option<String> {
    let direct_error = value.get("error");
    let response_error = value
        .get("response")
        .and_then(|response| response.get("error"));
    for error in [direct_error, response_error].into_iter().flatten() {
        let code = error.get("code").and_then(serde_json::Value::as_str)?;
        if code != "previous_response_not_found" {
            continue;
        }
        return Some(
            error
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("Previous response could not be found on the selected Codex account.")
                .to_string(),
        );
    }
    None
}

fn extract_runtime_response_ids_from_payload(payload: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .map(|value| extract_runtime_response_ids_from_value(&value))
        .unwrap_or_default()
}

fn extract_runtime_response_ids_from_body_bytes(body: &[u8]) -> Vec<String> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .map(|value| extract_runtime_response_ids_from_value(&value))
        .unwrap_or_default()
}

fn extract_runtime_turn_state_from_payload(payload: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .and_then(|value| extract_runtime_turn_state_from_value(&value))
}

fn extract_runtime_response_ids_from_value(value: &serde_json::Value) -> Vec<String> {
    let mut response_ids = Vec::new();

    if let Some(id) = value
        .get("response")
        .and_then(|response| response.get("id"))
        .and_then(serde_json::Value::as_str)
    {
        response_ids.push(id.to_string());
    }

    if value
        .get("object")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|object| object == "response")
        && let Some(id) = value.get("id").and_then(serde_json::Value::as_str)
        && !response_ids.iter().any(|existing| existing == id)
    {
        response_ids.push(id.to_string());
    }

    response_ids
}

fn extract_runtime_turn_state_from_value(value: &serde_json::Value) -> Option<String> {
    value
        .get("response")
        .and_then(|response| response.get("headers"))
        .and_then(extract_runtime_turn_state_from_headers_value)
        .or_else(|| {
            value
                .get("headers")
                .and_then(extract_runtime_turn_state_from_headers_value)
        })
}

fn extract_runtime_turn_state_from_headers_value(value: &serde_json::Value) -> Option<String> {
    let headers = value.as_object()?;
    headers.iter().find_map(|(name, value)| {
        if name.eq_ignore_ascii_case("x-codex-turn-state") {
            match value {
                serde_json::Value::String(value) => Some(value.clone()),
                serde_json::Value::Array(items) => items.iter().find_map(|item| match item {
                    serde_json::Value::String(value) => Some(value.clone()),
                    _ => None,
                }),
                _ => None,
            }
        } else {
            None
        }
    })
}

fn run_child(binary: &OsString, args: &[OsString], codex_home: &Path) -> Result<ExitStatus> {
    let status = Command::new(binary)
        .args(args)
        .env("CODEX_HOME", codex_home)
        .status()
        .with_context(|| format!("failed to execute {}", binary.to_string_lossy()))?;
    Ok(status)
}

fn exit_with_status(status: ExitStatus) -> Result<()> {
    std::process::exit(status.code().unwrap_or(1));
}

fn collect_run_profile_reports(
    state: &AppState,
    profile_names: Vec<String>,
    base_url: Option<&str>,
) -> Vec<RunProfileProbeReport> {
    let jobs = profile_names
        .into_iter()
        .enumerate()
        .filter_map(|(order_index, name)| {
            let profile = state.profiles.get(&name)?;
            Some(RunProfileProbeJob {
                name,
                order_index,
                codex_home: profile.codex_home.clone(),
            })
        })
        .collect();
    let base_url = base_url.map(str::to_owned);

    map_parallel(jobs, |job| {
        let auth = read_auth_summary(&job.codex_home);
        let result = if auth.quota_compatible {
            fetch_usage(&job.codex_home, base_url.as_deref()).map_err(|err| err.to_string())
        } else {
            Err("auth mode is not quota-compatible".to_string())
        };

        RunProfileProbeReport {
            name: job.name,
            order_index: job.order_index,
            auth,
            result,
        }
    })
}

fn ready_profile_candidates(
    reports: &[RunProfileProbeReport],
    include_code_review: bool,
    preferred_profile: Option<&str>,
    state: &AppState,
    persisted_usage_snapshots: Option<&BTreeMap<String, RuntimeProfileUsageSnapshot>>,
) -> Vec<ReadyProfileCandidate> {
    let candidates = reports
        .iter()
        .filter_map(|report| {
            if !report.auth.quota_compatible {
                return None;
            }

            let (usage, quota_source) = match report.result.as_ref() {
                Ok(usage) => (usage.clone(), RuntimeQuotaSource::LiveProbe),
                Err(_) => {
                    let snapshot = persisted_usage_snapshots
                        .and_then(|snapshots| snapshots.get(&report.name))?;
                    let now = Local::now().timestamp();
                    if !runtime_usage_snapshot_is_usable(snapshot, now) {
                        return None;
                    }
                    (
                        usage_from_runtime_usage_snapshot(snapshot),
                        RuntimeQuotaSource::PersistedSnapshot,
                    )
                }
            };
            if !collect_blocked_limits(&usage, include_code_review).is_empty() {
                return None;
            }

            Some(ReadyProfileCandidate {
                name: report.name.clone(),
                usage,
                order_index: report.order_index,
                preferred: preferred_profile == Some(report.name.as_str()),
                quota_source,
            })
        })
        .collect::<Vec<_>>();

    schedule_ready_profile_candidates(candidates, state, preferred_profile)
}

fn schedule_ready_profile_candidates(
    mut candidates: Vec<ReadyProfileCandidate>,
    state: &AppState,
    preferred_profile: Option<&str>,
) -> Vec<ReadyProfileCandidate> {
    if candidates.len() <= 1 {
        return candidates;
    }

    let now = Local::now().timestamp();
    let best_total_pressure = candidates
        .iter()
        .map(|candidate| ready_profile_score(candidate).total_pressure)
        .min()
        .unwrap_or(i64::MAX);

    candidates.sort_by_key(|candidate| {
        ready_profile_runtime_sort_key(candidate, state, best_total_pressure, now)
    });

    if let Some(preferred_name) = preferred_profile {
        if let Some(preferred_index) = candidates.iter().position(|candidate| {
            candidate.name == preferred_name
                && !profile_in_run_selection_cooldown(state, &candidate.name, now)
        }) {
            let preferred_score = ready_profile_score(&candidates[preferred_index]).total_pressure;
            let selected_score = ready_profile_score(&candidates[0]).total_pressure;

            if preferred_index > 0
                && score_within_bps(
                    preferred_score,
                    selected_score,
                    RUN_SELECTION_HYSTERESIS_BPS,
                )
            {
                let preferred_candidate = candidates.remove(preferred_index);
                candidates.insert(0, preferred_candidate);
            }
        }
    }

    candidates
}

fn ready_profile_runtime_sort_key(
    candidate: &ReadyProfileCandidate,
    state: &AppState,
    best_total_pressure: i64,
    now: i64,
) -> (
    usize,
    usize,
    i64,
    (
        i64,
        i64,
        i64,
        Reverse<i64>,
        Reverse<i64>,
        Reverse<i64>,
        i64,
        i64,
        usize,
        usize,
    ),
) {
    let score = ready_profile_score(candidate);
    let near_optimal = score_within_bps(
        score.total_pressure,
        best_total_pressure,
        RUN_SELECTION_NEAR_OPTIMAL_BPS,
    );
    let recently_used =
        near_optimal && profile_in_run_selection_cooldown(state, &candidate.name, now);
    let last_selected_at = if near_optimal {
        state
            .last_run_selected_at
            .get(&candidate.name)
            .copied()
            .unwrap_or(i64::MIN)
    } else {
        i64::MIN
    };

    (
        if near_optimal { 0usize } else { 1usize },
        if recently_used { 1usize } else { 0usize },
        last_selected_at,
        ready_profile_sort_key(candidate),
    )
}

fn ready_profile_sort_key(
    candidate: &ReadyProfileCandidate,
) -> (
    i64,
    i64,
    i64,
    Reverse<i64>,
    Reverse<i64>,
    Reverse<i64>,
    i64,
    i64,
    usize,
    usize,
) {
    let score = ready_profile_score(candidate);

    (
        score.total_pressure,
        score.weekly_pressure,
        score.five_hour_pressure,
        Reverse(score.reserve_floor),
        Reverse(score.weekly_remaining),
        Reverse(score.five_hour_remaining),
        score.weekly_reset_at,
        score.five_hour_reset_at,
        if candidate.preferred { 0usize } else { 1usize },
        candidate.order_index,
    )
}

fn ready_profile_score(candidate: &ReadyProfileCandidate) -> ReadyProfileScore {
    ready_profile_score_for_route(&candidate.usage, RuntimeRouteKind::Responses)
}

fn ready_profile_score_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> ReadyProfileScore {
    let weekly = required_main_window_snapshot(usage, "weekly");
    let five_hour = required_main_window_snapshot(usage, "5h");

    let weekly_pressure = weekly.map_or(i64::MAX, |window| window.pressure_score);
    let five_hour_pressure = five_hour.map_or(i64::MAX, |window| window.pressure_score);
    let weekly_remaining = weekly.map_or(0, |window| window.remaining_percent);
    let five_hour_remaining = five_hour.map_or(0, |window| window.remaining_percent);
    let weekly_weight = match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => 10,
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => 8,
    };
    let reserve_bias = match runtime_quota_pressure_band_for_route(usage, route_kind) {
        RuntimeQuotaPressureBand::Healthy => 0,
        RuntimeQuotaPressureBand::Thin => 250_000,
        RuntimeQuotaPressureBand::Critical => 1_000_000,
        RuntimeQuotaPressureBand::Exhausted | RuntimeQuotaPressureBand::Unknown => i64::MAX / 4,
    };

    ReadyProfileScore {
        total_pressure: reserve_bias
            .saturating_add(weekly_pressure.saturating_mul(weekly_weight))
            .saturating_add(five_hour_pressure),
        weekly_pressure,
        five_hour_pressure,
        reserve_floor: weekly_remaining.min(five_hour_remaining),
        weekly_remaining,
        five_hour_remaining,
        weekly_reset_at: weekly.map_or(i64::MAX, |window| window.reset_at),
        five_hour_reset_at: five_hour.map_or(i64::MAX, |window| window.reset_at),
    }
}

fn profile_in_run_selection_cooldown(state: &AppState, profile_name: &str, now: i64) -> bool {
    let Some(last_selected_at) = state.last_run_selected_at.get(profile_name).copied() else {
        return false;
    };

    now.saturating_sub(last_selected_at) < RUN_SELECTION_COOLDOWN_SECONDS
}

fn score_within_bps(candidate_score: i64, best_score: i64, bps: i64) -> bool {
    if candidate_score <= best_score {
        return true;
    }

    let lhs = i128::from(candidate_score).saturating_mul(10_000);
    let rhs = i128::from(best_score).saturating_mul(i128::from(10_000 + bps));
    lhs <= rhs
}

fn required_main_window_snapshot(usage: &UsageResponse, label: &str) -> Option<MainWindowSnapshot> {
    let window = find_main_window(usage.rate_limit.as_ref()?, label)?;
    let remaining_percent = remaining_percent(window.used_percent);
    let reset_at = window.reset_at.unwrap_or(i64::MAX);
    let seconds_until_reset = if reset_at == i64::MAX {
        i64::MAX
    } else {
        (reset_at - Local::now().timestamp()).max(0)
    };
    let pressure_score = seconds_until_reset
        .saturating_mul(1_000)
        .checked_div(remaining_percent.max(1))
        .unwrap_or(i64::MAX);

    Some(MainWindowSnapshot {
        remaining_percent,
        reset_at,
        pressure_score,
    })
}

fn active_profile_selection_order(state: &AppState, current_profile: &str) -> Vec<String> {
    std::iter::once(current_profile.to_string())
        .chain(profile_rotation_order(state, current_profile))
        .collect()
}

fn map_parallel<I, O, F>(inputs: Vec<I>, func: F) -> Vec<O>
where
    I: Send,
    O: Send,
    F: Fn(I) -> O + Sync,
{
    if inputs.len() <= 1 {
        return inputs.into_iter().map(func).collect();
    }

    thread::scope(|scope| {
        let func = &func;
        let mut handles = Vec::with_capacity(inputs.len());
        for input in inputs {
            handles.push(scope.spawn(move || func(input)));
        }

        handles
            .into_iter()
            .map(|handle| handle.join().expect("parallel worker panicked"))
            .collect()
    })
}
fn find_ready_profiles(
    state: &AppState,
    current_profile: &str,
    base_url: Option<&str>,
    include_code_review: bool,
) -> Vec<String> {
    ready_profile_candidates(
        &collect_run_profile_reports(
            state,
            profile_rotation_order(state, current_profile),
            base_url,
        ),
        include_code_review,
        None,
        state,
        None,
    )
    .into_iter()
    .map(|candidate| candidate.name)
    .collect()
}

fn profile_rotation_order(state: &AppState, current_profile: &str) -> Vec<String> {
    let names: Vec<String> = state.profiles.keys().cloned().collect();
    let Some(index) = names.iter().position(|name| name == current_profile) else {
        return names
            .into_iter()
            .filter(|name| name != current_profile)
            .collect();
    };

    names
        .iter()
        .skip(index + 1)
        .chain(names.iter().take(index))
        .cloned()
        .collect()
}

fn is_review_invocation(args: &[OsString]) -> bool {
    args.iter().any(|arg| arg == "review")
}

fn same_path(left: &Path, right: &Path) -> bool {
    normalize_path_for_compare(left) == normalize_path_for_compare(right)
}

fn normalize_path_for_compare(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn absolutize(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path);
    }
    let current_dir = env::current_dir().context("failed to determine current directory")?;
    Ok(current_dir.join(path))
}

fn default_codex_home() -> Result<PathBuf> {
    Ok(home_dir()
        .context("failed to determine home directory")?
        .join(DEFAULT_CODEX_DIR))
}

fn shared_codex_root() -> Result<PathBuf> {
    match env::var_os("PRODEX_SHARED_CODEX_HOME") {
        Some(path) => absolutize(PathBuf::from(path)),
        None => default_codex_home(),
    }
}

impl AppPaths {
    fn discover() -> Result<Self> {
        let root = match env::var_os("PRODEX_HOME") {
            Some(path) => absolutize(PathBuf::from(path))?,
            None => home_dir()
                .context("failed to determine home directory")?
                .join(DEFAULT_PRODEX_DIR),
        };

        Ok(Self {
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: shared_codex_root()?,
            legacy_shared_codex_root: root.join("shared"),
            root,
        })
    }
}

impl AppState {
    fn load_with_recovery(paths: &AppPaths) -> Result<RecoveredLoad<Self>> {
        cleanup_stale_login_dirs(paths);
        if !paths.state_file.exists() && !state_last_good_file_path(paths).exists() {
            return Ok(RecoveredLoad {
                value: Self::default(),
                recovered_from_backup: false,
            });
        }

        let loaded = load_json_file_with_backup::<Self>(
            &paths.state_file,
            &state_last_good_file_path(paths),
        )?;
        Ok(RecoveredLoad {
            value: compact_app_state(loaded.value, Local::now().timestamp()),
            recovered_from_backup: loaded.recovered_from_backup,
        })
    }

    fn load(paths: &AppPaths) -> Result<Self> {
        Ok(Self::load_with_recovery(paths)?.value)
    }

    fn save(&self, paths: &AppPaths) -> Result<()> {
        cleanup_stale_login_dirs(paths);
        let _lock = acquire_state_file_lock(paths)?;
        let existing = Self::load(paths)?;
        let merged = compact_app_state(
            merge_app_state_for_save(existing, self),
            Local::now().timestamp(),
        );
        let json =
            serde_json::to_string_pretty(&merged).context("failed to serialize prodex state")?;
        write_state_json_atomic(paths, &json)?;
        Ok(())
    }
}

fn unique_state_temp_file_path(state_file: &Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = STATE_SAVE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let file_name = format!(
        "{}.{}.{}.{}.tmp",
        state_file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("state.json"),
        std::process::id(),
        nanos,
        sequence
    );

    state_file.with_file_name(file_name)
}

fn codex_bin() -> OsString {
    env::var_os("PRODEX_CODEX_BIN").unwrap_or_else(|| OsString::from("codex"))
}

impl Drop for RuntimeBrokerLease {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn runtime_broker_key(upstream_base_url: &str, include_code_review: bool) -> String {
    let mut hasher = DefaultHasher::new();
    upstream_base_url.hash(&mut hasher);
    include_code_review.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn runtime_process_pid_alive(pid: u32) -> bool {
    let proc_dir = PathBuf::from(format!("/proc/{pid}"));
    if proc_dir.exists() {
        return true;
    }
    collect_process_rows().into_iter().any(|row| row.pid == pid)
}

fn runtime_random_token(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = STATE_SAVE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{}-{nanos:x}-{sequence:x}", std::process::id())
}

fn runtime_broker_startup_grace_seconds() -> i64 {
    let ready_timeout_seconds = runtime_broker_ready_timeout_ms().div_ceil(1_000) as i64;
    ready_timeout_seconds
        .saturating_add(1)
        .max(RUNTIME_BROKER_IDLE_GRACE_SECONDS)
}

fn load_runtime_broker_registry(
    paths: &AppPaths,
    broker_key: &str,
) -> Result<Option<RuntimeBrokerRegistry>> {
    let path = runtime_broker_registry_file_path(paths, broker_key);
    let backup_path = runtime_broker_registry_last_good_file_path(paths, broker_key);
    if !path.exists() && !backup_path.exists() {
        return Ok(None);
    }
    let loaded = load_json_file_with_backup::<RuntimeBrokerRegistry>(&path, &backup_path)?;
    Ok(Some(loaded.value))
}

fn save_runtime_broker_registry(
    paths: &AppPaths,
    broker_key: &str,
    registry: &RuntimeBrokerRegistry,
) -> Result<()> {
    let path = runtime_broker_registry_file_path(paths, broker_key);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(registry)
        .context("failed to serialize runtime broker registry")?;
    write_json_file_with_backup(
        &path,
        &runtime_broker_registry_last_good_file_path(paths, broker_key),
        &json,
        |content| {
            let _: RuntimeBrokerRegistry = serde_json::from_str(content)
                .context("failed to validate runtime broker registry")?;
            Ok(())
        },
    )
}

fn remove_runtime_broker_registry_if_token_matches(
    paths: &AppPaths,
    broker_key: &str,
    instance_token: &str,
) {
    let Ok(Some(existing)) = load_runtime_broker_registry(paths, broker_key) else {
        return;
    };
    if existing.instance_token != instance_token {
        return;
    }
    for path in [
        runtime_broker_registry_file_path(paths, broker_key),
        runtime_broker_registry_last_good_file_path(paths, broker_key),
    ] {
        let _ = fs::remove_file(path);
    }
}

fn runtime_broker_client() -> Result<Client> {
    Client::builder()
        .connect_timeout(Duration::from_millis(
            runtime_broker_health_connect_timeout_ms(),
        ))
        .timeout(Duration::from_millis(
            runtime_broker_health_read_timeout_ms(),
        ))
        .build()
        .context("failed to build runtime broker control client")
}

fn runtime_broker_health_url(registry: &RuntimeBrokerRegistry) -> String {
    format!("http://{}/__prodex/runtime/health", registry.listen_addr)
}

fn runtime_broker_activate_url(registry: &RuntimeBrokerRegistry) -> String {
    format!("http://{}/__prodex/runtime/activate", registry.listen_addr)
}

fn probe_runtime_broker_health(
    registry: &RuntimeBrokerRegistry,
) -> Result<Option<RuntimeBrokerHealth>> {
    let response = match runtime_broker_client()?
        .get(runtime_broker_health_url(registry))
        .header("X-Prodex-Admin-Token", &registry.admin_token)
        .send()
    {
        Ok(response) => response,
        Err(_) => return Ok(None),
    };
    if !response.status().is_success() {
        return Ok(None);
    }
    let health = response
        .json::<RuntimeBrokerHealth>()
        .context("failed to decode runtime broker health response")?;
    Ok(Some(health))
}

fn activate_runtime_broker_profile(
    registry: &RuntimeBrokerRegistry,
    current_profile: &str,
) -> Result<()> {
    let response = runtime_broker_client()?
        .post(runtime_broker_activate_url(registry))
        .header("X-Prodex-Admin-Token", &registry.admin_token)
        .json(&serde_json::json!({
            "current_profile": current_profile,
        }))
        .send()
        .context("failed to send runtime broker activation request")?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        bail!(
            "runtime broker activation failed with HTTP {}{}",
            status,
            if body.is_empty() {
                String::new()
            } else {
                format!(": {body}")
            }
        );
    }
    Ok(())
}

fn create_runtime_broker_lease(paths: &AppPaths, broker_key: &str) -> Result<RuntimeBrokerLease> {
    let lease_dir = runtime_broker_lease_dir(paths, broker_key);
    fs::create_dir_all(&lease_dir)
        .with_context(|| format!("failed to create {}", lease_dir.display()))?;
    let path = lease_dir.join(format!(
        "{}-{}.lease",
        std::process::id(),
        runtime_random_token("lease")
    ));
    fs::write(&path, format!("pid={}\n", std::process::id()))
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(RuntimeBrokerLease { path })
}

fn cleanup_runtime_broker_stale_leases(paths: &AppPaths, broker_key: &str) -> usize {
    let lease_dir = runtime_broker_lease_dir(paths, broker_key);
    let Ok(entries) = fs::read_dir(&lease_dir) else {
        return 0;
    };
    let mut live = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let pid = file_name
            .split('-')
            .next()
            .and_then(|value| value.parse::<u32>().ok());
        if pid.is_some_and(runtime_process_pid_alive) {
            live += 1;
        } else {
            let _ = fs::remove_file(path);
        }
    }
    live
}

fn wait_for_existing_runtime_broker_recovery_or_exit(
    paths: &AppPaths,
    broker_key: &str,
    upstream_base_url: &str,
    include_code_review: bool,
) -> Result<Option<RuntimeBrokerRegistry>> {
    let started_at = Instant::now();
    let poll_interval = Duration::from_millis(RUNTIME_BROKER_POLL_INTERVAL_MS);
    while started_at.elapsed() < Duration::from_millis(runtime_broker_ready_timeout_ms()) {
        let Some(existing) = load_runtime_broker_registry(paths, broker_key)? else {
            return Ok(None);
        };

        if existing.upstream_base_url == upstream_base_url
            && existing.include_code_review == include_code_review
            && let Some(health) = probe_runtime_broker_health(&existing)?
            && health.instance_token == existing.instance_token
        {
            return Ok(Some(existing));
        }

        if !runtime_process_pid_alive(existing.pid) {
            remove_runtime_broker_registry_if_token_matches(
                paths,
                broker_key,
                &existing.instance_token,
            );
            return Ok(None);
        }

        thread::sleep(poll_interval);
    }

    Ok(None)
}

fn wait_for_runtime_broker_ready(
    paths: &AppPaths,
    broker_key: &str,
    expected_instance_token: &str,
) -> Result<RuntimeBrokerRegistry> {
    let started_at = Instant::now();
    let poll_interval = Duration::from_millis(RUNTIME_BROKER_POLL_INTERVAL_MS);
    while started_at.elapsed() < Duration::from_millis(runtime_broker_ready_timeout_ms()) {
        if let Some(registry) = load_runtime_broker_registry(paths, broker_key)? {
            if registry.instance_token == expected_instance_token
                && let Some(health) = probe_runtime_broker_health(&registry)?
                && health.instance_token == expected_instance_token
            {
                return Ok(registry);
            }
        }
        thread::sleep(poll_interval);
    }
    bail!("timed out waiting for runtime broker readiness");
}

fn spawn_runtime_broker_process(
    paths: &AppPaths,
    current_profile: &str,
    upstream_base_url: &str,
    include_code_review: bool,
    broker_key: &str,
    instance_token: &str,
    admin_token: &str,
) -> Result<()> {
    let current_exe = env::current_exe().context("failed to locate current prodex binary")?;
    Command::new(current_exe)
        .arg("__runtime-broker")
        .arg("--current-profile")
        .arg(current_profile)
        .arg("--upstream-base-url")
        .arg(upstream_base_url)
        .arg("--include-code-review")
        .arg(include_code_review.to_string())
        .arg("--broker-key")
        .arg(broker_key)
        .arg("--instance-token")
        .arg(instance_token)
        .arg("--admin-token")
        .arg(admin_token)
        .env("PRODEX_HOME", &paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to spawn runtime broker process")?;
    Ok(())
}

fn ensure_runtime_rotation_proxy_endpoint(
    paths: &AppPaths,
    current_profile: &str,
    upstream_base_url: &str,
    include_code_review: bool,
) -> Result<RuntimeProxyEndpoint> {
    let broker_key = runtime_broker_key(upstream_base_url, include_code_review);
    let ensure_lock_path = runtime_broker_ensure_lock_path(paths, &broker_key);
    let _ensure_lock = acquire_json_file_lock(&ensure_lock_path)?;

    if let Some(existing) = wait_for_existing_runtime_broker_recovery_or_exit(
        paths,
        &broker_key,
        upstream_base_url,
        include_code_review,
    )? {
        activate_runtime_broker_profile(&existing, current_profile)?;
        let lease = create_runtime_broker_lease(paths, &broker_key)?;
        let listen_addr = existing.listen_addr.parse().with_context(|| {
            format!(
                "invalid runtime broker listen address {}",
                existing.listen_addr
            )
        })?;
        return Ok(RuntimeProxyEndpoint {
            listen_addr,
            _lease: Some(lease),
        });
    }

    if let Some(existing) = load_runtime_broker_registry(paths, &broker_key)? {
        if !runtime_process_pid_alive(existing.pid) {
            remove_runtime_broker_registry_if_token_matches(
                paths,
                &broker_key,
                &existing.instance_token,
            );
        } else if existing.upstream_base_url == upstream_base_url
            && existing.include_code_review == include_code_review
            && let Some(health) = probe_runtime_broker_health(&existing)?
            && health.instance_token == existing.instance_token
        {
            activate_runtime_broker_profile(&existing, current_profile)?;
            let lease = create_runtime_broker_lease(paths, &broker_key)?;
            let listen_addr = existing.listen_addr.parse().with_context(|| {
                format!(
                    "invalid runtime broker listen address {}",
                    existing.listen_addr
                )
            })?;
            return Ok(RuntimeProxyEndpoint {
                listen_addr,
                _lease: Some(lease),
            });
        }
    }

    let instance_token = runtime_random_token("broker");
    let admin_token = runtime_random_token("admin");
    spawn_runtime_broker_process(
        paths,
        current_profile,
        upstream_base_url,
        include_code_review,
        &broker_key,
        &instance_token,
        &admin_token,
    )?;
    let registry = wait_for_runtime_broker_ready(paths, &broker_key, &instance_token)?;
    activate_runtime_broker_profile(&registry, current_profile)?;
    let lease = create_runtime_broker_lease(paths, &broker_key)?;
    let listen_addr = registry.listen_addr.parse().with_context(|| {
        format!(
            "invalid runtime broker listen address {}",
            registry.listen_addr
        )
    })?;
    Ok(RuntimeProxyEndpoint {
        listen_addr,
        _lease: Some(lease),
    })
}
