use super::*;

mod execution;
mod plan;
mod profile;
mod proxy_args;
mod proxy_startup;

pub(super) use execution::{RuntimeLaunchStrategy, execute_runtime_launch};
use plan::cleanup_runtime_launch_plan;
pub(super) use plan::{ChildProcessPlan, RuntimeLaunchPlan};
#[cfg(test)]
pub(super) use prodex_profile_identity::validate_profile_name;
pub(super) use profile::{
    ensure_path_is_unique, record_run_selection, resolve_profile_name,
    should_enable_runtime_rotation_proxy,
};
#[cfg(test)]
pub(super) use proxy_args::normalize_run_codex_args;
pub(super) use proxy_args::runtime_proxy_codex_passthrough_args;
#[cfg(test)]
pub(super) use proxy_args::{runtime_proxy_codex_args, runtime_proxy_codex_args_with_mount_path};
#[cfg(test)]
pub(super) use proxy_startup::start_runtime_rotation_proxy;
#[cfg(test)]
pub(super) use proxy_startup::start_runtime_rotation_proxy_with_listen_addr;
pub(super) use proxy_startup::start_runtime_rotation_proxy_with_options;
pub(super) use proxy_startup::{
    RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH, start_runtime_local_rewrite_proxy,
};

pub(super) fn runtime_launch_cli_model_context_window_tokens(args: &[OsString]) -> Option<u64> {
    codex_cli_config_override_value(args, "model_context_window")
        .as_deref()
        .and_then(runtime_launch_parse_model_context_window_tokens)
}

pub(super) fn runtime_launch_config_model_context_window_tokens(codex_home: &Path) -> Option<u64> {
    let raw = fs::read_to_string(codex_home.join("config.toml")).ok()?;
    let value = raw.parse::<toml::Value>().ok()?;
    runtime_launch_toml_model_context_window_tokens(value.get("model_context_window")?)
}

fn runtime_launch_toml_model_context_window_tokens(value: &toml::Value) -> Option<u64> {
    match value {
        toml::Value::Integer(value) => u64::try_from(*value).ok().filter(|value| *value > 1),
        toml::Value::String(value) => runtime_launch_parse_model_context_window_tokens(value),
        _ => None,
    }
}

fn runtime_launch_parse_model_context_window_tokens(value: &str) -> Option<u64> {
    value.trim().parse::<u64>().ok().filter(|value| *value > 1)
}
