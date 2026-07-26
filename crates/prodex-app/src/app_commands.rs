use super::*;
use std::io::IsTerminal;

mod app_server_broker;
mod audit;
mod broker;
mod capability;
mod child_process;
mod cleanup;
mod context;
mod dashboard;
mod doctor;
mod gateway;
mod gui;
mod info;
mod info_handler;
mod log;
mod log_format;
mod log_tui;
mod log_upstream;
mod log_upstream_payload;
mod mcp_jsonl_bridge;
mod ping;
mod presidio;
mod quota;
mod redeem;
pub(crate) mod runtime_launch;
mod selection;
mod session;
mod shared;
mod status;

pub(crate) use self::app_server_broker::*;
pub(crate) use self::audit::*;
pub(crate) use self::broker::*;
pub(crate) use self::capability::*;
pub(crate) use self::child_process::*;
pub(crate) use self::cleanup::*;
pub(crate) use self::context::*;
pub(crate) use self::dashboard::*;
pub(crate) use self::doctor::*;
pub(crate) use self::gateway::*;
pub(crate) use self::gui::*;
pub(crate) use self::info::*;
pub(crate) use self::info_handler::*;
pub(crate) use self::log::*;
pub(crate) use self::mcp_jsonl_bridge::*;
pub(crate) use self::ping::*;
pub(crate) use self::presidio::*;
pub(crate) use self::quota::*;
pub(crate) use self::redeem::*;
pub(crate) use self::selection::*;
pub(crate) use self::session::*;
pub(crate) use self::shared::*;
pub(crate) use self::status::*;

pub(super) fn handle_run(args: RunArgs) -> Result<()> {
    runtime_launch::handle_run(args)
}

pub(crate) fn start_policy_gateway_backend_inner(
    preferred_listen_addr: Option<String>,
) -> Result<GatewayBackend> {
    runtime_launch::start_policy_gateway_backend_inner(preferred_listen_addr)
}

pub(crate) fn start_policy_gateway_application_inner(
    service_mode: RuntimePolicyServiceMode,
    preferred_listen_addr: Option<String>,
) -> Result<GatewayApplication> {
    runtime_launch::start_policy_gateway_application_inner(service_mode, preferred_listen_addr)
}

pub(super) fn handle_super(args: SuperArgs) -> Result<()> {
    args.validate_urls().map_err(anyhow::Error::msg)?;
    let use_presidio = if matches!(args.cli, Some(SuperCliAgent::Kiro | SuperCliAgent::Agy)) {
        false
    } else {
        match args.presidio_preference() {
            Some(use_presidio) => use_presidio,
            None => prompt_super_presidio_opt_in()?,
        }
    };
    if matches!(
        args.cli,
        Some(
            SuperCliAgent::Gemini
                | SuperCliAgent::Copilot
                | SuperCliAgent::Kiro
                | SuperCliAgent::Agy
        )
    ) {
        return crate::runtime_gemini_cli::handle_super_native_cli(args, use_presidio);
    }
    handle_runtime_tools(args.into_runtime_tool_args_with_presidio(use_presidio))
}

pub(super) fn prepare_runtime_launch_with_harness(
    request: RuntimeLaunchRequest<'_>,
    resolved_harness: prodex_provider_core::ResolvedHarnessMode,
) -> Result<PreparedRuntimeLaunch> {
    runtime_launch::prepare_runtime_launch_with_harness(request, resolved_harness)
}

pub(super) fn prepare_runtime_launch_dry_run(
    request: RuntimeLaunchRequest<'_>,
) -> Result<PreparedRuntimeLaunch> {
    runtime_launch::prepare_runtime_launch_dry_run(request)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn resolve_runtime_launch_profile_name(
    state: &AppState,
    requested: Option<&str>,
) -> Result<String> {
    runtime_launch::resolve_runtime_launch_profile_name(state, requested)
}

pub(super) fn prompt_super_presidio_opt_in() -> Result<bool> {
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        return Ok(false);
    }

    print_stderr_prompt("Use Presidio for data safety? [y/N] ")?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .context("failed to read Presidio prompt answer")?;
    Ok(super_presidio_opt_in_answer(&answer))
}

fn super_presidio_opt_in_answer(answer: &str) -> bool {
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

#[cfg(test)]
mod presidio_prompt_tests {
    use super::super_presidio_opt_in_answer;

    #[test]
    fn presidio_prompt_is_opt_in_and_accepts_yes_case_insensitively() {
        assert!(super_presidio_opt_in_answer("y\n"));
        assert!(super_presidio_opt_in_answer(" YES \n"));
        assert!(!super_presidio_opt_in_answer("\n"));
        assert!(!super_presidio_opt_in_answer("no\n"));
    }
}
