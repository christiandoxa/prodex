use super::*;
use crate::TestEnvVarGuard;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[path = "main_internal/runtime_proxy_backend.rs"]
mod runtime_proxy_backend;
#[path = "main_internal/runtime_test_auth.rs"]
mod runtime_test_auth;
#[path = "main_internal/runtime_test_support.rs"]
mod runtime_test_support;
#[path = "main_internal/runtime_test_websocket.rs"]
mod runtime_test_websocket;

use runtime_proxy_backend::*;
use runtime_test_auth::*;
use runtime_test_support::*;
use runtime_test_websocket::*;

#[path = "main_internal/runtime_proxy_continuation_helpers.rs"]
mod runtime_proxy_continuation_helpers;
use runtime_proxy_continuation_helpers::*;
use crate::app_server_broker::*;

#[path = "main_internal_body/context_commands.rs"]
mod context_commands;
#[path = "main_internal_body/app_server_broker.rs"]
mod app_server_broker;
#[path = "main_internal_body/quota_selection.rs"]
mod quota_selection;
#[path = "main_internal_body/info_and_broker.rs"]
mod info_and_broker;
#[path = "main_internal_body/smart_context_and_broker.rs"]
mod smart_context_and_broker;
#[path = "main_internal_body/cli_flags.rs"]
mod cli_flags;

#[path = "main_internal_body/startup_probe.rs"]
mod startup_probe;

#[path = "main_internal/runtime_proxy_selection_and_pressure.rs"]
mod runtime_proxy_selection_and_pressure;

#[path = "main_internal_body/update_doctor_launch.rs"]
mod update_doctor_launch;

#[path = "main_internal/runtime_proxy_continuations.rs"]
mod runtime_proxy_continuations;

#[path = "main_internal_body/runtime_broker_tuning.rs"]
mod runtime_broker_tuning;

#[path = "main_internal/runtime_broker_registry.rs"]
#[cfg(unix)]
mod runtime_broker_registry;

#[path = "main_internal_body/claude_launch.rs"]
mod claude_launch;

#[path = "main_internal/runtime_proxy_claude_and_anthropic.rs"]
mod runtime_proxy_claude_and_anthropic;
