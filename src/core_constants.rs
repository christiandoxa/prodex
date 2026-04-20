use super::*;

pub(crate) const DEFAULT_PRODEX_DIR: &str = ".prodex";
pub(crate) const DEFAULT_CODEX_DIR: &str = ".codex";
pub(crate) const DEFAULT_CHATGPT_BASE_URL: &str = "https://chatgpt.com/backend-api";
pub(crate) const RUNTIME_PROXY_OPENAI_UPSTREAM_PATH: &str = "/backend-api/codex";
pub(crate) const RUNTIME_PROXY_OPENAI_MOUNT_PATH: &str = "/backend-api/prodex";
pub(crate) const RUNTIME_PROXY_ANTHROPIC_MESSAGES_PATH: &str = "/v1/messages";
pub(crate) const RUNTIME_PROXY_ANTHROPIC_MODELS_PATH: &str = "/v1/models";
pub(crate) const RUNTIME_PROXY_ANTHROPIC_HEALTH_PATH: &str = "/health";
pub(crate) const LEGACY_RUNTIME_PROXY_OPENAI_MOUNT_PATH_PREFIX: &str = "/backend-api/prodex/v";
pub(crate) const PRODEX_CLAUDE_PROXY_API_KEY: &str = "prodex-runtime-proxy";
pub(crate) const PRODEX_INTERNAL_REQUEST_ORIGIN_HEADER: &str = "X-Prodex-Internal-Request-Origin";
pub(crate) const PRODEX_INTERNAL_REQUEST_ORIGIN_ANTHROPIC_MESSAGES: &str = "anthropic_messages";
pub(crate) const DEFAULT_PRODEX_CLAUDE_MODEL: &str = "gpt-5";
pub(crate) const PRODEX_CLAUDE_CONFIG_DIR_NAME: &str = ".claude-code";
pub(crate) const PRODEX_SHARED_CLAUDE_DIR_NAME: &str = "claude";
pub(crate) const DEFAULT_CLAUDE_CONFIG_DIR_NAME: &str = ".claude";
pub(crate) const DEFAULT_CLAUDE_CONFIG_FILE_NAME: &str = ".claude.json";
pub(crate) const DEFAULT_CLAUDE_SETTINGS_FILE_NAME: &str = "settings.json";
pub(crate) const PRODEX_CLAUDE_DEFAULT_WEB_TOOLS: &[&str] = &["WebSearch", "WebFetch"];
pub(crate) const PRODEX_CLAUDE_LEGACY_IMPORT_MARKER_NAME: &str = ".prodex-legacy-imported";
pub(crate) const RUNTIME_PROXY_ANTHROPIC_MODEL_CREATED_AT: &str = "2026-01-01T00:00:00Z";
pub(crate) const DEFAULT_WATCH_INTERVAL_SECONDS: u64 = 5;
pub(crate) const CHATGPT_AUTH_REFRESH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub(crate) const CHATGPT_AUTH_REFRESH_URL: &str = "https://auth.openai.com/oauth/token";
pub(crate) const CHATGPT_AUTH_REFRESH_INTERVAL_DAYS: i64 = 8;
pub(crate) const CHATGPT_AUTH_REFRESH_EXPIRY_SKEW_SECONDS: i64 =
    if cfg!(test) { 30 } else { 5 * 60 };
pub(crate) const RUN_SELECTION_NEAR_OPTIMAL_BPS: i64 = 1_000;
pub(crate) const RUN_SELECTION_HYSTERESIS_BPS: i64 = 500;
pub(crate) const RUN_SELECTION_COOLDOWN_SECONDS: i64 = 15 * 60;
pub(crate) const RESPONSE_PROFILE_BINDING_LIMIT: usize = 65_536;
pub(crate) const TURN_STATE_PROFILE_BINDING_LIMIT: usize = 4_096;
pub(crate) const SESSION_ID_PROFILE_BINDING_LIMIT: usize = 4_096;
pub(crate) const APP_STATE_LAST_RUN_RETENTION_SECONDS: i64 =
    if cfg!(test) { 60 } else { 90 * 24 * 60 * 60 };
pub(crate) const APP_STATE_SESSION_BINDING_RETENTION_SECONDS: i64 =
    if cfg!(test) { 60 } else { 30 * 24 * 60 * 60 };
pub(crate) const RUNTIME_SCORE_RETENTION_SECONDS: i64 =
    if cfg!(test) { 120 } else { 14 * 24 * 60 * 60 };
pub(crate) const RUNTIME_USAGE_SNAPSHOT_RETENTION_SECONDS: i64 =
    if cfg!(test) { 120 } else { 7 * 24 * 60 * 60 };
pub(crate) const PROD_EX_TMP_LOGIN_RETENTION_SECONDS: i64 =
    if cfg!(test) { 60 } else { 24 * 60 * 60 };
pub(crate) const ORPHAN_MANAGED_PROFILE_AUDIT_RETENTION_SECONDS: i64 =
    if cfg!(test) { 60 } else { 7 * 24 * 60 * 60 };
pub(crate) const RUNTIME_PROXY_LOG_RETENTION_SECONDS: i64 =
    if cfg!(test) { 120 } else { 7 * 24 * 60 * 60 };
pub(crate) const RUNTIME_PROXY_LOG_RETENTION_COUNT: usize = if cfg!(test) { 4 } else { 40 };
pub(crate) const PRODEX_CHAT_HISTORY_RETENTION_SECONDS: i64 = 7 * 24 * 60 * 60;
pub(crate) const RUNTIME_PREVIOUS_RESPONSE_RETRY_DELAYS_MS: [u64; 3] = [75, 200, 500];

pub(crate) const RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT: usize = if cfg!(test) { 4 } else { 12 };
pub(crate) const RUNTIME_PROXY_PRECOMMIT_BUDGET_MS: u64 = if cfg!(test) { 500 } else { 3_000 };
pub(crate) const RUNTIME_PROXY_PRECOMMIT_CONTINUATION_ATTEMPT_LIMIT: usize =
    RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT * 2;
pub(crate) const RUNTIME_PROXY_PRECOMMIT_CONTINUATION_BUDGET_MS: u64 =
    RUNTIME_PROXY_PRECOMMIT_BUDGET_MS * 4;
pub(crate) const RUNTIME_PROFILE_RETRY_BACKOFF_SECONDS: i64 = if cfg!(test) { 2 } else { 20 };
pub(crate) const RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS: i64 = if cfg!(test) { 2 } else { 15 };
pub(crate) const RUNTIME_PROFILE_TRANSPORT_BACKOFF_MAX_SECONDS: i64 =
    if cfg!(test) { 8 } else { 120 };
pub(crate) const RUNTIME_PROXY_LOCAL_OVERLOAD_BACKOFF_SECONDS: i64 = if cfg!(test) { 1 } else { 3 };
pub(crate) const RUNTIME_PROXY_ADMISSION_WAIT_BUDGET_MS: u64 = if cfg!(test) { 80 } else { 750 };
pub(crate) const RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS: u64 =
    if cfg!(test) { 80 } else { 750 };
pub(crate) const RUNTIME_PROXY_PRESSURE_ADMISSION_WAIT_BUDGET_MS: u64 =
    if cfg!(test) { 25 } else { 200 };
pub(crate) const RUNTIME_PROXY_PRESSURE_LONG_LIVED_QUEUE_WAIT_BUDGET_MS: u64 =
    if cfg!(test) { 25 } else { 200 };
pub(crate) const RUNTIME_PROXY_INTERACTIVE_WAIT_MULTIPLIER: u64 = 2;
pub(crate) const RUNTIME_PROXY_PRESSURE_PRECOMMIT_BUDGET_MS: u64 =
    if cfg!(test) { 150 } else { 800 };
#[allow(dead_code)]
pub(crate) const RUNTIME_PROXY_PRESSURE_PRECOMMIT_CONTINUATION_BUDGET_MS: u64 =
    if cfg!(test) { 250 } else { 1_500 };
pub(crate) const RUNTIME_PROXY_PRESSURE_PRECOMMIT_ATTEMPT_LIMIT: usize =
    if cfg!(test) { 2 } else { 6 };
#[allow(dead_code)]
pub(crate) const RUNTIME_PROXY_PRESSURE_PRECOMMIT_CONTINUATION_ATTEMPT_LIMIT: usize =
    if cfg!(test) { 4 } else { 8 };
pub(crate) const RUNTIME_PROXY_COMPACT_OWNER_RETRY_DELAY_MS: u64 = if cfg!(test) { 5 } else { 150 };
pub(crate) const RUNTIME_PROXY_SYNC_PROBE_PRESSURE_PAUSE_MS: u64 = 5;
pub(crate) const RUNTIME_PROFILE_INFLIGHT_SOFT_LIMIT: usize = if cfg!(test) { 1 } else { 4 };
pub(crate) const RUNTIME_PROFILE_INFLIGHT_HARD_LIMIT: usize = if cfg!(test) { 2 } else { 8 };
pub(crate) const RUNTIME_PROFILE_HEALTH_DECAY_SECONDS: i64 = if cfg!(test) { 2 } else { 60 };
pub(crate) const RUNTIME_PROFILE_USAGE_CACHE_FRESH_SECONDS: i64 = if cfg!(test) { 30 } else { 300 };
pub(crate) const UPDATE_CHECK_CACHE_TTL_SECONDS: i64 = if cfg!(test) { 5 } else { 21_600 };
pub(crate) const UPDATE_CHECK_STALE_CURRENT_TTL_SECONDS: i64 = if cfg!(test) { 1 } else { 300 };
pub(crate) const UPDATE_CHECK_HTTP_CONNECT_TIMEOUT_MS: u64 = if cfg!(test) { 200 } else { 800 };
pub(crate) const UPDATE_CHECK_HTTP_READ_TIMEOUT_MS: u64 = if cfg!(test) { 400 } else { 1200 };
pub(crate) const RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS: i64 =
    if cfg!(test) { 300 } else { 1800 };
pub(crate) const RUNTIME_PROFILE_QUOTA_QUARANTINE_FALLBACK_SECONDS: i64 =
    if cfg!(test) { 10 } else { 300 };
pub(crate) const RUNTIME_PROFILE_SYNC_PROBE_FALLBACK_LIMIT: usize = 3;
pub(crate) const RUNTIME_STARTUP_PROBE_WARM_LIMIT: usize = 3;
pub(crate) const RUNTIME_STARTUP_SYNC_PROBE_WARM_LIMIT: usize = if cfg!(test) {
    RUNTIME_STARTUP_PROBE_WARM_LIMIT
} else {
    1
};
pub(crate) const RUNTIME_STATE_SAVE_DEBOUNCE_MS: u64 = if cfg!(test) { 5 } else { 150 };
pub(crate) const RUNTIME_STATE_SAVE_QUEUE_PRESSURE_THRESHOLD: usize = 8;
pub(crate) const RUNTIME_CONTINUATION_JOURNAL_QUEUE_PRESSURE_THRESHOLD: usize = 8;
pub(crate) const RUNTIME_PROBE_REFRESH_QUEUE_PRESSURE_THRESHOLD: usize = 16;
pub(crate) const RUNTIME_PROFILE_AUTH_FAILURE_DECAY_SECONDS: i64 = if cfg!(test) { 5 } else { 300 };
pub(crate) const RUNTIME_PROFILE_AUTH_FAILURE_401_SCORE: u32 = if cfg!(test) { 3 } else { 12 };
pub(crate) const RUNTIME_PROFILE_AUTH_FAILURE_403_SCORE: u32 = if cfg!(test) { 1 } else { 2 };
pub(crate) const RUNTIME_BINDING_TOUCH_PERSIST_INTERVAL_SECONDS: i64 =
    if cfg!(test) { 1 } else { 60 };
pub(crate) const RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS: i64 = if cfg!(test) { 4 } else { 180 };
pub(crate) const RUNTIME_PROFILE_SUCCESS_STREAK_DECAY_SECONDS: i64 =
    if cfg!(test) { 8 } else { 300 };
pub(crate) const RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS: i64 = if cfg!(test) { 8 } else { 300 };
pub(crate) const RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY: u32 = 4;
pub(crate) const RUNTIME_PROFILE_CONNECT_FAILURE_HEALTH_PENALTY: u32 = 5;
pub(crate) const RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY: u32 = 2;
pub(crate) const RUNTIME_PROFILE_LATENCY_PENALTY_MAX: u32 = 12;
pub(crate) const RUNTIME_PROFILE_HEALTH_SUCCESS_RECOVERY_SCORE: u32 = 2;
pub(crate) const RUNTIME_PROFILE_BAD_PAIRING_PENALTY: u32 = 2;
pub(crate) const RUNTIME_PROFILE_HEALTH_MAX_SCORE: u32 = 16;
pub(crate) const RUNTIME_PROFILE_SUCCESS_STREAK_MAX: u32 = 3;
pub(crate) const QUOTA_HTTP_CONNECT_TIMEOUT_MS: u64 = if cfg!(test) { 250 } else { 5_000 };
pub(crate) const QUOTA_HTTP_READ_TIMEOUT_MS: u64 = if cfg!(test) { 500 } else { 10_000 };
// Match Codex's default Responses stream idle timeout so the local proxy stays transport-transparent.
pub(crate) const RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS: u64 = if cfg!(test) { 250 } else { 300_000 };
pub(crate) const RUNTIME_PROXY_HTTP_CONNECT_TIMEOUT_MS: u64 = if cfg!(test) { 250 } else { 5_000 };
pub(crate) const RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS: u64 =
    if cfg!(test) { 250 } else { 15_000 };
pub(crate) const RUNTIME_PROXY_WEBSOCKET_HAPPY_EYEBALLS_DELAY_MS: u64 =
    if cfg!(test) { 10 } else { 200 };
pub(crate) const RUNTIME_PROXY_WEBSOCKET_PRECOMMIT_PROGRESS_TIMEOUT_MS: u64 =
    if cfg!(test) { 120 } else { 8_000 };
pub(crate) const RUNTIME_PROXY_WEBSOCKET_PREVIOUS_RESPONSE_REUSE_STALE_MS: u64 = 60_000;
pub(crate) const RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS: u64 = if cfg!(test) { 50 } else { 1_000 };
pub(crate) const RUNTIME_PROXY_SSE_LOOKAHEAD_BYTES: usize = 8 * 1024;
pub(crate) const RUNTIME_PROXY_PREFETCH_QUEUE_CAPACITY: usize = 2;
pub(crate) const RUNTIME_PROXY_PREFETCH_MAX_CHUNK_BYTES: usize = 512 * 1024;
pub(crate) const RUNTIME_PROXY_PREFETCH_MAX_BUFFERED_BYTES: usize = 768 * 1024;
pub(crate) const RUNTIME_PROXY_PREFETCH_BACKPRESSURE_RETRY_MS: u64 =
    if cfg!(test) { 2 } else { 10 };
pub(crate) const RUNTIME_PROXY_PREFETCH_BACKPRESSURE_TIMEOUT_MS: u64 =
    if cfg!(test) { 40 } else { 1_000 };
pub(crate) const RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const RUNTIME_PROXY_COMPACT_BUFFERED_RESPONSE_MAX_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const RUNTIME_PROXY_HEAP_TRIM_MIN_RELEASE_BYTES: usize =
    if cfg!(test) { 1024 } else { 1024 * 1024 };
pub(crate) const RUNTIME_PROXY_HEAP_TRIM_MIN_INTERVAL_MS: u64 = if cfg!(test) { 0 } else { 2_000 };
pub(crate) const RUNTIME_PROXY_ANTHROPIC_WEB_SEARCH_FOLLOWUP_LIMIT: usize =
    if cfg!(test) { 2 } else { 4 };
pub(crate) const RUNTIME_PROXY_LOG_FILE_PREFIX: &str = "prodex-runtime";
pub(crate) const RUNTIME_PROXY_LATEST_LOG_POINTER: &str = "prodex-runtime-latest.path";
pub(crate) const RUNTIME_PROXY_DOCTOR_TAIL_BYTES: usize = 128 * 1024;
pub(crate) const PRODEX_SECRET_BACKEND_ENV: &str = "PRODEX_SECRET_BACKEND";
pub(crate) const PRODEX_SECRET_KEYRING_SERVICE_ENV: &str = "PRODEX_SECRET_KEYRING_SERVICE";
pub(crate) const CODEX_REFRESH_TOKEN_URL_OVERRIDE_ENV: &str = "CODEX_REFRESH_TOKEN_URL_OVERRIDE";
pub(crate) const INFO_RUNTIME_LOG_TAIL_BYTES: usize =
    if cfg!(test) { 64 * 1024 } else { 512 * 1024 };
pub(crate) const INFO_FORECAST_LOOKBACK_SECONDS: i64 = if cfg!(test) { 3_600 } else { 3 * 60 * 60 };
pub(crate) const INFO_FORECAST_MIN_SPAN_SECONDS: i64 = if cfg!(test) { 60 } else { 5 * 60 };
pub(crate) const INFO_RECENT_LOAD_WINDOW_SECONDS: i64 = if cfg!(test) { 600 } else { 30 * 60 };
pub(crate) const LAST_GOOD_FILE_SUFFIX: &str = ".last-good";
pub(crate) const RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_SECONDS: i64 =
    if cfg!(test) { 5 } else { 180 };
pub(crate) const RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD: u32 = 2;
pub(crate) const RUNTIME_CONTINUATION_SUSPECT_GRACE_SECONDS: i64 = if cfg!(test) { 5 } else { 120 };
pub(crate) const RUNTIME_CONTINUATION_DEAD_GRACE_SECONDS: i64 = if cfg!(test) { 5 } else { 900 };
pub(crate) const RUNTIME_CONTINUATION_VERIFIED_STALE_SECONDS: i64 =
    if cfg!(test) { 10 } else { 1_800 };
pub(crate) const RUNTIME_CONTINUATION_SUSPECT_NOT_FOUND_STREAK_LIMIT: u32 = 2;
pub(crate) const RUNTIME_CONTINUATION_CONFIDENCE_MAX: u32 = 8;
pub(crate) const RUNTIME_CONTINUATION_VERIFIED_CONFIDENCE_BONUS: u32 = 2;
pub(crate) const RUNTIME_CONTINUATION_TOUCH_CONFIDENCE_BONUS: u32 = 1;
pub(crate) const RUNTIME_CONTINUATION_SUSPECT_CONFIDENCE_PENALTY: u32 = 1;
pub(crate) const RUNTIME_SIDECAR_STALE_SAVE_RETRY_LIMIT: usize = if cfg!(test) { 3 } else { 6 };
pub(crate) const RUNTIME_BROKER_READY_TIMEOUT_MS: u64 = if cfg!(test) { 3_000 } else { 15_000 };
pub(crate) const RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS: u64 = if cfg!(test) { 250 } else { 750 };
pub(crate) const RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS: u64 = if cfg!(test) { 400 } else { 1_500 };
pub(crate) const RUNTIME_BROKER_POLL_INTERVAL_MS: u64 = if cfg!(test) { 25 } else { 100 };
pub(crate) const RUNTIME_BROKER_LEASE_SCAN_INTERVAL_MS: u64 = if cfg!(test) { 125 } else { 1_000 };
pub(crate) const RUNTIME_BROKER_IDLE_GRACE_SECONDS: i64 = if cfg!(test) { 1 } else { 5 };
pub(crate) const CLI_WIDTH: usize = 110;
pub(crate) const CLI_MIN_WIDTH: usize = 60;
pub(crate) const CLI_LABEL_WIDTH: usize = 16;
pub(crate) const CLI_MIN_LABEL_WIDTH: usize = 10;
pub(crate) const CLI_MAX_LABEL_WIDTH: usize = 24;
pub(crate) const CLI_TABLE_GAP: &str = "  ";
pub(crate) const CLI_TOP_LEVEL_AFTER_HELP: &str = "\
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
pub(crate) const CLI_PROFILE_AFTER_HELP: &str = "\
Examples:
  prodex profile list
  prodex profile add main --activate
  prodex profile export
  prodex profile export backup.json
  prodex profile import backup.json
  prodex profile import copilot
  prodex profile import copilot --name copilot-main --activate
  prodex profile import-current main
  prodex profile remove main
  prodex profile remove --all";
pub(crate) const CLI_LOGIN_AFTER_HELP: &str = "\
Examples:
  prodex login
  prodex login --profile main
  prodex login --device-auth";
pub(crate) const CLI_QUOTA_AFTER_HELP: &str = "\
Best practice:
  Use `prodex quota --all --detail` for the clearest live quota view across profiles.

Examples:
  prodex quota
  prodex quota --profile main --detail
  prodex quota --all --detail
  prodex quota --all --once
  prodex quota --raw --profile main";
pub(crate) const CLI_RUN_AFTER_HELP: &str = "\
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
pub(crate) const CLI_CLAUDE_AFTER_HELP: &str = "\
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
pub(crate) const CLI_CAVEMAN_AFTER_HELP: &str = "\
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
pub(crate) const CLI_DOCTOR_AFTER_HELP: &str = "\
Examples:
  prodex doctor
  prodex doctor --quota
  prodex doctor --runtime
  prodex doctor --runtime --json";
pub(crate) const CLI_AUDIT_AFTER_HELP: &str = "\
Examples:
  prodex audit
  prodex audit --tail 50
  prodex audit --component profile --action use
  prodex audit --json";
pub(crate) const CLI_CLEANUP_AFTER_HELP: &str = "\
Examples:
  prodex cleanup

Notes:
  Removes stale local artifacts and prunes Codex/Claude chat history older than one week.";
pub(crate) const SHARED_CODEX_DIR_NAMES: &[&str] = &[
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
pub(crate) const SHARED_CODEX_FILE_NAMES: &[&str] = &["history.jsonl", "config.toml"];
pub(crate) const SHARED_CODEX_SQLITE_PREFIXES: &[&str] = &["state_", "logs_"];
pub(crate) const SHARED_CODEX_SQLITE_SUFFIXES: &[&str] = &[".sqlite", ".sqlite-shm", ".sqlite-wal"];
pub(crate) static STATE_SAVE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub(crate) static RUNTIME_PROXY_LOG_SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub(crate) static RUNTIME_STATE_SAVE_QUEUE: OnceLock<Arc<RuntimeStateSaveQueue>> = OnceLock::new();
pub(crate) static RUNTIME_CONTINUATION_JOURNAL_SAVE_QUEUE: OnceLock<
    Arc<RuntimeContinuationJournalSaveQueue>,
> = OnceLock::new();
pub(crate) static RUNTIME_PROBE_REFRESH_QUEUE: OnceLock<Arc<RuntimeProbeRefreshQueue>> =
    OnceLock::new();
pub(crate) static RUNTIME_PERSISTENCE_MODE_BY_LOG_PATH: OnceLock<Mutex<BTreeMap<PathBuf, bool>>> =
    OnceLock::new();
pub(crate) static RUNTIME_BROKER_METADATA_BY_LOG_PATH: OnceLock<
    Mutex<BTreeMap<PathBuf, RuntimeBrokerMetadata>>,
> = OnceLock::new();
