use anyhow::{Context, Result};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use crate::{
    CavemanArgs, ChildProcessPlan, CodexUpdateArgs, RuntimeLaunchRequest, RuntimeProxyEndpoint,
    SUPER_LOCAL_PROVIDER_ID, codex_bin, codex_cli_config_override_value, codex_cli_profile_v2_name,
    prepare_runtime_launch_dry_run, preview_deepseek_provider_codex_args,
    preview_external_provider_catalog_codex_args, preview_gemini_provider_codex_args,
    preview_local_provider_catalog_codex_args, profile_openai_compatible_codex_args,
    runtime_caveman_extract_launch_prefixes, runtime_caveman_extract_presidio_prefix,
    runtime_launch_cli_model_context_window_tokens,
};
pub(crate) use prodex_runtime_launch::{
    RuntimeLaunchDryRunChild, codex_sandbox_removed_env, extract_prodex_dry_run_flag,
    prepare_codex_launch_args, prodex_dry_run_requested, remove_upstream_proxy_env,
};

pub(crate) fn codex_child_plan(codex_home: PathBuf, args: Vec<OsString>) -> ChildProcessPlan {
    prodex_runtime_launch::codex_child_plan(codex_bin(), codex_home, args, SUPER_LOCAL_PROVIDER_ID)
}

pub(crate) fn run_child_plan(
    plan: &ChildProcessPlan,
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
) -> Result<ExitStatus> {
    let mut command = Command::new(&plan.binary);
    command.args(&plan.args).env("CODEX_HOME", &plan.codex_home);
    for key in &plan.removed_env {
        command.env_remove(key);
    }
    for (key, value) in &plan.extra_env {
        command.env(key, value);
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to execute {}", plan.binary.to_string_lossy()))?;
    let _child_runtime_broker_lease = match runtime_proxy {
        Some(proxy) => match proxy.create_child_lease(child.id()) {
            Ok(lease) => Some(lease),
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(err);
            }
        },
        None => None,
    };
    let status = child
        .wait()
        .with_context(|| format!("failed to wait for {}", plan.binary.to_string_lossy()))?;
    Ok(status)
}

pub(crate) fn run_codex_direct_passthrough(args: Vec<OsString>) -> Result<ExitStatus> {
    let binary = codex_bin();
    let mut command = Command::new(&binary);
    command.args(args);
    for key in codex_sandbox_removed_env() {
        command.env_remove(key);
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to execute {}", binary.to_string_lossy()))?;
    let status = child
        .wait()
        .with_context(|| format!("failed to wait for {}", binary.to_string_lossy()))?;
    Ok(status)
}

pub(crate) fn handle_codex_update(args: CodexUpdateArgs) -> Result<()> {
    let mut command_args = Vec::with_capacity(args.codex_args.len() + 1);
    command_args.push(OsString::from("update"));
    command_args.extend(args.codex_args);
    exit_with_status(run_codex_direct_passthrough(command_args)?)
}

pub(crate) fn exit_with_status(status: ExitStatus) -> Result<()> {
    std::process::exit(status.code().unwrap_or(1));
}

pub(crate) fn handle_caveman_dry_run(args: CavemanArgs) -> Result<()> {
    let (_mem_mode, _rtk_enabled, _super_optimizer_overlay, codex_args) =
        runtime_caveman_extract_launch_prefixes(&args.codex_args);
    let (_presidio_enabled, codex_args) = runtime_caveman_extract_presidio_prefix(codex_args);
    let (_, codex_args) = extract_prodex_dry_run_flag(&codex_args);
    let (codex_args, include_code_review) =
        prepare_codex_launch_args(&codex_args, args.full_access);
    let model_provider_override = codex_cli_config_override_value(&codex_args, "model_provider");
    let profile_v2_name = codex_cli_profile_v2_name(&codex_args);
    let model_context_window_tokens = runtime_launch_cli_model_context_window_tokens(&codex_args);
    let request = RuntimeLaunchRequest {
        profile: args.profile.as_deref(),
        allow_auto_rotate: !args.no_auto_rotate,
        skip_quota_check: args.skip_quota_check,
        base_url: args.base_url.as_deref(),
        upstream_no_proxy: args.no_proxy,
        include_code_review,
        smart_context_enabled: args.smart_context,
        presidio_redaction_enabled: _presidio_enabled,
        model_context_window_tokens,
        force_runtime_proxy: false,
        model_provider_override: model_provider_override.as_deref(),
        profile_v2_name: profile_v2_name.as_deref(),
        external_provider: args
            .external_provider
            .map(crate::SuperExternalProvider::as_str),
        external_provider_api_key: args.external_provider_api_key.as_deref(),
    };
    print_runtime_launch_dry_run(
        "caveman",
        request,
        RuntimeLaunchDryRunChild::Caveman { codex_args },
    )
}

pub(crate) fn print_runtime_launch_dry_run(
    flow: &str,
    request: RuntimeLaunchRequest<'_>,
    child: RuntimeLaunchDryRunChild,
) -> Result<()> {
    let upstream_no_proxy = request.upstream_no_proxy;
    let presidio_redaction_enabled = request.presidio_redaction_enabled;
    let prepared = prepare_runtime_launch_dry_run(request)?;
    let runtime_proxy = runtime_proxy_codex_endpoint(prepared.runtime_proxy.as_ref());
    let child = profile_openai_compatible_dry_run_child(&prepared.codex_home, child)?;
    let plan = prodex_runtime_launch::runtime_launch_dry_run_plan(
        codex_bin(),
        &prepared.codex_home,
        &prepared.paths.managed_profiles_root,
        runtime_proxy,
        upstream_no_proxy,
        SUPER_LOCAL_PROVIDER_ID,
        child,
    );
    let mut output = prodex_runtime_launch::runtime_launch_dry_run_report(
        flow,
        &prepared.codex_home,
        runtime_proxy,
        &plan,
    );
    output.push_str(&format!(
        "Presidio redaction: {}",
        if presidio_redaction_enabled {
            "enabled"
        } else {
            "disabled"
        }
    ));
    output.push('\n');
    print!("{output}");
    Ok(())
}

fn profile_openai_compatible_dry_run_child(
    codex_home: &Path,
    child: RuntimeLaunchDryRunChild,
) -> Result<RuntimeLaunchDryRunChild> {
    match child {
        RuntimeLaunchDryRunChild::Codex { codex_args } => {
            let codex_args = profile_openai_compatible_codex_args(codex_home, &codex_args);
            let codex_args = preview_local_provider_catalog_codex_args(codex_home, &codex_args)?;
            let codex_args = preview_external_provider_catalog_codex_args(codex_home, &codex_args)?;
            let codex_args = preview_deepseek_provider_codex_args(codex_home, &codex_args)?;
            let codex_args = preview_gemini_provider_codex_args(codex_home, &codex_args)?;
            Ok(RuntimeLaunchDryRunChild::Codex { codex_args })
        }
        RuntimeLaunchDryRunChild::Caveman { codex_args } => {
            let codex_args = profile_openai_compatible_codex_args(codex_home, &codex_args);
            let codex_args = preview_local_provider_catalog_codex_args(codex_home, &codex_args)?;
            let codex_args = preview_external_provider_catalog_codex_args(codex_home, &codex_args)?;
            let codex_args = preview_deepseek_provider_codex_args(codex_home, &codex_args)?;
            let codex_args = preview_gemini_provider_codex_args(codex_home, &codex_args)?;
            Ok(RuntimeLaunchDryRunChild::Caveman { codex_args })
        }
    }
}

fn runtime_proxy_codex_endpoint(
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
) -> Option<prodex_runtime_launch::RuntimeProxyCodexEndpoint<'_>> {
    runtime_proxy.map(|proxy| prodex_runtime_launch::RuntimeProxyCodexEndpoint {
        listen_addr: proxy.listen_addr,
        openai_mount_path: &proxy.openai_mount_path,
        local_model_provider_id: proxy.local_model_provider_id.as_deref(),
    })
}
