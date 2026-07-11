pub(super) use serde_json::{Value, json};
pub(super) use std::fs;
pub(super) use std::process::Command;

#[path = "support/fixture.rs"]
mod fixture;
#[path = "support/process.rs"]
mod process;
#[cfg(unix)]
#[path = "support/pty.rs"]
mod pty;
#[path = "support/state.rs"]
mod state;
#[path = "support/temp_dir.rs"]
mod temp_dir;
#[path = "support/token.rs"]
mod token;
#[path = "support/usage_server.rs"]
mod usage_server;

pub(super) use fixture::{Fixture, setup_fixture, write_json};
pub(super) use process::{
    run_prodex, run_prodex_with_direct_provider, run_prodex_with_direct_provider_and_env,
    run_prodex_with_env, run_prodex_with_env_and_stdin, spawn_prodex_with_env,
};
#[cfg(unix)]
pub(super) use pty::run_prodex_with_pty_prompt_answer;
pub(super) use state::{
    active_profile, add_managed_profile, read_access_token, read_state,
    runtime_broker_registry_path, wait_for_runtime_broker_registry_path,
};
pub(super) use temp_dir::TestDir;
pub(super) use token::chatgpt_id_token;
pub(super) use usage_server::UsageServer;

pub(super) fn normalized_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
