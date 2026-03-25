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
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
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

const DEFAULT_PRODEX_DIR: &str = ".prodex";
const DEFAULT_CODEX_DIR: &str = ".codex";
const DEFAULT_CHATGPT_BASE_URL: &str = "https://chatgpt.com/backend-api";
const DEFAULT_WATCH_INTERVAL_SECONDS: u64 = 5;
const RUN_SELECTION_NEAR_OPTIMAL_BPS: i64 = 1_000;
const RUN_SELECTION_HYSTERESIS_BPS: i64 = 500;
const RUN_SELECTION_COOLDOWN_SECONDS: i64 = 15 * 60;
const RESPONSE_PROFILE_BINDING_LIMIT: usize = 16_384;
const TURN_STATE_PROFILE_BINDING_LIMIT: usize = 4_096;
const SESSION_ID_PROFILE_BINDING_LIMIT: usize = 4_096;
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
const RUNTIME_STATE_SAVE_DEBOUNCE_MS: u64 = if cfg!(test) { 5 } else { 150 };
const RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS: i64 = if cfg!(test) { 4 } else { 180 };
const RUNTIME_PROFILE_SUCCESS_STREAK_DECAY_SECONDS: i64 = if cfg!(test) { 8 } else { 300 };
const RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY: u32 = 4;
const RUNTIME_PROFILE_CONNECT_FAILURE_HEALTH_PENALTY: u32 = 5;
const RUNTIME_PROFILE_FORWARD_FAILURE_HEALTH_PENALTY: u32 = 3;
const RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY: u32 = 2;
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
const RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS: u64 = if cfg!(test) { 50 } else { 1_000 };
const RUNTIME_PROXY_SSE_LOOKAHEAD_BYTES: usize = 8 * 1024;
const RUNTIME_PROXY_LOG_FILE_PREFIX: &str = "prodex-runtime";
const RUNTIME_PROXY_LATEST_LOG_POINTER: &str = "prodex-runtime-latest.path";
const RUNTIME_PROXY_DOCTOR_TAIL_BYTES: usize = 128 * 1024;
const CLI_WIDTH: usize = 110;
const CLI_MIN_WIDTH: usize = 60;
const CLI_LABEL_WIDTH: usize = 16;
const CLI_MIN_LABEL_WIDTH: usize = 10;
const CLI_MAX_LABEL_WIDTH: usize = 24;
const CLI_TABLE_GAP: &str = "  ";
const SHARED_CODEX_DIR_NAMES: &[&str] = &["sessions", "archived_sessions", "shell_snapshots"];
const SHARED_CODEX_FILE_NAMES: &[&str] = &["history.jsonl"];
const SHARED_CODEX_SQLITE_PREFIXES: &[&str] = &["state_", "logs_"];
const SHARED_CODEX_SQLITE_SUFFIXES: &[&str] = &[".sqlite", ".sqlite-shm", ".sqlite-wal"];
static STATE_SAVE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static RUNTIME_STATE_SAVE_QUEUE: OnceLock<Arc<RuntimeStateSaveQueue>> = OnceLock::new();
static RUNTIME_PROBE_REFRESH_QUEUE: OnceLock<Arc<RuntimeProbeRefreshQueue>> = OnceLock::new();

fn create_runtime_proxy_log_path() -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    env::temp_dir().join(format!(
        "{RUNTIME_PROXY_LOG_FILE_PREFIX}-{}-{millis}.log",
        std::process::id()
    ))
}

fn runtime_proxy_latest_log_pointer_path() -> PathBuf {
    env::temp_dir().join(RUNTIME_PROXY_LATEST_LOG_POINTER)
}

fn initialize_runtime_proxy_log_path() -> PathBuf {
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

fn schedule_runtime_state_save(
    shared: &RuntimeRotationProxyShared,
    state: AppState,
    profile_scores: BTreeMap<String, RuntimeProfileHealth>,
    usage_snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot>,
    paths: AppPaths,
    reason: &str,
) {
    let revision = shared.state_save_revision.fetch_add(1, Ordering::SeqCst) + 1;
    let queue = runtime_state_save_queue();
    let mut pending = queue
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    pending.insert(
        paths.state_file.clone(),
        RuntimeStateSaveJob {
            paths,
            state,
            profile_scores,
            usage_snapshots,
            revision,
            latest_revision: Arc::clone(&shared.state_save_revision),
            log_path: shared.log_path.clone(),
            reason: reason.to_string(),
            ready_at: Instant::now() + runtime_state_save_debounce(reason),
        },
    );
    drop(pending);
    queue.wake.notify_one();
}

fn runtime_state_save_queue() -> Arc<RuntimeStateSaveQueue> {
    Arc::clone(RUNTIME_STATE_SAVE_QUEUE.get_or_init(|| {
        let queue = Arc::new(RuntimeStateSaveQueue {
            pending: Mutex::new(BTreeMap::new()),
            wake: Condvar::new(),
        });
        let worker_queue = Arc::clone(&queue);
        thread::spawn(move || runtime_state_save_worker_loop(worker_queue));
        queue
    }))
}

fn runtime_probe_refresh_queue() -> Arc<RuntimeProbeRefreshQueue> {
    Arc::clone(RUNTIME_PROBE_REFRESH_QUEUE.get_or_init(|| {
        let queue = Arc::new(RuntimeProbeRefreshQueue {
            pending: Mutex::new(BTreeMap::new()),
            wake: Condvar::new(),
        });
        let worker_queue = Arc::clone(&queue);
        thread::spawn(move || runtime_probe_refresh_worker_loop(worker_queue));
        queue
    }))
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
            match save_runtime_state_snapshot_if_latest(
                &job.paths,
                &job.state,
                &job.profile_scores,
                &job.usage_snapshots,
                job.revision,
                &job.latest_revision,
            ) {
                Ok(true) => runtime_proxy_log_to_path(
                    &job.log_path,
                    &format!(
                        "state_save_ok revision={} reason={}",
                        job.revision, job.reason
                    ),
                ),
                Ok(false) => runtime_proxy_log_to_path(
                    &job.log_path,
                    &format!(
                        "state_save_skipped revision={} reason={}",
                        job.revision, job.reason
                    ),
                ),
                Err(err) => runtime_proxy_log_to_path(
                    &job.log_path,
                    &format!(
                        "state_save_error revision={} reason={} stage=write error={err:#}",
                        job.revision, job.reason
                    ),
                ),
            }
        }
    }
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
    pending.insert(
        (state_file.clone(), profile_name.to_string()),
        RuntimeProbeRefreshJob {
            shared: shared.clone(),
            profile_name: profile_name.to_string(),
            codex_home: codex_home.to_path_buf(),
            upstream_base_url,
        },
    );
    drop(pending);
    queue.wake.notify_one();
    runtime_proxy_log(
        shared,
        format!("profile_probe_refresh_queued profile={profile_name} reason={reason}"),
    );
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
            let now = Local::now().timestamp();
            if let Ok(mut runtime) = job.shared.runtime.lock() {
                runtime.profile_probe_cache.insert(
                    job.profile_name.clone(),
                    RuntimeProfileProbeCacheEntry {
                        checked_at: now,
                        auth: auth.clone(),
                        result: result.clone(),
                    },
                );
            }
            match result {
                Ok(_) => runtime_proxy_log(
                    &job.shared,
                    format!("profile_probe_refresh_ok profile={}", job.profile_name),
                ),
                Err(err) => runtime_proxy_log(
                    &job.shared,
                    format!(
                        "profile_probe_refresh_error profile={} error={err}",
                        job.profile_name
                    ),
                ),
            }
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

fn state_lock_file_path(state_file: &Path) -> PathBuf {
    state_file.with_extension("json.lock")
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

fn merge_profile_bindings(
    existing: &BTreeMap<String, ResponseProfileBinding>,
    incoming: &BTreeMap<String, ResponseProfileBinding>,
    profiles: &BTreeMap<String, ProfileEntry>,
    max_entries: usize,
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
    prune_profile_bindings(&mut merged, max_entries);
    merged
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

    AppState {
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
            RESPONSE_PROFILE_BINDING_LIMIT,
        ),
        session_profile_bindings: merge_profile_bindings(
            &existing.session_profile_bindings,
            &snapshot.session_profile_bindings,
            &profiles,
            SESSION_ID_PROFILE_BINDING_LIMIT,
        ),
    }
}

fn merge_app_state_for_save(existing: AppState, desired: &AppState) -> AppState {
    let active_profile = desired
        .active_profile
        .clone()
        .filter(|profile_name| desired.profiles.contains_key(profile_name));
    AppState {
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
            RESPONSE_PROFILE_BINDING_LIMIT,
        ),
        session_profile_bindings: merge_profile_bindings(
            &existing.session_profile_bindings,
            &desired.session_profile_bindings,
            &desired.profiles,
            SESSION_ID_PROFILE_BINDING_LIMIT,
        ),
    }
}

fn write_state_json_atomic(paths: &AppPaths, json: &str) -> Result<()> {
    let temp_file = unique_state_temp_file_path(&paths.state_file);
    fs::write(&temp_file, json)
        .with_context(|| format!("failed to write {}", temp_file.display()))?;
    fs::rename(&temp_file, &paths.state_file).with_context(|| {
        format!(
            "failed to replace state file {}",
            paths.state_file.display()
        )
    })?;
    Ok(())
}

fn runtime_scores_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-scores.json")
}

fn runtime_usage_snapshots_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-usage-snapshots.json")
}

fn update_check_cache_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("update-check.json")
}

fn runtime_profile_score_profile_name(key: &str) -> &str {
    key.rsplit(':').next().unwrap_or(key)
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
    merged.retain(|key, _| profiles.contains_key(runtime_profile_score_profile_name(key)));
    merged
}

fn load_runtime_profile_scores(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<BTreeMap<String, RuntimeProfileHealth>> {
    let path = runtime_scores_file_path(paths);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut scores: BTreeMap<String, RuntimeProfileHealth> = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    scores.retain(|key, _| profiles.contains_key(runtime_profile_score_profile_name(key)));
    Ok(scores)
}

fn save_runtime_profile_scores(
    paths: &AppPaths,
    scores: &BTreeMap<String, RuntimeProfileHealth>,
) -> Result<()> {
    let path = runtime_scores_file_path(paths);
    let temp_file = unique_state_temp_file_path(&path);
    let json =
        serde_json::to_string_pretty(scores).context("failed to serialize runtime scores")?;
    fs::write(&temp_file, json)
        .with_context(|| format!("failed to write {}", temp_file.display()))?;
    fs::rename(&temp_file, &path)
        .with_context(|| format!("failed to replace runtime scores file {}", path.display()))?;
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
    merged.retain(|profile_name, _| profiles.contains_key(profile_name));
    merged
}

fn load_runtime_usage_snapshots(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<BTreeMap<String, RuntimeProfileUsageSnapshot>> {
    let path = runtime_usage_snapshots_file_path(paths);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot> =
        serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", path.display()))?;
    snapshots.retain(|profile_name, _| profiles.contains_key(profile_name));
    Ok(snapshots)
}

fn save_runtime_usage_snapshots(
    paths: &AppPaths,
    snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
) -> Result<()> {
    let path = runtime_usage_snapshots_file_path(paths);
    let temp_file = unique_state_temp_file_path(&path);
    let json = serde_json::to_string_pretty(snapshots)
        .context("failed to serialize runtime usage snapshots")?;
    fs::write(&temp_file, json)
        .with_context(|| format!("failed to write {}", temp_file.display()))?;
    fs::rename(&temp_file, &path).with_context(|| {
        format!(
            "failed to replace runtime usage snapshots file {}",
            path.display()
        )
    })?;
    Ok(())
}

fn save_runtime_state_snapshot_if_latest(
    paths: &AppPaths,
    snapshot: &AppState,
    profile_scores: &BTreeMap<String, RuntimeProfileHealth>,
    usage_snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    revision: u64,
    latest_revision: &AtomicU64,
) -> Result<bool> {
    if latest_revision.load(Ordering::SeqCst) != revision {
        return Ok(false);
    }

    let _lock = acquire_state_file_lock(paths)?;

    if latest_revision.load(Ordering::SeqCst) != revision {
        return Ok(false);
    }

    let existing = AppState::load(paths)?;
    let merged = merge_runtime_state_snapshot(existing, snapshot);
    let json = serde_json::to_string_pretty(&merged).context("failed to serialize prodex state")?;
    let existing_scores = load_runtime_profile_scores(paths, &merged.profiles)?;
    let merged_scores =
        merge_runtime_profile_scores(&existing_scores, profile_scores, &merged.profiles);
    let existing_usage_snapshots = load_runtime_usage_snapshots(paths, &merged.profiles)?;
    let merged_usage_snapshots =
        merge_runtime_usage_snapshots(&existing_usage_snapshots, usage_snapshots, &merged.profiles);

    if latest_revision.load(Ordering::SeqCst) != revision {
        return Ok(false);
    }

    write_state_json_atomic(paths, &json)?;
    save_runtime_profile_scores(paths, &merged_scores)?;
    save_runtime_usage_snapshots(paths, &merged_usage_snapshots)?;
    Ok(true)
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
    Doctor(DoctorArgs),
    #[command(trailing_var_arg = true)]
    Login(CodexPassthroughArgs),
    Logout(ProfileSelector),
    Quota(QuotaArgs),
    #[command(trailing_var_arg = true)]
    Run(RunArgs),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResponseProfileBinding {
    profile_name: String,
    bound_at: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct UpdateCheckCache {
    latest_version: String,
    checked_at: i64,
}

#[derive(Debug, Deserialize)]
struct CratesIoVersionResponse {
    #[serde(rename = "crate")]
    crate_info: CratesIoCrateInfo,
}

#[derive(Debug, Deserialize)]
struct CratesIoCrateInfo {
    max_version: String,
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

#[derive(Debug)]
struct BlockedLimit {
    message: String,
}

#[derive(Debug, Clone)]
struct AuthSummary {
    label: String,
    quota_compatible: bool,
}

#[derive(Debug, Clone)]
struct UsageAuth {
    access_token: String,
    account_id: Option<String>,
}

#[derive(Debug)]
struct QuotaReport {
    name: String,
    active: bool,
    auth: AuthSummary,
    result: std::result::Result<UsageResponse, String>,
}

#[derive(Debug)]
struct QuotaFetchJob {
    name: String,
    active: bool,
    codex_home: PathBuf,
}

#[derive(Debug)]
struct ProfileSummaryJob {
    name: String,
    active: bool,
    managed: bool,
    email: Option<String>,
    codex_home: PathBuf,
}

#[derive(Debug)]
struct ProfileSummaryReport {
    name: String,
    active: bool,
    managed: bool,
    auth: AuthSummary,
    email: Option<String>,
    codex_home: PathBuf,
}

#[derive(Debug)]
struct DoctorProfileReport {
    summary: ProfileSummaryReport,
    quota: Option<std::result::Result<UsageResponse, String>>,
}

#[derive(Debug)]
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
}

#[derive(Debug)]
struct RuntimeStateSaveJob {
    paths: AppPaths,
    state: AppState,
    profile_scores: BTreeMap<String, RuntimeProfileHealth>,
    usage_snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot>,
    revision: u64,
    latest_revision: Arc<AtomicU64>,
    log_path: PathBuf,
    reason: String,
    ready_at: Instant,
}

#[derive(Debug)]
struct RuntimeProbeRefreshQueue {
    pending: Mutex<BTreeMap<(PathBuf, String), RuntimeProbeRefreshJob>>,
    wake: Condvar,
}

#[derive(Debug, Clone)]
struct RuntimeProbeRefreshJob {
    shared: RuntimeRotationProxyShared,
    profile_name: String,
    codex_home: PathBuf,
    upstream_base_url: String,
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
    profile_probe_cache: BTreeMap<String, RuntimeProfileProbeCacheEntry>,
    profile_usage_snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot>,
    profile_retry_backoff_until: BTreeMap<String, i64>,
    profile_transport_backoff_until: BTreeMap<String, i64>,
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
    first_timestamp: Option<String>,
    last_timestamp: Option<String>,
    diagnosis: String,
}

struct RuntimeRotationProxy {
    server: Arc<TinyServer>,
    shutdown: Arc<AtomicBool>,
    worker_threads: Vec<thread::JoinHandle<()>>,
    accept_worker_count: usize,
    listen_addr: std::net::SocketAddr,
}

type RuntimeLocalWebSocket = WsSocket<Box<dyn TinyReadWrite + Send>>;
type RuntimeUpstreamWebSocket = WsSocket<MaybeTlsStream<TcpStream>>;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SharedCodexEntryKind {
    Directory,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SharedCodexEntry {
    name: String,
    kind: SharedCodexEntryKind,
}

fn section_header(title: &str) -> String {
    section_header_with_width(title, current_cli_width())
}

fn section_header_with_width(title: &str, total_width: usize) -> String {
    let prefix = format!("[ {title} ] ");
    let width = text_width(&prefix);
    if width >= total_width {
        return prefix;
    }

    format!("{prefix}{}", "=".repeat(total_width - width))
}

fn text_width(value: &str) -> usize {
    value.chars().count()
}

fn fit_cell(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    if text_width(value) <= width {
        return value.to_string();
    }

    if width <= 3 {
        return ".".repeat(width);
    }

    let mut output = String::new();
    for ch in value.chars().take(width - 3) {
        output.push(ch);
    }
    output.push_str("...");
    output
}

fn chunk_token(token: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    if token.is_empty() {
        return vec![String::new()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    for ch in token.chars() {
        current.push(ch);
        if text_width(&current) >= width {
            chunks.push(std::mem::take(&mut current));
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

fn wrap_text(input: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    for paragraph in input.lines() {
        if paragraph.trim().is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            for piece in chunk_token(word, width) {
                if current.is_empty() {
                    current.push_str(&piece);
                } else if text_width(&current) + 1 + text_width(&piece) <= width {
                    current.push(' ');
                    current.push_str(&piece);
                } else {
                    lines.push(std::mem::take(&mut current));
                    current.push_str(&piece);
                }
            }
        }

        if !current.is_empty() {
            lines.push(current);
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

fn current_cli_width() -> usize {
    terminal_width_chars()
        .unwrap_or(CLI_WIDTH)
        .max(CLI_MIN_WIDTH)
}

fn terminal_size_override_usize(env_key: &str) -> Option<usize> {
    env::var(env_key)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
}

fn terminal_dimensions_from_tty() -> Option<(usize, usize)> {
    let tty = fs::File::open("/dev/tty").ok()?;
    let output = Command::new("stty").arg("size").stdin(tty).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8(output.stdout).ok()?;
    let mut parts = text.split_whitespace();
    let rows = parts.next()?.parse::<usize>().ok()?;
    let cols = parts.next()?.parse::<usize>().ok()?;
    Some((rows, cols))
}

fn terminal_width_chars() -> Option<usize> {
    terminal_size_override_usize("PRODEX_TERM_COLUMNS")
        .or_else(|| terminal_dimensions_from_tty().map(|(_, cols)| cols))
}

fn terminal_height_lines() -> Option<usize> {
    terminal_size_override_usize("PRODEX_TERM_LINES")
        .or_else(|| terminal_size_override_usize("LINES"))
        .or_else(|| terminal_dimensions_from_tty().map(|(rows, _)| rows))
}

fn panel_label_width(fields: &[(String, String)], total_width: usize) -> usize {
    let longest = fields
        .iter()
        .map(|(label, _)| text_width(label) + 1)
        .max()
        .unwrap_or(CLI_LABEL_WIDTH);
    let max_by_width = total_width
        .saturating_sub(20)
        .clamp(CLI_MIN_LABEL_WIDTH, CLI_MAX_LABEL_WIDTH);
    let preferred_cap = (total_width / 4).clamp(CLI_MIN_LABEL_WIDTH, CLI_MAX_LABEL_WIDTH);
    longest.clamp(CLI_MIN_LABEL_WIDTH, max_by_width.min(preferred_cap))
}

fn format_field_lines_with_layout(
    label: &str,
    value: &str,
    total_width: usize,
    label_width: usize,
) -> Vec<String> {
    let label = format!("{label}:");
    let value_width = total_width.saturating_sub(label_width + 1).max(1);
    let wrapped = wrap_text(value, value_width);
    let mut lines = Vec::new();

    for (index, line) in wrapped.into_iter().enumerate() {
        let field_label = if index == 0 { label.as_str() } else { "" };
        lines.push(format!(
            "{field_label:<label_w$} {line}",
            label_w = label_width
        ));
    }

    lines
}

fn print_panel(title: &str, fields: &[(String, String)]) {
    let total_width = current_cli_width();
    let label_width = panel_label_width(fields, total_width);
    println!("{}", section_header_with_width(title, total_width));
    for (label, value) in fields {
        for line in format_field_lines_with_layout(label, value, total_width, label_width) {
            println!("{line}");
        }
    }
}

fn render_panel(title: &str, fields: &[(String, String)]) -> String {
    let total_width = current_cli_width();
    let label_width = panel_label_width(fields, total_width);
    let mut lines = vec![section_header_with_width(title, total_width)];
    for (label, value) in fields {
        lines.extend(format_field_lines_with_layout(
            label,
            value,
            total_width,
            label_width,
        ));
    }
    lines.join("\n")
}

fn print_wrapped_stderr(message: &str) {
    for line in wrap_text(message, current_cli_width()) {
        eprintln!("{line}");
    }
}

fn show_update_notice_if_available(command: &Commands) -> Result<()> {
    if !should_emit_update_notice(command) {
        return Ok(());
    }

    let paths = AppPaths::discover()?;
    if let Some(latest_version) = latest_prodex_version(&paths)?
        && version_is_newer(&latest_version, env!("CARGO_PKG_VERSION"))
    {
        print_wrapped_stderr(&section_header("Update Available"));
        print_wrapped_stderr(&format!(
            "A newer prodex release is available: {} -> {}",
            env!("CARGO_PKG_VERSION"),
            latest_version
        ));
        print_wrapped_stderr("Update with: cargo install prodex --force");
    }

    Ok(())
}

fn should_emit_update_notice(command: &Commands) -> bool {
    match command {
        Commands::Doctor(args) => !args.json,
        Commands::Quota(args) => !args.raw,
        _ => true,
    }
}

fn latest_prodex_version(paths: &AppPaths) -> Result<Option<String>> {
    if let Some(cached) = load_update_check_cache(paths)?
        && Local::now().timestamp().saturating_sub(cached.checked_at)
            < update_check_cache_ttl_seconds(&cached.latest_version, env!("CARGO_PKG_VERSION"))
    {
        return Ok(Some(cached.latest_version));
    }

    let latest_version = match fetch_latest_prodex_version() {
        Ok(version) => version,
        Err(_) => return Ok(None),
    };
    save_update_check_cache(
        paths,
        &UpdateCheckCache {
            latest_version: latest_version.clone(),
            checked_at: Local::now().timestamp(),
        },
    )?;
    Ok(Some(latest_version))
}

fn update_check_cache_ttl_seconds(cached_latest_version: &str, current_version: &str) -> i64 {
    if version_is_newer(cached_latest_version, current_version) {
        UPDATE_CHECK_CACHE_TTL_SECONDS
    } else {
        UPDATE_CHECK_STALE_CURRENT_TTL_SECONDS
    }
}

fn load_update_check_cache(paths: &AppPaths) -> Result<Option<UpdateCheckCache>> {
    let path = update_check_cache_file_path(paths);
    if !path.exists() {
        return Ok(None);
    }
    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let cache = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(Some(cache))
}

fn save_update_check_cache(paths: &AppPaths, cache: &UpdateCheckCache) -> Result<()> {
    fs::create_dir_all(&paths.root)
        .with_context(|| format!("failed to create {}", paths.root.display()))?;
    let path = update_check_cache_file_path(paths);
    let temp_file = unique_state_temp_file_path(&path);
    let json =
        serde_json::to_string_pretty(cache).context("failed to serialize update check cache")?;
    fs::write(&temp_file, json)
        .with_context(|| format!("failed to write {}", temp_file.display()))?;
    fs::rename(&temp_file, &path)
        .with_context(|| format!("failed to replace update cache file {}", path.display()))?;
    Ok(())
}

fn fetch_latest_prodex_version() -> Result<String> {
    let client = Client::builder()
        .connect_timeout(Duration::from_millis(UPDATE_CHECK_HTTP_CONNECT_TIMEOUT_MS))
        .timeout(Duration::from_millis(UPDATE_CHECK_HTTP_READ_TIMEOUT_MS))
        .build()
        .context("failed to build update-check HTTP client")?;
    let response = client
        .get("https://crates.io/api/v1/crates/prodex")
        .header(
            "User-Agent",
            format!("prodex/{}", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .context("failed to request crates.io prodex metadata")?;
    let status = response.status();
    let body = response
        .bytes()
        .context("failed to read crates.io prodex metadata")?;
    if !status.is_success() {
        bail!("crates.io returned HTTP {}", status.as_u16());
    }
    let payload: CratesIoVersionResponse =
        serde_json::from_slice(&body).context("failed to parse crates.io prodex metadata")?;
    Ok(payload.crate_info.max_version)
}

fn version_is_newer(candidate: &str, current: &str) -> bool {
    parse_release_version(candidate) > parse_release_version(current)
}

fn parse_release_version(version: &str) -> Vec<u64> {
    version
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

fn timeout_override_ms(env_key: &str, default_ms: u64) -> u64 {
    env::var(env_key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default_ms)
}

fn usize_override(env_key: &str, default_value: usize) -> usize {
    env::var(env_key)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default_value)
}

fn runtime_proxy_http_connect_timeout_ms() -> u64 {
    timeout_override_ms(
        "PRODEX_RUNTIME_PROXY_HTTP_CONNECT_TIMEOUT_MS",
        RUNTIME_PROXY_HTTP_CONNECT_TIMEOUT_MS,
    )
}

fn runtime_proxy_stream_idle_timeout_ms() -> u64 {
    timeout_override_ms(
        "PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS",
        RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS,
    )
}

fn runtime_proxy_sse_lookahead_timeout_ms() -> u64 {
    timeout_override_ms(
        "PRODEX_RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS",
        RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS,
    )
}

fn runtime_proxy_websocket_connect_timeout_ms() -> u64 {
    timeout_override_ms(
        "PRODEX_RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS",
        RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS,
    )
}

fn runtime_proxy_admission_wait_budget_ms() -> u64 {
    timeout_override_ms(
        "PRODEX_RUNTIME_PROXY_ADMISSION_WAIT_BUDGET_MS",
        RUNTIME_PROXY_ADMISSION_WAIT_BUDGET_MS,
    )
}

fn runtime_proxy_admission_wait_poll_ms() -> u64 {
    timeout_override_ms(
        "PRODEX_RUNTIME_PROXY_ADMISSION_WAIT_POLL_MS",
        RUNTIME_PROXY_ADMISSION_WAIT_POLL_MS,
    )
}

fn runtime_proxy_long_lived_queue_wait_budget_ms() -> u64 {
    timeout_override_ms(
        "PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS",
        RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS,
    )
}

fn runtime_proxy_long_lived_queue_wait_poll_ms() -> u64 {
    timeout_override_ms(
        "PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_POLL_MS",
        RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_POLL_MS,
    )
}

fn toml_string_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let _ = show_update_notice_if_available(&cli.command);
    match cli.command {
        Commands::Profile(command) => handle_profile_command(command),
        Commands::UseProfile(selector) => handle_set_active_profile(selector),
        Commands::Current => handle_current_profile(),
        Commands::Doctor(args) => handle_doctor(args),
        Commands::Login(args) => handle_codex_login(args),
        Commands::Logout(selector) => handle_codex_logout(selector),
        Commands::Quota(args) => handle_quota(args),
        Commands::Run(args) => handle_run(args),
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

fn handle_add_profile(args: AddProfileArgs) -> Result<()> {
    validate_profile_name(&args.name)?;

    if args.codex_home.is_some() && (args.copy_from.is_some() || args.copy_current) {
        bail!("--codex-home cannot be combined with --copy-from or --copy-current");
    }

    if args.copy_from.is_some() && args.copy_current {
        bail!("use either --copy-from or --copy-current");
    }

    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;

    if state.profiles.contains_key(&args.name) {
        bail!("profile '{}' already exists", args.name);
    }

    let managed = args.codex_home.is_none();
    let source_home = if args.copy_current {
        Some(default_codex_home()?)
    } else if let Some(path) = args.copy_from {
        Some(absolutize(path)?)
    } else {
        None
    };

    let codex_home = match args.codex_home {
        Some(path) => {
            let home = absolutize(path)?;
            create_codex_home_if_missing(&home)?;
            home
        }
        None => {
            fs::create_dir_all(&paths.managed_profiles_root).with_context(|| {
                format!(
                    "failed to create managed profile root {}",
                    paths.managed_profiles_root.display()
                )
            })?;
            let home = absolutize(paths.managed_profiles_root.join(&args.name))?;
            if let Some(source) = source_home.as_deref() {
                copy_codex_home(source, &home)?;
            } else {
                create_codex_home_if_missing(&home)?;
            }
            home
        }
    };

    if managed {
        prepare_managed_codex_home(&paths, &codex_home)?;
    }

    ensure_path_is_unique(&state, &codex_home)?;

    state.profiles.insert(
        args.name.clone(),
        ProfileEntry {
            codex_home: codex_home.clone(),
            managed,
            email: None,
        },
    );

    if state.active_profile.is_none() || args.activate {
        state.active_profile = Some(args.name.clone());
    }

    state.save(&paths)?;

    let storage_message = if source_home.is_some() {
        "Source copied into managed profile home.".to_string()
    } else if managed {
        "Managed profile home created.".to_string()
    } else {
        "Existing CODEX_HOME registered.".to_string()
    };

    let mut fields = vec![
        (
            "Result".to_string(),
            format!("Added profile '{}'.", args.name),
        ),
        ("Profile".to_string(), args.name.clone()),
        ("CODEX_HOME".to_string(), codex_home.display().to_string()),
        ("Storage".to_string(), storage_message),
    ];
    if state.active_profile.as_deref() == Some(args.name.as_str()) {
        fields.push(("Active".to_string(), args.name.clone()));
    }
    print_panel("Profile Added", &fields);

    Ok(())
}

fn handle_import_current_profile(args: ImportCurrentArgs) -> Result<()> {
    handle_add_profile(AddProfileArgs {
        name: args.name,
        codex_home: None,
        copy_from: None,
        copy_current: true,
        activate: true,
    })
}

fn handle_list_profiles() -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;

    if state.profiles.is_empty() {
        let fields = vec![
            ("Status".to_string(), "No profiles configured.".to_string()),
            (
                "Create".to_string(),
                "prodex profile add <name>".to_string(),
            ),
            (
                "Import".to_string(),
                "prodex profile import-current".to_string(),
            ),
        ];
        print_panel("Profiles", &fields);
        return Ok(());
    }

    let summary_fields = vec![
        ("Count".to_string(), state.profiles.len().to_string()),
        (
            "Active".to_string(),
            state.active_profile.as_deref().unwrap_or("-").to_string(),
        ),
    ];
    print_panel("Profiles", &summary_fields);

    for summary in collect_profile_summaries(&state) {
        let kind = if summary.managed {
            "managed"
        } else {
            "external"
        };

        println!();
        let fields = vec![
            (
                "Current".to_string(),
                if summary.active {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
            ),
            ("Kind".to_string(), kind.to_string()),
            ("Auth".to_string(), summary.auth.label),
            (
                "Email".to_string(),
                summary.email.as_deref().unwrap_or("-").to_string(),
            ),
            ("Path".to_string(), summary.codex_home.display().to_string()),
        ];
        print_panel(&format!("Profile {}", summary.name), &fields);
    }

    Ok(())
}

fn handle_remove_profile(args: RemoveProfileArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;

    let Some(profile) = state.profiles.remove(&args.name) else {
        bail!("profile '{}' does not exist", args.name);
    };

    if args.delete_home {
        if !profile.managed {
            bail!(
                "refusing to delete external path {}",
                profile.codex_home.display()
            );
        }
        if profile.codex_home.exists() {
            fs::remove_dir_all(&profile.codex_home)
                .with_context(|| format!("failed to delete {}", profile.codex_home.display()))?;
        }
    }

    state.last_run_selected_at.remove(&args.name);
    state
        .response_profile_bindings
        .retain(|_, binding| binding.profile_name != args.name);
    state
        .session_profile_bindings
        .retain(|_, binding| binding.profile_name != args.name);

    if state.active_profile.as_deref() == Some(args.name.as_str()) {
        state.active_profile = state.profiles.keys().next().cloned();
    }

    state.save(&paths)?;

    let mut fields = vec![(
        "Result".to_string(),
        format!("Removed profile '{}'.", args.name),
    )];
    fields.push((
        "Deleted home".to_string(),
        if args.delete_home {
            "Yes".to_string()
        } else {
            "No".to_string()
        },
    ));
    fields.push((
        "Active".to_string(),
        state
            .active_profile
            .clone()
            .unwrap_or_else(|| "cleared".to_string()),
    ));
    print_panel("Profile Removed", &fields);

    Ok(())
}

fn handle_set_active_profile(selector: ProfileSelector) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;
    let name = resolve_profile_name(&state, selector.profile.as_deref())?;
    state.active_profile = Some(name.clone());
    state.save(&paths)?;

    let profile = state
        .profiles
        .get(&name)
        .with_context(|| format!("profile '{}' disappeared from state", name))?;

    let fields = vec![
        ("Result".to_string(), format!("Active profile: {name}")),
        (
            "CODEX_HOME".to_string(),
            profile.codex_home.display().to_string(),
        ),
    ];
    print_panel("Active Profile", &fields);
    Ok(())
}

fn handle_current_profile() -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;

    let Some(active) = state.active_profile.as_deref() else {
        let mut fields = vec![("Status".to_string(), "No active profile.".to_string())];
        if state.profiles.len() == 1 {
            if let Some((name, profile)) = state.profiles.iter().next() {
                fields.push(("Only profile".to_string(), name.clone()));
                fields.push((
                    "CODEX_HOME".to_string(),
                    profile.codex_home.display().to_string(),
                ));
            }
        }
        print_panel("Active Profile", &fields);
        return Ok(());
    };

    let profile = state
        .profiles
        .get(active)
        .with_context(|| format!("active profile '{}' is missing", active))?;

    let fields = vec![
        ("Profile".to_string(), active.to_string()),
        (
            "CODEX_HOME".to_string(),
            profile.codex_home.display().to_string(),
        ),
        (
            "Managed".to_string(),
            if profile.managed {
                "Yes".to_string()
            } else {
                "No".to_string()
            },
        ),
        (
            "Email".to_string(),
            profile.email.as_deref().unwrap_or("-").to_string(),
        ),
        (
            "Auth".to_string(),
            read_auth_summary(&profile.codex_home).label,
        ),
    ];
    print_panel("Active Profile", &fields);
    Ok(())
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

fn runtime_doctor_json_value(summary: &RuntimeDoctorSummary) -> serde_json::Value {
    let marker_counts = summary
        .marker_counts
        .iter()
        .map(|(marker, count)| ((*marker).to_string(), serde_json::Value::from(*count)))
        .collect::<serde_json::Map<String, serde_json::Value>>();
    let marker_last_fields = summary
        .marker_last_fields
        .iter()
        .map(|(marker, fields)| {
            let fields = fields
                .iter()
                .map(|(key, value)| (key.clone(), serde_json::Value::from(value.clone())))
                .collect::<serde_json::Map<String, serde_json::Value>>();
            ((*marker).to_string(), serde_json::Value::Object(fields))
        })
        .collect::<serde_json::Map<String, serde_json::Value>>();
    let facet_counts = summary
        .facet_counts
        .iter()
        .map(|(facet, counts)| {
            let counts = counts
                .iter()
                .map(|(value, count)| (value.clone(), serde_json::Value::from(*count)))
                .collect::<serde_json::Map<String, serde_json::Value>>();
            (facet.clone(), serde_json::Value::Object(counts))
        })
        .collect::<serde_json::Map<String, serde_json::Value>>();
    serde_json::json!({
        "log_path": summary.log_path.as_ref().map(|path| path.display().to_string()),
        "pointer_exists": summary.pointer_exists,
        "log_exists": summary.log_exists,
        "line_count": summary.line_count,
        "first_timestamp": summary.first_timestamp,
        "last_timestamp": summary.last_timestamp,
        "marker_counts": marker_counts,
        "marker_last_fields": marker_last_fields,
        "facet_counts": facet_counts,
        "last_marker_line": summary.last_marker_line,
        "diagnosis": summary.diagnosis,
    })
}

fn runtime_doctor_fields() -> Vec<(String, String)> {
    let pointer_path = runtime_proxy_latest_log_pointer_path();
    let summary = collect_runtime_doctor_summary();
    let latest_log = summary
        .log_path
        .as_ref()
        .map(|path| {
            format!(
                "{} ({})",
                path.display(),
                if summary.log_exists {
                    "exists"
                } else {
                    "missing"
                }
            )
        })
        .unwrap_or_else(|| "-".to_string());

    vec![
        (
            "Log pointer".to_string(),
            format!(
                "{} ({})",
                pointer_path.display(),
                if summary.pointer_exists {
                    "exists"
                } else {
                    "missing"
                }
            ),
        ),
        ("Latest log".to_string(), latest_log),
        (
            "Log sample".to_string(),
            format!("{} lines", summary.line_count),
        ),
        (
            "Queue overload".to_string(),
            runtime_doctor_marker_count(&summary, "runtime_proxy_queue_overloaded").to_string(),
        ),
        (
            "Active limit".to_string(),
            runtime_doctor_marker_count(&summary, "runtime_proxy_active_limit_reached").to_string(),
        ),
        (
            "Lane limit".to_string(),
            runtime_doctor_marker_count(&summary, "runtime_proxy_lane_limit_reached").to_string(),
        ),
        (
            "Overload backoff".to_string(),
            runtime_doctor_marker_count(&summary, "runtime_proxy_overload_backoff").to_string(),
        ),
        (
            "Connect failures".to_string(),
            (runtime_doctor_marker_count(&summary, "upstream_connect_timeout")
                + runtime_doctor_marker_count(&summary, "upstream_connect_error"))
            .to_string(),
        ),
        (
            "Pre-commit budget".to_string(),
            runtime_doctor_marker_count(&summary, "precommit_budget_exhausted").to_string(),
        ),
        (
            "Retry backoff".to_string(),
            runtime_doctor_marker_count(&summary, "profile_retry_backoff").to_string(),
        ),
        (
            "Transport backoff".to_string(),
            runtime_doctor_marker_count(&summary, "profile_transport_backoff").to_string(),
        ),
        (
            "Health penalties".to_string(),
            runtime_doctor_marker_count(&summary, "profile_health").to_string(),
        ),
        (
            "Bad pairing".to_string(),
            runtime_doctor_marker_count(&summary, "profile_bad_pairing").to_string(),
        ),
        (
            "Selection picks".to_string(),
            runtime_doctor_marker_count(&summary, "selection_pick").to_string(),
        ),
        (
            "Selection skips".to_string(),
            runtime_doctor_marker_count(&summary, "selection_skip_current").to_string(),
        ),
        (
            "Stream read errors".to_string(),
            runtime_doctor_marker_count(&summary, "stream_read_error").to_string(),
        ),
        (
            "State save errors".to_string(),
            runtime_doctor_marker_count(&summary, "state_save_error").to_string(),
        ),
        (
            "Probe refresh".to_string(),
            runtime_doctor_marker_count(&summary, "profile_probe_refresh_start").to_string(),
        ),
        (
            "Probe refresh errors".to_string(),
            runtime_doctor_marker_count(&summary, "profile_probe_refresh_error").to_string(),
        ),
        (
            "Hot lane".to_string(),
            runtime_doctor_top_facet(&summary, "lane").unwrap_or_else(|| "-".to_string()),
        ),
        (
            "Hot route".to_string(),
            runtime_doctor_top_facet(&summary, "route").unwrap_or_else(|| "-".to_string()),
        ),
        (
            "Hot profile".to_string(),
            runtime_doctor_top_facet(&summary, "profile").unwrap_or_else(|| "-".to_string()),
        ),
        (
            "Last marker".to_string(),
            summary
                .last_marker_line
                .clone()
                .unwrap_or_else(|| "-".to_string()),
        ),
        ("Diagnosis".to_string(), summary.diagnosis),
    ]
}

fn runtime_doctor_marker_count(summary: &RuntimeDoctorSummary, marker: &'static str) -> usize {
    summary.marker_counts.get(marker).copied().unwrap_or(0)
}

fn runtime_doctor_top_facet(summary: &RuntimeDoctorSummary, facet: &str) -> Option<String> {
    summary.facet_counts.get(facet).and_then(|counts| {
        counts
            .iter()
            .max_by_key(|(value, count)| (**count, value.as_str()))
            .map(|(value, count)| format!("{value} ({count})"))
    })
}

fn collect_runtime_doctor_summary() -> RuntimeDoctorSummary {
    let pointer_path = runtime_proxy_latest_log_pointer_path();
    let pointer_content = fs::read_to_string(&pointer_path).ok();
    let log_path = pointer_content
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    let pointer_exists = pointer_path.exists();
    let log_exists = log_path.as_ref().is_some_and(|path| path.exists());

    let mut summary = if let Some(log_path) = log_path.as_ref().filter(|path| path.exists()) {
        match read_runtime_log_tail(log_path, RUNTIME_PROXY_DOCTOR_TAIL_BYTES) {
            Ok(tail) => summarize_runtime_log_tail(&tail),
            Err(err) => RuntimeDoctorSummary {
                diagnosis: format!("Failed to read the latest runtime log tail: {err}"),
                ..RuntimeDoctorSummary::default()
            },
        }
    } else {
        RuntimeDoctorSummary::default()
    };

    summary.pointer_exists = pointer_exists;
    summary.log_exists = log_exists;
    summary.log_path = log_path;
    if summary.diagnosis.is_empty() {
        summary.diagnosis = if !summary.pointer_exists {
            "No runtime log pointer has been created yet.".to_string()
        } else if !summary.log_exists {
            "Latest runtime log path does not exist.".to_string()
        } else if summary.line_count == 0 {
            "Latest runtime log is empty.".to_string()
        } else if runtime_doctor_marker_count(&summary, "runtime_proxy_overload_backoff") > 0 {
            "Recent local proxy overload backoff was triggered.".to_string()
        } else if runtime_doctor_marker_count(&summary, "runtime_proxy_lane_limit_reached") > 0 {
            "Recent per-lane admission limit was triggered.".to_string()
        } else if runtime_doctor_marker_count(&summary, "runtime_proxy_active_limit_reached") > 0 {
            "Recent global active-request admission limit was triggered.".to_string()
        } else if runtime_doctor_marker_count(&summary, "runtime_proxy_queue_overloaded") > 0 {
            "Recent proxy saturation detected before commit.".to_string()
        } else if runtime_doctor_marker_count(&summary, "profile_inflight_saturated") > 0 {
            "Recent per-profile in-flight saturation forced a fail-fast response.".to_string()
        } else if runtime_doctor_marker_count(&summary, "profile_bad_pairing") > 0 {
            "Recent route-specific bad pairing memory is steering fresh selection away from a flaky account.".to_string()
        } else if runtime_doctor_marker_count(&summary, "selection_pick") > 0
            || runtime_doctor_marker_count(&summary, "selection_skip_current") > 0
        {
            "Recent selection decisions were logged; inspect the last marker for why a profile was picked or skipped.".to_string()
        } else if runtime_doctor_marker_count(&summary, "precommit_budget_exhausted") > 0 {
            "Recent candidate selection exhausted before commit.".to_string()
        } else if runtime_doctor_marker_count(&summary, "stream_read_error") > 0 {
            "Recent upstream stream read failure detected after commit.".to_string()
        } else if runtime_doctor_marker_count(&summary, "upstream_connect_timeout") > 0
            || runtime_doctor_marker_count(&summary, "upstream_connect_error") > 0
        {
            "Recent upstream connect failures detected.".to_string()
        } else if runtime_doctor_marker_count(&summary, "state_save_error") > 0 {
            "Recent runtime state save failures detected.".to_string()
        } else if runtime_doctor_marker_count(&summary, "profile_probe_refresh_error") > 0 {
            "Recent background quota refresh failures detected; fresh selection may rely on stale quota snapshots.".to_string()
        } else if runtime_doctor_marker_count(&summary, "profile_probe_refresh_start") > 0 {
            "Background quota refresh activity was detected; inspect the last marker for the most recent profile refresh.".to_string()
        } else if runtime_doctor_marker_count(&summary, "first_upstream_chunk") > 0
            && runtime_doctor_marker_count(&summary, "first_local_chunk") == 0
        {
            "Upstream produced data but the local writer did not emit a first chunk in the sampled tail."
                .to_string()
        } else {
            "No recent overload or stream-failure markers were detected in the sampled runtime tail."
                .to_string()
        };
    }
    summary
}

fn read_runtime_log_tail(path: &Path, max_bytes: usize) -> Result<Vec<u8>> {
    let mut file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let len = file
        .metadata()
        .with_context(|| format!("failed to inspect {}", path.display()))?
        .len();
    let start = len.saturating_sub(max_bytes as u64);
    file.seek(SeekFrom::Start(start))
        .with_context(|| format!("failed to seek {}", path.display()))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .with_context(|| format!("failed to read {}", path.display()))?;
    if start > 0
        && let Some(position) = buffer.iter().position(|byte| *byte == b'\n')
    {
        buffer.drain(..=position);
    }
    Ok(buffer)
}

fn summarize_runtime_log_tail(tail: &[u8]) -> RuntimeDoctorSummary {
    let text = String::from_utf8_lossy(tail);
    let mut summary = RuntimeDoctorSummary::default();
    for line in text.lines() {
        summary.line_count += 1;
        if let Some(timestamp) = runtime_doctor_line_timestamp(line) {
            if summary.first_timestamp.is_none() {
                summary.first_timestamp = Some(timestamp.clone());
            }
            summary.last_timestamp = Some(timestamp);
        }
        if let Some(marker) = runtime_doctor_marker_name(line) {
            *summary.marker_counts.entry(marker).or_insert(0) += 1;
            summary.last_marker_line = Some(runtime_doctor_truncate_line(line, 160));
            let fields = runtime_doctor_parse_fields(line);
            for facet in ["lane", "route", "profile", "reason", "transport"] {
                if let Some(value) = fields.get(facet).cloned() {
                    *summary
                        .facet_counts
                        .entry(facet.to_string())
                        .or_default()
                        .entry(value)
                        .or_insert(0) += 1;
                }
            }
            if !fields.is_empty() {
                summary.marker_last_fields.insert(marker, fields);
            }
        }
    }
    summary
}

fn runtime_doctor_line_timestamp(line: &str) -> Option<String> {
    let end = line.find("] ")?;
    line.strip_prefix('[')
        .and_then(|trimmed| trimmed.get(..end.saturating_sub(1)))
        .map(ToString::to_string)
}

fn runtime_doctor_parse_fields(line: &str) -> BTreeMap<String, String> {
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

fn runtime_doctor_marker_name(line: &str) -> Option<&'static str> {
    [
        "runtime_proxy_queue_overloaded",
        "runtime_proxy_active_limit_reached",
        "runtime_proxy_lane_limit_reached",
        "runtime_proxy_overload_backoff",
        "profile_inflight_saturated",
        "upstream_connect_timeout",
        "upstream_connect_error",
        "precommit_budget_exhausted",
        "profile_retry_backoff",
        "profile_transport_backoff",
        "profile_health",
        "profile_bad_pairing",
        "selection_pick",
        "selection_skip_current",
        "stream_read_error",
        "first_upstream_chunk",
        "first_local_chunk",
        "state_save_error",
        "profile_probe_refresh_start",
        "profile_probe_refresh_error",
    ]
    .into_iter()
    .find(|marker| line.contains(marker))
}

fn runtime_doctor_truncate_line(line: &str, limit: usize) -> String {
    let trimmed = line.trim();
    let count = trimmed.chars().count();
    if count <= limit {
        return trimmed.to_string();
    }
    trimmed
        .chars()
        .take(limit.saturating_sub(1))
        .collect::<String>()
        + "…"
}

fn handle_codex_login(args: CodexPassthroughArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;
    let status = if let Some(profile_name) = args.profile.as_deref() {
        login_into_profile(&paths, &mut state, profile_name, &args.codex_args)?
    } else {
        login_with_auto_profile(&paths, &mut state, &args.codex_args)?
    };
    exit_with_status(status)
}

fn login_into_profile(
    paths: &AppPaths,
    state: &mut AppState,
    profile_name: &str,
    codex_args: &[OsString],
) -> Result<ExitStatus> {
    let profile_name = resolve_profile_name(state, Some(profile_name))?;
    let profile = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    let codex_home = profile.codex_home.clone();
    let managed = profile.managed;

    if managed {
        prepare_managed_codex_home(paths, &codex_home)?;
    } else {
        create_codex_home_if_missing(&codex_home)?;
    }

    let status = run_codex_login(&codex_home, codex_args)?;
    if !status.success() {
        return Ok(status);
    }

    if let Ok(email) = fetch_profile_email(&codex_home) {
        if let Some(profile) = state.profiles.get_mut(&profile_name) {
            profile.email = Some(email);
        }
    }

    let account_email = state
        .profiles
        .get(&profile_name)
        .and_then(|profile| profile.email.clone())
        .unwrap_or_else(|| "-".to_string());
    state.active_profile = Some(profile_name.clone());
    state.save(paths)?;
    let fields = vec![
        (
            "Result".to_string(),
            format!("Logged in successfully for profile '{profile_name}'."),
        ),
        ("Account".to_string(), account_email),
        ("Profile".to_string(), profile_name),
        ("CODEX_HOME".to_string(), codex_home.display().to_string()),
    ];
    print_panel("Login", &fields);
    Ok(status)
}

fn login_with_auto_profile(
    paths: &AppPaths,
    state: &mut AppState,
    codex_args: &[OsString],
) -> Result<ExitStatus> {
    let login_home = create_temporary_login_home(paths)?;
    let status = run_codex_login(&login_home, codex_args)?;
    if !status.success() {
        remove_dir_if_exists(&login_home)?;
        return Ok(status);
    }

    let email = fetch_profile_email(&login_home).with_context(|| {
        format!(
            "failed to resolve the logged-in account email from {}",
            login_home.display()
        )
    })?;

    if let Some(profile_name) = find_profile_by_email(state, &email)? {
        let codex_home = state
            .profiles
            .get(&profile_name)
            .with_context(|| format!("profile '{}' is missing", profile_name))?;
        let managed = codex_home.managed;
        let codex_home = codex_home.codex_home.clone();
        create_codex_home_if_missing(&codex_home)?;
        copy_directory_contents(&login_home, &codex_home)?;
        if managed {
            prepare_managed_codex_home(paths, &codex_home)?;
        }
        if let Some(profile) = state.profiles.get_mut(&profile_name) {
            profile.email = Some(email.clone());
        }
        remove_dir_if_exists(&login_home)?;
        state.active_profile = Some(profile_name.clone());
        state.save(paths)?;

        let fields = vec![
            (
                "Result".to_string(),
                format!("Logged in as {email}. Reusing profile '{profile_name}'."),
            ),
            ("Account".to_string(), email),
            ("Profile".to_string(), profile_name),
            ("CODEX_HOME".to_string(), codex_home.display().to_string()),
        ];
        print_panel("Login", &fields);
        return Ok(status);
    }

    let profile_name = unique_profile_name_for_email(paths, state, &email);
    let codex_home = absolutize(paths.managed_profiles_root.join(&profile_name))?;
    persist_login_home(&login_home, &codex_home)?;
    prepare_managed_codex_home(paths, &codex_home)?;

    state.profiles.insert(
        profile_name.clone(),
        ProfileEntry {
            codex_home: codex_home.clone(),
            managed: true,
            email: Some(email.clone()),
        },
    );
    state.active_profile = Some(profile_name.clone());
    state.save(paths)?;

    let fields = vec![
        (
            "Result".to_string(),
            format!("Logged in as {email}. Created profile '{profile_name}'."),
        ),
        ("Account".to_string(), email),
        ("Profile".to_string(), profile_name),
        ("CODEX_HOME".to_string(), codex_home.display().to_string()),
    ];
    print_panel("Login", &fields);
    Ok(status)
}

fn run_codex_login(codex_home: &Path, codex_args: &[OsString]) -> Result<ExitStatus> {
    let mut command_args = vec![OsString::from("login")];
    command_args.extend(codex_args.iter().cloned());
    run_child(&codex_bin(), &command_args, codex_home)
}

fn create_temporary_login_home(paths: &AppPaths) -> Result<PathBuf> {
    fs::create_dir_all(&paths.managed_profiles_root).with_context(|| {
        format!(
            "failed to create managed profile root {}",
            paths.managed_profiles_root.display()
        )
    })?;

    for attempt in 0..100 {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let candidate = paths
            .managed_profiles_root
            .join(format!(".login-{}-{stamp}-{attempt}", std::process::id()));
        if candidate.exists() {
            continue;
        }
        create_codex_home_if_missing(&candidate)?;
        return Ok(candidate);
    }

    bail!("failed to allocate a temporary CODEX_HOME for login")
}

fn fetch_profile_email(codex_home: &Path) -> Result<String> {
    let auth_email_error = match read_profile_email_from_auth(codex_home) {
        Ok(Some(email)) => return Ok(email),
        Ok(None) => None,
        Err(err) => Some(err),
    };

    match fetch_profile_email_from_usage(codex_home) {
        Ok(email) => Ok(email),
        Err(usage_error) => {
            if let Some(auth_error) = auth_email_error {
                bail!(
                    "failed to read account email from auth.json ({auth_error:#}) and quota endpoint ({usage_error:#})"
                );
            }
            Err(usage_error)
        }
    }
}

fn read_profile_email_from_auth(codex_home: &Path) -> Result<Option<String>> {
    let auth_path = codex_home.join("auth.json");
    if !auth_path.is_file() {
        return Ok(None);
    }

    let content = fs::read_to_string(&auth_path)
        .with_context(|| format!("failed to read {}", auth_path.display()))?;
    let stored_auth: StoredAuth = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", auth_path.display()))?;
    let id_token = stored_auth
        .tokens
        .as_ref()
        .and_then(|tokens| tokens.id_token.as_deref())
        .map(str::trim)
        .filter(|token| !token.is_empty());

    let Some(id_token) = id_token else {
        return Ok(None);
    };

    parse_email_from_id_token(id_token)
        .with_context(|| format!("failed to parse id_token in {}", auth_path.display()))
}

fn parse_email_from_id_token(raw_jwt: &str) -> Result<Option<String>> {
    let mut parts = raw_jwt.split('.');
    let (_header_b64, payload_b64, _sig_b64) = match (parts.next(), parts.next(), parts.next()) {
        (Some(header), Some(payload), Some(signature))
            if !header.is_empty() && !payload.is_empty() && !signature.is_empty() =>
        {
            (header, payload, signature)
        }
        _ => bail!("invalid JWT format"),
    };

    let payload_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload_b64)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(payload_b64))
        .context("failed to decode JWT payload")?;
    let claims: IdTokenClaims =
        serde_json::from_slice(&payload_bytes).context("failed to parse JWT payload JSON")?;

    Ok(claims
        .email
        .or_else(|| claims.profile.and_then(|profile| profile.email))
        .map(|email| email.trim().to_string())
        .filter(|email| !email.is_empty()))
}

fn fetch_profile_email_from_usage(codex_home: &Path) -> Result<String> {
    let usage = fetch_usage(codex_home, None)?;
    let email = usage
        .email
        .as_deref()
        .map(str::trim)
        .filter(|email| !email.is_empty())
        .context("quota endpoint did not return an email")?;
    Ok(email.to_string())
}

fn find_profile_by_email(state: &mut AppState, email: &str) -> Result<Option<String>> {
    let target_email = normalize_email(email);
    let summaries = collect_profile_summaries(state);

    for summary in &summaries {
        if summary
            .email
            .as_deref()
            .is_some_and(|cached| normalize_email(cached) == target_email)
        {
            return Ok(Some(summary.name.clone()));
        }
    }

    let discovered = map_parallel(
        summaries
            .into_iter()
            .filter_map(|summary| {
                if summary.email.is_some() || !summary.auth.quota_compatible {
                    return None;
                }

                Some(ProfileEmailLookupJob {
                    name: summary.name,
                    codex_home: summary.codex_home,
                })
            })
            .collect(),
        |job| (job.name, fetch_profile_email(&job.codex_home).ok()),
    );

    let mut matched_profile = None;
    for (name, fetched_email) in discovered {
        let Some(fetched_email) = fetched_email else {
            continue;
        };

        if matched_profile.is_none() && normalize_email(&fetched_email) == target_email {
            matched_profile = Some(name.clone());
        }
        if let Some(profile) = state.profiles.get_mut(&name) {
            profile.email = Some(fetched_email);
        }
    }

    Ok(matched_profile)
}

fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

fn deserialize_null_default<'de, D, T>(deserializer: D) -> std::result::Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

fn profile_name_from_email(email: &str) -> String {
    let normalized = normalize_email(email);
    let mut profile_name = String::new();

    for ch in normalized.chars() {
        match ch {
            'a'..='z' | '0'..='9' | '.' | '_' | '-' => profile_name.push(ch),
            '@' => profile_name.push('_'),
            _ => profile_name.push('-'),
        }
    }

    let profile_name = profile_name
        .trim_matches(|ch| matches!(ch, '.' | '_' | '-'))
        .to_string();
    if profile_name.is_empty() || profile_name == "." || profile_name == ".." {
        "profile".to_string()
    } else {
        profile_name
    }
}

fn unique_profile_name_for_email(paths: &AppPaths, state: &AppState, email: &str) -> String {
    let base_name = profile_name_from_email(email);
    if is_available_profile_name(paths, state, &base_name) {
        return base_name;
    }

    for suffix in 2.. {
        let candidate = format!("{base_name}-{suffix}");
        if is_available_profile_name(paths, state, &candidate) {
            return candidate;
        }
    }

    unreachable!("integer suffix space should not be exhausted")
}

fn is_available_profile_name(paths: &AppPaths, state: &AppState, candidate: &str) -> bool {
    !state.profiles.contains_key(candidate) && !paths.managed_profiles_root.join(candidate).exists()
}

fn persist_login_home(source: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        bail!(
            "refusing to overwrite existing login destination {}",
            destination.display()
        );
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(_) => {
            copy_codex_home(source, destination)?;
            remove_dir_if_exists(source)
        }
    }
}

fn remove_dir_if_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    fs::remove_dir_all(path).with_context(|| format!("failed to delete {}", path.display()))
}

fn handle_codex_logout(selector: ProfileSelector) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let profile_name = resolve_profile_name(&state, selector.profile.as_deref())?;
    let codex_home = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?
        .codex_home
        .clone();

    let status = run_child(&codex_bin(), &[OsString::from("logout")], &codex_home)?;
    exit_with_status(status)
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

    let runtime_proxy = if should_enable_runtime_rotation_proxy(
        &state,
        &selected_profile_name,
        allow_auto_rotate,
    ) {
        let proxy = start_runtime_rotation_proxy(
            &paths,
            &state,
            &selected_profile_name,
            quota_base_url(args.base_url.as_deref()),
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

fn copy_codex_home(source: &Path, destination: &Path) -> Result<()> {
    if !source.is_dir() {
        bail!("copy source {} is not a directory", source.display());
    }

    if same_path(source, destination) {
        bail!("copy source and destination are the same path");
    }

    if destination.exists() && !dir_is_empty(destination)? {
        bail!(
            "destination {} already exists and is not empty",
            destination.display()
        );
    }

    create_codex_home_if_missing(destination)?;
    copy_directory_contents(source, destination)
}

fn copy_directory_contents(source: &Path, destination: &Path) -> Result<()> {
    for entry in fs::read_dir(source)
        .with_context(|| format!("failed to read directory {}", source.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", source.display()))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to read metadata for {}", source_path.display()))?;

        if file_type.is_dir() {
            create_codex_home_if_missing(&destination_path)?;
            copy_directory_contents(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &destination_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source_path.display(),
                    destination_path.display()
                )
            })?;
        } else if file_type.is_symlink() {
            #[cfg(unix)]
            {
                let target = fs::read_link(&source_path)
                    .with_context(|| format!("failed to read symlink {}", source_path.display()))?;
                std::os::unix::fs::symlink(target, &destination_path).with_context(|| {
                    format!("failed to recreate symlink {}", destination_path.display())
                })?;
            }
            #[cfg(not(unix))]
            {
                bail!("symlinks are not supported on this platform");
            }
        }
    }

    Ok(())
}

fn prepare_managed_codex_home(paths: &AppPaths, codex_home: &Path) -> Result<()> {
    create_codex_home_if_missing(codex_home)?;
    migrate_legacy_shared_codex_root(paths)?;
    fs::create_dir_all(&paths.shared_codex_root)
        .with_context(|| format!("failed to create {}", paths.shared_codex_root.display()))?;

    for entry in shared_codex_entries(paths, codex_home)? {
        ensure_shared_codex_entry(paths, codex_home, &entry)?;
    }

    Ok(())
}

fn migrate_legacy_shared_codex_root(paths: &AppPaths) -> Result<()> {
    if same_path(&paths.shared_codex_root, &paths.legacy_shared_codex_root)
        || !paths.legacy_shared_codex_root.exists()
    {
        return Ok(());
    }

    fs::create_dir_all(&paths.shared_codex_root)
        .with_context(|| format!("failed to create {}", paths.shared_codex_root.display()))?;

    let mut entries = SHARED_CODEX_DIR_NAMES
        .iter()
        .map(|name| SharedCodexEntry {
            name: (*name).to_string(),
            kind: SharedCodexEntryKind::Directory,
        })
        .chain(SHARED_CODEX_FILE_NAMES.iter().map(|name| SharedCodexEntry {
            name: (*name).to_string(),
            kind: SharedCodexEntryKind::File,
        }))
        .collect::<Vec<_>>();

    let mut sqlite_entries = BTreeSet::new();
    collect_shared_codex_sqlite_entries(&paths.legacy_shared_codex_root, &mut sqlite_entries)?;
    for name in sqlite_entries {
        entries.push(SharedCodexEntry {
            name,
            kind: SharedCodexEntryKind::File,
        });
    }

    for entry in entries {
        let legacy_path = paths.legacy_shared_codex_root.join(&entry.name);
        let shared_path = paths.shared_codex_root.join(&entry.name);
        migrate_shared_codex_entry(&legacy_path, &shared_path, entry.kind)?;
    }

    Ok(())
}

fn shared_codex_entries(paths: &AppPaths, codex_home: &Path) -> Result<Vec<SharedCodexEntry>> {
    let mut entries = SHARED_CODEX_DIR_NAMES
        .iter()
        .map(|name| SharedCodexEntry {
            name: (*name).to_string(),
            kind: SharedCodexEntryKind::Directory,
        })
        .chain(SHARED_CODEX_FILE_NAMES.iter().map(|name| SharedCodexEntry {
            name: (*name).to_string(),
            kind: SharedCodexEntryKind::File,
        }))
        .collect::<Vec<_>>();

    let mut sqlite_entries = BTreeSet::new();
    let mut scan_roots = vec![paths.shared_codex_root.clone(), codex_home.to_path_buf()];
    scan_roots.sort();
    scan_roots.dedup();

    for root in scan_roots {
        collect_shared_codex_sqlite_entries(&root, &mut sqlite_entries)?;
    }

    for name in sqlite_entries {
        entries.push(SharedCodexEntry {
            name,
            kind: SharedCodexEntryKind::File,
        });
    }

    Ok(entries)
}

fn collect_shared_codex_sqlite_entries(root: &Path, names: &mut BTreeSet<String>) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(root).with_context(|| format!("failed to read {}", root.display()))? {
        let entry = entry.with_context(|| format!("failed to read entry in {}", root.display()))?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if is_shared_codex_sqlite_name(&file_name) {
            names.insert(file_name.into_owned());
        }
    }

    Ok(())
}

fn is_shared_codex_sqlite_name(file_name: &str) -> bool {
    SHARED_CODEX_SQLITE_PREFIXES
        .iter()
        .any(|prefix| file_name.starts_with(prefix))
        && SHARED_CODEX_SQLITE_SUFFIXES
            .iter()
            .any(|suffix| file_name.ends_with(suffix))
}

fn ensure_shared_codex_entry(
    paths: &AppPaths,
    codex_home: &Path,
    entry: &SharedCodexEntry,
) -> Result<()> {
    let local_path = codex_home.join(&entry.name);
    let shared_path = paths.shared_codex_root.join(&entry.name);
    if let Some(parent) = shared_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    migrate_shared_codex_entry(&local_path, &shared_path, entry.kind)?;

    if entry.kind == SharedCodexEntryKind::Directory && !shared_path.exists() {
        create_codex_home_if_missing(&shared_path)?;
    }

    ensure_symlink_to_shared(&local_path, &shared_path, entry.kind)
}

fn migrate_shared_codex_entry(
    local_path: &Path,
    shared_path: &Path,
    kind: SharedCodexEntryKind,
) -> Result<()> {
    let metadata = match fs::symlink_metadata(local_path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to inspect {}", local_path.display()));
        }
    };

    if metadata.file_type().is_symlink() {
        remove_path(local_path)?;
        return Ok(());
    }

    match kind {
        SharedCodexEntryKind::Directory => {
            if !metadata.is_dir() {
                bail!(
                    "expected {} to be a directory for shared Codex session state",
                    local_path.display()
                );
            }

            if !shared_path.exists() {
                move_directory(local_path, shared_path)?;
                return Ok(());
            }
            if !shared_path.is_dir() {
                bail!(
                    "expected {} to be a directory for shared Codex session state",
                    shared_path.display()
                );
            }

            copy_directory_contents(local_path, shared_path)?;
            fs::remove_dir_all(local_path)
                .with_context(|| format!("failed to remove {}", local_path.display()))?;
        }
        SharedCodexEntryKind::File => {
            if !metadata.is_file() {
                bail!(
                    "expected {} to be a file for shared Codex session state",
                    local_path.display()
                );
            }

            if !shared_path.exists() {
                move_file(local_path, shared_path)?;
                return Ok(());
            }
            if !shared_path.is_file() {
                bail!(
                    "expected {} to be a file for shared Codex session state",
                    shared_path.display()
                );
            }

            if local_path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "history.jsonl")
            {
                append_file_contents(local_path, shared_path)?;
            }

            fs::remove_file(local_path)
                .with_context(|| format!("failed to remove {}", local_path.display()))?;
        }
    }

    Ok(())
}

fn move_directory(source: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(_) => {
            create_codex_home_if_missing(destination)?;
            copy_directory_contents(source, destination)?;
            fs::remove_dir_all(source)
                .with_context(|| format!("failed to remove {}", source.display()))
        }
    }
}

fn move_file(source: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(source, destination).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source.display(),
                    destination.display()
                )
            })?;
            fs::remove_file(source)
                .with_context(|| format!("failed to remove {}", source.display()))
        }
    }
}

fn append_file_contents(source: &Path, destination: &Path) -> Result<()> {
    let content =
        fs::read(source).with_context(|| format!("failed to read {}", source.display()))?;
    if content.is_empty() {
        return Ok(());
    }

    use std::io::Write as _;

    let mut destination_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(destination)
        .with_context(|| format!("failed to open {}", destination.display()))?;

    let destination_len = destination_file
        .metadata()
        .with_context(|| format!("failed to inspect {}", destination.display()))?
        .len();
    if destination_len > 0 {
        destination_file
            .write_all(b"\n")
            .with_context(|| format!("failed to append separator to {}", destination.display()))?;
    }

    destination_file.write_all(&content).with_context(|| {
        format!(
            "failed to append {} to {}",
            source.display(),
            destination.display()
        )
    })
}

fn ensure_symlink_to_shared(
    local_path: &Path,
    shared_path: &Path,
    kind: SharedCodexEntryKind,
) -> Result<()> {
    if local_path.exists() {
        remove_path(local_path)?;
    } else if fs::symlink_metadata(local_path).is_ok() {
        remove_path(local_path)?;
    }

    create_symlink(shared_path, local_path, kind)
}

fn create_symlink(target: &Path, link: &Path, _kind: SharedCodexEntryKind) -> Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).with_context(|| {
            format!(
                "failed to link shared Codex session state {} -> {}",
                link.display(),
                target.display()
            )
        })?;
    }

    #[cfg(windows)]
    {
        match _kind {
            SharedCodexEntryKind::Directory => std::os::windows::fs::symlink_dir(target, link),
            SharedCodexEntryKind::File => std::os::windows::fs::symlink_file(target, link),
        }
        .with_context(|| {
            format!(
                "failed to link shared Codex session state {} -> {}",
                link.display(),
                target.display()
            )
        })?;
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = _kind;
        bail!("shared Codex session links are not supported on this platform");
    }

    Ok(())
}

fn remove_path(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    let file_type = metadata.file_type();

    if file_type.is_symlink() {
        fs::remove_file(path)
            .or_else(|_| fs::remove_dir(path))
            .with_context(|| format!("failed to remove symbolic link {}", path.display()))?;
        return Ok(());
    }

    if metadata.is_dir() {
        fs::remove_dir_all(path).with_context(|| format!("failed to remove {}", path.display()))?;
    } else {
        fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))?;
    }

    Ok(())
}

fn create_codex_home_if_missing(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o700);
        let _ = fs::set_permissions(path, permissions);
    }
    Ok(())
}

fn dir_is_empty(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(true);
    }
    let mut entries =
        fs::read_dir(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(entries.next().is_none())
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
    let persisted_profile_scores =
        load_runtime_profile_scores(paths, &state.profiles).unwrap_or_else(|_| BTreeMap::new());
    let persisted_usage_snapshots =
        load_runtime_usage_snapshots(paths, &state.profiles).unwrap_or_else(|_| BTreeMap::new());
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
            state: state.clone(),
            upstream_base_url: upstream_base_url.clone(),
            include_code_review,
            current_profile: current_profile.to_string(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: state.session_profile_bindings.clone(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: persisted_usage_snapshots,
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: persisted_profile_scores,
        })),
    };
    runtime_proxy_log_to_path(
        &log_path,
        &format!(
            "runtime proxy started listen_addr={listen_addr} current_profile={current_profile} include_code_review={include_code_review} upstream_base_url={upstream_base_url}"
        ),
    );
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
    })
}

impl Drop for RuntimeRotationProxy {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        for _ in 0..self.accept_worker_count {
            self.server.unblock();
        }
        for worker in self.worker_threads.drain(..) {
            let _ = worker.join();
        }
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
    let budget = Duration::from_millis(runtime_proxy_admission_wait_budget_ms());
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
                            "runtime_proxy_admission_wait_exhausted transport={transport} path={path} waited_ms={} reason={}",
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
    let budget = Duration::from_millis(runtime_proxy_long_lived_queue_wait_budget_ms());
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
                            "runtime_proxy_queue_wait_exhausted transport={transport} path={path} waited_ms={} reason=long_lived_queue_full",
                            elapsed.as_millis()
                        ),
                    );
                    return Err((RuntimeProxyQueueRejection::Full, item));
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

fn runtime_request_turn_state(request: &RuntimeProxyRequest) -> Option<String> {
    request.headers.iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("x-codex-turn-state")
            .then(|| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn runtime_request_session_id(request: &RuntimeProxyRequest) -> Option<String> {
    request.headers.iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("session_id")
            .then(|| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn runtime_response_bound_profile(
    shared: &RuntimeRotationProxyShared,
    previous_response_id: &str,
) -> Result<Option<String>> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    Ok(runtime
        .state
        .response_profile_bindings
        .get(previous_response_id)
        .map(|binding| binding.profile_name.clone())
        .filter(|profile_name| runtime.state.profiles.contains_key(profile_name)))
}

fn runtime_turn_state_bound_profile(
    shared: &RuntimeRotationProxyShared,
    turn_state: &str,
) -> Result<Option<String>> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    Ok(runtime
        .turn_state_bindings
        .get(turn_state)
        .map(|binding| binding.profile_name.clone())
        .filter(|profile_name| runtime.state.profiles.contains_key(profile_name)))
}

fn runtime_session_bound_profile(
    shared: &RuntimeRotationProxyShared,
    session_id: &str,
) -> Result<Option<String>> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    Ok(runtime
        .session_id_bindings
        .get(session_id)
        .map(|binding| binding.profile_name.clone())
        .filter(|profile_name| runtime.state.profiles.contains_key(profile_name)))
}

fn remember_runtime_turn_state(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state: Option<&str>,
) -> Result<()> {
    let Some(turn_state) = turn_state.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let bound_at = Local::now().timestamp();
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
        prune_profile_bindings(
            &mut runtime.turn_state_bindings,
            TURN_STATE_PROFILE_BINDING_LIMIT,
        );
        runtime_proxy_log(
            shared,
            format!("binding turn_state profile={profile_name} value={turn_state}"),
        );
    }
    Ok(())
}

fn remember_runtime_session_id(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    session_id: Option<&str>,
) -> Result<()> {
    let Some(session_id) = session_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let bound_at = Local::now().timestamp();
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
        prune_profile_bindings(
            &mut runtime.session_id_bindings,
            SESSION_ID_PROFILE_BINDING_LIMIT,
        );
        prune_profile_bindings(
            &mut runtime.state.session_profile_bindings,
            SESSION_ID_PROFILE_BINDING_LIMIT,
        );
        let state_snapshot = runtime.state.clone();
        let profile_scores_snapshot = runtime.profile_health.clone();
        let usage_snapshots = runtime.profile_usage_snapshots.clone();
        let paths_snapshot = runtime.paths.clone();
        drop(runtime);
        schedule_runtime_state_save(
            shared,
            state_snapshot,
            profile_scores_snapshot,
            usage_snapshots,
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

fn remember_runtime_response_ids(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    response_ids: &[String],
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
    }
    if changed {
        prune_profile_bindings(
            &mut runtime.state.response_profile_bindings,
            RESPONSE_PROFILE_BINDING_LIMIT,
        );
        let state_snapshot = runtime.state.clone();
        let profile_scores_snapshot = runtime.profile_health.clone();
        let usage_snapshots = runtime.profile_usage_snapshots.clone();
        let paths_snapshot = runtime.paths.clone();
        drop(runtime);
        schedule_runtime_state_save(
            shared,
            state_snapshot,
            profile_scores_snapshot,
            usage_snapshots,
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
        changed = true;
    }

    if let Some(turn_state) = turn_state
        && runtime
            .turn_state_bindings
            .get(turn_state)
            .is_some_and(|binding| binding.profile_name == profile_name)
    {
        runtime.turn_state_bindings.remove(turn_state);
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
        changed = true;
    }

    if changed {
        let state_snapshot = runtime.state.clone();
        let profile_scores_snapshot = runtime.profile_health.clone();
        let usage_snapshots = runtime.profile_usage_snapshots.clone();
        let paths_snapshot = runtime.paths.clone();
        drop(runtime);
        schedule_runtime_state_save(
            shared,
            state_snapshot,
            profile_scores_snapshot,
            usage_snapshots,
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
) -> bool {
    let (attempt_limit, budget_ms) = if continuation {
        (
            RUNTIME_PROXY_PRECOMMIT_CONTINUATION_ATTEMPT_LIMIT,
            RUNTIME_PROXY_PRECOMMIT_CONTINUATION_BUDGET_MS,
        )
    } else {
        (
            RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT,
            RUNTIME_PROXY_PRECOMMIT_BUDGET_MS,
        )
    };

    attempts >= attempt_limit || started_at.elapsed() >= Duration::from_millis(budget_ms)
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
    pinned_profile: Option<&str>,
    turn_state_profile: Option<&str>,
    session_profile: Option<&str>,
    discover_previous_response_owner: bool,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
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
        return next_runtime_previous_response_candidate(shared, excluded_profiles, route_kind);
    }

    if let Some(profile_name) = session_profile.filter(|name| !excluded_profiles.contains(*name)) {
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
        if quota_summary.route_band <= RuntimeQuotaPressureBand::Critical {
            return Ok(Some(profile_name.to_string()));
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=session profile={} reason={} quota_source={} {}",
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
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let (state, current_profile) = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        (runtime.state.clone(), runtime.current_profile.clone())
    };

    for name in active_profile_selection_order(&state, &current_profile) {
        if excluded_profiles.contains(&name) {
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
    let (current_profile, codex_home, in_selection_backoff, inflight_count, health_score) = {
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
            runtime_profile_inflight_count(&runtime, &runtime.current_profile),
            runtime_profile_health_score(&runtime, &runtime.current_profile, now, route_kind),
        )
    };
    let (quota_summary, quota_source) =
        runtime_profile_quota_summary_for_route(shared, &current_profile, route_kind)?;

    if in_selection_backoff
        || health_score > 0
        || inflight_count >= RUNTIME_PROFILE_INFLIGHT_SOFT_LIMIT
        || quota_summary.route_band > RuntimeQuotaPressureBand::Healthy
    {
        let reason = if in_selection_backoff {
            "selection_backoff"
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
                "selection_skip_current route={} profile={} reason={} inflight={} health={} soft_limit={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                current_profile,
                reason,
                inflight_count,
                health_score,
                RUNTIME_PROFILE_INFLIGHT_SOFT_LIMIT,
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
    let previous_response_id = runtime_request_previous_response_id_from_text(request_text);
    let request_turn_state = runtime_request_turn_state(handshake_request);
    let request_session_id = runtime_request_session_id(handshake_request);
    let mut bound_profile = previous_response_id
        .as_deref()
        .map(|response_id| runtime_response_bound_profile(shared, response_id))
        .transpose()?
        .flatten();
    let mut turn_state_profile = request_turn_state
        .as_deref()
        .map(|value| runtime_turn_state_bound_profile(shared, value))
        .transpose()?
        .flatten();
    let mut session_profile = if bound_profile.is_none() && turn_state_profile.is_none() {
        websocket_session.profile_name.clone().or(request_session_id
            .as_deref()
            .map(|session_id| runtime_session_bound_profile(shared, session_id))
            .transpose()?
            .flatten())
    } else {
        None
    };
    let mut pinned_profile = bound_profile.clone().or(session_profile.clone());
    let mut excluded_profiles = BTreeSet::new();
    let mut last_failure = None;
    let mut previous_response_retry_candidate: Option<String> = None;
    let mut previous_response_retry_index = 0usize;
    let mut candidate_turn_state_retry_profile: Option<String> = None;
    let mut candidate_turn_state_retry_value: Option<String> = None;
    let mut saw_inflight_saturation = false;
    let selection_started_at = Instant::now();
    let mut selection_attempts = 0usize;

    loop {
        if runtime_proxy_precommit_budget_exhausted(
            selection_started_at,
            selection_attempts,
            previous_response_id.is_some(),
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} websocket_session={session_id} precommit_budget_exhausted attempts={selection_attempts} elapsed_ms={}",
                    selection_started_at.elapsed().as_millis()
                ),
            );
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
                        handshake_request,
                        request_text,
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
                        RuntimeWebsocketAttempt::PreviousResponseNotFound { payload, .. } => {
                            forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                            return Ok(());
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
            pinned_profile.as_deref(),
            turn_state_profile.as_deref(),
            session_profile.as_deref(),
            previous_response_id.is_some(),
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
                        handshake_request,
                        request_text,
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
                        RuntimeWebsocketAttempt::PreviousResponseNotFound { payload, .. } => {
                            forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                            return Ok(());
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
        if previous_response_id.is_none()
            && pinned_profile.is_none()
            && turn_state_profile.is_none()
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
            handshake_request,
            request_text,
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
            RuntimeWebsocketAttempt::PreviousResponseNotFound {
                profile_name,
                payload,
                turn_state,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?}",
                        turn_state
                    ),
                );
                if previous_response_retry_candidate.as_deref() != Some(profile_name.as_str()) {
                    previous_response_retry_candidate = Some(profile_name.clone());
                    previous_response_retry_index = 0;
                }
                if turn_state.is_some() {
                    candidate_turn_state_retry_profile = Some(profile_name.clone());
                    candidate_turn_state_retry_value = turn_state;
                }
                if let Some(delay) =
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
    let request_session_id = runtime_request_session_id(handshake_request);
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
    let (mut upstream_socket, mut upstream_turn_state, mut inflight_guard) =
        if reuse_existing_session {
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
        return Err(transport_error);
    }

    let mut committed = false;
    loop {
        match upstream_socket.read() {
            Ok(WsMessage::Text(text)) => {
                if let Some(turn_state) = extract_runtime_turn_state_from_payload(text.as_ref()) {
                    remember_runtime_turn_state(shared, profile_name, Some(turn_state.as_str()))?;
                    upstream_turn_state = Some(turn_state);
                }
                match runtime_proxy_websocket_retry_action(text.as_ref()) {
                    Some(RuntimeWebsocketAttempt::QuotaBlocked { .. }) if !committed => {
                        let _ = upstream_socket.close(None);
                        websocket_session.reset();
                        return Ok(RuntimeWebsocketAttempt::QuotaBlocked {
                            profile_name: profile_name.to_string(),
                            payload: RuntimeWebsocketErrorPayload::Text(text.to_string()),
                        });
                    }
                    Some(RuntimeWebsocketAttempt::PreviousResponseNotFound { .. })
                        if !committed =>
                    {
                        let _ = upstream_socket.close(None);
                        websocket_session.reset();
                        return Ok(RuntimeWebsocketAttempt::PreviousResponseNotFound {
                            profile_name: profile_name.to_string(),
                            payload: RuntimeWebsocketErrorPayload::Text(text.to_string()),
                            turn_state: upstream_turn_state.clone(),
                        });
                    }
                    _ => {}
                }

                if !committed {
                    remember_runtime_session_id(
                        shared,
                        profile_name,
                        request_session_id.as_deref(),
                    )?;
                    remember_runtime_turn_state(
                        shared,
                        profile_name,
                        upstream_turn_state.as_deref(),
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
                }
                remember_runtime_response_ids(
                    shared,
                    profile_name,
                    &extract_runtime_response_ids_from_payload(text.as_ref()),
                )?;
                local_socket
                    .send(WsMessage::Text(text.clone()))
                    .with_context(|| {
                        websocket_session.reset();
                        "failed to forward runtime websocket text frame"
                    })?;
                if is_runtime_terminal_event(text.as_ref()) {
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
                if !committed {
                    remember_runtime_session_id(
                        shared,
                        profile_name,
                        request_session_id.as_deref(),
                    )?;
                    remember_runtime_turn_state(
                        shared,
                        profile_name,
                        upstream_turn_state.as_deref(),
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
                }
                local_socket
                    .send(WsMessage::Binary(payload))
                    .with_context(|| {
                        websocket_session.reset();
                        "failed to forward runtime websocket binary frame"
                    })?;
            }
            Ok(WsMessage::Ping(payload)) => {
                upstream_socket
                    .send(WsMessage::Pong(payload))
                    .context("failed to respond to upstream websocket ping")?;
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
            Ok(WsMessage::Close(frame)) => {
                websocket_session.reset();
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
                return Err(transport_error);
            }
            Err(WsError::ConnectionClosed) | Err(WsError::AlreadyClosed) => {
                websocket_session.reset();
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
                return Err(transport_error);
            }
            Err(err) => {
                websocket_session.reset();
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
    let io_timeout = Duration::from_millis(runtime_proxy_stream_idle_timeout_ms());
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

fn is_runtime_terminal_event(payload: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .and_then(|value| {
            value
                .get("type")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .is_some_and(|kind| matches!(kind.as_str(), "response.completed" | "response.failed"))
}

fn proxy_runtime_standard_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<tiny_http::ResponseBox> {
    let request_session_id = runtime_request_session_id(request);
    let session_profile = request_session_id
        .as_deref()
        .map(|session_id| runtime_session_bound_profile(shared, session_id))
        .transpose()?
        .flatten();
    if !is_runtime_compact_path(&request.path_and_query) {
        let profile_name = session_profile.unwrap_or(runtime_proxy_current_profile(shared)?);
        return proxy_runtime_standard_request_for_profile(
            request_id,
            request,
            shared,
            &profile_name,
        );
    }

    let current_profile = runtime_proxy_current_profile(shared)?;
    let mut excluded_profiles = BTreeSet::new();
    let mut conservative_overload_retried_profiles = BTreeSet::new();
    let mut last_failure = None;
    let mut saw_inflight_saturation = false;
    let selection_started_at = Instant::now();
    let mut selection_attempts = 0usize;

    loop {
        if runtime_proxy_precommit_budget_exhausted(selection_started_at, selection_attempts, false)
        {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http compact_precommit_budget_exhausted attempts={selection_attempts} elapsed_ms={}",
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
                None => {
                    let fallback_profile = session_profile
                        .clone()
                        .unwrap_or_else(|| current_profile.clone());
                    proxy_runtime_standard_request_for_profile(
                        request_id,
                        request,
                        shared,
                        &fallback_profile,
                    )
                }
            };
        }

        let Some(candidate_name) = select_runtime_response_candidate_for_route(
            shared,
            &excluded_profiles,
            None,
            None,
            session_profile.as_deref(),
            false,
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
                None => {
                    let fallback_profile = session_profile
                        .clone()
                        .unwrap_or_else(|| current_profile.clone());
                    proxy_runtime_standard_request_for_profile(
                        request_id,
                        request,
                        shared,
                        &fallback_profile,
                    )
                }
            };
        };
        selection_attempts = selection_attempts.saturating_add(1);

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
                    "request={request_id} transport=http profile_inflight_saturated profile={candidate_name} hard_limit={RUNTIME_PROFILE_INFLIGHT_HARD_LIMIT}"
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
        }
    }
}

fn proxy_runtime_standard_request_for_profile(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<tiny_http::ResponseBox> {
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
        remember_runtime_session_id(shared, profile_name, request_session_id.as_deref())?;
        return Ok(RuntimeStandardAttempt::Success {
            profile_name: profile_name.to_string(),
            response: forward_runtime_proxy_response(shared, response, Vec::new()).map_err(
                |err| {
                    note_runtime_profile_transport_failure(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Compact,
                        "compact_forward_response",
                        &err,
                    );
                    err
                },
            )?,
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
    let previous_response_id = runtime_request_previous_response_id(request);
    let request_turn_state = runtime_request_turn_state(request);
    let request_session_id = runtime_request_session_id(request);
    let mut bound_profile = previous_response_id
        .as_deref()
        .map(|response_id| runtime_response_bound_profile(shared, response_id))
        .transpose()?
        .flatten();
    let mut turn_state_profile = request_turn_state
        .as_deref()
        .map(|value| runtime_turn_state_bound_profile(shared, value))
        .transpose()?
        .flatten();
    let mut session_profile = if bound_profile.is_none() && turn_state_profile.is_none() {
        request_session_id
            .as_deref()
            .map(|session_id| runtime_session_bound_profile(shared, session_id))
            .transpose()?
            .flatten()
    } else {
        None
    };
    let mut pinned_profile = bound_profile.clone();
    let mut excluded_profiles = BTreeSet::new();
    let mut last_failure = None;
    let mut previous_response_retry_candidate: Option<String> = None;
    let mut previous_response_retry_index = 0usize;
    let mut candidate_turn_state_retry_profile: Option<String> = None;
    let mut candidate_turn_state_retry_value: Option<String> = None;
    let mut saw_inflight_saturation = false;
    let selection_started_at = Instant::now();
    let mut selection_attempts = 0usize;

    loop {
        if runtime_proxy_precommit_budget_exhausted(
            selection_started_at,
            selection_attempts,
            previous_response_id.is_some(),
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http precommit_budget_exhausted attempts={selection_attempts} elapsed_ms={}",
                    selection_started_at.elapsed().as_millis()
                ),
            );
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
                        request,
                        shared,
                        &current_profile,
                        request_turn_state.as_deref(),
                    )? {
                        RuntimeResponsesAttempt::Success {
                            profile_name,
                            response,
                        } => {
                            commit_runtime_proxy_profile_selection_with_notice(
                                shared,
                                &profile_name,
                                RuntimeRouteKind::Responses,
                            )?;
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
                        RuntimeResponsesAttempt::PreviousResponseNotFound { response, .. } => {
                            return Ok(response);
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
            pinned_profile.as_deref(),
            turn_state_profile.as_deref(),
            session_profile.as_deref(),
            previous_response_id.is_some(),
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
                        request,
                        shared,
                        &current_profile,
                        request_turn_state.as_deref(),
                    )? {
                        RuntimeResponsesAttempt::Success {
                            profile_name,
                            response,
                        } => {
                            commit_runtime_proxy_profile_selection_with_notice(
                                shared,
                                &profile_name,
                                RuntimeRouteKind::Responses,
                            )?;
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
                        RuntimeResponsesAttempt::PreviousResponseNotFound { response, .. } => {
                            return Ok(response);
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
            request,
            shared,
            &candidate_name,
            turn_state_override,
        )? {
            RuntimeResponsesAttempt::Success {
                profile_name,
                response,
            } => {
                commit_runtime_proxy_profile_selection_with_notice(
                    shared,
                    &profile_name,
                    RuntimeRouteKind::Responses,
                )?;
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
            RuntimeResponsesAttempt::PreviousResponseNotFound {
                profile_name,
                response,
                turn_state,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?}",
                        turn_state
                    ),
                );
                if previous_response_retry_candidate.as_deref() != Some(profile_name.as_str()) {
                    previous_response_retry_candidate = Some(profile_name.clone());
                    previous_response_retry_index = 0;
                }
                if turn_state.is_some() {
                    candidate_turn_state_retry_profile = Some(profile_name.clone());
                    candidate_turn_state_retry_value = turn_state;
                }
                if let Some(delay) =
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
        request_session_id.as_deref(),
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
        let profile_scores_snapshot = runtime.profile_health.clone();
        let usage_snapshots = runtime.profile_usage_snapshots.clone();
        let paths_snapshot = runtime.paths.clone();
        drop(runtime);
        if usage_snapshot_changed {
            schedule_runtime_state_save(
                shared,
                state_snapshot,
                profile_scores_snapshot,
                usage_snapshots,
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

    if let Some((index, candidate)) = available_candidates
        .iter()
        .filter(|(_, candidate)| {
            !runtime_profile_name_in_selection_backoff(
                &candidate.name,
                &retry_backoff_until,
                &transport_backoff_until,
                now,
            )
        })
        .min_by_key(|(index, candidate)| {
            (
                runtime_quota_pressure_sort_key_for_route(&candidate.usage, route_kind),
                runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
                runtime_profile_health_sort_key(&candidate.name, &profile_health, now, route_kind),
                *index,
                runtime_profile_selection_jitter(shared, &candidate.name, route_kind),
            )
        })
    {
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

    let fallback = available_candidates
        .into_iter()
        .min_by_key(|(index, candidate)| {
            (
                runtime_profile_backoff_sort_key(
                    &candidate.name,
                    &retry_backoff_until,
                    &transport_backoff_until,
                    now,
                ),
                runtime_quota_pressure_sort_key_for_route(&candidate.usage, route_kind),
                runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
                runtime_profile_health_sort_key(&candidate.name, &profile_health, now, route_kind),
                *index,
                runtime_profile_selection_jitter(shared, &candidate.name, route_kind),
            )
        })
        .map(|(index, candidate)| {
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
                        now,
                    ),
                    index,
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            candidate.name
        });

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
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let auth = runtime
        .state
        .profiles
        .get(profile_name)
        .map(|profile| read_auth_summary(&profile.codex_home))
        .unwrap_or(AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        });
    runtime.profile_probe_cache.insert(
        profile_name.to_string(),
        RuntimeProfileProbeCacheEntry {
            checked_at: Local::now().timestamp(),
            auth,
            result: Ok(usage.clone()),
        },
    );
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
        runtime
            .turn_state_bindings
            .retain(|_, binding| binding.profile_name != profile_name);
        runtime
            .session_id_bindings
            .retain(|_, binding| binding.profile_name != profile_name);
        runtime_proxy_log(
            shared,
            format!(
                "quota_release_profile_affinity profile={profile_name} reason=usage_snapshot_exhausted {}",
                runtime_quota_summary_log_fields(quota_summary)
            ),
        );
    }
    let state_snapshot = runtime.state.clone();
    let profile_scores_snapshot = runtime.profile_health.clone();
    let usage_snapshots = runtime.profile_usage_snapshots.clone();
    let paths_snapshot = runtime.paths.clone();
    drop(runtime);
    schedule_runtime_state_save(
        shared,
        state_snapshot,
        profile_scores_snapshot,
        usage_snapshots,
        paths_snapshot,
        &format!("usage_snapshot:{profile_name}"),
    );
    Ok(())
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

fn prune_runtime_profile_selection_backoff(runtime: &mut RuntimeRotationState, now: i64) {
    prune_runtime_profile_retry_backoff(runtime, now);
    prune_runtime_profile_transport_backoff(runtime, now);
}

fn runtime_profile_name_in_selection_backoff(
    profile_name: &str,
    retry_backoff_until: &BTreeMap<String, i64>,
    transport_backoff_until: &BTreeMap<String, i64>,
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
}

fn runtime_profile_backoff_sort_key(
    profile_name: &str,
    retry_backoff_until: &BTreeMap<String, i64>,
    transport_backoff_until: &BTreeMap<String, i64>,
    now: i64,
) -> (usize, i64, i64) {
    let retry_until = retry_backoff_until
        .get(profile_name)
        .copied()
        .filter(|until| *until > now);
    let transport_until = transport_backoff_until
        .get(profile_name)
        .copied()
        .filter(|until| *until > now);

    match (transport_until, retry_until) {
        (None, None) => (0, 0, 0),
        (Some(transport_until), None) => (1, transport_until, 0),
        (None, Some(retry_until)) => (2, retry_until, 0),
        (Some(transport_until), Some(retry_until)) => (
            3,
            transport_until.min(retry_until),
            transport_until.max(retry_until),
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
    let profile_scores_snapshot = runtime.profile_health.clone();
    let usage_snapshots = runtime.profile_usage_snapshots.clone();
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
        profile_scores_snapshot,
        usage_snapshots,
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
    let state_snapshot = runtime.state.clone();
    let profile_scores_snapshot = runtime.profile_health.clone();
    let usage_snapshots = runtime.profile_usage_snapshots.clone();
    let paths_snapshot = runtime.paths.clone();
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "profile_health profile={profile_name} route={} score={next_score} delta={delta} reason={reason}",
            runtime_route_kind_label(route_kind)
        ),
    );
    schedule_runtime_state_save(
        shared,
        state_snapshot,
        profile_scores_snapshot,
        usage_snapshots,
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
    runtime.profile_retry_backoff_until.insert(
        profile_name.to_string(),
        now.saturating_add(RUNTIME_PROFILE_RETRY_BACKOFF_SECONDS),
    );
    runtime_proxy_log(
        shared,
        format!(
            "profile_retry_backoff profile={profile_name} until={}",
            now.saturating_add(RUNTIME_PROFILE_RETRY_BACKOFF_SECONDS)
        ),
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
    runtime_proxy_log(
        shared,
        format!(
            "profile_transport_backoff profile={profile_name} until={until} seconds={next_backoff_seconds} context={context}"
        ),
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
    let switched = runtime.current_profile != profile_name;
    let now = Local::now().timestamp();
    let cleared_retry_backoff = runtime
        .profile_retry_backoff_until
        .remove(profile_name)
        .is_some();
    let cleared_transport_backoff = runtime
        .profile_transport_backoff_until
        .remove(profile_name)
        .is_some();
    let cleared_health =
        clear_runtime_profile_health_for_route(&mut runtime, profile_name, route_kind, now);
    runtime.current_profile = profile_name.to_string();
    let state_changed = runtime.state.active_profile.as_deref() != Some(profile_name);
    runtime.state.active_profile = Some(profile_name.to_string());
    record_run_selection(&mut runtime.state, profile_name);
    let state_snapshot = runtime.state.clone();
    let profile_scores_snapshot = runtime.profile_health.clone();
    let usage_snapshots = runtime.profile_usage_snapshots.clone();
    let paths_snapshot = runtime.paths.clone();
    drop(runtime);
    let should_persist = switched
        || state_changed
        || cleared_retry_backoff
        || cleared_transport_backoff
        || cleared_health;
    if should_persist {
        schedule_runtime_state_save(
            shared,
            state_snapshot,
            profile_scores_snapshot,
            usage_snapshots,
            paths_snapshot,
            &format!("profile_commit:{profile_name}"),
        );
    }
    runtime_proxy_log(
        shared,
        format!(
            "profile_commit profile={profile_name} route={} switched={switched} persisted={should_persist}",
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
    Ok(response)
}

fn send_runtime_proxy_upstream_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
) -> Result<reqwest::Response> {
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
    request_session_id: Option<&str>,
    response: reqwest::Response,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    inflight_guard: RuntimeProfileInFlightGuard,
) -> Result<RuntimeResponsesAttempt> {
    let turn_state = runtime_proxy_header_value(response.headers(), "x-codex-turn-state");
    remember_runtime_session_id(shared, profile_name, request_session_id)?;
    remember_runtime_turn_state(shared, profile_name, turn_state.as_deref())?;
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
                    _inflight_guard: Some(inflight_guard),
                }),
            });
        }
        RuntimeSseInspection::PreviousResponseNotFound(prelude) => {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http sse_previous_response_not_found profile={profile_name} prelude_bytes={}",
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
                    _inflight_guard: Some(inflight_guard),
                }),
                turn_state,
            });
        }
    };
    remember_runtime_response_ids(shared, profile_name, &response_ids)?;

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
                self.remember_response_ids(shared, profile_name);
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
        self.remember_response_ids(shared, profile_name);
    }

    fn remember_response_ids(&mut self, shared: &RuntimeRotationProxyShared, profile_name: &str) {
        let fresh_ids = extract_runtime_response_ids_from_sse(&self.data_lines)
            .into_iter()
            .filter(|response_id| self.remembered_response_ids.insert(response_id.clone()))
            .collect::<Vec<_>>();
        if fresh_ids.is_empty() {
            return;
        }
        let _ = remember_runtime_response_ids(shared, profile_name, &fresh_ids);
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
    )?;
    for (name, value) in response.headers {
        write!(writer, "{name}: {value}\r\n")?;
    }
    writer.write_all(b"\r\n")?;
    writer.flush()?;

    let mut buffer = [0_u8; 8192];
    let mut total_bytes = 0usize;
    let mut chunk_count = 0usize;
    let started_at = Instant::now();
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
                return Err(err);
            }
        };
        if read == 0 {
            break;
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
        }
        write!(writer, "{:X}\r\n", read)?;
        writer.write_all(&buffer[..read])?;
        writer.write_all(b"\r\n")?;
        writer.flush()?;
    }
    writer.write_all(b"0\r\n\r\n")?;
    writer.flush()?;
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
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| extract_runtime_proxy_quota_message_from_value(&value))
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
    let direct_error = value.get("error");
    let response_error = value
        .get("response")
        .and_then(|response| response.get("error"));
    for error in [direct_error, response_error].into_iter().flatten() {
        let message = error
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("Upstream Codex account quota was exhausted.");
        let code = error.get("code").and_then(serde_json::Value::as_str);
        let code_matches = matches!(code, Some("insufficient_quota" | "rate_limit_exceeded"));
        let message_matches = runtime_proxy_usage_limit_message(message);
        if !(code_matches || message_matches) {
            continue;
        }
        return Some(message.to_string());
    }
    None
}

fn runtime_proxy_usage_limit_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("you've hit your usage limit")
        || lower.contains("you have hit your usage limit")
        || lower.contains("usage limit")
            && (lower.contains("try again at")
                || lower.contains("request to your admin")
                || lower.contains("more access now"))
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

fn extract_runtime_turn_state_from_payload(payload: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .and_then(|value| extract_runtime_turn_state_from_value(&value))
}

fn extract_runtime_response_ids_from_value(value: &serde_json::Value) -> Vec<String> {
    value
        .get("response")
        .and_then(|response| response.get("id"))
        .and_then(serde_json::Value::as_str)
        .map(|id| vec![id.to_string()])
        .unwrap_or_default()
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

fn collect_quota_reports(state: &AppState, base_url: Option<&str>) -> Vec<QuotaReport> {
    let jobs = state
        .profiles
        .iter()
        .map(|(name, profile)| QuotaFetchJob {
            name: name.clone(),
            active: state.active_profile.as_deref() == Some(name.as_str()),
            codex_home: profile.codex_home.clone(),
        })
        .collect();
    let base_url = base_url.map(str::to_owned);

    map_parallel(jobs, |job| QuotaReport {
        name: job.name,
        active: job.active,
        auth: read_auth_summary(&job.codex_home),
        result: fetch_usage(&job.codex_home, base_url.as_deref()).map_err(|err| err.to_string()),
    })
}

fn collect_profile_summaries(state: &AppState) -> Vec<ProfileSummaryReport> {
    let jobs = state
        .profiles
        .iter()
        .map(|(name, profile)| ProfileSummaryJob {
            name: name.clone(),
            active: state.active_profile.as_deref() == Some(name.as_str()),
            managed: profile.managed,
            email: profile.email.clone(),
            codex_home: profile.codex_home.clone(),
        })
        .collect();

    map_parallel(jobs, |job| ProfileSummaryReport {
        name: job.name,
        active: job.active,
        managed: job.managed,
        auth: read_auth_summary(&job.codex_home),
        email: job.email,
        codex_home: job.codex_home,
    })
}

fn collect_doctor_profile_reports(
    state: &AppState,
    include_quota: bool,
) -> Vec<DoctorProfileReport> {
    map_parallel(collect_profile_summaries(state), |summary| {
        DoctorProfileReport {
            quota: include_quota
                .then(|| fetch_usage(&summary.codex_home, None).map_err(|err| err.to_string())),
            summary,
        }
    })
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

fn fetch_usage(codex_home: &Path, base_url: Option<&str>) -> Result<UsageResponse> {
    let usage: UsageResponse = serde_json::from_value(fetch_usage_json(codex_home, base_url)?)
        .with_context(|| {
            format!(
                "invalid JSON returned by quota backend for {}",
                codex_home.display()
            )
        })?;
    Ok(usage)
}

fn fetch_usage_json(codex_home: &Path, base_url: Option<&str>) -> Result<serde_json::Value> {
    let auth = read_usage_auth(codex_home)?;
    let usage_url = usage_url(&quota_base_url(base_url));
    let client = Client::builder()
        .connect_timeout(Duration::from_millis(QUOTA_HTTP_CONNECT_TIMEOUT_MS))
        .timeout(Duration::from_millis(QUOTA_HTTP_READ_TIMEOUT_MS))
        .build()
        .context("failed to build quota HTTP client")?;

    let mut request = client
        .get(&usage_url)
        .header("Authorization", format!("Bearer {}", auth.access_token))
        .header("User-Agent", "codex-cli");

    if let Some(account_id) = auth.account_id.as_deref() {
        request = request.header("ChatGPT-Account-Id", account_id);
    }

    let response = request
        .send()
        .with_context(|| format!("failed to request quota endpoint {}", usage_url))?;
    let status = response.status();
    let body = response
        .bytes()
        .context("failed to read quota response body")?;

    if !status.is_success() {
        let body_text = format_response_body(&body);
        if body_text.is_empty() {
            bail!("request failed (HTTP {}) to {}", status.as_u16(), usage_url);
        }
        bail!(
            "request failed (HTTP {}) to {}: {}",
            status.as_u16(),
            usage_url,
            body_text
        );
    }

    let usage = serde_json::from_slice(&body).with_context(|| {
        format!(
            "invalid JSON returned by quota backend for {}",
            codex_home.display()
        )
    })?;

    Ok(usage)
}

fn print_quota_reports(reports: &[QuotaReport], detail: bool) {
    print!("{}", render_quota_reports(reports, detail));
}

fn render_quota_reports(reports: &[QuotaReport], detail: bool) -> String {
    render_quota_reports_with_layout(reports, detail, None, current_cli_width())
}

#[derive(Clone, Copy)]
struct QuotaReportColumnWidths {
    profile: usize,
    current: usize,
    auth: usize,
    account: usize,
    plan: usize,
    remaining: usize,
}

fn quota_report_column_widths(total_width: usize) -> QuotaReportColumnWidths {
    const MIN_WIDTHS: [usize; 6] = [12, 3, 4, 14, 4, 13];
    const EXTRA_WEIGHTS: [usize; 6] = [12, 1, 3, 13, 4, 18];
    const DISTRIBUTION_ORDER: [usize; 6] = [5, 3, 0, 4, 2, 1];

    let gap_width = text_width(CLI_TABLE_GAP) * 5;
    let min_total = MIN_WIDTHS.iter().sum::<usize>();
    let available = total_width.saturating_sub(gap_width).max(min_total);

    let mut widths = MIN_WIDTHS;
    let mut remaining_extra = available.saturating_sub(min_total);
    let total_weight = EXTRA_WEIGHTS.iter().sum::<usize>().max(1);

    for (width, weight) in widths.iter_mut().zip(EXTRA_WEIGHTS) {
        let extra = remaining_extra * weight / total_weight;
        *width += extra;
    }

    let assigned = widths.iter().sum::<usize>().saturating_sub(min_total);
    remaining_extra = remaining_extra.saturating_sub(assigned);
    for index in DISTRIBUTION_ORDER.into_iter().cycle().take(remaining_extra) {
        widths[index] += 1;
    }

    QuotaReportColumnWidths {
        profile: widths[0],
        current: widths[1],
        auth: widths[2],
        account: widths[3],
        plan: widths[4],
        remaining: widths[5],
    }
}

fn render_quota_reports_with_line_limit(
    reports: &[QuotaReport],
    detail: bool,
    max_lines: Option<usize>,
) -> String {
    render_quota_reports_with_layout(reports, detail, max_lines, current_cli_width())
}

fn render_quota_reports_with_layout(
    reports: &[QuotaReport],
    detail: bool,
    max_lines: Option<usize>,
    total_width: usize,
) -> String {
    let column_widths = quota_report_column_widths(total_width);

    let mut sections = Vec::new();

    for report in sort_quota_reports_for_display(reports) {
        let active = if report.active { "*" } else { "" }.to_string();
        let auth = report.auth.label.clone();

        let (email, plan, main, status, resets) = match &report.result {
            Ok(usage) => {
                let blocked = collect_blocked_limits(usage, false);
                let status = if blocked.is_empty() {
                    "Ready".to_string()
                } else {
                    format!("Blocked: {}", format_blocked_limits(&blocked))
                };
                (
                    display_optional(usage.email.as_deref()).to_string(),
                    display_optional(usage.plan_type.as_deref()).to_string(),
                    format_main_windows_compact(usage),
                    status,
                    Some(format!("resets: {}", format_main_reset_summary(usage))),
                )
            }
            Err(err) => (
                "-".to_string(),
                "-".to_string(),
                "-".to_string(),
                format!("Error: {}", first_line_of_error(err)),
                Some("resets: unavailable".to_string()),
            ),
        };

        let mut section = Vec::new();
        section.push(format!(
            "{:<name_w$}{}{:<act_w$}{}{:<auth_w$}{}{:<email_w$}{}{:<plan_w$}{}{:<main_w$}",
            fit_cell(&report.name, column_widths.profile),
            CLI_TABLE_GAP,
            fit_cell(&active, column_widths.current),
            CLI_TABLE_GAP,
            fit_cell(&auth, column_widths.auth),
            CLI_TABLE_GAP,
            fit_cell(&email, column_widths.account),
            CLI_TABLE_GAP,
            fit_cell(&plan, column_widths.plan),
            CLI_TABLE_GAP,
            fit_cell(&main, column_widths.remaining),
            name_w = column_widths.profile,
            act_w = column_widths.current,
            auth_w = column_widths.auth,
            email_w = column_widths.account,
            plan_w = column_widths.plan,
            main_w = column_widths.remaining,
        ));
        section.extend(
            wrap_text(
                &format!("status: {status}"),
                total_width.saturating_sub(2).max(1),
            )
            .into_iter()
            .map(|line| format!("  {line}")),
        );
        if detail {
            if let Some(resets) = resets.as_deref() {
                section.extend(
                    wrap_text(resets, total_width.saturating_sub(2).max(1))
                        .into_iter()
                        .map(|line| format!("  {line}")),
                );
            }
        }
        section.push(String::new());
        sections.push(section);
    }

    let header = format!(
        "{:<name_w$}  {:<act_w$}  {:<auth_w$}  {:<email_w$}  {:<plan_w$}  {:<main_w$}",
        "PROFILE",
        "CUR",
        "AUTH",
        "ACCOUNT",
        "PLAN",
        "REMAINING",
        name_w = column_widths.profile,
        act_w = column_widths.current,
        auth_w = column_widths.auth,
        email_w = column_widths.account,
        plan_w = column_widths.plan,
        main_w = column_widths.remaining,
    );
    let mut output = vec![
        section_header_with_width("Quota Overview", total_width),
        header.clone(),
        "-".repeat(text_width(&header)),
    ];
    if let Some(max_lines) = max_lines {
        let mut remaining = max_lines.saturating_sub(output.len());
        let total_profiles = sections.len();
        let mut shown_profiles = 0_usize;

        for (index, section) in sections.into_iter().enumerate() {
            let more_profiles_remain = index + 1 < total_profiles;
            let reserve_for_notice = usize::from(more_profiles_remain);
            if section.len() + reserve_for_notice > remaining {
                break;
            }
            remaining = remaining.saturating_sub(section.len());
            output.extend(section);
            shown_profiles += 1;
        }

        let hidden_profiles = total_profiles.saturating_sub(shown_profiles);
        if hidden_profiles > 0 {
            let notice = format!(
                "  showing top {shown_profiles} of {total_profiles} profiles due to terminal height"
            );
            if remaining == 0 {
                if !output.is_empty() {
                    output.pop();
                }
            }
            output.push(notice);
        }
    } else {
        for section in sections {
            output.extend(section);
        }
    }
    output.join("\n")
}

fn sort_quota_reports_for_display(reports: &[QuotaReport]) -> Vec<&QuotaReport> {
    let mut sorted = reports.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| {
        quota_report_sort_key(left)
            .cmp(&quota_report_sort_key(right))
            .then_with(|| left.name.cmp(&right.name))
    });
    sorted
}

fn quota_report_sort_key(report: &QuotaReport) -> (usize, i64) {
    (
        quota_report_status_rank(report),
        quota_report_earliest_main_reset_epoch(report).unwrap_or(i64::MAX),
    )
}

fn quota_report_status_rank(report: &QuotaReport) -> usize {
    match &report.result {
        Ok(usage) if collect_blocked_limits(usage, false).is_empty() => 0,
        Ok(_) => 1,
        Err(_) => 2,
    }
}

fn quota_report_earliest_main_reset_epoch(report: &QuotaReport) -> Option<i64> {
    earliest_required_main_reset_epoch(report.result.as_ref().ok()?)
}

fn earliest_required_main_reset_epoch(usage: &UsageResponse) -> Option<i64> {
    ["5h", "weekly"]
        .into_iter()
        .filter_map(|label| {
            find_main_window(usage.rate_limit.as_ref()?, label).and_then(|window| window.reset_at)
        })
        .min()
}

fn format_main_windows(usage: &UsageResponse) -> String {
    usage
        .rate_limit
        .as_ref()
        .map(format_window_pair)
        .unwrap_or_else(|| "-".to_string())
}

fn format_main_windows_compact(usage: &UsageResponse) -> String {
    usage
        .rate_limit
        .as_ref()
        .map(format_window_pair_compact)
        .unwrap_or_else(|| "-".to_string())
}

fn format_main_reset_summary(usage: &UsageResponse) -> String {
    usage
        .rate_limit
        .as_ref()
        .map(format_main_reset_pair)
        .unwrap_or_else(|| "5h unavailable | weekly unavailable".to_string())
}

fn format_main_reset_pair(rate_limit: &WindowPair) -> String {
    [
        format_main_reset_window(rate_limit, "5h"),
        format_main_reset_window(rate_limit, "weekly"),
    ]
    .join(" | ")
}

fn format_main_reset_window(rate_limit: &WindowPair, label: &str) -> String {
    match find_main_window(rate_limit, label) {
        Some(window) => {
            let reset = window
                .reset_at
                .map(|epoch| format_precise_reset_time(Some(epoch)))
                .unwrap_or_else(|| "unknown".to_string());
            format!("{label} {reset}")
        }
        None => format!("{label} unavailable"),
    }
}

fn format_window_pair(rate_limit: &WindowPair) -> String {
    let mut parts = Vec::new();
    if let Some(primary) = rate_limit.primary_window.as_ref() {
        parts.push(format_window_status(primary));
    }
    if let Some(secondary) = rate_limit.secondary_window.as_ref() {
        parts.push(format_window_status(secondary));
    }

    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join(" | ")
    }
}

fn format_window_pair_compact(rate_limit: &WindowPair) -> String {
    let mut parts = Vec::new();
    if let Some(primary) = rate_limit.primary_window.as_ref() {
        parts.push(format_window_status_compact(primary));
    }
    if let Some(secondary) = rate_limit.secondary_window.as_ref() {
        parts.push(format_window_status_compact(secondary));
    }

    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join(" | ")
    }
}

fn format_named_window_status(label: &str, window: &UsageWindow) -> String {
    format!("{label}: {}", format_window_details(window))
}

fn format_window_status(window: &UsageWindow) -> String {
    format_named_window_status(&window_label(window.limit_window_seconds), window)
}

fn format_window_status_compact(window: &UsageWindow) -> String {
    let label = window_label(window.limit_window_seconds);
    match window.used_percent {
        Some(used) => {
            let remaining = remaining_percent(Some(used));
            format!("{label} {remaining}% left")
        }
        None => format!("{label} ?"),
    }
}

fn format_window_details(window: &UsageWindow) -> String {
    let reset = format_reset_time(window.reset_at);
    match window.used_percent {
        Some(used) => {
            let remaining = remaining_percent(window.used_percent);
            format!("{remaining}% left ({used}% used), resets {reset}")
        }
        None => format!("usage unknown, resets {reset}"),
    }
}

fn collect_blocked_limits(usage: &UsageResponse, include_code_review: bool) -> Vec<BlockedLimit> {
    let mut blocked = Vec::new();

    if let Some(main) = usage.rate_limit.as_ref() {
        push_required_main_window(&mut blocked, main, "5h");
        push_required_main_window(&mut blocked, main, "weekly");
    } else {
        blocked.push(BlockedLimit {
            message: "5h quota unavailable".to_string(),
        });
        blocked.push(BlockedLimit {
            message: "weekly quota unavailable".to_string(),
        });
    }

    for additional in &usage.additional_rate_limits {
        let label = additional
            .limit_name
            .as_deref()
            .or(additional.metered_feature.as_deref());
        push_blocked_window(
            &mut blocked,
            label,
            additional.rate_limit.primary_window.as_ref(),
        );
        push_blocked_window(
            &mut blocked,
            label,
            additional.rate_limit.secondary_window.as_ref(),
        );
    }

    if include_code_review {
        if let Some(code_review) = usage.code_review_rate_limit.as_ref() {
            push_blocked_window(
                &mut blocked,
                Some("code-review"),
                code_review.primary_window.as_ref(),
            );
            push_blocked_window(
                &mut blocked,
                Some("code-review"),
                code_review.secondary_window.as_ref(),
            );
        }
    }

    blocked
}

fn push_required_main_window(
    blocked: &mut Vec<BlockedLimit>,
    main: &WindowPair,
    required_label: &str,
) {
    let Some(window) = find_main_window(main, required_label) else {
        blocked.push(BlockedLimit {
            message: format!("{required_label} quota unavailable"),
        });
        return;
    };

    match window.used_percent {
        Some(used) if used < 100 => {}
        Some(_) => blocked.push(BlockedLimit {
            message: format!(
                "{required_label} exhausted until {}",
                format_reset_time(window.reset_at)
            ),
        }),
        None => blocked.push(BlockedLimit {
            message: format!("{required_label} quota unknown"),
        }),
    }
}

fn find_main_window<'a>(main: &'a WindowPair, expected_label: &str) -> Option<&'a UsageWindow> {
    [main.primary_window.as_ref(), main.secondary_window.as_ref()]
        .into_iter()
        .flatten()
        .find(|window| window_label(window.limit_window_seconds) == expected_label)
}

fn push_blocked_window(
    blocked: &mut Vec<BlockedLimit>,
    name: Option<&str>,
    window: Option<&UsageWindow>,
) {
    let Some(window) = window else {
        return;
    };
    let Some(used) = window.used_percent else {
        return;
    };
    if used < 100 {
        return;
    }

    let label = match name {
        Some(base) if !base.is_empty() => {
            format!("{base} {}", window_label(window.limit_window_seconds))
        }
        _ => window_label(window.limit_window_seconds),
    };

    blocked.push(BlockedLimit {
        message: format!(
            "{label} exhausted until {}",
            format_reset_time(window.reset_at)
        ),
    });
}

fn format_blocked_limits(blocked: &[BlockedLimit]) -> String {
    blocked
        .iter()
        .map(|limit| limit.message.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

fn remaining_percent(used_percent: Option<i64>) -> i64 {
    let Some(used) = used_percent else {
        return 0;
    };
    (100 - used).clamp(0, 100)
}

fn window_label(seconds: Option<i64>) -> String {
    let Some(seconds) = seconds else {
        return "usage".to_string();
    };

    if (17_700..=18_300).contains(&seconds) {
        return "5h".to_string();
    }
    if (601_200..=608_400).contains(&seconds) {
        return "weekly".to_string();
    }
    if (2_505_600..=2_678_400).contains(&seconds) {
        return "monthly".to_string();
    }

    format!("{seconds}s")
}

fn format_reset_time(epoch: Option<i64>) -> String {
    let Some(epoch) = epoch else {
        return "-".to_string();
    };

    Local
        .timestamp_opt(epoch, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M %Z").to_string())
        .unwrap_or_else(|| epoch.to_string())
}

fn format_precise_reset_time(epoch: Option<i64>) -> String {
    let Some(epoch) = epoch else {
        return "-".to_string();
    };

    Local
        .timestamp_opt(epoch, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S %Z").to_string())
        .unwrap_or_else(|| epoch.to_string())
}

fn display_optional(value: Option<&str>) -> &str {
    value.unwrap_or("-")
}

fn render_profile_quota(profile_name: &str, usage: &UsageResponse) -> String {
    let blocked = collect_blocked_limits(usage, false);
    let status = if blocked.is_empty() {
        "Ready".to_string()
    } else {
        format!("Blocked ({})", format_blocked_limits(&blocked))
    };
    let mut fields = vec![
        ("Profile".to_string(), profile_name.to_string()),
        (
            "Account".to_string(),
            display_optional(usage.email.as_deref()).to_string(),
        ),
        (
            "Plan".to_string(),
            display_optional(usage.plan_type.as_deref()).to_string(),
        ),
        ("Status".to_string(), status),
        ("Main".to_string(), format_main_windows(usage)),
    ];

    if let Some(code_review) = usage.code_review_rate_limit.as_ref() {
        fields.push(("Code review".to_string(), format_window_pair(code_review)));
    }

    for (name, value) in format_additional_limits(usage) {
        fields.push((name, value));
    }

    render_panel(&format!("Quota {profile_name}"), &fields)
}

fn format_additional_limits(usage: &UsageResponse) -> Vec<(String, String)> {
    let mut lines = Vec::new();

    for additional in &usage.additional_rate_limits {
        let name = additional
            .limit_name
            .as_deref()
            .or(additional.metered_feature.as_deref())
            .unwrap_or("Additional");

        if let Some(primary) = additional.rate_limit.primary_window.as_ref() {
            lines.push((
                additional_window_label(name, primary),
                format_window_details(primary),
            ));
        }
        if let Some(secondary) = additional.rate_limit.secondary_window.as_ref() {
            lines.push((
                additional_window_label(name, secondary),
                format_window_details(secondary),
            ));
        }
    }

    lines
}

fn additional_window_label(base: &str, window: &UsageWindow) -> String {
    format!("{base} {}", window_label(window.limit_window_seconds))
}

fn first_line_of_error(input: &str) -> String {
    input
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("-")
        .trim()
        .to_string()
}

fn quota_watch_enabled(args: &QuotaArgs) -> bool {
    !args.raw && !args.once
}

fn render_profile_quota_watch_output(
    profile_name: &str,
    updated: &str,
    usage_result: std::result::Result<UsageResponse, String>,
) -> String {
    let header = render_panel(
        "Quota Watch",
        &[
            ("Profile".to_string(), profile_name.to_string()),
            ("Updated".to_string(), updated.to_string()),
        ],
    );
    let body = match usage_result {
        Ok(usage) => render_profile_quota(profile_name, &usage),
        Err(err) => render_panel(
            "Quota Watch",
            &[("Error".to_string(), first_line_of_error(&err))],
        ),
    };
    format!("{header}\n\n{body}\n")
}

fn render_all_quota_watch_output(
    updated: &str,
    state_result: std::result::Result<AppState, String>,
    base_url: Option<&str>,
    detail: bool,
) -> String {
    match state_result {
        Ok(state) if !state.profiles.is_empty() => {
            let header = render_panel(
                "Quota Watch",
                &[
                    ("Profiles".to_string(), state.profiles.len().to_string()),
                    ("Updated".to_string(), updated.to_string()),
                ],
            );
            let reports = collect_quota_reports(&state, base_url);
            let available_report_lines = quota_watch_available_report_lines(&header);
            format!(
                "{header}\n\n{}\n",
                render_quota_reports_with_line_limit(&reports, detail, available_report_lines)
            )
        }
        Ok(_) => {
            render_panel(
                "Quota Watch",
                &[
                    ("Updated".to_string(), updated.to_string()),
                    ("Error".to_string(), "No profiles configured".to_string()),
                ],
            ) + "\n"
        }
        Err(err) => {
            render_panel(
                "Quota Watch",
                &[
                    ("Updated".to_string(), updated.to_string()),
                    ("Error".to_string(), first_line_of_error(&err)),
                ],
            ) + "\n"
        }
    }
}

fn redraw_quota_watch(output: &str) -> Result<()> {
    print!("\x1b[H\x1b[2J{output}");
    io::stdout()
        .flush()
        .context("failed to flush quota watch output")?;
    Ok(())
}

fn quota_watch_available_report_lines(header: &str) -> Option<usize> {
    let terminal_height = terminal_height_lines()?;
    let reserved = header.lines().count().saturating_add(1);
    Some(terminal_height.saturating_sub(reserved))
}

fn watch_quota(profile_name: &str, codex_home: &Path, base_url: Option<&str>) -> Result<()> {
    loop {
        let updated = Local::now().format("%Y-%m-%d %H:%M:%S %Z").to_string();
        let output = render_profile_quota_watch_output(
            profile_name,
            &updated,
            fetch_usage(codex_home, base_url).map_err(|err| err.to_string()),
        );
        redraw_quota_watch(&output)?;
        thread::sleep(Duration::from_secs(DEFAULT_WATCH_INTERVAL_SECONDS));
    }
}

fn watch_all_quotas(paths: &AppPaths, base_url: Option<&str>, detail: bool) -> Result<()> {
    loop {
        let updated = Local::now().format("%Y-%m-%d %H:%M:%S %Z").to_string();
        let output = render_all_quota_watch_output(
            &updated,
            AppState::load(paths).map_err(|err| err.to_string()),
            base_url,
            detail,
        );
        redraw_quota_watch(&output)?;
        thread::sleep(Duration::from_secs(DEFAULT_WATCH_INTERVAL_SECONDS));
    }
}

fn read_auth_summary(codex_home: &Path) -> AuthSummary {
    let auth_path = codex_home.join("auth.json");
    if !auth_path.is_file() {
        return AuthSummary {
            label: "no-auth".to_string(),
            quota_compatible: false,
        };
    }

    let content = match fs::read_to_string(&auth_path) {
        Ok(content) => content,
        Err(_) => {
            return AuthSummary {
                label: "unreadable-auth".to_string(),
                quota_compatible: false,
            };
        }
    };

    let stored_auth: StoredAuth = match serde_json::from_str(&content) {
        Ok(auth) => auth,
        Err(_) => {
            return AuthSummary {
                label: "invalid-auth".to_string(),
                quota_compatible: false,
            };
        }
    };

    let has_chatgpt_token = stored_auth
        .tokens
        .as_ref()
        .and_then(|tokens| tokens.access_token.as_deref())
        .is_some_and(|token| !token.trim().is_empty());
    let has_api_key = stored_auth
        .openai_api_key
        .as_deref()
        .is_some_and(|key| !key.trim().is_empty());

    if has_chatgpt_token {
        return AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        };
    }

    if matches!(stored_auth.auth_mode.as_deref(), Some("api_key")) || has_api_key {
        return AuthSummary {
            label: "api-key".to_string(),
            quota_compatible: false,
        };
    }

    AuthSummary {
        label: stored_auth
            .auth_mode
            .unwrap_or_else(|| "auth-present".to_string()),
        quota_compatible: false,
    }
}

fn read_usage_auth(codex_home: &Path) -> Result<UsageAuth> {
    let auth_path = codex_home.join("auth.json");
    if !auth_path.is_file() {
        bail!(
            "auth file not found at {}. Run `codex login` first.",
            auth_path.display()
        );
    }

    let content = fs::read_to_string(&auth_path)
        .with_context(|| format!("failed to read {}", auth_path.display()))?;
    let stored_auth: StoredAuth = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", auth_path.display()))?;

    let has_api_key = stored_auth
        .openai_api_key
        .as_deref()
        .is_some_and(|key| !key.trim().is_empty());
    if matches!(stored_auth.auth_mode.as_deref(), Some("api_key")) || has_api_key {
        bail!("quota endpoint requires a ChatGPT access token. Run `codex login` first.");
    }

    let tokens = stored_auth
        .tokens
        .as_ref()
        .context("auth tokens are missing from auth.json")?;
    let access_token = tokens
        .access_token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .context("access token not found in auth.json")?
        .to_string();
    let account_id = tokens
        .account_id
        .as_deref()
        .map(str::trim)
        .filter(|account_id| !account_id.is_empty())
        .map(ToOwned::to_owned);

    Ok(UsageAuth {
        access_token,
        account_id,
    })
}

fn quota_base_url(explicit: Option<&str>) -> String {
    explicit
        .map(ToOwned::to_owned)
        .or_else(|| env::var("CODEX_CHATGPT_BASE_URL").ok())
        .unwrap_or_else(|| DEFAULT_CHATGPT_BASE_URL.to_string())
        .trim_end_matches('/')
        .to_string()
}

fn usage_url(base_url: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    if base_url.contains("/backend-api") {
        format!("{base_url}/wham/usage")
    } else {
        format!("{base_url}/api/codex/usage")
    }
}

fn format_response_body(body: &[u8]) -> String {
    if body.is_empty() {
        return String::new();
    }

    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
        return serde_json::to_string_pretty(&value)
            .unwrap_or_else(|_| String::from_utf8_lossy(body).trim().to_string());
    }

    String::from_utf8_lossy(body).trim().to_string()
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

fn format_binary_resolution(binary: &OsString) -> String {
    let configured = binary.to_string_lossy();
    match resolve_binary_path(binary) {
        Some(path) => format!("{configured} ({})", path.display()),
        None => format!("{configured} (not found)"),
    }
}

fn resolve_binary_path(binary: &OsString) -> Option<PathBuf> {
    let candidate = PathBuf::from(binary);
    if candidate.components().count() > 1 {
        if candidate.is_file() {
            return Some(fs::canonicalize(&candidate).unwrap_or(candidate));
        }
        return None;
    }

    let path_var = env::var_os("PATH")?;
    for directory in env::split_paths(&path_var) {
        let full_path = directory.join(&candidate);
        if full_path.is_file() {
            return Some(full_path);
        }
    }

    None
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
    fn load(paths: &AppPaths) -> Result<Self> {
        if !paths.state_file.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&paths.state_file)
            .with_context(|| format!("failed to read {}", paths.state_file.display()))?;
        let state = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", paths.state_file.display()))?;
        Ok(state)
    }

    fn save(&self, paths: &AppPaths) -> Result<()> {
        let _lock = acquire_state_file_lock(paths)?;
        let existing = Self::load(paths)?;
        let merged = merge_app_state_for_save(existing, self);
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

#[cfg(test)]
mod update_notice_tests {
    use super::*;

    #[test]
    fn version_is_newer_compares_semver_like_versions() {
        assert!(version_is_newer("0.2.47", "0.2.46"));
        assert!(version_is_newer("1.0.0", "0.9.9"));
        assert!(!version_is_newer("0.2.46", "0.2.46"));
        assert!(!version_is_newer("0.2.45", "0.2.46"));
    }

    #[test]
    fn update_notice_is_suppressed_for_machine_output_modes() {
        assert!(!should_emit_update_notice(&Commands::Doctor(DoctorArgs {
            quota: false,
            runtime: true,
            json: true,
        })));
        assert!(!should_emit_update_notice(&Commands::Quota(QuotaArgs {
            profile: None,
            all: false,
            detail: false,
            raw: true,
            watch: false,
            once: false,
            base_url: None,
        })));
        assert!(should_emit_update_notice(&Commands::Current));
    }

    #[test]
    fn update_check_cache_ttl_is_short_when_cached_version_matches_current() {
        assert_eq!(
            update_check_cache_ttl_seconds("0.2.47", "0.2.47"),
            UPDATE_CHECK_STALE_CURRENT_TTL_SECONDS
        );
        assert_eq!(
            update_check_cache_ttl_seconds("0.2.46", "0.2.47"),
            UPDATE_CHECK_STALE_CURRENT_TTL_SECONDS
        );
        assert_eq!(
            update_check_cache_ttl_seconds("0.2.48", "0.2.47"),
            UPDATE_CHECK_CACHE_TTL_SECONDS
        );
    }

    #[test]
    fn normalize_run_codex_args_rewrites_session_id_to_resume() {
        let args = vec![
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
            OsString::from("continue from here"),
        ];
        assert_eq!(
            normalize_run_codex_args(&args),
            vec![
                OsString::from("resume"),
                OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
                OsString::from("continue from here"),
            ]
        );
    }

    #[test]
    fn normalize_run_codex_args_keeps_regular_prompt_intact() {
        let args = vec![OsString::from("fix this bug")];
        assert_eq!(normalize_run_codex_args(&args), args);
    }
}
