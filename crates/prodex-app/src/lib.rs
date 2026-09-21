#![recursion_limit = "256"]

use anyhow::{Context, Result, bail};
#[cfg(test)]
use base64::Engine;
use chrono::Local;
use redaction::redaction_redact_secret_like_text;
use reqwest::blocking::Client;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Cursor, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TrySendError};
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
mod codex_binary;
mod command_dispatch;
mod core_constants;
mod housekeeping;
mod presidio_runtime;
mod profile_commands;
mod profile_identity;
mod profile_local_config;
mod proxy_config;
mod quota_support;
mod reports;
mod runtime_allocator;
mod runtime_background;
mod runtime_broker;
mod runtime_broker_shared;
mod runtime_capabilities;
mod runtime_catalog_config;
mod runtime_claude_auth;
mod runtime_config;
mod runtime_core_shared;
mod runtime_deepseek_config;
mod runtime_doctor;
mod runtime_external_provider_config;
mod runtime_gemini_auth;
mod runtime_gemini_config;
mod runtime_kiro_acp;
mod runtime_launch;
mod runtime_launch_shared;
mod runtime_local_provider_config;
mod runtime_operational_metrics;
mod runtime_panic;
mod runtime_persistence;
mod runtime_policy;
mod runtime_proxy;
mod runtime_proxy_shared;
mod runtime_save_shared;
mod runtime_secret_backend;
mod runtime_state_shared;
mod runtime_store;
mod runtime_thread_index;
mod runtime_tools;
mod secret_store_support;
mod shared_codex_fs;
mod shared_types;
mod super_expose;
#[cfg(test)]
mod test_support;
mod update_notice;

#[cfg(feature = "bench-support")]
#[doc(hidden)]
pub mod bench_support;

#[cfg(feature = "allocation-bench-support")]
#[doc(hidden)]
pub mod allocation_bench_support {
    pub use prodex_bench_support::{RuntimeAllocationSnapshot, runtime_allocation_snapshot};
}

use app_commands::*;
pub(crate) use app_state::*;
pub(crate) use cli_args::*;
pub(crate) use codex_binary::{codex_bin, validate_selected_codex_binary};
pub(crate) use codex_config::*;
pub(crate) use core_constants::*;
use housekeeping::*;
pub(crate) use presidio_runtime::*;
pub(crate) use prodex_core::AppPaths;
use profile_commands::*;
use profile_identity::*;
use profile_local_config::*;
use proxy_config::*;
use quota_support::*;
use runtime_background::*;
use runtime_broker::*;
use runtime_broker_shared::*;
use runtime_capabilities::*;
use runtime_claude_auth::*;
use runtime_config::*;
use runtime_core_shared::*;
use runtime_deepseek_config::*;
use runtime_doctor::*;
use runtime_external_provider_config::*;
use runtime_gemini_auth::*;
use runtime_gemini_config::*;
use runtime_launch::*;
use runtime_launch_shared::*;
use runtime_local_provider_config::*;
pub(crate) use runtime_operational_metrics::*;
use runtime_persistence::*;
use runtime_policy::*;
use runtime_proxy::*;
use runtime_proxy_shared::*;
pub(crate) use runtime_save_shared::*;
pub(crate) use runtime_secret_backend::*;
pub(crate) use runtime_state_shared::*;
use runtime_store::*;
pub(crate) use runtime_thread_index::*;
use runtime_tools::*;
use shared_codex_fs::*;
pub(crate) use shared_types::*;
use terminal_ui::*;
#[cfg(test)]
pub(crate) use test_support::*;
use update_notice::*;

#[cfg(test)]
#[path = "../tests/src/runtime_proxy_contract.rs"]
mod runtime_proxy_contract_tests;
#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod test_env_guard_tests;

struct RuntimeRotationProxy {
    runtime_config: Arc<RuntimeConfig>,
    server: Arc<TinyServer>,
    draining: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    worker_threads: Vec<thread::JoinHandle<()>>,
    accept_worker_count: usize,
    listen_addr: std::net::SocketAddr,
    realtime_ws_sidecar_addr: Option<std::net::SocketAddr>,
    realtime_ws_model: Option<String>,
    log_path: PathBuf,
    active_request_count: Arc<AtomicUsize>,
    owner_lock: Option<StateFileLock>,
    _live_log_source: Option<RuntimeLiveLogSourceGuard>,
    _marker_guard: RuntimeProxyMarkerGuard,
}

type RuntimeLocalWebSocket = WsSocket<Box<dyn TinyReadWrite + Send>>;
type RuntimeUpstreamWebSocket = WsSocket<MaybeTlsStream<TcpStream>>;

fn runtime_set_upstream_websocket_io_timeout(
    socket: &mut RuntimeUpstreamWebSocket,
    timeout: Option<Duration>,
) -> io::Result<()> {
    runtime_set_upstream_websocket_read_timeout(socket, timeout)?;
    match socket.get_mut() {
        MaybeTlsStream::Plain(stream) => {
            stream.set_write_timeout(timeout)?;
        }
        MaybeTlsStream::Rustls(stream) => {
            stream.sock.set_write_timeout(timeout)?;
        }
        _ => {}
    }
    Ok(())
}

fn runtime_set_upstream_websocket_read_timeout(
    socket: &mut RuntimeUpstreamWebSocket,
    timeout: Option<Duration>,
) -> io::Result<()> {
    match socket.get_mut() {
        MaybeTlsStream::Plain(stream) => stream.set_read_timeout(timeout),
        MaybeTlsStream::Rustls(stream) => stream.sock.set_read_timeout(timeout),
        _ => Ok(()),
    }
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

const RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS: u64 = 32;
const RUNTIME_REQUEST_SEQUENCE_LOCAL_START: u64 = 1;

fn runtime_proxy_request_id_from_entropy(request_id_entropy: u128, local_sequence: u64) -> u64 {
    let local_mask = (1_u64 << RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS) - 1;
    let mut hasher = DefaultHasher::new();
    request_id_entropy.hash(&mut hasher);
    local_sequence.hash(&mut hasher);
    let mut request_prefix = hasher.finish() >> RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS;
    if request_prefix == 0 {
        request_prefix = 1;
    }
    (request_prefix << RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS) | (local_sequence & local_mask)
}

fn runtime_proxy_request_sequence_seed(log_path: &Path) -> u64 {
    let request_id_entropy = prodex_domain::RequestId::new().as_uuid().as_u128();
    let pid = std::process::id();
    let thread_id = thread::current().id();
    let mut hasher = DefaultHasher::new();
    request_id_entropy.hash(&mut hasher);
    pid.hash(&mut hasher);
    thread_id.hash(&mut hasher);
    log_path.hash(&mut hasher);
    let mut instance_prefix = hasher.finish() >> RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS;
    if instance_prefix == 0 {
        instance_prefix = 1;
    }
    (instance_prefix << RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS) | RUNTIME_REQUEST_SEQUENCE_LOCAL_START
}

fn runtime_proxy_next_request_id(shared: &RuntimeRotationProxyShared) -> u64 {
    let local_sequence = shared.request_sequence.fetch_add(1, Ordering::Relaxed);
    runtime_proxy_request_id_from_entropy(
        prodex_domain::RequestId::new().as_uuid().as_u128(),
        local_sequence,
    )
}

#[cfg(test)]
mod runtime_request_id_tests {
    use super::{
        RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS, RUNTIME_REQUEST_SEQUENCE_LOCAL_START,
        runtime_proxy_request_id_from_entropy, runtime_proxy_request_sequence_seed,
    };
    use std::path::Path;

    #[test]
    fn runtime_request_sequence_seed_reserves_instance_high_bits() {
        let seed =
            runtime_proxy_request_sequence_seed(Path::new("/tmp/prodex-runtime-instance-a.log"));

        assert_eq!(
            seed & ((1_u64 << RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS) - 1),
            RUNTIME_REQUEST_SEQUENCE_LOCAL_START
        );
        assert_ne!(seed >> RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS, 0);
    }

    #[test]
    fn runtime_request_sequence_seed_includes_runtime_identity_material() {
        let first =
            runtime_proxy_request_sequence_seed(Path::new("/tmp/prodex-runtime-instance-a.log"));
        let second =
            runtime_proxy_request_sequence_seed(Path::new("/tmp/prodex-runtime-instance-b.log"));

        assert_ne!(
            first >> RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS,
            second >> RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS
        );
    }

    #[test]
    fn runtime_request_id_uses_entropy_for_high_bits() {
        let local_sequence = 42;
        let first = runtime_proxy_request_id_from_entropy(1, local_sequence);
        let second = runtime_proxy_request_id_from_entropy(2, local_sequence);

        assert_eq!(
            first & ((1_u64 << RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS) - 1),
            local_sequence
        );
        assert_eq!(
            second & ((1_u64 << RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS) - 1),
            local_sequence
        );
        assert_ne!(
            first >> RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS,
            second >> RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS
        );
    }
}

pub fn main_entry() {
    let result = match app_commands::runtime_launch::goal_resume::handle_runtime_goal_session_notify_if_requested() {
        Ok(true) => Ok(()),
        Ok(false) => run(),
        Err(err) => Err(err),
    };
    if let Err(err) = result {
        if main_entry_error_is_broken_pipe(&err) {
            return;
        }
        let _ = writeln!(
            io::stderr().lock(),
            "Error: {}",
            main_entry_error_message(&err)
        );
        std::process::exit(main_entry_exit_code(&err));
    }
}

fn main_entry_error_is_broken_pipe(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        cause
            .downcast_ref::<io::Error>()
            .is_some_and(|error| error.kind() == io::ErrorKind::BrokenPipe)
    })
}

fn main_entry_exit_code(err: &anyhow::Error) -> i32 {
    err.downcast_ref::<command_dispatch::ProdexCommandExit>()
        .map_or(1, command_dispatch::ProdexCommandExit::code)
}

fn main_entry_error_message(err: &anyhow::Error) -> String {
    redaction_redact_secret_like_text(&format!("{err:#}"))
}

fn run() -> Result<()> {
    let command = parse_cli_command_or_exit();
    run_command(command)
}

fn run_command(command: Commands) -> Result<()> {
    let super_dry_run = command_dispatch::command_is_super_dry_run(&command);
    let minimal_startup = command_uses_minimal_startup(&command);
    if !super_dry_run && !minimal_startup {
        create_codex_home_if_missing(&AppPaths::discover()?.root)?;
        if command_dispatch::command_should_show_update_notice(&command) {
            let _ = show_update_notice_if_available(&command);
        }
    }
    validate_command_runtime_policy(&command)?;
    if !super_dry_run && !minimal_startup {
        schedule_prodex_auto_runtime_housekeeping(&command);
    }
    command_dispatch::execute_command(command)
}

fn command_uses_minimal_startup(command: &Commands) -> bool {
    command_dispatch::command_uses_native_agy(command)
        || matches!(command, Commands::McpJsonlBridge(_) | Commands::Ping(_))
        || matches!(
            command,
            Commands::Doctor(args)
                if args.install
                    && !args.quota
                    && !args.runtime
                    && !args.repair_import_auth_journals
                    && args.bundle.is_none()
        )
}

#[cfg(test)]
mod minimal_startup_tests {
    use super::{command_uses_minimal_startup, parse_cli_command_from};

    #[test]
    fn doctor_install_alone_uses_minimal_startup() {
        let install = parse_cli_command_from(["prodex", "doctor", "--install"]).unwrap();
        let combined =
            parse_cli_command_from(["prodex", "doctor", "--install", "--runtime"]).unwrap();
        let bridge =
            parse_cli_command_from(["prodex", "__mcp-jsonl-bridge", "mcp-server"]).unwrap();
        let ping = parse_cli_command_from(["prodex", "ping", "openai"]).unwrap();
        let antigravity =
            parse_cli_command_from(["prodex", "s", "gemini", "--cli", "agy", "--no-presidio"])
                .unwrap();

        assert!(command_uses_minimal_startup(&install));
        assert!(command_uses_minimal_startup(&bridge));
        assert!(command_uses_minimal_startup(&ping));
        assert!(command_uses_minimal_startup(&antigravity));
        assert!(!command_uses_minimal_startup(&combined));
    }
}

fn validate_command_runtime_policy(command: &Commands) -> Result<()> {
    if command.launches_runtime() && !command_dispatch::command_uses_native_agy(command) {
        ensure_runtime_policy_valid()?;
    }
    Ok(())
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
    prodex_cli::parse_cli_command_from(args)
}

fn claude_bin() -> OsString {
    env::var_os("PRODEX_CLAUDE_BIN").unwrap_or_else(|| OsString::from("claude"))
}

fn gemini_bin() -> OsString {
    env::var_os("PRODEX_GEMINI_BIN").unwrap_or_else(|| OsString::from("gemini"))
}

fn copilot_bin() -> OsString {
    env::var_os("PRODEX_COPILOT_BIN").unwrap_or_else(|| OsString::from("copilot"))
}

fn agy_bin() -> OsString {
    env::var_os("PRODEX_AGY_BIN").unwrap_or_else(|| OsString::from("agy"))
}

fn command_exists_on_path(command: &str, paths: Option<&std::ffi::OsStr>) -> bool {
    prodex_core::resolve_binary_path_in_path(&OsString::from(command), paths).is_some()
}

fn kiro_bin() -> OsString {
    kiro_bin_from(env::var_os("PRODEX_KIRO_BIN"), env::var_os("PATH"))
}

fn kiro_bin_from(override_path: Option<OsString>, paths: Option<OsString>) -> OsString {
    if let Some(path) = override_path {
        return path;
    }
    if command_exists_on_path("kiro-cli-chat", paths.as_deref()) {
        return OsString::from("kiro-cli-chat");
    }
    if command_exists_on_path("kiro-cli", paths.as_deref()) {
        return OsString::from("kiro-cli");
    }
    OsString::from("kiro-cli-chat")
}

#[cfg(test)]
mod kiro_bin_tests {
    use super::kiro_bin_from;
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn kiro_bin_prefers_explicit_override() {
        assert_eq!(
            kiro_bin_from(Some(OsString::from("/tmp/custom-kiro")), None),
            OsString::from("/tmp/custom-kiro")
        );
    }

    #[test]
    fn kiro_bin_falls_back_to_kiro_cli_when_chat_binary_is_missing() {
        let root = env::temp_dir().join(format!(
            "prodex-kiro-bin-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("temp dir should exist");
        fs::write(
            root.join("kiro-cli"),
            b"#!/bin/sh
",
        )
        .expect("fake binary should write");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(root.join("kiro-cli"), fs::Permissions::from_mode(0o755))
                .expect("fake binary should be executable");
        }

        assert_eq!(
            kiro_bin_from(None, Some(root.clone().into_os_string())),
            OsString::from("kiro-cli")
        );
        let _ = fs::remove_dir_all(root);
    }
}

#[cfg(test)]
#[path = "../tests/support/main_internal_harness.rs"]
mod main_internal_tests;
