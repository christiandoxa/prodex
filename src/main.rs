use anyhow::{Context, Result, bail};
use base64::Engine;
use chrono::{Local, TimeZone};
use clap::{Args, Parser, Subcommand};
use dirs::home_dir;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Cursor, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tiny_http::{
    Header as TinyHeader, ReadWrite as TinyReadWrite, Response as TinyResponse,
    Server as TinyServer, StatusCode as TinyStatusCode,
};
use tokio::runtime::{Builder as TokioRuntimeBuilder, Runtime as TokioRuntime};
use tungstenite::client::IntoClientRequest;
#[cfg(test)]
use tungstenite::connect as ws_connect;
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
const RUNTIME_PREVIOUS_RESPONSE_RETRY_DELAYS_MS: [u64; 3] = [75, 200, 500];
const RUNTIME_PROFILE_REPORT_CACHE_TTL_SECONDS: i64 = if cfg!(test) { 60 } else { 30 };
const RUNTIME_PROFILE_RETRY_BACKOFF_SECONDS: i64 = if cfg!(test) { 2 } else { 20 };
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
const CLI_WIDTH: usize = 110;
const CLI_LABEL_WIDTH: usize = 16;
const CLI_TABLE_GAP: &str = "  ";
const SHARED_CODEX_DIR_NAMES: &[&str] = &["sessions", "archived_sessions", "shell_snapshots"];
const SHARED_CODEX_FILE_NAMES: &[&str] = &["history.jsonl"];
const SHARED_CODEX_SQLITE_PREFIXES: &[&str] = &["state_", "logs_"];
const SHARED_CODEX_SQLITE_SUFFIXES: &[&str] = &[".sqlite", ".sqlite-shm", ".sqlite-wal"];
static STATE_SAVE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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
    #[arg(long)]
    watch: bool,
    #[arg(long)]
    base_url: Option<String>,
}

#[derive(Args, Debug)]
struct DoctorArgs {
    #[arg(long)]
    quota: bool,
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

#[derive(Debug, Clone)]
struct RuntimeProxyRequest {
    method: String,
    path_and_query: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

#[derive(Debug, Clone)]
struct RuntimeRotationProxyShared {
    client: Client,
    async_client: reqwest::Client,
    async_runtime: Arc<TokioRuntime>,
    runtime: Arc<Mutex<RuntimeRotationState>>,
    log_path: PathBuf,
    request_sequence: Arc<AtomicU64>,
}

#[derive(Debug, Clone)]
struct RuntimeRotationState {
    paths: AppPaths,
    state: AppState,
    upstream_base_url: String,
    include_code_review: bool,
    current_profile: String,
    turn_state_bindings: BTreeMap<String, ResponseProfileBinding>,
    profile_probe_cache: BTreeMap<String, RuntimeProfileProbeCacheEntry>,
    profile_retry_backoff_until: BTreeMap<String, i64>,
}

#[derive(Debug, Clone)]
struct RuntimeProfileProbeCacheEntry {
    checked_at: i64,
    auth: AuthSummary,
    result: std::result::Result<UsageResponse, String>,
}

struct RuntimeRotationProxy {
    server: Arc<TinyServer>,
    shutdown: Arc<AtomicBool>,
    worker_threads: Vec<thread::JoinHandle<()>>,
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
    let prefix = format!("[ {title} ] ");
    let width = text_width(&prefix);
    if width >= CLI_WIDTH {
        return prefix;
    }

    format!("{prefix}{}", "=".repeat(CLI_WIDTH - width))
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

fn format_field_lines(label: &str, value: &str) -> Vec<String> {
    let label = format!("{label}:");
    let value_width = CLI_WIDTH.saturating_sub(CLI_LABEL_WIDTH + 1).max(1);
    let wrapped = wrap_text(value, value_width);
    let mut lines = Vec::new();

    for (index, line) in wrapped.into_iter().enumerate() {
        let field_label = if index == 0 { label.as_str() } else { "" };
        lines.push(format!(
            "{field_label:<label_w$} {line}",
            label_w = CLI_LABEL_WIDTH
        ));
    }

    lines
}

fn print_panel(title: &str, fields: &[(String, String)]) {
    println!("{}", section_header(title));
    for (label, value) in fields {
        for line in format_field_lines(label, value) {
            println!("{line}");
        }
    }
}

fn render_panel(title: &str, fields: &[(String, String)]) -> String {
    let mut lines = vec![section_header(title)];
    for (label, value) in fields {
        lines.extend(format_field_lines(label, value));
    }
    lines.join("\n")
}

fn print_wrapped_stderr(message: &str) {
    for line in wrap_text(message, CLI_WIDTH) {
        eprintln!("{line}");
    }
}

fn timeout_override_ms(env_key: &str, default_ms: u64) -> u64 {
    env::var(env_key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default_ms)
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
    if args.all && args.watch {
        bail!("--all cannot be combined with --watch");
    }

    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;

    if args.all {
        if state.profiles.is_empty() {
            bail!("no profiles configured");
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

    if args.watch {
        return watch_quota(&profile_name, &codex_home, args.base_url.as_deref());
    }

    let usage = fetch_usage(&codex_home, args.base_url.as_deref())?;
    println!("{}", render_profile_quota(&profile_name, &usage));
    Ok(())
}

fn handle_run(args: RunArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;
    let profile_name = resolve_profile_name(&state, args.profile.as_deref())?;
    let mut selected_profile_name = profile_name.clone();
    let explicit_profile_requested = args.profile.is_some();
    let allow_auto_rotate = !args.no_auto_rotate;
    let include_code_review = is_review_invocation(&args.codex_args);
    let mut codex_home = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?
        .codex_home
        .clone();

    if !args.skip_quota_check {
        if allow_auto_rotate && !explicit_profile_requested && state.profiles.len() > 1 {
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
        .map(|proxy| runtime_proxy_codex_args(proxy.listen_addr, &args.codex_args))
        .unwrap_or_else(|| args.codex_args.clone());

    let status = run_child(&codex_bin(), &runtime_args, &codex_home)?;
    exit_with_status(status)
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
            .worker_threads(1)
            .enable_all()
            .build()
            .context("failed to build runtime auto-rotate async runtime")?,
    );
    let shared = RuntimeRotationProxyShared {
        client: Client::builder()
            .connect_timeout(Duration::from_millis(
                runtime_proxy_http_connect_timeout_ms(),
            ))
            .build()
            .context("failed to build runtime auto-rotate HTTP client")?,
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
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths: paths.clone(),
            state: state.clone(),
            upstream_base_url: upstream_base_url.clone(),
            include_code_review,
            current_profile: current_profile.to_string(),
            turn_state_bindings: BTreeMap::new(),
            profile_probe_cache: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
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
    let worker_count = state.profiles.len().clamp(2, 4);

    for _ in 0..worker_count {
        let server: Arc<TinyServer> = Arc::clone(&server);
        let shutdown = Arc::clone(&shutdown);
        let shared = shared.clone();
        worker_threads.push(thread::spawn(move || {
            while !shutdown.load(Ordering::SeqCst) {
                match server.recv_timeout(Duration::from_millis(200)) {
                    Ok(Some(request)) => handle_runtime_rotation_proxy_request(request, &shared),
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
        listen_addr,
    })
}

impl Drop for RuntimeRotationProxy {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        for _ in 0..self.worker_threads.len() {
            self.server.unblock();
        }
        for worker in self.worker_threads.drain(..) {
            let _ = worker.join();
        }
    }
}

fn handle_runtime_rotation_proxy_request(
    mut request: tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
) {
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

    let response = match (|| -> Result<tiny_http::ResponseBox> {
        let profile_name = runtime_proxy_current_profile(shared)?;
        proxy_runtime_standard_request(request_id, &captured, shared, &profile_name)
    })() {
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
        runtime.state.save(&runtime.paths).with_context(|| {
            format!(
                "failed to persist runtime response affinity bindings for '{}'",
                profile_name
            )
        })?;
        runtime_proxy_log(
            shared,
            format!(
                "binding response_ids profile={profile_name} count={} first={:?}",
                response_ids.len(),
                response_ids.first()
            ),
        );
    }
    Ok(())
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

fn select_runtime_response_candidate(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    pinned_profile: Option<&str>,
    turn_state_profile: Option<&str>,
    discover_previous_response_owner: bool,
) -> Result<Option<String>> {
    if let Some(profile_name) = pinned_profile.filter(|name| !excluded_profiles.contains(*name)) {
        return Ok(Some(profile_name.to_string()));
    }

    if let Some(profile_name) = turn_state_profile.filter(|name| !excluded_profiles.contains(*name))
    {
        return Ok(Some(profile_name.to_string()));
    }

    if discover_previous_response_owner {
        return next_runtime_previous_response_candidate(shared, excluded_profiles);
    }

    if let Some(profile_name) =
        runtime_proxy_optimistic_current_candidate(shared, excluded_profiles)?
    {
        return Ok(Some(profile_name));
    }

    next_runtime_response_candidate(shared, excluded_profiles)
}

fn next_runtime_previous_response_candidate(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
) -> Result<Option<String>> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;

    Ok(
        active_profile_selection_order(&runtime.state, &runtime.current_profile)
            .into_iter()
            .find(|name| {
                !excluded_profiles.contains(name)
                    && runtime.state.profiles.get(name).is_some_and(|profile| {
                        read_auth_summary(&profile.codex_home).quota_compatible
                    })
            }),
    )
}

fn runtime_proxy_optimistic_current_candidate(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
) -> Result<Option<String>> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    prune_runtime_profile_retry_backoff(&mut runtime, now);

    if excluded_profiles.contains(&runtime.current_profile) {
        return Ok(None);
    }

    let Some(profile) = runtime.state.profiles.get(&runtime.current_profile) else {
        return Ok(None);
    };
    if !read_auth_summary(&profile.codex_home).quota_compatible {
        return Ok(None);
    }
    if runtime_profile_in_retry_backoff(&runtime, &runtime.current_profile, now) {
        return Ok(None);
    }

    Ok(Some(runtime.current_profile.clone()))
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
        let _ = local_socket.close(None);
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
    ) {
        self.upstream_socket = Some(socket);
        self.profile_name = Some(profile_name.to_string());
        self.turn_state = turn_state;
    }

    fn reset(&mut self) {
        self.upstream_socket = None;
        self.profile_name = None;
        self.turn_state = None;
    }

    fn close(&mut self) {
        if let Some(mut socket) = self.upstream_socket.take() {
            let _ = socket.close(None);
        }
        self.profile_name = None;
        self.turn_state = None;
    }
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
    let bound_profile = previous_response_id
        .as_deref()
        .map(|response_id| runtime_response_bound_profile(shared, response_id))
        .transpose()?
        .flatten();
    let mut turn_state_profile = request_turn_state
        .as_deref()
        .map(|value| runtime_turn_state_bound_profile(shared, value))
        .transpose()?
        .flatten();
    let session_profile = if bound_profile.is_none() && turn_state_profile.is_none() {
        websocket_session.profile_name.clone()
    } else {
        None
    };
    let mut pinned_profile = bound_profile.clone().or(session_profile).or_else(|| {
        previous_response_id
            .as_ref()
            .and_then(|_| runtime_proxy_current_profile(shared).ok())
    });
    let mut excluded_profiles = BTreeSet::new();
    let mut last_failure = None;
    let mut previous_response_retry_candidate: Option<String> = None;
    let mut previous_response_retry_index = 0usize;
    let mut candidate_turn_state_retry_profile: Option<String> = None;
    let mut candidate_turn_state_retry_value: Option<String> = None;

    loop {
        let Some(candidate_name) = select_runtime_response_candidate(
            shared,
            &excluded_profiles,
            pinned_profile.as_deref(),
            turn_state_profile.as_deref(),
            previous_response_id.is_some(),
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
            match last_failure {
                Some(RuntimeUpstreamFailureResponse::Websocket(payload)) => {
                    forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                }
                _ => {
                    send_runtime_proxy_websocket_error(
                        local_socket,
                        429,
                        "insufficient_quota",
                        "All runtime auto-rotate candidates are currently blocked.",
                    )?;
                }
            }
            return Ok(());
        };
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
                if bound_profile.as_deref() == Some(profile_name.as_str()) {
                    forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                    return Ok(());
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
                last_failure = Some(RuntimeUpstreamFailureResponse::Websocket(payload));
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
                    thread::sleep(delay);
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
    let (mut upstream_socket, mut upstream_turn_state) = if websocket_session
        .can_reuse(profile_name, turn_state_override)
    {
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
            RuntimeWebsocketConnectResult::Connected { socket, turn_state } => (socket, turn_state),
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
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket upstream_send_error profile={profile_name} error={err}"
            ),
        );
        return Err(anyhow::anyhow!(
            "failed to send runtime websocket request upstream: {err}"
        ));
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
                    remember_runtime_turn_state(
                        shared,
                        profile_name,
                        upstream_turn_state.as_deref(),
                    )?;
                    commit_runtime_proxy_profile_selection_with_notice(shared, profile_name)?;
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
                    websocket_session.store(upstream_socket, profile_name, upstream_turn_state);
                    return Ok(RuntimeWebsocketAttempt::Delivered);
                }
            }
            Ok(WsMessage::Binary(payload)) => {
                if !committed {
                    remember_runtime_turn_state(
                        shared,
                        profile_name,
                        upstream_turn_state.as_deref(),
                    )?;
                    commit_runtime_proxy_profile_selection_with_notice(shared, profile_name)?;
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
                let _ = local_socket.close(frame);
                bail!("runtime websocket upstream closed before response.completed");
            }
            Err(WsError::ConnectionClosed) | Err(WsError::AlreadyClosed) => {
                websocket_session.reset();
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=websocket upstream_connection_closed profile={profile_name}"
                    ),
                );
                bail!("runtime websocket upstream closed before response.completed");
            }
            Err(err) => {
                websocket_session.reset();
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=websocket upstream_read_error profile={profile_name} error={err}"
                    ),
                );
                return Err(anyhow::anyhow!(
                    "runtime websocket upstream failed before response.completed: {err}"
                ));
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
    request.headers_mut().insert(
        WsHeaderName::from_static("user-agent"),
        WsHeaderValue::from_static("codex-cli"),
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
            Err(anyhow::anyhow!(
                "failed to connect runtime websocket upstream: {err}"
            ))
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
    profile_name: &str,
) -> Result<tiny_http::ResponseBox> {
    let response =
        send_runtime_proxy_upstream_request(request_id, request, shared, profile_name, None)?;
    forward_runtime_proxy_response(response, Vec::new())
}

fn proxy_runtime_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<RuntimeResponsesReply> {
    let previous_response_id = runtime_request_previous_response_id(request);
    let request_turn_state = runtime_request_turn_state(request);
    let bound_profile = previous_response_id
        .as_deref()
        .map(|response_id| runtime_response_bound_profile(shared, response_id))
        .transpose()?
        .flatten();
    let mut turn_state_profile = request_turn_state
        .as_deref()
        .map(|value| runtime_turn_state_bound_profile(shared, value))
        .transpose()?
        .flatten();
    let mut pinned_profile = bound_profile.clone().or_else(|| {
        previous_response_id
            .as_ref()
            .and_then(|_| runtime_proxy_current_profile(shared).ok())
    });
    let mut excluded_profiles = BTreeSet::new();
    let mut last_failure = None;
    let mut previous_response_retry_candidate: Option<String> = None;
    let mut previous_response_retry_index = 0usize;
    let mut candidate_turn_state_retry_profile: Option<String> = None;
    let mut candidate_turn_state_retry_value: Option<String> = None;

    loop {
        let Some(candidate_name) = select_runtime_response_candidate(
            shared,
            &excluded_profiles,
            pinned_profile.as_deref(),
            turn_state_profile.as_deref(),
            previous_response_id.is_some(),
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
            return Ok(match last_failure {
                Some(RuntimeUpstreamFailureResponse::Http(response)) => response,
                _ => RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_response(
                    429,
                    "insufficient_quota",
                    "All runtime auto-rotate candidates are currently blocked.",
                )),
            });
        };
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
                commit_runtime_proxy_profile_selection_with_notice(shared, &profile_name)?;
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
                if bound_profile.as_deref() == Some(profile_name.as_str()) {
                    return Ok(response);
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
                    thread::sleep(delay);
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
    let response = send_runtime_proxy_upstream_responses_request(
        request_id,
        request,
        shared,
        profile_name,
        turn_state_override,
    )?;
    let response_turn_state = runtime_proxy_header_value(response.headers(), "x-codex-turn-state");
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let parts = buffer_runtime_proxy_async_response_parts(shared, response, Vec::new())?;
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
    prepare_runtime_proxy_responses_success(request_id, response, shared, profile_name)
}

fn next_runtime_response_candidate(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
) -> Result<Option<String>> {
    let now = Local::now().timestamp();
    let (state, current_profile, include_code_review, upstream_base_url, cached_reports) = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_retry_backoff(&mut runtime, now);
        (
            runtime.state.clone(),
            runtime.current_profile.clone(),
            runtime.include_code_review,
            runtime.upstream_base_url.clone(),
            runtime.profile_probe_cache.clone(),
        )
    };

    let mut reports = Vec::new();
    let mut probe_jobs = Vec::new();
    for (order_index, name) in active_profile_selection_order(&state, &current_profile)
        .into_iter()
        .enumerate()
    {
        if excluded_profiles.contains(&name) {
            continue;
        }
        if let Some(entry) = cached_reports.get(&name)
            && runtime_profile_probe_cache_is_fresh(entry, now)
        {
            reports.push(RunProfileProbeReport {
                name,
                order_index,
                auth: entry.auth.clone(),
                result: entry.result.clone(),
            });
            continue;
        }

        let Some(profile) = state.profiles.get(&name) else {
            continue;
        };
        probe_jobs.push(RunProfileProbeJob {
            name,
            order_index,
            codex_home: profile.codex_home.clone(),
        });
    }

    if !probe_jobs.is_empty() {
        let base_url = Some(upstream_base_url.clone());
        let fresh_reports = map_parallel(probe_jobs, |job| {
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
        for report in &fresh_reports {
            runtime.profile_probe_cache.insert(
                report.name.clone(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: report.auth.clone(),
                    result: report.result.clone(),
                },
            );
        }
        drop(runtime);

        reports.extend(fresh_reports);
    }

    reports.sort_by_key(|report| report.order_index);
    let candidates = ready_profile_candidates(
        &reports,
        include_code_review,
        Some(current_profile.as_str()),
        &state,
    );

    Ok(candidates
        .into_iter()
        .map(|candidate| candidate.name)
        .find(|name| !excluded_profiles.contains(name)))
}

fn runtime_profile_probe_cache_is_fresh(entry: &RuntimeProfileProbeCacheEntry, now: i64) -> bool {
    now.saturating_sub(entry.checked_at) < RUNTIME_PROFILE_REPORT_CACHE_TTL_SECONDS
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

fn prune_runtime_profile_retry_backoff(runtime: &mut RuntimeRotationState, now: i64) {
    runtime
        .profile_retry_backoff_until
        .retain(|_, until| *until > now);
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
    prune_runtime_profile_retry_backoff(&mut runtime, now);
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

fn commit_runtime_proxy_profile_selection(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<bool> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let switched = runtime.current_profile != profile_name;
    runtime.profile_retry_backoff_until.remove(profile_name);
    runtime.current_profile = profile_name.to_string();
    runtime.state.active_profile = Some(profile_name.to_string());
    record_run_selection(&mut runtime.state, profile_name);
    runtime.state.save(&runtime.paths).with_context(|| {
        format!("failed to save runtime auto-rotate state for '{profile_name}'")
    })?;
    runtime_proxy_log(
        shared,
        format!("profile_commit profile={profile_name} switched={switched}"),
    );
    Ok(switched)
}

fn commit_runtime_proxy_profile_selection_with_notice(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<()> {
    let _ = commit_runtime_proxy_profile_selection(shared, profile_name)?;
    Ok(())
}

fn send_runtime_proxy_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
) -> Result<reqwest::blocking::Response> {
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

    let mut upstream_request = shared.client.request(method, &upstream_url);
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
        .header("User-Agent", "codex-cli")
        .body(request.body.clone());

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
    let response = upstream_request.send().with_context(|| {
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
        .header("User-Agent", "codex-cli")
        .body(request.body.clone());

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
    response: reqwest::blocking::Response,
    prelude: Vec<u8>,
) -> Result<tiny_http::ResponseBox> {
    let status = TinyStatusCode(response.status().as_u16());
    let mut headers = Vec::new();
    for (name, value) in response.headers() {
        if should_skip_runtime_response_header(name.as_str()) {
            continue;
        }
        if let Ok(header) = TinyHeader::from_bytes(name.as_str(), value.as_bytes()) {
            headers.push(header);
        }
    }

    let content_length = response
        .content_length()
        .and_then(|length| usize::try_from(length).ok())
        .and_then(|length| length.checked_add(prelude.len()));
    let reader: Box<dyn Read + Send> = if prelude.is_empty() {
        Box::new(response)
    } else {
        Box::new(Cursor::new(prelude).chain(response))
    };

    Ok(TinyResponse::new(status, headers, reader, content_length, None).boxed())
}

fn prepare_runtime_proxy_responses_success(
    request_id: u64,
    response: reqwest::Response,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<RuntimeResponsesAttempt> {
    let turn_state = runtime_proxy_header_value(response.headers(), "x-codex-turn-state");
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
        let read = self.inner.read(buf)?;
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
        let (sender, receiver) = mpsc::sync_channel::<RuntimePrefetchChunk>(8);
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
    sender: SyncSender<RuntimePrefetchChunk>,
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
        .or_else(|| {
            let text = String::from_utf8_lossy(body).trim().to_string();
            (!text.is_empty()).then_some(text)
        })
}

fn extract_runtime_proxy_previous_response_message(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| extract_runtime_proxy_previous_response_message_from_value(&value))
}

fn extract_runtime_proxy_quota_message_from_value(value: &serde_json::Value) -> Option<String> {
    let direct_error = value.get("error");
    let response_error = value
        .get("response")
        .and_then(|response| response.get("error"));
    for error in [direct_error, response_error].into_iter().flatten() {
        let code = error.get("code").and_then(serde_json::Value::as_str)?;
        if !matches!(code, "insufficient_quota" | "rate_limit_exceeded") {
            continue;
        }
        return Some(
            error
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("Upstream Codex account quota was exhausted.")
                .to_string(),
        );
    }
    None
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
) -> Vec<ReadyProfileCandidate> {
    let candidates = reports
        .iter()
        .filter_map(|report| {
            if !report.auth.quota_compatible {
                return None;
            }

            let usage = report.result.as_ref().ok()?;
            if !collect_blocked_limits(usage, include_code_review).is_empty() {
                return None;
            }

            Some(ReadyProfileCandidate {
                name: report.name.clone(),
                usage: usage.clone(),
                order_index: report.order_index,
                preferred: preferred_profile == Some(report.name.as_str()),
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
    let weekly = required_main_window_snapshot(&candidate.usage, "weekly");
    let five_hour = required_main_window_snapshot(&candidate.usage, "5h");

    let weekly_pressure = weekly.map_or(i64::MAX, |window| window.pressure_score);
    let five_hour_pressure = five_hour.map_or(i64::MAX, |window| window.pressure_score);
    let weekly_remaining = weekly.map_or(0, |window| window.remaining_percent);
    let five_hour_remaining = five_hour.map_or(0, |window| window.remaining_percent);

    ReadyProfileScore {
        total_pressure: weekly_pressure
            .saturating_mul(8)
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
    const PROFILE_COL_WIDTH: usize = 24;
    const CUR_COL_WIDTH: usize = 3;
    const AUTH_COL_WIDTH: usize = 7;
    const ACCOUNT_COL_WIDTH: usize = 27;
    const PLAN_COL_WIDTH: usize = 8;
    const REMAINING_COL_WIDTH: usize = 31;

    let mut rows = Vec::new();

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

        rows.push((
            report.name.clone(),
            active,
            auth,
            email,
            plan,
            main,
            status,
            resets,
        ));
    }

    println!("{}", section_header("Quota Overview"));
    let header = format!(
        "{:<name_w$}  {:<act_w$}  {:<auth_w$}  {:<email_w$}  {:<plan_w$}  {:<main_w$}",
        "PROFILE",
        "CUR",
        "AUTH",
        "ACCOUNT",
        "PLAN",
        "REMAINING",
        name_w = PROFILE_COL_WIDTH,
        act_w = CUR_COL_WIDTH,
        auth_w = AUTH_COL_WIDTH,
        email_w = ACCOUNT_COL_WIDTH,
        plan_w = PLAN_COL_WIDTH,
        main_w = REMAINING_COL_WIDTH,
    );
    println!("{header}");
    println!("{}", "-".repeat(text_width(&header)));

    for (name, active, auth, email, plan, main, status, resets) in rows {
        println!(
            "{:<name_w$}{}{:<act_w$}{}{:<auth_w$}{}{:<email_w$}{}{:<plan_w$}{}{:<main_w$}",
            fit_cell(&name, PROFILE_COL_WIDTH),
            CLI_TABLE_GAP,
            fit_cell(&active, CUR_COL_WIDTH),
            CLI_TABLE_GAP,
            fit_cell(&auth, AUTH_COL_WIDTH),
            CLI_TABLE_GAP,
            fit_cell(&email, ACCOUNT_COL_WIDTH),
            CLI_TABLE_GAP,
            fit_cell(&plan, PLAN_COL_WIDTH),
            CLI_TABLE_GAP,
            fit_cell(&main, REMAINING_COL_WIDTH),
            name_w = PROFILE_COL_WIDTH,
            act_w = CUR_COL_WIDTH,
            auth_w = AUTH_COL_WIDTH,
            email_w = ACCOUNT_COL_WIDTH,
            plan_w = PLAN_COL_WIDTH,
            main_w = REMAINING_COL_WIDTH,
        );
        for line in wrap_text(&format!("status: {status}"), CLI_WIDTH - 2) {
            println!("  {line}");
        }
        if detail {
            if let Some(resets) = resets.as_deref() {
                for line in wrap_text(resets, CLI_WIDTH - 2) {
                    println!("  {line}");
                }
            }
        }
        println!();
    }
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

fn watch_quota(profile_name: &str, codex_home: &Path, base_url: Option<&str>) -> Result<()> {
    loop {
        print!("\x1b[H\x1b[2J");
        let header = vec![
            ("Profile".to_string(), profile_name.to_string()),
            (
                "Updated".to_string(),
                Local::now().format("%Y-%m-%d %H:%M:%S %Z").to_string(),
            ),
        ];
        println!("{}", render_panel("Quota Watch", &header));
        println!();

        match fetch_usage(codex_home, base_url) {
            Ok(usage) => println!("{}", render_profile_quota(profile_name, &usage)),
            Err(err) => {
                let fields = vec![("Error".to_string(), first_line_of_error(&err.to_string()))];
                println!("{}", render_panel("Quota Watch", &fields));
            }
        }

        io::stdout()
            .flush()
            .context("failed to flush quota watch output")?;
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
        fs::create_dir_all(&paths.root)
            .with_context(|| format!("failed to create {}", paths.root.display()))?;

        let json =
            serde_json::to_string_pretty(self).context("failed to serialize prodex state")?;
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
mod tests {
    use super::*;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    fn usage_with_main_windows(
        five_hour_remaining: i64,
        five_hour_reset_offset_seconds: i64,
        weekly_remaining: i64,
        weekly_reset_offset_seconds: i64,
    ) -> UsageResponse {
        let now = Local::now().timestamp();
        UsageResponse {
            email: None,
            plan_type: None,
            rate_limit: Some(WindowPair {
                primary_window: Some(UsageWindow {
                    used_percent: Some((100 - five_hour_remaining).clamp(0, 100)),
                    reset_at: Some(now + five_hour_reset_offset_seconds),
                    limit_window_seconds: Some(18_000),
                }),
                secondary_window: Some(UsageWindow {
                    used_percent: Some((100 - weekly_remaining).clamp(0, 100)),
                    reset_at: Some(now + weekly_reset_offset_seconds),
                    limit_window_seconds: Some(604_800),
                }),
            }),
            code_review_rate_limit: None,
            additional_rate_limits: Vec::new(),
        }
    }

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Self {
            let unique = format!(
                "prodex-runtime-test-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system clock should be after unix epoch")
                    .as_nanos()
            );
            let path = std::env::temp_dir().join(unique);
            fs::create_dir_all(&path).expect("failed to create test temp dir");
            Self { path }
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    struct RuntimeProxyBackend {
        addr: SocketAddr,
        shutdown: Arc<AtomicBool>,
        responses_accounts: Arc<Mutex<Vec<String>>>,
        usage_accounts: Arc<Mutex<Vec<String>>>,
        thread: Option<JoinHandle<()>>,
    }

    #[derive(Clone, Copy)]
    enum RuntimeProxyBackendMode {
        HttpOnly,
        HttpOnlyInitialBodyStall,
        HttpOnlySlowStream,
        HttpOnlyPreviousResponseNeedsTurnState,
        Websocket,
        WebsocketPreviousResponseNeedsTurnState,
    }

    impl RuntimeProxyBackend {
        fn start() -> Self {
            Self::start_with_mode(RuntimeProxyBackendMode::HttpOnly)
        }

        fn start_http_initial_body_stall() -> Self {
            Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
        }

        fn start_http_slow_stream() -> Self {
            Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlySlowStream)
        }

        fn start_http_previous_response_needs_turn_state() -> Self {
            Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyPreviousResponseNeedsTurnState)
        }

        fn start_websocket() -> Self {
            Self::start_with_mode(RuntimeProxyBackendMode::Websocket)
        }

        fn start_websocket_previous_response_needs_turn_state() -> Self {
            Self::start_with_mode(RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState)
        }

        fn start_with_mode(mode: RuntimeProxyBackendMode) -> Self {
            let listener =
                TcpListener::bind("127.0.0.1:0").expect("failed to bind runtime proxy backend");
            let addr = listener
                .local_addr()
                .expect("failed to read runtime proxy backend address");
            listener
                .set_nonblocking(true)
                .expect("failed to set runtime proxy backend nonblocking");

            let shutdown = Arc::new(AtomicBool::new(false));
            let responses_accounts = Arc::new(Mutex::new(Vec::new()));
            let usage_accounts = Arc::new(Mutex::new(Vec::new()));
            let shutdown_flag = Arc::clone(&shutdown);
            let responses_accounts_flag = Arc::clone(&responses_accounts);
            let usage_accounts_flag = Arc::clone(&usage_accounts);
            let thread = thread::spawn(move || {
                while !shutdown_flag.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let responses_accounts_flag = Arc::clone(&responses_accounts_flag);
                            let usage_accounts_flag = Arc::clone(&usage_accounts_flag);
                            let websocket_enabled = matches!(
                                mode,
                                RuntimeProxyBackendMode::Websocket
                                    | RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState
                            );
                            thread::spawn(move || {
                                if websocket_enabled
                                    && runtime_proxy_backend_is_websocket_upgrade(&stream)
                                {
                                    handle_runtime_proxy_backend_websocket(
                                        stream,
                                        &responses_accounts_flag,
                                        mode,
                                    );
                                } else {
                                    handle_runtime_proxy_backend_request(
                                        stream,
                                        &responses_accounts_flag,
                                        &usage_accounts_flag,
                                        mode,
                                    );
                                }
                            });
                        }
                        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(_) => break,
                    }
                }
            });

            Self {
                addr,
                shutdown,
                responses_accounts,
                usage_accounts,
                thread: Some(thread),
            }
        }

        fn base_url(&self) -> String {
            format!("http://{}/backend-api", self.addr)
        }

        fn responses_accounts(&self) -> Vec<String> {
            self.responses_accounts
                .lock()
                .expect("responses_accounts poisoned")
                .clone()
        }

        fn usage_accounts(&self) -> Vec<String> {
            self.usage_accounts
                .lock()
                .expect("usage_accounts poisoned")
                .clone()
        }
    }

    impl Drop for RuntimeProxyBackend {
        fn drop(&mut self) {
            self.shutdown.store(true, Ordering::SeqCst);
            let _ = TcpStream::connect(self.addr);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    fn handle_runtime_proxy_backend_request(
        mut stream: TcpStream,
        responses_accounts: &Arc<Mutex<Vec<String>>>,
        usage_accounts: &Arc<Mutex<Vec<String>>>,
        _mode: RuntimeProxyBackendMode,
    ) {
        let request = match read_http_request(&mut stream) {
            Some(request) => request,
            None => return,
        };

        let path = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/");
        let account_id = request_header(&request, "ChatGPT-Account-Id").unwrap_or_default();
        let turn_state = request_header(&request, "x-codex-turn-state");

        let (status_line, content_type, body, response_turn_state, initial_body_stall, chunk_delay) =
            if path.ends_with("/backend-api/wham/usage") {
                usage_accounts
                    .lock()
                    .expect("usage_accounts poisoned")
                    .push(account_id.clone());
                let body = match account_id.as_str() {
                    "main-account" => runtime_proxy_usage_body("main@example.com"),
                    "second-account" => runtime_proxy_usage_body("second@example.com"),
                    "third-account" => runtime_proxy_usage_body("third@example.com"),
                    _ => serde_json::json!({ "error": "unauthorized" }).to_string(),
                };
                let status = if matches!(
                    account_id.as_str(),
                    "main-account" | "second-account" | "third-account"
                ) {
                    "HTTP/1.1 200 OK"
                } else {
                    "HTTP/1.1 401 Unauthorized"
                };
                (status, "application/json", body, None, None, None)
            } else if path.ends_with("/backend-api/codex/responses") {
                responses_accounts
                    .lock()
                    .expect("responses_accounts poisoned")
                    .push(account_id.clone());
                let previous_response_id = request_previous_response_id(&request);
                match account_id.as_str() {
                "main-account" => (
                    "HTTP/1.1 200 OK",
                    "text/event-stream",
                    concat!(
                        "event: response.failed\r\n",
                        "data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"insufficient_quota\",\"message\":\"main quota exhausted\"}}}\r\n",
                        "\r\n"
                    )
                    .to_string(),
                    None,
                    matches!(_mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                        .then_some(Duration::from_millis(750)),
                    matches!(_mode, RuntimeProxyBackendMode::HttpOnlySlowStream)
                        .then_some(Duration::from_millis(100)),
                ),
                "second-account"
                    if matches!(
                        _mode,
                        RuntimeProxyBackendMode::HttpOnlyPreviousResponseNeedsTurnState
                    ) && previous_response_id.as_deref() == Some("resp-second")
                        && turn_state.as_deref() != Some("turn-second") =>
                {
                    (
                        "HTTP/1.1 400 Bad Request",
                        "application/json",
                        serde_json::json!({
                            "type": "error",
                            "status": 400,
                            "error": {
                                "code": "previous_response_not_found",
                                "message": format!(
                                    "Previous response with id '{}' not found.",
                                    previous_response_id.as_deref().unwrap_or_default()
                                ),
                                "param": "previous_response_id",
                            }
                        })
                        .to_string(),
                        Some("turn-second".to_string()),
                        None,
                        None,
                    )
                }
                "second-account" if previous_response_id.as_deref() == Some("resp-second") => {
                    (
                        "HTTP/1.1 200 OK",
                        "text/event-stream",
                        concat!(
                            "event: response.created\r\n",
                            "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-second-next\"}}\r\n",
                            "\r\n",
                            "event: response.completed\r\n",
                            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-second-next\"}}\r\n",
                            "\r\n"
                        )
                        .to_string(),
                        None,
                        matches!(_mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                            .then_some(Duration::from_millis(750)),
                        matches!(_mode, RuntimeProxyBackendMode::HttpOnlySlowStream)
                            .then_some(Duration::from_millis(100)),
                    )
                }
                "second-account" if previous_response_id.is_some() => (
                    "HTTP/1.1 400 Bad Request",
                    "application/json",
                    serde_json::json!({
                        "type": "error",
                        "status": 400,
                        "error": {
                            "code": "previous_response_not_found",
                            "message": format!(
                                "Previous response with id '{}' not found.",
                                previous_response_id.as_deref().unwrap_or_default()
                            ),
                            "param": "previous_response_id",
                        }
                    })
                    .to_string(),
                    None,
                    matches!(_mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                        .then_some(Duration::from_millis(750)),
                    None,
                ),
                "second-account" => (
                    "HTTP/1.1 200 OK",
                    "text/event-stream",
                    concat!(
                        "event: response.created\r\n",
                        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-second\"}}\r\n",
                        "\r\n",
                        "event: response.completed\r\n",
                        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-second\"}}\r\n",
                        "\r\n"
                        )
                        .to_string(),
                        None,
                        matches!(_mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                            .then_some(Duration::from_millis(750)),
                        matches!(_mode, RuntimeProxyBackendMode::HttpOnlySlowStream)
                            .then_some(Duration::from_millis(100)),
                ),
                "third-account" if previous_response_id.is_some() => (
                    "HTTP/1.1 400 Bad Request",
                    "application/json",
                    serde_json::json!({
                        "type": "error",
                        "status": 400,
                        "error": {
                            "code": "previous_response_not_found",
                            "message": format!(
                                "Previous response with id '{}' not found.",
                                previous_response_id.as_deref().unwrap_or_default()
                            ),
                            "param": "previous_response_id",
                        }
                    })
                    .to_string(),
                    None,
                    matches!(_mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                        .then_some(Duration::from_millis(750)),
                    None,
                ),
                "third-account" => (
                    "HTTP/1.1 200 OK",
                    "text/event-stream",
                    concat!(
                        "event: response.created\r\n",
                        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-third\"}}\r\n",
                        "\r\n",
                        "event: response.completed\r\n",
                        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-third\"}}\r\n",
                        "\r\n"
                        )
                        .to_string(),
                        None,
                        matches!(_mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                            .then_some(Duration::from_millis(750)),
                        matches!(_mode, RuntimeProxyBackendMode::HttpOnlySlowStream)
                            .then_some(Duration::from_millis(100)),
                ),
                _ => (
                    "HTTP/1.1 200 OK",
                    "text/event-stream",
                    concat!(
                        "event: response.failed\r\n",
                        "data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"rate_limit_exceeded\",\"message\":\"unexpected account\"}}}\r\n",
                        "\r\n"
                        )
                        .to_string(),
                    None,
                    matches!(_mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                        .then_some(Duration::from_millis(750)),
                    matches!(_mode, RuntimeProxyBackendMode::HttpOnlySlowStream)
                        .then_some(Duration::from_millis(100)),
                ),
            }
            } else {
                (
                    "HTTP/1.1 404 Not Found",
                    "application/json",
                    serde_json::json!({ "error": "not_found" }).to_string(),
                    None,
                    None,
                    None,
                )
            };

        let mut headers = format!(
            "{status_line}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n",
            body.len(),
        );
        if let Some(turn_state) = response_turn_state.as_deref() {
            headers.push_str(&format!("x-codex-turn-state: {turn_state}\r\n"));
        }
        headers.push_str("\r\n");
        let _ = stream.write_all(headers.as_bytes());
        let _ = stream.flush();
        if let Some(delay) = initial_body_stall {
            thread::sleep(delay);
        }
        if content_type == "text/event-stream"
            && let Some(delay) = chunk_delay
        {
            let body_bytes = body.as_bytes();
            let chunk_size = body_bytes.len().max(1).div_ceil(4);
            for (index, chunk) in body_bytes.chunks(chunk_size).enumerate() {
                let _ = stream.write_all(chunk);
                let _ = stream.flush();
                if index + 1 < body_bytes.chunks(chunk_size).len() {
                    thread::sleep(delay);
                }
            }
        } else {
            let _ = stream.write_all(body.as_bytes());
            let _ = stream.flush();
        }
    }

    fn runtime_proxy_backend_is_websocket_upgrade(stream: &TcpStream) -> bool {
        let mut buffer = [0_u8; 2048];
        let Ok(read) = stream.peek(&mut buffer) else {
            return false;
        };
        if read == 0 {
            return false;
        }
        let request = String::from_utf8_lossy(&buffer[..read]).to_ascii_lowercase();
        request.contains("upgrade: websocket")
    }

    fn handle_runtime_proxy_backend_websocket(
        stream: TcpStream,
        responses_accounts: &Arc<Mutex<Vec<String>>>,
        mode: RuntimeProxyBackendMode,
    ) {
        let account_id = Arc::new(Mutex::new(String::new()));
        let request_turn_state = Arc::new(Mutex::new(None::<String>));
        let captured_account_id = Arc::clone(&account_id);
        let captured_turn_state = Arc::clone(&request_turn_state);
        let callback =
            move |req: &tungstenite::handshake::server::Request,
                  response: tungstenite::handshake::server::Response| {
                if let Some(value) = req
                    .headers()
                    .get("ChatGPT-Account-Id")
                    .and_then(|value| value.to_str().ok())
                {
                    *captured_account_id
                        .lock()
                        .expect("captured_account_id poisoned") = value.to_string();
                }
                if let Some(value) = req
                    .headers()
                    .get("x-codex-turn-state")
                    .and_then(|value| value.to_str().ok())
                {
                    *captured_turn_state
                        .lock()
                        .expect("captured_turn_state poisoned") = Some(value.to_string());
                }
                let mut response = response;
                if matches!(
                    mode,
                    RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState
                ) && req
                    .headers()
                    .get("ChatGPT-Account-Id")
                    .and_then(|value| value.to_str().ok())
                    == Some("second-account")
                {
                    response.headers_mut().insert(
                        tungstenite::http::header::HeaderName::from_static("x-codex-turn-state"),
                        tungstenite::http::HeaderValue::from_static("turn-second"),
                    );
                }
                Ok(response)
            };
        let mut websocket = tungstenite::accept_hdr(stream, callback)
            .expect("backend websocket handshake should succeed");
        let account_id = account_id.lock().expect("account_id poisoned").clone();
        let mut effective_turn_state = request_turn_state
            .lock()
            .expect("request_turn_state poisoned")
            .clone();
        responses_accounts
            .lock()
            .expect("responses_accounts poisoned")
            .push(account_id.clone());
        let response_turn_state = (matches!(
            mode,
            RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState
        ) && account_id == "second-account")
            .then(|| "turn-second".to_string());

        loop {
            let request = match websocket.read() {
                Ok(WsMessage::Text(text)) => text.to_string(),
                Ok(WsMessage::Ping(payload)) => {
                    websocket
                        .send(WsMessage::Pong(payload))
                        .expect("backend websocket pong should be sent");
                    continue;
                }
                Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => continue,
                Ok(WsMessage::Close(_))
                | Err(WsError::ConnectionClosed)
                | Err(WsError::AlreadyClosed) => break,
                Ok(other) => panic!("backend websocket expects text requests, got {other:?}"),
                Err(err) => panic!("backend websocket failed to read request: {err}"),
            };
            let previous_response_id = runtime_request_previous_response_id_from_text(&request);

            match account_id.as_str() {
                "main-account" => {
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "error",
                                "status": 429,
                                "error": {
                                    "code": "insufficient_quota",
                                    "message": "main quota exhausted",
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("quota error should be sent");
                }
                "second-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState
                    ) && previous_response_id.as_deref() == Some("resp-second")
                        && effective_turn_state.as_deref() != Some("turn-second") =>
                {
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "error",
                                "status": 400,
                                "error": {
                                    "code": "previous_response_not_found",
                                    "message": format!(
                                        "Previous response with id '{}' not found.",
                                        previous_response_id.as_deref().unwrap_or_default()
                                    ),
                                    "param": "previous_response_id",
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("previous_response_not_found should be sent");
                }
                "second-account" if previous_response_id.as_deref() == Some("resp-second") => {
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.created",
                                "response": {
                                    "id": "resp-second-next"
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.created should be sent");
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.completed",
                                "response": {
                                    "id": "resp-second-next"
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.completed should be sent");
                }
                "second-account" if previous_response_id.is_some() => {
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "error",
                                "status": 400,
                                "error": {
                                    "code": "previous_response_not_found",
                                    "message": format!(
                                        "Previous response with id '{}' not found.",
                                        previous_response_id.as_deref().unwrap_or_default()
                                    ),
                                    "param": "previous_response_id",
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("previous_response_not_found should be sent");
                }
                "second-account" => {
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.created",
                                "response": {
                                    "id": "resp-second"
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.created should be sent");
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.completed",
                                "response": {
                                    "id": "resp-second"
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.completed should be sent");
                }
                "third-account" if previous_response_id.is_some() => {
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "error",
                                "status": 400,
                                "error": {
                                    "code": "previous_response_not_found",
                                    "message": format!(
                                        "Previous response with id '{}' not found.",
                                        previous_response_id.as_deref().unwrap_or_default()
                                    ),
                                    "param": "previous_response_id",
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("previous_response_not_found should be sent");
                }
                "third-account" => {
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.created",
                                "response": {
                                    "id": "resp-third"
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.created should be sent");
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.completed",
                                "response": {
                                    "id": "resp-third"
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.completed should be sent");
                }
                _ => {
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "error",
                                "status": 429,
                                "error": {
                                    "code": "rate_limit_exceeded",
                                    "message": "unexpected account",
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("unexpected account error should be sent");
                }
            }

            if response_turn_state.is_some() {
                effective_turn_state = response_turn_state.clone();
            }
        }
    }

    fn read_http_request(stream: &mut TcpStream) -> Option<String> {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
        let mut buffer = [0_u8; 1024];
        let mut request = Vec::new();

        loop {
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    request.extend_from_slice(&buffer[..read]);
                    if let Some(header_end) =
                        request.windows(4).position(|window| window == b"\r\n\r\n")
                    {
                        let header_len = header_end + 4;
                        let header_text = String::from_utf8_lossy(&request[..header_len]);
                        let content_length = request_header(&header_text, "Content-Length")
                            .and_then(|value| value.parse::<usize>().ok())
                            .unwrap_or(0);
                        while request.len() < header_len + content_length {
                            match stream.read(&mut buffer) {
                                Ok(0) => break,
                                Ok(read) => request.extend_from_slice(&buffer[..read]),
                                Err(err)
                                    if matches!(
                                        err.kind(),
                                        std::io::ErrorKind::WouldBlock
                                            | std::io::ErrorKind::TimedOut
                                    ) =>
                                {
                                    break;
                                }
                                Err(_) => return None,
                            }
                        }
                        break;
                    }
                }
                Err(err)
                    if matches!(
                        err.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    break;
                }
                Err(_) => return None,
            }
        }

        (!request.is_empty()).then(|| String::from_utf8_lossy(&request).into_owned())
    }

    fn request_header(request: &str, header_name: &str) -> Option<String> {
        request.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.trim().eq_ignore_ascii_case(header_name) {
                Some(value.trim().to_string())
            } else {
                None
            }
        })
    }

    fn request_previous_response_id(request: &str) -> Option<String> {
        let body = request
            .split_once("\r\n\r\n")
            .map(|(_, body)| body)
            .unwrap_or_default();
        runtime_request_previous_response_id_from_text(body)
    }

    fn runtime_proxy_usage_body(email: &str) -> String {
        serde_json::json!({
            "email": email,
            "plan_type": "plus",
            "rate_limit": {
                "primary_window": {
                    "used_percent": 5,
                    "reset_at": future_epoch(18_000),
                    "limit_window_seconds": 18_000
                },
                "secondary_window": {
                    "used_percent": 5,
                    "reset_at": future_epoch(604_800),
                    "limit_window_seconds": 604_800
                }
            }
        })
        .to_string()
    }

    fn future_epoch(offset_seconds: i64) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_secs() as i64
            + offset_seconds
    }

    fn write_auth_json(path: &Path, account_id: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create auth parent dir");
        }
        fs::write(
            path,
            serde_json::json!({
                "tokens": {
                    "access_token": "test-token",
                    "account_id": account_id,
                }
            })
            .to_string(),
        )
        .expect("failed to write auth.json");
    }

    #[test]
    fn validates_profile_names() {
        assert!(validate_profile_name("alpha-1").is_ok());
        assert!(validate_profile_name("bad/name").is_err());
        assert!(validate_profile_name("bad space").is_err());
    }

    #[test]
    fn recognizes_known_windows() {
        assert_eq!(window_label(Some(18_000)), "5h");
        assert_eq!(window_label(Some(604_800)), "weekly");
        assert_eq!(window_label(Some(2_592_000)), "monthly");
    }

    #[test]
    fn blocks_when_main_window_is_exhausted() {
        let usage = UsageResponse {
            email: None,
            plan_type: None,
            rate_limit: Some(WindowPair {
                primary_window: Some(UsageWindow {
                    used_percent: Some(100),
                    reset_at: Some(1_700_000_000),
                    limit_window_seconds: Some(18_000),
                }),
                secondary_window: None,
            }),
            code_review_rate_limit: None,
            additional_rate_limits: Vec::new(),
        };

        let blocked = collect_blocked_limits(&usage, false);
        assert_eq!(blocked.len(), 2);
        assert!(blocked[0].message.starts_with("5h exhausted until "));
        assert_eq!(blocked[1].message, "weekly quota unavailable");
    }

    #[test]
    fn blocks_when_weekly_window_is_missing() {
        let usage = UsageResponse {
            email: None,
            plan_type: None,
            rate_limit: Some(WindowPair {
                primary_window: Some(UsageWindow {
                    used_percent: Some(20),
                    reset_at: Some(1_700_000_000),
                    limit_window_seconds: Some(18_000),
                }),
                secondary_window: None,
            }),
            code_review_rate_limit: None,
            additional_rate_limits: Vec::new(),
        };

        let blocked = collect_blocked_limits(&usage, false);
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].message, "weekly quota unavailable");
    }

    #[test]
    fn compact_window_format_uses_scale_of_100() {
        let window = UsageWindow {
            used_percent: Some(37),
            reset_at: None,
            limit_window_seconds: Some(18_000),
        };

        assert_eq!(format_window_status_compact(&window), "5h 63% left");
        assert!(format_window_status(&window).contains("63% left"));
        assert!(format_window_status(&window).contains("37% used"));
    }

    #[test]
    fn main_reset_summary_lists_required_windows() {
        let usage = UsageResponse {
            email: None,
            plan_type: None,
            rate_limit: Some(WindowPair {
                primary_window: Some(UsageWindow {
                    used_percent: Some(20),
                    reset_at: Some(1_700_000_000),
                    limit_window_seconds: Some(18_000),
                }),
                secondary_window: Some(UsageWindow {
                    used_percent: Some(30),
                    reset_at: Some(1_700_000_000),
                    limit_window_seconds: Some(604_800),
                }),
            }),
            code_review_rate_limit: None,
            additional_rate_limits: Vec::new(),
        };

        let summary = format_main_reset_summary(&usage);
        assert!(summary.starts_with("5h "));
        assert!(summary.contains(" | weekly "));
        assert!(summary.contains(&format_precise_reset_time(Some(1_700_000_000))));
    }

    #[test]
    fn main_reset_summary_marks_missing_required_window() {
        let usage = UsageResponse {
            email: None,
            plan_type: None,
            rate_limit: Some(WindowPair {
                primary_window: Some(UsageWindow {
                    used_percent: Some(20),
                    reset_at: Some(1_700_000_000),
                    limit_window_seconds: Some(18_000),
                }),
                secondary_window: None,
            }),
            code_review_rate_limit: None,
            additional_rate_limits: Vec::new(),
        };

        assert_eq!(
            format_main_reset_summary(&usage),
            format!(
                "5h {} | weekly unavailable",
                format_precise_reset_time(Some(1_700_000_000))
            )
        );
    }

    #[test]
    fn map_parallel_runs_jobs_concurrently_and_preserves_order() {
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(Mutex::new(0usize));
        let started = Instant::now();

        let output = map_parallel(vec![1, 2, 3, 4], {
            let active = Arc::clone(&active);
            let max_active = Arc::clone(&max_active);
            move |value| {
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                {
                    let mut seen_max = max_active.lock().expect("max_active poisoned");
                    *seen_max = (*seen_max).max(current);
                }

                thread::sleep(Duration::from_millis(50));
                active.fetch_sub(1, Ordering::SeqCst);
                value * 10
            }
        });

        assert_eq!(output, vec![10, 20, 30, 40]);
        assert!(
            *max_active.lock().expect("max_active poisoned") >= 2,
            "parallel worker count never exceeded one"
        );
        assert!(
            started.elapsed() < Duration::from_millis(150),
            "parallel execution took too long: {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn ready_profile_ranking_prefers_soon_recovering_weekly_capacity() {
        let candidates = vec![
            ReadyProfileCandidate {
                name: "slow".to_string(),
                usage: usage_with_main_windows(100, 18_000, 100, 604_800),
                order_index: 0,
                preferred: false,
            },
            ReadyProfileCandidate {
                name: "fast".to_string(),
                usage: usage_with_main_windows(80, 18_000, 80, 86_400),
                order_index: 1,
                preferred: false,
            },
        ];

        let mut ranked = candidates.clone();
        ranked.sort_by_key(ready_profile_sort_key);
        assert_eq!(ranked[0].name, "fast");
    }

    #[test]
    fn ready_profile_ranking_prefers_larger_reserve_when_resets_match() {
        let candidates = vec![
            ReadyProfileCandidate {
                name: "thin".to_string(),
                usage: usage_with_main_windows(65, 18_000, 70, 604_800),
                order_index: 0,
                preferred: false,
            },
            ReadyProfileCandidate {
                name: "deep".to_string(),
                usage: usage_with_main_windows(95, 18_000, 98, 604_800),
                order_index: 1,
                preferred: false,
            },
        ];

        let mut ranked = candidates.clone();
        ranked.sort_by_key(ready_profile_sort_key);
        assert_eq!(ranked[0].name, "deep");
    }

    #[test]
    fn scheduler_prefers_rested_profile_within_near_optimal_band() {
        let now = Local::now().timestamp();
        let state = AppState {
            active_profile: None,
            profiles: BTreeMap::new(),
            last_run_selected_at: BTreeMap::from([
                ("fresh".to_string(), now),
                ("rested".to_string(), now - 3_600),
            ]),
            response_profile_bindings: BTreeMap::new(),
        };
        let candidates = vec![
            ReadyProfileCandidate {
                name: "fresh".to_string(),
                usage: usage_with_main_windows(100, 18_000, 100, 604_800),
                order_index: 0,
                preferred: false,
            },
            ReadyProfileCandidate {
                name: "rested".to_string(),
                usage: usage_with_main_windows(96, 18_000, 96, 604_800),
                order_index: 1,
                preferred: false,
            },
        ];

        let ranked = schedule_ready_profile_candidates(candidates, &state, None);
        assert_eq!(ranked[0].name, "rested");
    }

    #[test]
    fn scheduler_keeps_preferred_profile_when_gain_is_small() {
        let state = AppState {
            active_profile: Some("active".to_string()),
            profiles: BTreeMap::new(),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
        };
        let candidates = vec![
            ReadyProfileCandidate {
                name: "better".to_string(),
                usage: usage_with_main_windows(100, 18_000, 100, 604_800),
                order_index: 0,
                preferred: false,
            },
            ReadyProfileCandidate {
                name: "active".to_string(),
                usage: usage_with_main_windows(96, 18_000, 96, 604_800),
                order_index: 1,
                preferred: true,
            },
        ];

        let ranked = schedule_ready_profile_candidates(candidates, &state, Some("active"));
        assert_eq!(ranked[0].name, "active");
    }

    #[test]
    fn scheduler_allows_switch_when_preferred_profile_is_in_cooldown() {
        let now = Local::now().timestamp();
        let state = AppState {
            active_profile: Some("active".to_string()),
            profiles: BTreeMap::new(),
            last_run_selected_at: BTreeMap::from([("active".to_string(), now)]),
            response_profile_bindings: BTreeMap::new(),
        };
        let candidates = vec![
            ReadyProfileCandidate {
                name: "better".to_string(),
                usage: usage_with_main_windows(100, 18_000, 100, 604_800),
                order_index: 0,
                preferred: false,
            },
            ReadyProfileCandidate {
                name: "active".to_string(),
                usage: usage_with_main_windows(96, 18_000, 96, 604_800),
                order_index: 1,
                preferred: true,
            },
        ];

        let ranked = schedule_ready_profile_candidates(candidates, &state, Some("active"));
        assert_eq!(ranked[0].name, "better");
    }

    #[test]
    fn quota_overview_sort_prioritizes_status_then_nearest_reset() {
        let reports = vec![
            QuotaReport {
                name: "blocked".to_string(),
                active: false,
                auth: AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                },
                result: Ok(usage_with_main_windows(0, 3_600, 80, 86_400)),
            },
            QuotaReport {
                name: "ready-late".to_string(),
                active: false,
                auth: AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                },
                result: Ok(usage_with_main_windows(90, 7_200, 95, 172_800)),
            },
            QuotaReport {
                name: "error".to_string(),
                active: false,
                auth: AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                },
                result: Err("boom".to_string()),
            },
            QuotaReport {
                name: "ready-early".to_string(),
                active: false,
                auth: AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                },
                result: Ok(usage_with_main_windows(90, 1_800, 95, 259_200)),
            },
        ];

        let names = sort_quota_reports_for_display(&reports)
            .into_iter()
            .map(|report| report.name.clone())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["ready-early", "ready-late", "blocked", "error"]);
    }

    #[test]
    fn rotates_profiles_after_current_profile() {
        let state = AppState {
            active_profile: Some("beta".to_string()),
            profiles: BTreeMap::from([
                (
                    "alpha".to_string(),
                    ProfileEntry {
                        codex_home: PathBuf::from("/tmp/alpha"),
                        managed: true,
                        email: None,
                    },
                ),
                (
                    "beta".to_string(),
                    ProfileEntry {
                        codex_home: PathBuf::from("/tmp/beta"),
                        managed: true,
                        email: None,
                    },
                ),
                (
                    "gamma".to_string(),
                    ProfileEntry {
                        codex_home: PathBuf::from("/tmp/gamma"),
                        managed: true,
                        email: None,
                    },
                ),
            ]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
        };

        assert_eq!(
            profile_rotation_order(&state, "beta"),
            vec!["gamma".to_string(), "alpha".to_string()]
        );
    }

    #[test]
    fn backend_api_base_url_maps_to_wham_usage() {
        assert_eq!(
            usage_url("https://chatgpt.com/backend-api"),
            "https://chatgpt.com/backend-api/wham/usage"
        );
    }

    #[test]
    fn custom_base_url_maps_to_codex_usage() {
        assert_eq!(
            usage_url("http://127.0.0.1:8080"),
            "http://127.0.0.1:8080/api/codex/usage"
        );
    }

    #[test]
    fn profile_name_is_derived_from_email() {
        assert_eq!(
            profile_name_from_email("Main+Ops@Example.com"),
            "main-ops_example.com"
        );
    }

    #[test]
    fn unique_profile_name_adds_numeric_suffix() {
        let state = AppState {
            active_profile: None,
            profiles: BTreeMap::from([(
                "main_example.com".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/existing"),
                    managed: true,
                    email: Some("other@example.com".to_string()),
                },
            )]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
        };
        let paths = AppPaths {
            root: PathBuf::from("/tmp/prodex-test"),
            state_file: PathBuf::from("/tmp/prodex-test/state.json"),
            managed_profiles_root: PathBuf::from("/tmp/prodex-test/profiles"),
            shared_codex_root: PathBuf::from("/tmp/prodex-test/default-codex"),
            legacy_shared_codex_root: PathBuf::from("/tmp/prodex-test/shared"),
        };

        assert_eq!(
            unique_profile_name_for_email(&paths, &state, "main@example.com"),
            "main_example.com-2"
        );
    }

    #[test]
    fn parses_email_from_chatgpt_id_token() {
        let id_token = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL3Byb2ZpbGUiOnsiZW1haWwiOiJ1c2VyQGV4YW1wbGUuY29tIn19.c2ln";

        assert_eq!(
            parse_email_from_id_token(id_token).expect("id token should parse"),
            Some("user@example.com".to_string())
        );
    }

    #[test]
    fn usage_response_accepts_null_additional_rate_limits() {
        let usage: UsageResponse = serde_json::from_value(serde_json::json!({
            "email": "user@example.com",
            "plan_type": "plus",
            "rate_limit": null,
            "code_review_rate_limit": null,
            "additional_rate_limits": null
        }))
        .expect("usage response should parse");

        assert!(usage.additional_rate_limits.is_empty());
    }

    #[test]
    fn previous_response_owner_discovery_ignores_retry_backoff() {
        let temp_dir = TestDir::new();
        let main_home = temp_dir.path.join("homes/main");
        let second_home = temp_dir.path.join("homes/second");
        write_auth_json(&main_home.join("auth.json"), "main-account");
        write_auth_json(&second_home.join("auth.json"), "second-account");

        let paths = AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        };
        let state = AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([
                (
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                    },
                ),
            ]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
        };
        let runtime = RuntimeRotationState {
            paths,
            state,
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            turn_state_bindings: BTreeMap::new(),
            profile_probe_cache: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::from([(
                "second".to_string(),
                Local::now().timestamp().saturating_add(60),
            )]),
        };
        let shared = RuntimeRotationProxyShared {
            client: Client::builder().build().expect("client"),
            async_client: reqwest::Client::builder().build().expect("async client"),
            async_runtime: Arc::new(
                TokioRuntimeBuilder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .build()
                    .expect("async runtime"),
            ),
            log_path: temp_dir.path.join("runtime-proxy.log"),
            request_sequence: Arc::new(AtomicU64::new(1)),
            runtime: Arc::new(Mutex::new(runtime)),
        };
        let excluded = BTreeSet::from(["main".to_string()]);

        assert_eq!(
            next_runtime_previous_response_candidate(&shared, &excluded)
                .expect("candidate selection should succeed"),
            Some("second".to_string())
        );
    }

    #[test]
    fn turn_state_affinity_prefers_bound_profile() {
        let temp_dir = TestDir::new();
        let main_home = temp_dir.path.join("homes/main");
        let second_home = temp_dir.path.join("homes/second");
        write_auth_json(&main_home.join("auth.json"), "main-account");
        write_auth_json(&second_home.join("auth.json"), "second-account");

        let paths = AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        };
        let state = AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([
                (
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                    },
                ),
            ]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
        };
        let runtime = RuntimeRotationState {
            paths,
            state,
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            turn_state_bindings: BTreeMap::from([(
                "turn-second".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: Local::now().timestamp(),
                },
            )]),
            profile_probe_cache: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
        };
        let shared = RuntimeRotationProxyShared {
            client: Client::builder().build().expect("client"),
            async_client: reqwest::Client::builder().build().expect("async client"),
            async_runtime: Arc::new(
                TokioRuntimeBuilder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .build()
                    .expect("async runtime"),
            ),
            log_path: temp_dir.path.join("runtime-proxy.log"),
            request_sequence: Arc::new(AtomicU64::new(1)),
            runtime: Arc::new(Mutex::new(runtime)),
        };
        let turn_state_profile = runtime_turn_state_bound_profile(&shared, "turn-second")
            .expect("turn-state lookup should succeed");

        assert_eq!(
            select_runtime_response_candidate(
                &shared,
                &BTreeSet::new(),
                None,
                turn_state_profile.as_deref(),
                false,
            )
            .expect("candidate selection should succeed"),
            Some("second".to_string())
        );
    }

    #[test]
    fn runtime_sse_tap_reader_keeps_response_affinity_when_prelude_splits_event() {
        let temp_dir = TestDir::new();
        let second_home = temp_dir.path.join("homes/second");
        write_auth_json(&second_home.join("auth.json"), "second-account");

        let paths = AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        };
        let state = AppState {
            active_profile: Some("second".to_string()),
            profiles: BTreeMap::from([(
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            )]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
        };
        let runtime = RuntimeRotationState {
            paths: paths.clone(),
            state,
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "second".to_string(),
            turn_state_bindings: BTreeMap::new(),
            profile_probe_cache: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
        };
        let shared = RuntimeRotationProxyShared {
            client: Client::builder().build().expect("client"),
            async_client: reqwest::Client::builder().build().expect("async client"),
            async_runtime: Arc::new(
                TokioRuntimeBuilder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .build()
                    .expect("async runtime"),
            ),
            log_path: temp_dir.path.join("runtime-proxy.log"),
            request_sequence: Arc::new(AtomicU64::new(1)),
            runtime: Arc::new(Mutex::new(runtime)),
        };

        let prelude =
            b"event: response.created\r\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-second";
        let remainder =
            b"\"}}\r\n\r\nevent: response.completed\r\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-second\"}}\r\n\r\n";
        let mut reader = RuntimeSseTapReader::new(
            Cursor::new(prelude.to_vec()).chain(Cursor::new(remainder.to_vec())),
            shared.clone(),
            "second".to_string(),
            prelude,
            &[],
        );
        let mut body = Vec::new();
        reader
            .read_to_end(&mut body)
            .expect("split SSE payload should be readable");

        let persisted = AppState::load(&paths).expect("state should reload");
        assert_eq!(
            persisted
                .response_profile_bindings
                .get("resp-second")
                .map(|binding| binding.profile_name.as_str()),
            Some("second")
        );
    }

    #[test]
    fn section_headers_use_cli_width() {
        assert_eq!(text_width(&section_header("Quota Overview")), CLI_WIDTH);
    }

    #[test]
    fn field_lines_do_not_exceed_cli_width() {
        let fields = format_field_lines(
            "Path",
            "/tmp/some/really/long/path/that/should/still/stay/inside/the/configured/cli/width/when/rendered",
        );

        assert!(fields.iter().all(|line| text_width(line) <= CLI_WIDTH));
    }

    #[test]
    fn runtime_proxy_injects_codex_backend_overrides() {
        let args = runtime_proxy_codex_args(
            "127.0.0.1:4455".parse().expect("socket addr"),
            &[OsString::from("exec"), OsString::from("hello")],
        );
        let rendered = args
            .iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert_eq!(rendered[0], "-c");
        assert!(rendered[1].contains("chatgpt_base_url=\"http://127.0.0.1:4455/backend-api\""));
        assert_eq!(rendered[2], "-c");
        assert_eq!(
            rendered[3],
            "openai_base_url=\"http://127.0.0.1:4455/backend-api/codex\""
        );
        assert_eq!(&rendered[4..], ["exec", "hello"]);
    }

    #[test]
    fn runtime_proxy_maps_openai_prefix_to_upstream_backend_api() {
        assert_eq!(
            runtime_proxy_upstream_url(
                "https://chatgpt.com/backend-api",
                "/backend-api/codex/responses"
            ),
            "https://chatgpt.com/backend-api/codex/responses"
        );
    }

    #[test]
    fn runtime_proxy_retries_quota_blocked_response_on_another_profile() {
        let temp_dir = TestDir::new();
        let backend = RuntimeProxyBackend::start();
        let paths = AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        };
        let main_home = temp_dir.path.join("homes/main");
        let second_home = temp_dir.path.join("homes/second");
        write_auth_json(&main_home.join("auth.json"), "main-account");
        write_auth_json(&second_home.join("auth.json"), "second-account");

        let state = AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([
                (
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home.clone(),
                        managed: true,
                        email: Some("main@example.com".to_string()),
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home.clone(),
                        managed: true,
                        email: Some("second@example.com".to_string()),
                    },
                ),
            ]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
        };
        state.save(&paths).expect("failed to save initial state");

        let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
            .expect("runtime proxy should start");
        let response = Client::builder()
            .build()
            .expect("client")
            .post(format!(
                "http://{}/backend-api/codex/responses",
                proxy.listen_addr
            ))
            .header("Content-Type", "application/json")
            .body("{\"input\":[]}")
            .send()
            .expect("runtime proxy request should succeed");
        let body = response.text().expect("response body should be readable");

        assert!(body.contains("\"response.created\""));
        assert!(!body.contains("main quota exhausted"));
        assert_eq!(
            backend.responses_accounts(),
            vec!["main-account".to_string(), "second-account".to_string()]
        );

        let persisted = AppState::load(&paths).expect("state should reload");
        assert_eq!(persisted.active_profile.as_deref(), Some("second"));
    }

    #[test]
    fn runtime_proxy_uses_current_profile_without_runtime_quota_probe() {
        let temp_dir = TestDir::new();
        let backend = RuntimeProxyBackend::start();
        let paths = AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        };
        let main_home = temp_dir.path.join("homes/main");
        let second_home = temp_dir.path.join("homes/second");
        write_auth_json(&main_home.join("auth.json"), "main-account");
        write_auth_json(&second_home.join("auth.json"), "second-account");

        let state = AppState {
            active_profile: Some("second".to_string()),
            profiles: BTreeMap::from([
                (
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                    },
                ),
            ]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
        };
        state.save(&paths).expect("failed to save initial state");

        let proxy =
            start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
                .expect("runtime proxy should start");

        let response = Client::builder()
            .build()
            .expect("client")
            .post(format!(
                "http://{}/backend-api/codex/responses",
                proxy.listen_addr
            ))
            .header("Content-Type", "application/json")
            .body("{\"input\":[]}")
            .send()
            .expect("runtime proxy request should succeed");
        let body = response.text().expect("response body should be readable");

        assert!(body.contains("\"response.created\""));
        assert_eq!(
            backend.responses_accounts(),
            vec!["second-account".to_string()]
        );
        assert!(backend.usage_accounts().is_empty());
    }

    #[test]
    fn runtime_proxy_reuses_rotated_profile_without_reprobing_quota() {
        let temp_dir = TestDir::new();
        let backend = RuntimeProxyBackend::start();
        let paths = AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        };
        let main_home = temp_dir.path.join("homes/main");
        let second_home = temp_dir.path.join("homes/second");
        write_auth_json(&main_home.join("auth.json"), "main-account");
        write_auth_json(&second_home.join("auth.json"), "second-account");

        let state = AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([
                (
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                    },
                ),
            ]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
        };
        state.save(&paths).expect("failed to save initial state");

        let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
            .expect("runtime proxy should start");
        let client = Client::builder().build().expect("client");

        let first = client
            .post(format!(
                "http://{}/backend-api/codex/responses",
                proxy.listen_addr
            ))
            .header("Content-Type", "application/json")
            .body("{\"input\":[]}")
            .send()
            .expect("first runtime proxy request should succeed");
        let first_body = first
            .text()
            .expect("first response body should be readable");
        assert!(first_body.contains("\"response.created\""));
        assert_eq!(backend.usage_accounts(), vec!["second-account".to_string()]);

        let second = client
            .post(format!(
                "http://{}/backend-api/codex/responses",
                proxy.listen_addr
            ))
            .header("Content-Type", "application/json")
            .body("{\"input\":[]}")
            .send()
            .expect("second runtime proxy request should succeed");
        let second_body = second
            .text()
            .expect("second response body should be readable");
        assert!(second_body.contains("\"response.created\""));

        assert_eq!(
            backend.responses_accounts(),
            vec![
                "main-account".to_string(),
                "second-account".to_string(),
                "second-account".to_string(),
            ]
        );
        assert_eq!(backend.usage_accounts(), vec!["second-account".to_string()]);
    }

    #[test]
    fn runtime_proxy_passes_through_upstream_http_error_response() {
        let temp_dir = TestDir::new();
        let backend = RuntimeProxyBackend::start();
        let paths = AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        };
        let main_home = temp_dir.path.join("homes/main");
        write_auth_json(&main_home.join("auth.json"), "main-account");

        let state = AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            )]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
        };
        let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
            .expect("runtime proxy should start");

        let response = Client::builder()
            .build()
            .expect("client")
            .post(format!(
                "http://{}/backend-api/codex/responses",
                proxy.listen_addr
            ))
            .header("Content-Type", "application/json")
            .body("{\"input\":[]}")
            .send()
            .expect("runtime proxy request should succeed");
        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = response.text().expect("response body should be readable");

        assert_eq!(status, reqwest::StatusCode::OK);
        assert!(content_type.contains("text/event-stream"));
        assert!(body.contains("\"type\":\"response.failed\""));
        assert!(body.contains("\"code\":\"insufficient_quota\""));
        assert!(body.contains("main quota exhausted"));
    }

    #[test]
    fn runtime_proxy_aborts_stalled_http_fallback_before_long_hang() {
        let temp_dir = TestDir::new();
        let backend = RuntimeProxyBackend::start_http_initial_body_stall();
        let paths = AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        };
        let main_home = temp_dir.path.join("homes/main");
        write_auth_json(&main_home.join("auth.json"), "main-account");

        let state = AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            )]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
        };
        let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
            .expect("runtime proxy should start");

        let client = Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .expect("client");
        let started = std::time::Instant::now();
        let result = client
            .post(format!(
                "http://{}/backend-api/codex/responses",
                proxy.listen_addr
            ))
            .header("Content-Type", "application/json")
            .body("{\"input\":[]}")
            .send();
        let elapsed = started.elapsed();

        assert!(
            elapsed < Duration::from_secs(2),
            "stalled HTTP fallback took too long: {elapsed:?}"
        );
        match result {
            Ok(response) => {
                assert_eq!(
                    response.status(),
                    reqwest::StatusCode::OK,
                    "runtime proxy should surface a dropped stream, not a synthetic HTTP error"
                );
                let content_type = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                assert!(
                    content_type.contains("text/event-stream"),
                    "runtime proxy should keep responses transport semantics on failure"
                );
                match response.text() {
                    Ok(body) => assert!(
                        body.is_empty(),
                        "aborted runtime stream should terminate without a synthetic payload"
                    ),
                    Err(_) => {}
                }
            }
            Err(_) => {}
        }
    }

    #[test]
    fn runtime_proxy_keeps_healthy_long_http_stream_alive() {
        let temp_dir = TestDir::new();
        let backend = RuntimeProxyBackend::start_http_slow_stream();
        let paths = AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        };
        let second_home = temp_dir.path.join("homes/second");
        write_auth_json(&second_home.join("auth.json"), "second-account");

        let state = AppState {
            active_profile: Some("second".to_string()),
            profiles: BTreeMap::from([(
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            )]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
        };
        let proxy =
            start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
                .expect("runtime proxy should start");

        let client = Client::builder().build().expect("client");
        let started = std::time::Instant::now();
        let response_started = started;
        let response = client
            .post(format!(
                "http://{}/backend-api/codex/responses",
                proxy.listen_addr
            ))
            .header("Content-Type", "application/json")
            .body("{\"input\":[]}")
            .send()
            .expect("runtime proxy request should succeed");
        let response_ready = response_started.elapsed();
        let body = response.text().expect("response body should be readable");
        let elapsed = started.elapsed();

        assert!(
            response_ready < Duration::from_millis(100),
            "runtime proxy waited too long before starting HTTP stream passthrough: {response_ready:?}"
        );
        assert!(
            elapsed >= Duration::from_millis(300),
            "slow healthy stream completed too quickly to cover multi-chunk runtime read: {elapsed:?}"
        );
        assert!(body.contains("\"response.completed\""));
        assert_eq!(
            backend.responses_accounts(),
            vec!["second-account".to_string()]
        );
    }

    #[test]
    fn runtime_proxies_bind_distinct_local_ports() {
        let backend = RuntimeProxyBackend::start();
        let temp_dir = TestDir::new();
        let main_home = temp_dir.path.join("homes/main");
        let second_home = temp_dir.path.join("homes/second");
        write_auth_json(&main_home.join("auth.json"), "main-account");
        write_auth_json(&second_home.join("auth.json"), "second-account");

        let state = AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([
                (
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                    },
                ),
            ]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
        };

        let paths_one = AppPaths {
            root: temp_dir.path.join("prodex-one"),
            state_file: temp_dir.path.join("prodex-one/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex-one/profiles"),
            shared_codex_root: temp_dir.path.join("shared-one"),
            legacy_shared_codex_root: temp_dir.path.join("prodex-one/shared"),
        };
        let paths_two = AppPaths {
            root: temp_dir.path.join("prodex-two"),
            state_file: temp_dir.path.join("prodex-two/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex-two/profiles"),
            shared_codex_root: temp_dir.path.join("shared-two"),
            legacy_shared_codex_root: temp_dir.path.join("prodex-two/shared"),
        };

        let proxy_one =
            start_runtime_rotation_proxy(&paths_one, &state, "main", backend.base_url(), false)
                .expect("first runtime proxy should start");
        let proxy_two =
            start_runtime_rotation_proxy(&paths_two, &state, "main", backend.base_url(), false)
                .expect("second runtime proxy should start");

        assert_ne!(proxy_one.listen_addr, proxy_two.listen_addr);
    }

    #[test]
    fn runtime_proxy_websocket_rotates_on_upstream_websocket_quota_error() {
        let backend = RuntimeProxyBackend::start_websocket();
        let temp_dir = TestDir::new();
        let main_home = temp_dir.path.join("homes/main");
        let second_home = temp_dir.path.join("homes/second");
        write_auth_json(&main_home.join("auth.json"), "main-account");
        write_auth_json(&second_home.join("auth.json"), "second-account");

        let state = AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([
                (
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                    },
                ),
            ]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
        };

        let paths = AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        };
        let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
            .expect("runtime proxy should start");

        let (mut socket, _response) = ws_connect(format!(
            "ws://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .expect("runtime proxy websocket handshake should succeed");
        socket
            .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
            .expect("runtime proxy websocket request should be sent");

        let mut payloads = Vec::new();
        loop {
            match socket
                .read()
                .expect("runtime proxy websocket should stay open")
            {
                WsMessage::Text(text) => {
                    let text = text.to_string();
                    let done = is_runtime_terminal_event(&text);
                    payloads.push(text);
                    if done {
                        break;
                    }
                }
                WsMessage::Ping(payload) => {
                    socket
                        .send(WsMessage::Pong(payload))
                        .expect("pong should be sent");
                }
                WsMessage::Pong(_) | WsMessage::Frame(_) => {}
                other => panic!("unexpected websocket message: {other:?}"),
            }
        }

        assert!(
            payloads
                .iter()
                .any(|payload| payload.contains("\"response.created\""))
        );
        assert!(
            payloads
                .iter()
                .any(|payload| payload.contains("\"response.completed\""))
        );
        assert!(
            !payloads
                .iter()
                .any(|payload| payload.contains("main quota exhausted"))
        );
        assert_eq!(
            backend.responses_accounts(),
            vec!["main-account".to_string(), "second-account".to_string()]
        );

        let persisted = AppState::load(&paths).expect("state should reload");
        assert_eq!(persisted.active_profile.as_deref(), Some("second"));
    }

    #[test]
    fn runtime_proxy_passes_through_upstream_websocket_error_payload() {
        let backend = RuntimeProxyBackend::start_websocket();
        let temp_dir = TestDir::new();
        let main_home = temp_dir.path.join("homes/main");
        write_auth_json(&main_home.join("auth.json"), "main-account");

        let state = AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            )]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
        };

        let paths = AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        };
        let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
            .expect("runtime proxy should start");

        let (mut socket, _response) = ws_connect(format!(
            "ws://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .expect("runtime proxy websocket handshake should succeed");
        socket
            .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
            .expect("runtime proxy websocket request should be sent");

        let payload = socket
            .read()
            .expect("runtime proxy websocket should return an upstream error payload");
        let text = payload
            .into_text()
            .expect("upstream error payload should stay text")
            .to_string();

        assert!(text.contains("\"type\":\"error\""));
        assert!(text.contains("\"status\":429"));
        assert!(text.contains("\"code\":\"insufficient_quota\""));
        assert!(text.contains("main quota exhausted"));
    }

    #[test]
    fn runtime_proxy_keeps_previous_response_affinity_for_http_requests() {
        let backend = RuntimeProxyBackend::start();
        let temp_dir = TestDir::new();
        let main_home = temp_dir.path.join("homes/main");
        let second_home = temp_dir.path.join("homes/second");
        let third_home = temp_dir.path.join("homes/third");
        write_auth_json(&main_home.join("auth.json"), "main-account");
        write_auth_json(&second_home.join("auth.json"), "second-account");
        write_auth_json(&third_home.join("auth.json"), "third-account");

        let state = AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([
                (
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                    },
                ),
                (
                    "third".to_string(),
                    ProfileEntry {
                        codex_home: third_home,
                        managed: true,
                        email: Some("third@example.com".to_string()),
                    },
                ),
            ]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
        };

        let paths = AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        };
        let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
            .expect("runtime proxy should start");
        let client = Client::builder().build().expect("client");

        let first = client
            .post(format!(
                "http://{}/backend-api/codex/responses",
                proxy.listen_addr
            ))
            .header("Content-Type", "application/json")
            .body("{\"input\":[]}")
            .send()
            .expect("first runtime proxy request should succeed");
        let first_body = first
            .text()
            .expect("first response body should be readable");
        assert!(first_body.contains("\"resp-second\""));

        let second = client
            .post(format!(
                "http://{}/backend-api/codex/responses",
                proxy.listen_addr
            ))
            .header("Content-Type", "application/json")
            .body("{\"previous_response_id\":\"resp-second\",\"input\":[]}")
            .send()
            .expect("second runtime proxy request should succeed");
        let second_body = second
            .text()
            .expect("second response body should be readable");
        assert!(second_body.contains("\"resp-second-next\""));
        assert_eq!(
            backend.responses_accounts(),
            vec![
                "main-account".to_string(),
                "second-account".to_string(),
                "second-account".to_string(),
            ]
        );
    }

    #[test]
    fn runtime_proxy_persists_previous_response_affinity_across_restart() {
        let backend = RuntimeProxyBackend::start();
        let temp_dir = TestDir::new();
        let main_home = temp_dir.path.join("homes/main");
        let second_home = temp_dir.path.join("homes/second");
        let third_home = temp_dir.path.join("homes/third");
        write_auth_json(&main_home.join("auth.json"), "main-account");
        write_auth_json(&second_home.join("auth.json"), "second-account");
        write_auth_json(&third_home.join("auth.json"), "third-account");

        let initial_state = AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([
                (
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                    },
                ),
                (
                    "third".to_string(),
                    ProfileEntry {
                        codex_home: third_home,
                        managed: true,
                        email: Some("third@example.com".to_string()),
                    },
                ),
            ]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
        };

        let paths = AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        };
        initial_state
            .save(&paths)
            .expect("failed to save initial state");

        let client = Client::builder().build().expect("client");
        {
            let proxy = start_runtime_rotation_proxy(
                &paths,
                &initial_state,
                "main",
                backend.base_url(),
                false,
            )
            .expect("runtime proxy should start");

            let first = client
                .post(format!(
                    "http://{}/backend-api/codex/responses",
                    proxy.listen_addr
                ))
                .header("Content-Type", "application/json")
                .body("{\"input\":[]}")
                .send()
                .expect("first runtime proxy request should succeed");
            let first_body = first
                .text()
                .expect("first response body should be readable");
            assert!(first_body.contains("\"resp-second\""));
        }

        let mut resumed_state = AppState::load(&paths).expect("state should reload");
        resumed_state.active_profile = Some("third".to_string());
        resumed_state
            .save(&paths)
            .expect("failed to save resumed state");

        let resumed_proxy = start_runtime_rotation_proxy(
            &paths,
            &resumed_state,
            "third",
            backend.base_url(),
            false,
        )
        .expect("resumed runtime proxy should start");

        let second = client
            .post(format!(
                "http://{}/backend-api/codex/responses",
                resumed_proxy.listen_addr
            ))
            .header("Content-Type", "application/json")
            .body("{\"previous_response_id\":\"resp-second\",\"input\":[]}")
            .send()
            .expect("second runtime proxy request should succeed");
        let second_body = second
            .text()
            .expect("second response body should be readable");
        assert!(second_body.contains("\"resp-second-next\""));
        assert_eq!(
            backend.responses_accounts(),
            vec![
                "main-account".to_string(),
                "second-account".to_string(),
                "second-account".to_string(),
            ]
        );
    }

    #[test]
    fn runtime_proxy_discovers_previous_response_owner_without_saved_binding_http() {
        let backend = RuntimeProxyBackend::start();
        let temp_dir = TestDir::new();
        let main_home = temp_dir.path.join("homes/main");
        let second_home = temp_dir.path.join("homes/second");
        let third_home = temp_dir.path.join("homes/third");
        write_auth_json(&main_home.join("auth.json"), "main-account");
        write_auth_json(&second_home.join("auth.json"), "second-account");
        write_auth_json(&third_home.join("auth.json"), "third-account");

        let state = AppState {
            active_profile: Some("third".to_string()),
            profiles: BTreeMap::from([
                (
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                    },
                ),
                (
                    "third".to_string(),
                    ProfileEntry {
                        codex_home: third_home,
                        managed: true,
                        email: Some("third@example.com".to_string()),
                    },
                ),
            ]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
        };

        let paths = AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        };
        let proxy =
            start_runtime_rotation_proxy(&paths, &state, "third", backend.base_url(), false)
                .expect("runtime proxy should start");
        let client = Client::builder().build().expect("client");

        let response = client
            .post(format!(
                "http://{}/backend-api/codex/responses",
                proxy.listen_addr
            ))
            .header("Content-Type", "application/json")
            .body("{\"previous_response_id\":\"resp-second\",\"input\":[]}")
            .send()
            .expect("runtime proxy request should succeed");
        let body = response.text().expect("response body should be readable");

        let accounts = backend.responses_accounts();
        assert!(
            body.contains("\"resp-second-next\""),
            "unexpected HTTP discovery body: {body}; accounts: {accounts:?}"
        );
        assert_eq!(
            accounts,
            vec![
                "third-account".to_string(),
                "third-account".to_string(),
                "third-account".to_string(),
                "third-account".to_string(),
                "main-account".to_string(),
                "second-account".to_string(),
            ]
        );
    }

    #[test]
    fn runtime_proxy_discovers_previous_response_owner_without_saved_binding_websocket() {
        let backend = RuntimeProxyBackend::start_websocket();
        let temp_dir = TestDir::new();
        let main_home = temp_dir.path.join("homes/main");
        let second_home = temp_dir.path.join("homes/second");
        let third_home = temp_dir.path.join("homes/third");
        write_auth_json(&main_home.join("auth.json"), "main-account");
        write_auth_json(&second_home.join("auth.json"), "second-account");
        write_auth_json(&third_home.join("auth.json"), "third-account");

        let state = AppState {
            active_profile: Some("third".to_string()),
            profiles: BTreeMap::from([
                (
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                    },
                ),
                (
                    "third".to_string(),
                    ProfileEntry {
                        codex_home: third_home,
                        managed: true,
                        email: Some("third@example.com".to_string()),
                    },
                ),
            ]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
        };

        let paths = AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        };
        let proxy =
            start_runtime_rotation_proxy(&paths, &state, "third", backend.base_url(), false)
                .expect("runtime proxy should start");

        let (mut socket, _response) = ws_connect(format!(
            "ws://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .expect("runtime proxy websocket handshake should succeed");
        socket
            .send(WsMessage::Text(
                "{\"previous_response_id\":\"resp-second\",\"input\":[]}"
                    .to_string()
                    .into(),
            ))
            .expect("runtime proxy websocket request should be sent");

        let mut payloads = Vec::new();
        loop {
            match socket
                .read()
                .expect("runtime proxy websocket should stay open")
            {
                WsMessage::Text(text) => {
                    let text = text.to_string();
                    let done = is_runtime_terminal_event(&text);
                    payloads.push(text);
                    if done {
                        break;
                    }
                }
                WsMessage::Ping(payload) => {
                    socket
                        .send(WsMessage::Pong(payload))
                        .expect("pong should be sent");
                }
                WsMessage::Pong(_) | WsMessage::Frame(_) => {}
                other => panic!("unexpected websocket message: {other:?}"),
            }
        }

        assert!(
            payloads
                .iter()
                .any(|payload| payload.contains("\"resp-second-next\"")),
            "unexpected websocket discovery payloads: {payloads:?}"
        );
        assert_eq!(
            backend.responses_accounts(),
            vec![
                "third-account".to_string(),
                "third-account".to_string(),
                "third-account".to_string(),
                "third-account".to_string(),
                "main-account".to_string(),
                "second-account".to_string(),
            ]
        );
    }

    #[test]
    fn runtime_proxy_retries_previous_response_with_upstream_turn_state_http() {
        let backend = RuntimeProxyBackend::start_http_previous_response_needs_turn_state();
        let temp_dir = TestDir::new();
        let second_home = temp_dir.path.join("homes/second");
        write_auth_json(&second_home.join("auth.json"), "second-account");

        let state = AppState {
            active_profile: Some("second".to_string()),
            profiles: BTreeMap::from([(
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            )]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
        };

        let paths = AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        };
        let proxy =
            start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
                .expect("runtime proxy should start");
        let client = Client::builder().build().expect("client");

        let response = client
            .post(format!(
                "http://{}/backend-api/codex/responses",
                proxy.listen_addr
            ))
            .header("Content-Type", "application/json")
            .body("{\"previous_response_id\":\"resp-second\",\"input\":[]}")
            .send()
            .expect("runtime proxy request should succeed");
        let body = response.text().expect("response body should be readable");

        assert!(
            body.contains("\"resp-second-next\""),
            "unexpected HTTP retry body: {body}"
        );
        assert_eq!(
            backend.responses_accounts(),
            vec!["second-account".to_string(), "second-account".to_string()]
        );
    }

    #[test]
    fn runtime_proxy_retries_previous_response_with_upstream_turn_state_websocket() {
        let backend = RuntimeProxyBackend::start_websocket_previous_response_needs_turn_state();
        let temp_dir = TestDir::new();
        let second_home = temp_dir.path.join("homes/second");
        write_auth_json(&second_home.join("auth.json"), "second-account");

        let state = AppState {
            active_profile: Some("second".to_string()),
            profiles: BTreeMap::from([(
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            )]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
        };

        let paths = AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        };
        let proxy =
            start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
                .expect("runtime proxy should start");

        let (mut socket, _response) = ws_connect(format!(
            "ws://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .expect("runtime proxy websocket handshake should succeed");
        socket
            .send(WsMessage::Text(
                "{\"previous_response_id\":\"resp-second\",\"input\":[]}"
                    .to_string()
                    .into(),
            ))
            .expect("runtime proxy websocket request should be sent");

        let mut payloads = Vec::new();
        loop {
            match socket
                .read()
                .expect("runtime proxy websocket should stay open")
            {
                WsMessage::Text(text) => {
                    let text = text.to_string();
                    let done = is_runtime_terminal_event(&text);
                    payloads.push(text);
                    if done {
                        break;
                    }
                }
                WsMessage::Ping(payload) => {
                    socket
                        .send(WsMessage::Pong(payload))
                        .expect("pong should be sent");
                }
                WsMessage::Pong(_) | WsMessage::Frame(_) => {}
                other => panic!("unexpected websocket message: {other:?}"),
            }
        }

        assert!(
            payloads
                .iter()
                .any(|payload| payload.contains("\"resp-second-next\"")),
            "unexpected websocket retry payloads: {payloads:?}"
        );
        assert_eq!(
            backend.responses_accounts(),
            vec!["second-account".to_string(), "second-account".to_string()]
        );
    }

    #[test]
    fn runtime_proxy_keeps_previous_response_affinity_for_websocket_requests() {
        let backend = RuntimeProxyBackend::start_websocket();
        let temp_dir = TestDir::new();
        let main_home = temp_dir.path.join("homes/main");
        let second_home = temp_dir.path.join("homes/second");
        let third_home = temp_dir.path.join("homes/third");
        write_auth_json(&main_home.join("auth.json"), "main-account");
        write_auth_json(&second_home.join("auth.json"), "second-account");
        write_auth_json(&third_home.join("auth.json"), "third-account");

        let state = AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([
                (
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                    },
                ),
                (
                    "third".to_string(),
                    ProfileEntry {
                        codex_home: third_home,
                        managed: true,
                        email: Some("third@example.com".to_string()),
                    },
                ),
            ]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
        };

        let paths = AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        };
        let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
            .expect("runtime proxy should start");

        let (mut socket, _response) = ws_connect(format!(
            "ws://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .expect("runtime proxy websocket handshake should succeed");
        socket
            .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
            .expect("first runtime proxy websocket request should be sent");

        let mut first_payloads = Vec::new();
        loop {
            match socket
                .read()
                .expect("runtime proxy websocket should stay open")
            {
                WsMessage::Text(text) => {
                    let text = text.to_string();
                    let done = is_runtime_terminal_event(&text);
                    first_payloads.push(text);
                    if done {
                        break;
                    }
                }
                WsMessage::Ping(payload) => {
                    socket
                        .send(WsMessage::Pong(payload))
                        .expect("pong should be sent");
                }
                WsMessage::Pong(_) | WsMessage::Frame(_) => {}
                other => panic!("unexpected websocket message: {other:?}"),
            }
        }
        assert!(
            first_payloads
                .iter()
                .any(|payload| payload.contains("\"resp-second\""))
        );

        socket
            .send(WsMessage::Text(
                "{\"previous_response_id\":\"resp-second\",\"input\":[]}"
                    .to_string()
                    .into(),
            ))
            .expect("second runtime proxy websocket request should be sent");

        let mut second_payloads = Vec::new();
        loop {
            match socket
                .read()
                .expect("runtime proxy websocket should stay open")
            {
                WsMessage::Text(text) => {
                    let text = text.to_string();
                    let done = is_runtime_terminal_event(&text);
                    second_payloads.push(text);
                    if done {
                        break;
                    }
                }
                WsMessage::Ping(payload) => {
                    socket
                        .send(WsMessage::Pong(payload))
                        .expect("pong should be sent");
                }
                WsMessage::Pong(_) | WsMessage::Frame(_) => {}
                other => panic!("unexpected websocket message: {other:?}"),
            }
        }
        assert!(
            second_payloads
                .iter()
                .any(|payload| payload.contains("\"resp-second-next\""))
        );
        assert_eq!(
            backend.responses_accounts(),
            vec!["main-account".to_string(), "second-account".to_string()]
        );
    }
}
