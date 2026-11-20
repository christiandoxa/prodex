use super::{
    AppPaths, AppState, AppStateIoExt, PreparedRuntimeLaunch, Result, RunArgs, RunCommandStrategy,
    RuntimeLaunchDryRunChild, RuntimeLaunchPreparationBuilder, RuntimeLaunchRequest,
    RuntimeLaunchSelection, RuntimeLaunchStrategy, codex_child_plan, default_codex_home,
    execute_runtime_launch, exit_with_status, extract_prodex_dry_run_flag,
    is_codex_command_server_subcommand, print_runtime_launch_dry_run,
    print_runtime_launch_dry_run_report, prodex_dry_run_requested, remove_upstream_proxy_env,
    run_child_plan, validate_credential_free_http_url, validate_runtime_launch_upstream_base_url,
};
use std::ffi::OsString;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunLaunchRoute {
    ManagedRuntime,
    CodexCommandServerManagedStdio,
}

pub(crate) fn handle_run(args: RunArgs) -> Result<()> {
    if codex_remote_exec_requested(&args.codex_args) {
        return execute_codex_remote_exec(args);
    }
    if let Some(base_url) = args.base_url.as_deref() {
        validate_credential_free_http_url(base_url, "runtime upstream base URL")?;
    }
    let route = run_launch_route(&args);
    let strategy = RunCommandStrategy::new(args)?;
    if strategy.dry_run {
        return print_runtime_launch_dry_run(
            "run",
            strategy.runtime_request(),
            RuntimeLaunchDryRunChild::Codex {
                codex_args: strategy.codex_args.clone(),
            },
            None,
        );
    }
    match route {
        RunLaunchRoute::ManagedRuntime => execute_runtime_launch(strategy),
        RunLaunchRoute::CodexCommandServerManagedStdio => {
            execute_codex_command_server_managed_runtime(strategy)
        }
    }
}

fn run_launch_route(args: &RunArgs) -> RunLaunchRoute {
    if !args.dry_run
        && !prodex_dry_run_requested(&args.codex_args)
        && is_codex_command_server_subcommand(&args.codex_args)
    {
        return RunLaunchRoute::CodexCommandServerManagedStdio;
    }
    RunLaunchRoute::ManagedRuntime
}

fn codex_remote_exec_requested(args: &[OsString]) -> bool {
    prodex_runtime_launch::is_codex_exec_invocation(args)
        && crate::runtime_tools::runtime_overlay_transport_flags(args).remote
}

fn execute_codex_remote_exec(args: RunArgs) -> Result<()> {
    anyhow::ensure!(
        args.profile.is_none()
            && args.base_url.is_none()
            && !args.auto_rotate
            && !args.auto_redeem
            && !args.full_access,
        "configure the profile, upstream URL, rotation, redemption, and permissions on the remote app server"
    );
    let (dry_run, codex_args) =
        extract_prodex_dry_run_flag(&args.codex_args_with_feature_overrides()?);
    if dry_run || args.dry_run {
        return print_runtime_launch_dry_run_report(
            "run",
            "Codex exec uses an existing app server. The server owns session state and inference.\n",
        );
    }
    // Native remote exec rejects client configuration overrides. Keep local inference setup out of its launch.
    let home = default_codex_home(&AppPaths::discover()?)?;
    let mut child = codex_child_plan(home, codex_args);
    if args.no_proxy {
        remove_upstream_proxy_env(&mut child);
    }
    exit_with_status(run_child_plan(&child, None)?)
}

fn execute_codex_command_server_managed_runtime(mut strategy: RunCommandStrategy) -> Result<()> {
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
    let after_result = strategy.after_child_exit(&status, &plan);
    prodex_runtime_launch::cleanup_runtime_launch_plan(&plan);
    after_result?;
    exit_with_status(status)
}

fn prepare_codex_command_server_runtime_launch(
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
        paths,
        state,
        selection,
    }
    .build_with_terminal_output(false)
}
