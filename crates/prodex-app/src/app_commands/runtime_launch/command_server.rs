use super::{
    AppPaths, AppState, AppStateIoExt, ChildProcessPlan, CodexRuntimeFeatureArgs,
    PreparedRuntimeLaunch, Result, RunArgs, RunCommandStrategy, RuntimeLaunchPreparationBuilder,
    RuntimeLaunchRequest, RuntimeLaunchSelection, RuntimeLaunchStrategy, RuntimeProxyEndpoint,
    exit_with_status, is_codex_command_server_subcommand, prodex_dry_run_requested, run_child_plan,
    validate_runtime_launch_upstream_base_url,
};
use std::ffi::OsString;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RunLaunchRoute {
    ManagedRuntime,
    CodexCommandServerManagedStdio,
}

pub(super) fn run_launch_route(args: &RunArgs) -> RunLaunchRoute {
    if !args.dry_run
        && !prodex_dry_run_requested(&args.codex_args)
        && is_codex_command_server_subcommand(&args.codex_args)
    {
        return RunLaunchRoute::CodexCommandServerManagedStdio;
    }
    RunLaunchRoute::ManagedRuntime
}

pub(super) fn execute_codex_command_server_managed_runtime(
    mut strategy: RunCommandStrategy,
) -> Result<()> {
    let prepared = prepare_codex_command_server_runtime_launch(strategy.runtime_request())?;
    let plan = strategy.build_plan(&prepared, prepared.runtime_proxy.as_ref())?;
    let runtime_proxy = prepared.runtime_proxy;
    let status = match run_child_plan(&plan.child, runtime_proxy.as_ref()) {
        Ok(status) => status,
        Err(err) => {
            prodex_runtime_launch::cleanup_runtime_launch_plan(&plan);
            return Err(err);
        }
    };
    drop(runtime_proxy);
    let after_result = strategy.after_child_exit(&status);
    prodex_runtime_launch::cleanup_runtime_launch_plan(&plan);
    after_result?;
    exit_with_status(status)
}

pub(in crate::app_commands) fn codex_app_server_broker_launch(
    profile: Option<&str>,
) -> Result<(ChildProcessPlan, Option<RuntimeProxyEndpoint>)> {
    let strategy = RunCommandStrategy::new(RunArgs {
        profile: profile.map(str::to_string),
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: true,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![OsString::from("app-server")],
    })?;
    let prepared = prepare_codex_command_server_runtime_launch(strategy.runtime_request())?;
    let plan = strategy.build_plan(&prepared, prepared.runtime_proxy.as_ref())?;
    Ok((plan.child, prepared.runtime_proxy))
}

pub(super) fn prepare_codex_command_server_runtime_launch(
    request: RuntimeLaunchRequest<'_>,
) -> Result<PreparedRuntimeLaunch> {
    let paths = AppPaths::discover()?;
    let state = AppState::load_and_repair(&paths)?;
    let selection = RuntimeLaunchSelection::resolve(
        &paths,
        &state,
        request.profile,
        request.model_provider_override,
        request.profile_v2_name,
        request.external_provider,
        request.external_provider_api_key,
    )?;
    validate_runtime_launch_upstream_base_url(&selection, &request)?;
    RuntimeLaunchPreparationBuilder {
        request,
        resolved_harness: prodex_provider_core::resolve_harness_mode(None, None),
        paths,
        state,
        selection,
    }
    .build_with_terminal_output(false)
}
