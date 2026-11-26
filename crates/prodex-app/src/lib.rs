#![recursion_limit = "256"]

use anyhow::{Context, Result, bail};
#[cfg(test)]
use base64::Engine;
use chrono::Local;
use redaction::redaction_redact_secret_like_text;
use reqwest::blocking::Client;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Cursor, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
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
#[cfg(test)]
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
mod json_boundary;
mod native_cli;
mod presidio_runtime;
mod process_resources;
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
mod runtime_opencode_auth;
mod runtime_operational_metrics;
mod runtime_panic;
mod runtime_persistence;
mod runtime_policy;
mod runtime_proxy;
mod runtime_proxy_shared;
mod runtime_request_memory;
mod runtime_save_shared;
mod runtime_secret_backend;
mod runtime_state_shared;
mod runtime_store;
mod runtime_thread_index;
mod runtime_tools;
mod secret_store_support;
mod shared_codex_fs;
mod shared_types;
mod startup;
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
use native_cli::{agy_bin, claude_bin, copilot_bin, gemini_bin, kiro_bin};
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
pub use startup::main_entry;
use terminal_ui::*;
#[cfg(test)]
pub(crate) use test_support::*;

#[cfg(test)]
#[path = "../tests/src/runtime_proxy_contract.rs"]
mod runtime_proxy_contract_tests;
#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod test_env_guard_tests;

#[cfg(test)]
#[path = "../tests/support/main_internal_harness.rs"]
mod main_internal_tests;
