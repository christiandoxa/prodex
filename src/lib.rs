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
use std::io::{self, BufRead, Cursor, Read, Write};
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
mod runtime_save_shared;
mod runtime_state_shared;
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
pub(crate) use runtime_save_shared::*;
pub(crate) use runtime_state_shared::*;
use runtime_store::*;
use shared_codex_fs::*;
pub(crate) use shared_types::*;
use terminal_ui::*;
use update_notice::*;

#[cfg(test)]
static TEST_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(test)]
thread_local! {
    static TEST_ENV_LOCK_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) struct TestEnvLockGuard {
    _guard: Option<std::sync::MutexGuard<'static, ()>>,
}

#[cfg(test)]
pub(crate) fn acquire_test_env_lock() -> TestEnvLockGuard {
    let guard = TEST_ENV_LOCK_DEPTH.with(|depth| {
        let current = depth.get();
        depth.set(current + 1);
        if current == 0 {
            Some(
                TEST_ENV_LOCK
                    .get_or_init(|| Mutex::new(()))
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()),
            )
        } else {
            None
        }
    });

    TestEnvLockGuard { _guard: guard }
}

#[cfg(test)]
pub(crate) struct TestEnvVarGuard {
    _lock: Option<TestEnvLockGuard>,
    key: Option<&'static str>,
    previous: Option<OsString>,
}

#[cfg(test)]
impl TestEnvVarGuard {
    pub(crate) fn lock() -> Self {
        Self {
            _lock: Some(acquire_test_env_lock()),
            key: None,
            previous: None,
        }
    }

    pub(crate) fn set(key: &'static str, value: &str) -> Self {
        let lock = acquire_test_env_lock();
        let previous = env::var_os(key);
        // SAFETY: test env mutation is serialized by the shared env lock guard.
        unsafe { env::set_var(key, value) };
        Self {
            _lock: Some(lock),
            key: Some(key),
            previous,
        }
    }

    pub(crate) fn unset(key: &'static str) -> Self {
        let lock = acquire_test_env_lock();
        let previous = env::var_os(key);
        // SAFETY: test env mutation is serialized by the shared env lock guard.
        unsafe { env::remove_var(key) };
        Self {
            _lock: Some(lock),
            key: Some(key),
            previous,
        }
    }
}

#[cfg(test)]
impl Drop for TestEnvVarGuard {
    fn drop(&mut self) {
        if let Some(key) = self.key {
            if let Some(value) = self.previous.as_ref() {
                // SAFETY: test env mutation is serialized by the shared env lock guard.
                unsafe { env::set_var(key, value) };
            } else {
                // SAFETY: test env mutation is serialized by the shared env lock guard.
                unsafe { env::remove_var(key) };
            }
        }
    }
}

#[cfg(test)]
impl Drop for TestEnvLockGuard {
    fn drop(&mut self) {
        TEST_ENV_LOCK_DEPTH.with(|depth| {
            let current = depth.get();
            if current > 0 {
                depth.set(current - 1);
            }
        });
    }
}

#[cfg(test)]
static TEST_RUNTIME_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(test)]
thread_local! {
    static TEST_RUNTIME_LOCK_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) struct TestRuntimeLockGuard {
    _guard: Option<std::sync::MutexGuard<'static, ()>>,
}

#[cfg(test)]
pub(crate) fn acquire_test_runtime_lock() -> TestRuntimeLockGuard {
    let guard = TEST_RUNTIME_LOCK_DEPTH.with(|depth| {
        let current = depth.get();
        depth.set(current + 1);
        if current == 0 {
            Some(
                TEST_RUNTIME_LOCK
                    .get_or_init(|| Mutex::new(()))
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()),
            )
        } else {
            None
        }
    });

    TestRuntimeLockGuard { _guard: guard }
}

#[cfg(test)]
impl Drop for TestRuntimeLockGuard {
    fn drop(&mut self) {
        TEST_RUNTIME_LOCK_DEPTH.with(|depth| {
            let current = depth.get();
            if current > 0 {
                depth.set(current - 1);
            }
        });
    }
}

#[cfg(test)]
mod test_env_guard_tests {
    use super::*;

    #[test]
    fn test_env_var_guard_restores_previous_value_and_supports_nested_reentry() {
        let key = "PRODEX_TEST_ENV_GUARD_REENTRY";
        let previous = env::var_os(key);

        {
            let _outer = TestEnvVarGuard::set(key, "outer");
            assert_eq!(env::var(key).ok().as_deref(), Some("outer"));

            {
                let _inner = TestEnvVarGuard::set(key, "inner");
                assert_eq!(env::var(key).ok().as_deref(), Some("inner"));
            }

            assert_eq!(env::var(key).ok().as_deref(), Some("outer"));
        }

        assert_eq!(env::var_os(key), previous);
    }
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
