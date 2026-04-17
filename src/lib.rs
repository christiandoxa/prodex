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
mod app_state;
mod audit_log;
mod cli_args;
mod command_dispatch;
mod core_constants;
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
mod shared_types;
#[path = "cli_render.rs"]
mod terminal_ui;
mod update_notice;

use app_commands::*;
pub(crate) use app_state::*;
use audit_log::*;
pub(crate) use cli_args::*;
pub(crate) use core_constants::*;
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
pub(crate) use shared_types::*;
use terminal_ui::*;
use update_notice::*;

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
