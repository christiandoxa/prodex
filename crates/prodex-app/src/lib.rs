#![recursion_limit = "256"]

use anyhow::{Context, Result, bail};
#[cfg(test)]
use base64::Engine;
use chrono::Local;
#[cfg(test)]
use chrono::TimeZone;
use reqwest::blocking::Client;
#[cfg(test)]
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::ffi::OsString;
use std::fs;
#[cfg(test)]
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
mod app_server_broker;
mod app_state;
mod audit_log;
mod cli_args;
mod command_dispatch;
mod core_constants;
mod dashboard;
mod dashboard_html;
mod expose;
mod housekeeping;
mod presidio_runtime;
mod profile_commands;
mod profile_identity;
mod profile_local_config;
mod proxy_config;
mod quota_support;
mod runtime_anthropic;
mod runtime_background;
mod runtime_broker;
mod runtime_broker_shared;
mod runtime_capabilities;
mod runtime_caveman;
mod runtime_claude;
mod runtime_claude_auth;
#[path = "runtime_tuning.rs"]
mod runtime_config;
mod runtime_core_shared;
mod runtime_deepseek_config;
mod runtime_doctor;
mod runtime_external_provider_config;
mod runtime_gemini_auth;
mod runtime_gemini_cli;
mod runtime_gemini_cli_compat;
mod runtime_gemini_config;
#[allow(dead_code)]
mod runtime_kiro_acp;
mod runtime_launch;
mod runtime_launch_shared;
mod runtime_local_provider_config;
mod runtime_panic;
mod runtime_persistence;
mod runtime_policy;
mod runtime_proxy;
mod runtime_proxy_shared;
mod runtime_save_shared;
mod runtime_state_shared;
mod runtime_store;
mod shared_codex_fs;
mod shared_types;
#[cfg(test)]
mod test_support;
mod update_notice;

#[cfg(feature = "bench-support")]
#[doc(hidden)]
pub mod bench_support;

use app_commands::*;
pub(crate) use app_state::*;
use audit_log::*;
pub(crate) use cli_args::*;
pub(crate) use codex_config::*;
use command_dispatch::CommandDispatchExt;
pub(crate) use core_constants::*;
use dashboard::*;
use expose::*;
use housekeeping::*;
pub(crate) use presidio_runtime::*;
pub(crate) use prodex_core::AppPaths;
use profile_commands::*;
use profile_identity::*;
use profile_local_config::*;
use proxy_config::*;
use quota_support::*;
use runtime_anthropic::*;
use runtime_background::*;
use runtime_broker::*;
use runtime_broker_shared::*;
use runtime_capabilities::*;
use runtime_caveman::*;
use runtime_claude::*;
use runtime_claude_auth::*;
use runtime_config::*;
use runtime_core_shared::*;
use runtime_deepseek_config::*;
use runtime_doctor::*;
use runtime_external_provider_config::*;
use runtime_gemini_auth::*;
use runtime_gemini_cli_compat::*;
use runtime_gemini_config::*;
use runtime_launch::*;
use runtime_launch_shared::*;
use runtime_local_provider_config::*;
use runtime_persistence::*;
use runtime_policy::*;
use runtime_proxy::*;
use runtime_proxy_shared::*;
pub(crate) use runtime_save_shared::*;
pub(crate) use runtime_state_shared::*;
use runtime_store::*;
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
    server: Arc<TinyServer>,
    shutdown: Arc<AtomicBool>,
    worker_threads: Vec<thread::JoinHandle<()>>,
    accept_worker_count: usize,
    listen_addr: std::net::SocketAddr,
    gemini_live_sidecar_addr: Option<std::net::SocketAddr>,
    gemini_live_sidecar_model: Option<String>,
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
        std::process::exit(main_entry_exit_code(&err));
    }
}

fn main_entry_exit_code(err: &anyhow::Error) -> i32 {
    err.downcast_ref::<command_dispatch::ProdexCommandExit>()
        .map_or(1, command_dispatch::ProdexCommandExit::code)
}

fn run() -> Result<()> {
    let command = parse_cli_command_or_exit();
    if command.should_show_update_notice() {
        let _ = show_update_notice_if_available(&command);
    }
    ensure_runtime_policy_valid()?;
    schedule_prodex_auto_runtime_housekeeping(&command);
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
    prodex_cli::parse_cli_command_from(args)
}

fn codex_bin() -> OsString {
    env::var_os("PRODEX_CODEX_BIN").unwrap_or_else(|| OsString::from("codex"))
}

fn claude_bin() -> OsString {
    env::var_os("PRODEX_CLAUDE_BIN").unwrap_or_else(|| OsString::from("claude"))
}

fn gemini_bin() -> OsString {
    env::var_os("PRODEX_GEMINI_BIN").unwrap_or_else(|| OsString::from("gemini"))
}

fn agy_bin() -> OsString {
    env::var_os("PRODEX_AGY_BIN").unwrap_or_else(|| OsString::from("agy"))
}

fn command_exists_on_path(command: &str) -> bool {
    env::var_os("PATH")
        .is_some_and(|paths| env::split_paths(&paths).any(|dir| dir.join(command).is_file()))
}

fn kiro_bin() -> OsString {
    if let Some(path) = env::var_os("PRODEX_KIRO_BIN") {
        return path;
    }
    if command_exists_on_path("kiro-cli-chat") {
        return OsString::from("kiro-cli-chat");
    }
    if command_exists_on_path("kiro-cli") {
        return OsString::from("kiro-cli");
    }
    OsString::from("kiro-cli-chat")
}

#[cfg(test)]
mod kiro_bin_tests {
    use super::kiro_bin;
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn with_env_var(key: &str, value: Option<OsString>, f: impl FnOnce()) {
        let previous = env::var_os(key);
        match value {
            Some(value) => unsafe { env::set_var(key, value) },
            None => unsafe { env::remove_var(key) },
        }
        f();
        match previous {
            Some(value) => unsafe { env::set_var(key, value) },
            None => unsafe { env::remove_var(key) },
        }
    }

    #[test]
    fn kiro_bin_prefers_explicit_override() {
        with_env_var(
            "PRODEX_KIRO_BIN",
            Some(OsString::from("/tmp/custom-kiro")),
            || assert_eq!(kiro_bin(), OsString::from("/tmp/custom-kiro")),
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

        let previous_path = env::var_os("PATH");
        with_env_var("PRODEX_KIRO_BIN", None, || {
            unsafe { env::set_var("PATH", &root) };
            assert_eq!(kiro_bin(), OsString::from("kiro-cli"));
        });
        match previous_path {
            Some(value) => unsafe { env::set_var("PATH", value) },
            None => unsafe { env::remove_var("PATH") },
        }
        let _ = fs::remove_dir_all(root);
    }
}

#[cfg(test)]
#[path = "../tests/support/main_internal_harness.rs"]
mod main_internal_tests;

#[cfg(test)]
#[path = "../tests/support/compat_replay_body.rs"]
mod compat_replay_tests;
