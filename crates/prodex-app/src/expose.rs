use crate::ExposeArgs;
use anyhow::{Context, Result};
use base64::Engine;
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use redaction::redaction_redact_secret_like_text;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use terminal_ui::print_panel;

mod ui;

use ui::{expose_json_response, expose_text_response};

const EXPOSE_BASE_PATH: &str = "/expose";
const EXPOSE_SESSION_COOKIE: &str = "prodex_expose_session";
const EXPOSE_SCROLLBACK_LIMIT: usize = 1024 * 1024;
const EXPOSE_MAX_INPUT_BYTES: usize = 16 * 1024;
const EXPOSE_MAX_MCP_BODY_BYTES: usize = 128 * 1024;
const EXPOSE_INPUT_RATE_BYTES: usize = 64 * 1024;
const EXPOSE_INPUT_RATE_REQUESTS: usize = 64;
const EXPOSE_CLIENT_QUEUE_CAPACITY: usize = 64;
const EXPOSE_REQUEST_QUEUE_CAPACITY: usize = 64;
const EXPOSE_MAX_HEADER_BYTES: usize = 16 * 1024;
const EXPOSE_MAX_HEADERS: usize = 64;
const EXPOSE_MAX_REQUEST_TARGET_BYTES: usize = 2048;
const EXPOSE_SHORT_REQUEST_WORKERS: usize = 4;
const EXPOSE_MAX_CLIENTS_LIMIT: usize = 32;
const EXPOSE_CLOUDFLARED_LINE_MAX_BYTES: usize = 8 * 1024;
const EXPOSE_BOOTSTRAP_TTL: Duration = Duration::from_secs(2 * 60);
const EXPOSE_SESSION_TTL: Duration = Duration::from_secs(15 * 60);
const EXPOSE_SESSION_IDLE_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const EXPOSE_INPUT_RATE_WINDOW: Duration = Duration::from_secs(1);
const EXPOSE_STREAM_KEEPALIVE: Duration = Duration::from_secs(15);

mod http;
mod mcp;
mod routes;
mod run_manager;
mod runtime;
mod session;
mod super_expose;

use http::*;
use mcp::*;
use routes::*;
use runtime::*;
use session::*;

pub(crate) fn handle_expose(args: ExposeArgs) -> Result<()> {
    runtime::handle_expose(args)
}

#[cfg(test)]
#[path = "expose/mcp_tests.rs"]
mod mcp_tests;
#[cfg(test)]
#[path = "expose/mcp_parallel_tests.rs"]
mod mcp_parallel_tests;
#[cfg(test)]
#[path = "expose/run_manager_tests.rs"]
mod run_manager_tests;
#[cfg(test)]
#[path = "expose/tests.rs"]
mod tests;
