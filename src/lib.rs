use anyhow::{Context, Result, bail};
use base64::Engine;
use chrono::{Local, TimeZone};
use clap::{Args, Parser, Subcommand};
use dirs::home_dir;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::cmp::Reverse;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Cursor, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
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

mod app_commands;
mod audit_log;
mod cli_args;
mod command_dispatch;
mod housekeeping;
mod profile_commands;
mod profile_identity;
mod quota_support;
mod runtime_anthropic;
mod runtime_background;
mod runtime_broker;
mod runtime_broker_shared;
mod runtime_capabilities;
mod runtime_caveman;
mod runtime_claude;
#[path = "runtime_tuning.rs"]
mod runtime_config;
mod runtime_core_shared;
mod runtime_doctor;
mod runtime_launch;
mod runtime_launch_shared;
mod runtime_mem;
mod runtime_metrics;
mod runtime_persistence;
mod runtime_policy;
mod runtime_proxy;
mod runtime_proxy_shared;
mod runtime_store;
mod secret_store;
mod shared_codex_fs;
#[path = "cli_render.rs"]
mod terminal_ui;
mod update_notice;

use app_commands::*;
use audit_log::*;
pub(crate) use cli_args::*;
use housekeeping::*;
use profile_commands::*;
use profile_identity::*;
use quota_support::*;
use runtime_anthropic::*;
use runtime_background::*;
use runtime_broker::*;
use runtime_broker_shared::*;
use runtime_capabilities::*;
use runtime_caveman::*;
use runtime_claude::*;
use runtime_config::*;
use runtime_core_shared::*;
use runtime_doctor::*;
use runtime_launch::*;
use runtime_launch_shared::*;
use runtime_mem::*;
use runtime_persistence::*;
use runtime_policy::*;
use runtime_proxy::*;
use runtime_proxy_shared::*;
use runtime_store::*;
use shared_codex_fs::*;
use terminal_ui::*;
use update_notice::*;

const DEFAULT_PRODEX_DIR: &str = ".prodex";
const DEFAULT_CODEX_DIR: &str = ".codex";
const DEFAULT_CHATGPT_BASE_URL: &str = "https://chatgpt.com/backend-api";
const RUNTIME_PROXY_OPENAI_UPSTREAM_PATH: &str = "/backend-api/codex";
const RUNTIME_PROXY_OPENAI_MOUNT_PATH: &str = "/backend-api/prodex";
const RUNTIME_PROXY_ANTHROPIC_MESSAGES_PATH: &str = "/v1/messages";
const RUNTIME_PROXY_ANTHROPIC_MODELS_PATH: &str = "/v1/models";
const RUNTIME_PROXY_ANTHROPIC_HEALTH_PATH: &str = "/health";
const LEGACY_RUNTIME_PROXY_OPENAI_MOUNT_PATH_PREFIX: &str = "/backend-api/prodex/v";
const PRODEX_CLAUDE_PROXY_API_KEY: &str = "prodex-runtime-proxy";
const PRODEX_INTERNAL_REQUEST_ORIGIN_HEADER: &str = "X-Prodex-Internal-Request-Origin";
const PRODEX_INTERNAL_REQUEST_ORIGIN_ANTHROPIC_MESSAGES: &str = "anthropic_messages";
const DEFAULT_PRODEX_CLAUDE_MODEL: &str = "gpt-5";
const PRODEX_CLAUDE_CONFIG_DIR_NAME: &str = ".claude-code";
const PRODEX_SHARED_CLAUDE_DIR_NAME: &str = "claude";
const DEFAULT_CLAUDE_CONFIG_DIR_NAME: &str = ".claude";
const DEFAULT_CLAUDE_CONFIG_FILE_NAME: &str = ".claude.json";
const DEFAULT_CLAUDE_SETTINGS_FILE_NAME: &str = "settings.json";
const PRODEX_CLAUDE_DEFAULT_WEB_TOOLS: &[&str] = &["WebSearch", "WebFetch"];
const PRODEX_CLAUDE_LEGACY_IMPORT_MARKER_NAME: &str = ".prodex-legacy-imported";
const RUNTIME_PROXY_ANTHROPIC_MODEL_CREATED_AT: &str = "2026-01-01T00:00:00Z";
const DEFAULT_WATCH_INTERVAL_SECONDS: u64 = 5;
const CHATGPT_AUTH_REFRESH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const CHATGPT_AUTH_REFRESH_URL: &str = "https://auth.openai.com/oauth/token";
const CHATGPT_AUTH_REFRESH_INTERVAL_DAYS: i64 = 8;
const CHATGPT_AUTH_REFRESH_EXPIRY_SKEW_SECONDS: i64 = if cfg!(test) { 30 } else { 5 * 60 };
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
const RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS: u64 = if cfg!(test) { 80 } else { 750 };
const RUNTIME_PROXY_PRESSURE_ADMISSION_WAIT_BUDGET_MS: u64 = if cfg!(test) { 25 } else { 200 };
const RUNTIME_PROXY_PRESSURE_LONG_LIVED_QUEUE_WAIT_BUDGET_MS: u64 =
    if cfg!(test) { 25 } else { 200 };
const RUNTIME_PROXY_INTERACTIVE_WAIT_MULTIPLIER: u64 = 2;
const RUNTIME_PROXY_PRESSURE_PRECOMMIT_BUDGET_MS: u64 = if cfg!(test) { 150 } else { 800 };
#[allow(dead_code)]
const RUNTIME_PROXY_PRESSURE_PRECOMMIT_CONTINUATION_BUDGET_MS: u64 =
    if cfg!(test) { 250 } else { 1_500 };
const RUNTIME_PROXY_PRESSURE_PRECOMMIT_ATTEMPT_LIMIT: usize = if cfg!(test) { 2 } else { 6 };
#[allow(dead_code)]
const RUNTIME_PROXY_PRESSURE_PRECOMMIT_CONTINUATION_ATTEMPT_LIMIT: usize =
    if cfg!(test) { 4 } else { 8 };
const RUNTIME_PROXY_COMPACT_OWNER_RETRY_DELAY_MS: u64 = if cfg!(test) { 5 } else { 150 };
const RUNTIME_PROXY_SYNC_PROBE_PRESSURE_PAUSE_MS: u64 = 5;
const RUNTIME_PROFILE_INFLIGHT_SOFT_LIMIT: usize = if cfg!(test) { 1 } else { 4 };
const RUNTIME_PROFILE_INFLIGHT_HARD_LIMIT: usize = if cfg!(test) { 2 } else { 8 };
const RUNTIME_PROFILE_HEALTH_DECAY_SECONDS: i64 = if cfg!(test) { 2 } else { 60 };
const RUNTIME_PROFILE_USAGE_CACHE_FRESH_SECONDS: i64 = if cfg!(test) { 30 } else { 300 };
const UPDATE_CHECK_CACHE_TTL_SECONDS: i64 = if cfg!(test) { 5 } else { 21_600 };
const UPDATE_CHECK_STALE_CURRENT_TTL_SECONDS: i64 = if cfg!(test) { 1 } else { 300 };
const UPDATE_CHECK_HTTP_CONNECT_TIMEOUT_MS: u64 = if cfg!(test) { 200 } else { 800 };
const UPDATE_CHECK_HTTP_READ_TIMEOUT_MS: u64 = if cfg!(test) { 400 } else { 1200 };
const RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS: i64 = if cfg!(test) { 300 } else { 1800 };
const RUNTIME_PROFILE_QUOTA_QUARANTINE_FALLBACK_SECONDS: i64 = if cfg!(test) { 10 } else { 300 };
const RUNTIME_PROFILE_SYNC_PROBE_FALLBACK_LIMIT: usize = 3;
const RUNTIME_STARTUP_PROBE_WARM_LIMIT: usize = 3;
const RUNTIME_STARTUP_SYNC_PROBE_WARM_LIMIT: usize = if cfg!(test) {
    RUNTIME_STARTUP_PROBE_WARM_LIMIT
} else {
    1
};
const RUNTIME_STATE_SAVE_DEBOUNCE_MS: u64 = if cfg!(test) { 5 } else { 150 };
const RUNTIME_STATE_SAVE_QUEUE_PRESSURE_THRESHOLD: usize = 8;
const RUNTIME_CONTINUATION_JOURNAL_QUEUE_PRESSURE_THRESHOLD: usize = 8;
const RUNTIME_PROBE_REFRESH_QUEUE_PRESSURE_THRESHOLD: usize = 16;
const RUNTIME_PROFILE_AUTH_FAILURE_DECAY_SECONDS: i64 = if cfg!(test) { 5 } else { 300 };
const RUNTIME_PROFILE_AUTH_FAILURE_401_SCORE: u32 = if cfg!(test) { 3 } else { 12 };
const RUNTIME_PROFILE_AUTH_FAILURE_403_SCORE: u32 = if cfg!(test) { 1 } else { 2 };
const RUNTIME_BINDING_TOUCH_PERSIST_INTERVAL_SECONDS: i64 = if cfg!(test) { 1 } else { 60 };
const RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS: i64 = if cfg!(test) { 4 } else { 180 };
const RUNTIME_PROFILE_SUCCESS_STREAK_DECAY_SECONDS: i64 = if cfg!(test) { 8 } else { 300 };
const RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS: i64 = if cfg!(test) { 8 } else { 300 };
const RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY: u32 = 4;
const RUNTIME_PROFILE_CONNECT_FAILURE_HEALTH_PENALTY: u32 = 5;
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
const RUNTIME_PROXY_WEBSOCKET_HAPPY_EYEBALLS_DELAY_MS: u64 = if cfg!(test) { 10 } else { 200 };
const RUNTIME_PROXY_WEBSOCKET_PRECOMMIT_PROGRESS_TIMEOUT_MS: u64 =
    if cfg!(test) { 120 } else { 8_000 };
const RUNTIME_PROXY_WEBSOCKET_PREVIOUS_RESPONSE_REUSE_STALE_MS: u64 = 60_000;
const RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS: u64 = if cfg!(test) { 50 } else { 1_000 };
const RUNTIME_PROXY_SSE_LOOKAHEAD_BYTES: usize = 8 * 1024;
const RUNTIME_PROXY_PREFETCH_QUEUE_CAPACITY: usize = 2;
const RUNTIME_PROXY_PREFETCH_MAX_CHUNK_BYTES: usize = 512 * 1024;
const RUNTIME_PROXY_PREFETCH_MAX_BUFFERED_BYTES: usize = 768 * 1024;
const RUNTIME_PROXY_PREFETCH_BACKPRESSURE_RETRY_MS: u64 = if cfg!(test) { 2 } else { 10 };
const RUNTIME_PROXY_PREFETCH_BACKPRESSURE_TIMEOUT_MS: u64 = if cfg!(test) { 40 } else { 1_000 };
const RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES: usize = 4 * 1024 * 1024;
const RUNTIME_PROXY_COMPACT_BUFFERED_RESPONSE_MAX_BYTES: usize = 32 * 1024 * 1024;
const RUNTIME_PROXY_HEAP_TRIM_MIN_RELEASE_BYTES: usize =
    if cfg!(test) { 1024 } else { 1024 * 1024 };
const RUNTIME_PROXY_HEAP_TRIM_MIN_INTERVAL_MS: u64 = if cfg!(test) { 0 } else { 2_000 };
const RUNTIME_PROXY_ANTHROPIC_WEB_SEARCH_FOLLOWUP_LIMIT: usize = if cfg!(test) { 2 } else { 4 };
const RUNTIME_PROXY_LOG_FILE_PREFIX: &str = "prodex-runtime";
const RUNTIME_PROXY_LATEST_LOG_POINTER: &str = "prodex-runtime-latest.path";
const RUNTIME_PROXY_DOCTOR_TAIL_BYTES: usize = 128 * 1024;
const PRODEX_SECRET_BACKEND_ENV: &str = "PRODEX_SECRET_BACKEND";
const PRODEX_SECRET_KEYRING_SERVICE_ENV: &str = "PRODEX_SECRET_KEYRING_SERVICE";
const CODEX_REFRESH_TOKEN_URL_OVERRIDE_ENV: &str = "CODEX_REFRESH_TOKEN_URL_OVERRIDE";
const INFO_RUNTIME_LOG_TAIL_BYTES: usize = if cfg!(test) { 64 * 1024 } else { 512 * 1024 };
const INFO_FORECAST_LOOKBACK_SECONDS: i64 = if cfg!(test) { 3_600 } else { 3 * 60 * 60 };
const INFO_FORECAST_MIN_SPAN_SECONDS: i64 = if cfg!(test) { 60 } else { 5 * 60 };
const INFO_RECENT_LOAD_WINDOW_SECONDS: i64 = if cfg!(test) { 600 } else { 30 * 60 };
const LAST_GOOD_FILE_SUFFIX: &str = ".last-good";
const RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_SECONDS: i64 = if cfg!(test) { 5 } else { 180 };
const RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD: u32 = 2;
const RUNTIME_CONTINUATION_SUSPECT_GRACE_SECONDS: i64 = if cfg!(test) { 5 } else { 120 };
const RUNTIME_CONTINUATION_DEAD_GRACE_SECONDS: i64 = if cfg!(test) { 5 } else { 900 };
const RUNTIME_CONTINUATION_VERIFIED_STALE_SECONDS: i64 = if cfg!(test) { 10 } else { 1_800 };
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
const RUNTIME_BROKER_LEASE_SCAN_INTERVAL_MS: u64 = if cfg!(test) { 125 } else { 1_000 };
const RUNTIME_BROKER_IDLE_GRACE_SECONDS: i64 = if cfg!(test) { 1 } else { 5 };
const CLI_WIDTH: usize = 110;
const CLI_MIN_WIDTH: usize = 60;
const CLI_LABEL_WIDTH: usize = 16;
const CLI_MIN_LABEL_WIDTH: usize = 10;
const CLI_MAX_LABEL_WIDTH: usize = 24;
const CLI_TABLE_GAP: &str = "  ";
const CLI_TOP_LEVEL_AFTER_HELP: &str = "\
Tips:
  Bare `prodex` invocation defaults to `prodex run`.
  Use `prodex quota --all --detail` for the clearest quota view across profiles.
  Use `prodex <command> -h` to see every parameter for that command.

Examples:
  prodex
  prodex exec \"review this repo\"
  prodex profile list
  prodex quota --all --detail
  prodex run --profile main";
const CLI_PROFILE_AFTER_HELP: &str = "\
Examples:
  prodex profile list
  prodex profile add main --activate
  prodex profile export
  prodex profile export backup.json
  prodex profile import backup.json
  prodex profile import-current main
  prodex profile remove main
  prodex profile remove --all";
const CLI_LOGIN_AFTER_HELP: &str = "\
Examples:
  prodex login
  prodex login --profile main
  prodex login --device-auth";
const CLI_QUOTA_AFTER_HELP: &str = "\
Best practice:
  Use `prodex quota --all --detail` for the clearest live quota view across profiles.

Examples:
  prodex quota
  prodex quota --profile main --detail
  prodex quota --all --detail
  prodex quota --all --once
  prodex quota --raw --profile main";
const CLI_RUN_AFTER_HELP: &str = "\
Examples:
  prodex
  prodex run
  prodex run mem
  prodex exec \"review this repo\"
  prodex run --profile main
  prodex run exec \"review this repo\"
  prodex run 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9

Notes:
  Auto-rotate is enabled by default.
  Bare `prodex <args>` is treated as `prodex run <args>`.
  A lone session id is forwarded as `codex resume <session-id>`.";
const CLI_CLAUDE_AFTER_HELP: &str = "\
Examples:
  prodex claude --print \"summarize this repo\"
  prodex claude mem
  prodex claude caveman
  prodex claude caveman mem
  prodex claude caveman -- -p \"summarize this repo briefly\"
  prodex claude --profile main --print \"review the latest changes\"
  prodex claude --skip-quota-check -- --help

Notes:
  Prodex injects a local Anthropic-compatible proxy via `ANTHROPIC_BASE_URL`.
  Prefix Claude args with `caveman` and/or `mem` to load the Caveman or Claude-Mem plugin for that session only.
  Use `PRODEX_CLAUDE_BIN` to point prodex at a specific Claude Code binary.
  Claude defaults to the current Codex model from `config.toml` when available.
  Use `PRODEX_CLAUDE_MODEL` to override the upstream Responses model mapping.
  Use `PRODEX_CLAUDE_REASONING_EFFORT` to force the upstream Responses reasoning effort.
  Use `PRODEX_CLAUDE_NATIVE_CLIENT_TOOLS=shell,computer` to opt into native client-tool translation on supported models.";
const CLI_CAVEMAN_AFTER_HELP: &str = "\
Examples:
  prodex caveman
  prodex caveman mem
  prodex caveman --profile main
  prodex caveman exec \"review latest diff in caveman mode\"
  prodex caveman 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9

Notes:
  Prodex launches Codex from a temporary overlay `CODEX_HOME` so Caveman stays isolated from the base profile.
  The selected profile's auth, shared sessions, and quota behavior stay the same as `prodex run`.
  Prefix Codex args with `mem` to point Claude-Mem transcript watching at the selected Prodex session path.
  Caveman activation is sourced from Julius Brussee's Caveman plugin and a session-start hook adapted for the current Codex hooks schema.";
const CLI_DOCTOR_AFTER_HELP: &str = "\
Examples:
  prodex doctor
  prodex doctor --quota
  prodex doctor --runtime
  prodex doctor --runtime --json";
const CLI_AUDIT_AFTER_HELP: &str = "\
Examples:
  prodex audit
  prodex audit --tail 50
  prodex audit --component profile --action use
  prodex audit --json";
const CLI_CLEANUP_AFTER_HELP: &str = "\
Examples:
  prodex cleanup";
const SHARED_CODEX_DIR_NAMES: &[&str] = &[
    "sessions",
    "archived_sessions",
    "shell_snapshots",
    "memories",
    "memories_extensions",
    "rules",
    "skills",
    "plugins",
    ".tmp/marketplaces",
];
const SHARED_CODEX_FILE_NAMES: &[&str] = &["history.jsonl", "config.toml"];
const SHARED_CODEX_SQLITE_PREFIXES: &[&str] = &["state_", "logs_"];
const SHARED_CODEX_SQLITE_SUFFIXES: &[&str] = &[".sqlite", ".sqlite-shm", ".sqlite-wal"];
static STATE_SAVE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static RUNTIME_PROXY_LOG_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static RUNTIME_STATE_SAVE_QUEUE: OnceLock<Arc<RuntimeStateSaveQueue>> = OnceLock::new();
static RUNTIME_CONTINUATION_JOURNAL_SAVE_QUEUE: OnceLock<Arc<RuntimeContinuationJournalSaveQueue>> =
    OnceLock::new();
static RUNTIME_PROBE_REFRESH_QUEUE: OnceLock<Arc<RuntimeProbeRefreshQueue>> = OnceLock::new();
static RUNTIME_PERSISTENCE_MODE_BY_LOG_PATH: OnceLock<Mutex<BTreeMap<PathBuf, bool>>> =
    OnceLock::new();
static RUNTIME_BROKER_METADATA_BY_LOG_PATH: OnceLock<
    Mutex<BTreeMap<PathBuf, RuntimeBrokerMetadata>>,
> = OnceLock::new();

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
    #[serde(default)]
    last_refresh: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct StoredTokens {
    access_token: Option<String>,
    account_id: Option<String>,
    id_token: Option<String>,
    refresh_token: Option<String>,
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

#[derive(Debug, Clone)]
struct RunProfileProbeJob {
    name: String,
    order_index: usize,
    codex_home: PathBuf,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
struct RuntimeStateSaveSnapshot {
    paths: AppPaths,
    state: AppState,
    continuations: RuntimeContinuationStore,
    profile_scores: BTreeMap<String, RuntimeProfileHealth>,
    usage_snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot>,
    backoffs: RuntimeProfileBackoffs,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
enum RuntimeStateSavePayload {
    Snapshot(RuntimeStateSaveSnapshot),
    Live(RuntimeRotationProxyShared),
}

#[derive(Debug)]
struct RuntimeStateSaveJob {
    payload: RuntimeStateSavePayload,
    revision: u64,
    latest_revision: Arc<AtomicU64>,
    log_path: PathBuf,
    reason: String,
    queued_at: Instant,
    ready_at: Instant,
}

#[derive(Debug, Clone)]
struct RuntimeContinuationJournalSnapshot {
    paths: AppPaths,
    continuations: RuntimeContinuationStore,
    profiles: BTreeMap<String, ProfileEntry>,
}

#[derive(Debug, Clone)]
enum RuntimeContinuationJournalSavePayload {
    Snapshot(RuntimeContinuationJournalSnapshot),
    Live(RuntimeRotationProxyShared),
}

#[derive(Debug)]
struct RuntimeContinuationJournalSaveJob {
    payload: RuntimeContinuationJournalSavePayload,
    log_path: PathBuf,
    reason: String,
    saved_at: i64,
    queued_at: Instant,
    ready_at: Instant,
}

trait RuntimeScheduledSaveJob {
    fn ready_at(&self) -> Instant;
}

impl RuntimeScheduledSaveJob for RuntimeStateSaveJob {
    fn ready_at(&self) -> Instant {
        self.ready_at
    }
}

impl RuntimeScheduledSaveJob for RuntimeContinuationJournalSaveJob {
    fn ready_at(&self) -> Instant {
        self.ready_at
    }
}

enum RuntimeDueJobs<K, J> {
    Due(BTreeMap<K, J>),
    Wait(Duration),
}

fn runtime_take_due_scheduled_jobs<K, J>(
    pending: &mut BTreeMap<K, J>,
    now: Instant,
) -> RuntimeDueJobs<K, J>
where
    K: Ord + Clone,
    J: RuntimeScheduledSaveJob,
{
    let next_ready_at = pending
        .values()
        .map(RuntimeScheduledSaveJob::ready_at)
        .min()
        .expect("scheduled save jobs should be present");
    if next_ready_at > now {
        return RuntimeDueJobs::Wait(next_ready_at.saturating_duration_since(now));
    }

    let due_keys = pending
        .iter()
        .filter_map(|(key, job)| (job.ready_at() <= now).then_some(key.clone()))
        .collect::<Vec<_>>();
    let mut due = BTreeMap::new();
    for key in due_keys {
        if let Some(job) = pending.remove(&key) {
            due.insert(key, job);
        }
    }
    RuntimeDueJobs::Due(due)
}

#[derive(Debug)]
struct RuntimeProbeRefreshQueue {
    pending: Mutex<BTreeMap<(PathBuf, String), RuntimeProbeRefreshJob>>,
    wake: Condvar,
    active: Arc<AtomicUsize>,
    wait: Arc<(Mutex<()>, Condvar)>,
    revision: Arc<AtomicU64>,
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
    profile_usage_auth: BTreeMap<String, RuntimeProfileUsageAuthCacheEntry>,
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
struct RuntimeProfileUsageAuthCacheEntry {
    auth: UsageAuth,
    location: secret_store::SecretLocation,
    revision: Option<secret_store::SecretRevision>,
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
    #[serde(default, deserialize_with = "deserialize_null_default")]
    retry_backoff_until: BTreeMap<String, i64>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    transport_backoff_until: BTreeMap<String, i64>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
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
    compat_warning_count: usize,
    top_client_family: Option<String>,
    top_client: Option<String>,
    top_tool_surface: Option<String>,
    top_compat_warning: Option<String>,
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
const RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX: &str = "__response_turn_state__:";

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
    transport_backoff_until: Option<i64>,
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

fn runtime_proxy_log(shared: &RuntimeRotationProxyShared, message: impl AsRef<str>) {
    runtime_proxy_log_to_path(&shared.log_path, message.as_ref());
}

fn runtime_proxy_next_request_id(shared: &RuntimeRotationProxyShared) -> u64 {
    shared.request_sequence.fetch_add(1, Ordering::Relaxed)
}

pub fn main_entry() {
    if let Err(err) = run() {
        eprintln!("Error: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let command = parse_cli_command_or_exit();
    if command.should_show_update_notice() {
        let _ = show_update_notice_if_available(&command);
    }
    ensure_runtime_policy_valid()?;
    command.execute()
}

fn parse_cli_command_or_exit() -> Commands {
    match parse_cli_command_from(env::args_os()) {
        Ok(command) => command,
        Err(err) => err.exit(),
    }
}

fn parse_cli_command_from<I, T>(args: I) -> std::result::Result<Commands, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let raw_args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    let parse_args = if should_default_cli_invocation_to_run(&raw_args) {
        rewrite_cli_args_as_run(&raw_args)
    } else {
        raw_args
    };
    Ok(Cli::try_parse_from(parse_args)?.command)
}

fn should_default_cli_invocation_to_run(args: &[OsString]) -> bool {
    let Some(first_arg) = args.get(1).and_then(|arg| arg.to_str()) else {
        return true;
    };

    !matches!(
        first_arg,
        "-h" | "--help"
            | "-V"
            | "--version"
            | "profile"
            | "use"
            | "current"
            | "info"
            | "doctor"
            | "audit"
            | "cleanup"
            | "login"
            | "logout"
            | "quota"
            | "run"
            | "caveman"
            | "claude"
            | "help"
            | "__runtime-broker"
    )
}

fn rewrite_cli_args_as_run(args: &[OsString]) -> Vec<OsString> {
    let mut rewritten = Vec::with_capacity(args.len() + 1);
    rewritten.push(
        args.first()
            .cloned()
            .unwrap_or_else(|| OsString::from("prodex")),
    );
    rewritten.push(OsString::from("run"));
    rewritten.extend(args.iter().skip(1).cloned());
    rewritten
}

fn handle_profile_command(command: ProfileCommands) -> Result<()> {
    match command {
        ProfileCommands::Add(args) => handle_add_profile(args),
        ProfileCommands::Export(args) => handle_export_profiles(args),
        ProfileCommands::Import(args) => handle_import_profiles(args),
        ProfileCommands::ImportCurrent(args) => handle_import_current_profile(args),
        ProfileCommands::List => handle_list_profiles(),
        ProfileCommands::Remove(args) => handle_remove_profile(args),
        ProfileCommands::Use(selector) => handle_set_active_profile(selector),
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
            shared_codex_root: match env::var_os("PRODEX_SHARED_CODEX_HOME") {
                Some(path) => resolve_shared_codex_root(&root, PathBuf::from(path)),
                None => prodex_default_shared_codex_root(&root),
            },
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

fn codex_bin() -> OsString {
    env::var_os("PRODEX_CODEX_BIN").unwrap_or_else(|| OsString::from("codex"))
}

fn claude_bin() -> OsString {
    env::var_os("PRODEX_CLAUDE_BIN").unwrap_or_else(|| OsString::from("claude"))
}

#[cfg(test)]
#[path = "../tests/support/main_internal_harness.rs"]
mod main_internal_tests;

#[cfg(test)]
#[path = "../tests/support/compat_replay_body.rs"]
mod compat_replay_tests;
