use super::{
    AppPaths, PreparedRuntimeLaunch, RuntimeLaunchPlan, RuntimeProxyEndpoint,
    RuntimeToolLaunchStrategy, ensure_presidio_services_for_super_launch,
    ensure_required_presidio_services_for_super_launch, prepare_desktop_overlay_home,
    prepare_runtime_overlay_home, redaction_redact_secret_like_text,
    write_provider_runtime_codex_auth,
};
use crate::app_commands::runtime_launch::goal_resume::add_runtime_goal_session_tracking;
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub(crate) struct RuntimeOverlayCleanup(Option<PathBuf>);

impl RuntimeOverlayCleanup {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self(Some(path))
    }

    pub(crate) fn keep(mut self) -> PathBuf {
        self.0.take().expect("runtime overlay cleanup path missing")
    }
}

impl Drop for RuntimeOverlayCleanup {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

pub(crate) fn resolve_runtime_optional_tool_plan(
    selected_tools: &prodex_optional_tools::OptionalToolSet,
    required_tools: &prodex_optional_tools::OptionalToolSet,
) -> Result<prodex_optional_tools::ToolActivationPlan> {
    let selected = selected_tools
        .iter()
        .filter(|tool| *tool != prodex_optional_tools::OptionalToolId::Presidio)
        .collect();
    let required = required_tools
        .iter()
        .filter(|tool| *tool != prodex_optional_tools::OptionalToolId::Presidio)
        .collect();
    let plan = prodex_optional_tools::resolve_optional_tools_for_launch(&selected, &required);
    if let Some(unavailable) = plan
        .unavailable
        .iter()
        .find(|health| required.contains(health.id))
    {
        bail!(
            "required optional tool {} is unavailable: {}; run `prodex capability super-doctor`",
            unavailable.id,
            redaction_redact_secret_like_text(&unavailable.detail)
        );
    }
    Ok(plan)
}

pub(crate) fn project_in_app_resume_model_settings(
    codex_home: &Path,
    codex_args: &mut Vec<std::ffi::OsString>,
    profile_v2_name: Option<&str>,
    settings_to_project: [(&str, bool); 3],
) -> Result<()> {
    // Named-profile values outrank config.toml. Preserve the CLI projection without mutating the
    // user-authored profile; ordinary launches still localize the settings below.
    if profile_v2_name.is_some() {
        return Ok(());
    }
    let mut projected_args = codex_args.clone();
    let mut settings = Vec::new();
    for (key, should_project) in settings_to_project {
        if !should_project {
            continue;
        }
        let Some(value) = crate::codex_cli_config_override_value(codex_args, key) else {
            continue;
        };
        let mut removed = false;
        while crate::app_commands::runtime_launch::remove_first_codex_config_override_pair(
            &mut projected_args,
            key,
        ) {
            removed = true;
        }
        if removed {
            settings.push((key, value));
        }
    }
    if settings.is_empty() {
        return Ok(());
    }

    let config_args = settings
        .into_iter()
        .flat_map(|(key, value)| {
            [
                std::ffi::OsString::from("-c"),
                std::ffi::OsString::from(format!(
                    "{key}={}",
                    crate::runtime_catalog_config::toml_string_literal(&value)
                )),
            ]
        })
        .collect::<Vec<_>>();
    crate::runtime_desktop::configure_desktop_codex_home(codex_home, &config_args, false, None)?;
    *codex_args = projected_args;
    Ok(())
}

struct PreparedOverlayLaunch {
    cleanup: RuntimeOverlayCleanup,
    overlay_home: PathBuf,
    tool_plan: prodex_optional_tools::ToolActivationPlan,
    preference_context: crate::ModelPreferenceContext,
    runtime_args: Vec<std::ffi::OsString>,
}

pub(super) fn build_plan(
    strategy: &mut RuntimeToolLaunchStrategy,
    prepared: &PreparedRuntimeLaunch,
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
) -> Result<RuntimeLaunchPlan> {
    let PreparedOverlayLaunch {
        cleanup,
        overlay_home,
        tool_plan,
        preference_context,
        mut runtime_args,
    } = prepare_overlay_launch(strategy, prepared, runtime_proxy)?;
    if let Some(sub_agent) = strategy.sub_agent.as_ref() {
        super::write_sub_agent_overlay(&overlay_home, sub_agent)?;
    }
    let stage_started = Instant::now();
    prodex_optional_tools::activate_optional_tools_for_codex(
        &overlay_home,
        &tool_plan,
        strategy.presidio_enabled,
    )?;
    crate::runtime_launch::emit_runtime_timing(
        "startup.optional_tool_activation_ms",
        stage_started,
    );
    #[cfg(unix)]
    let companion =
        prepare_session_app_server_companion(strategy, &overlay_home, &mut runtime_args)?;
    let child = prepare_child_plan(
        strategy,
        prepared,
        &overlay_home,
        &runtime_args,
        runtime_proxy,
        &preference_context,
    )?;
    let plan = RuntimeLaunchPlan::new(child).with_cleanup_path(cleanup.keep());
    #[cfg(unix)]
    let plan = attach_session_app_server_companion(
        strategy,
        prepared,
        &overlay_home,
        runtime_proxy,
        plan,
        companion,
    );
    Ok(plan)
}

fn prepare_overlay_launch(
    strategy: &mut RuntimeToolLaunchStrategy,
    prepared: &PreparedRuntimeLaunch,
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
) -> Result<PreparedOverlayLaunch> {
    let stage_started = Instant::now();
    if !strategy.args.dry_run {
        strategy.resume_session_path =
            crate::app_commands::runtime_launch::resume_repair::repair_resume_session_in_shared_home(
                &prepared.paths.shared_codex_root,
                &strategy.codex_args,
            )?;
    }
    crate::runtime_launch::emit_runtime_timing("startup.resume_repair_ms", stage_started);
    let stage_started = Instant::now();
    let tool_plan = resolve_optional_tool_plan(strategy, prepared)?;
    crate::runtime_launch::emit_runtime_timing("startup.optional_tool_prepare_ms", stage_started);
    let stage_started = Instant::now();
    let overlay_home = prepare_overlay_home(strategy, prepared)?;
    crate::runtime_launch::emit_runtime_timing("startup.overlay_prepare_ms", stage_started);
    let cleanup = RuntimeOverlayCleanup::new(overlay_home.clone());
    let stage_started = Instant::now();
    let scope_args = strategy.base_runtime_codex_args(&overlay_home)?;
    let preference_context =
        crate::resolve_fresh_model_preference_context(&prepared.paths, &overlay_home, &scope_args)?;
    crate::runtime_launch::emit_runtime_timing(
        "startup.model_preference_context_ms",
        stage_started,
    );
    let stage_started = Instant::now();
    let mut runtime_args =
        strategy.prepare_runtime_codex_args(&overlay_home, runtime_proxy, &preference_context)?;
    if strategy.desktop_command.is_none()
        && let Some(monitor) = strategy.goal_usage_limit_monitor.as_ref()
    {
        add_runtime_goal_session_tracking(
            &overlay_home,
            strategy.profile_v2_name.as_deref(),
            &mut runtime_args,
            &monitor.marker_path,
        )?;
    }
    if strategy.desktop_command.is_none()
        && !prodex_runtime_launch::is_codex_exec_invocation(&runtime_args)
        && !prodex_runtime_launch::codex_resume_requested(&runtime_args)
    {
        crate::project_in_app_resume_model_settings(
            &overlay_home,
            &mut runtime_args,
            strategy.profile_v2_name.as_deref(),
            [
                ("model", preference_context.explicit_model.is_none()),
                ("model_provider", strategy.model_provider_override.is_none()),
                (
                    "model_reasoning_effort",
                    preference_context.explicit_effort.is_none(),
                ),
            ],
        )?;
    }
    crate::runtime_launch::emit_runtime_timing(
        "startup.provider_catalog_prepare_ms",
        stage_started,
    );
    Ok(PreparedOverlayLaunch {
        cleanup,
        overlay_home,
        tool_plan,
        preference_context,
        runtime_args,
    })
}

fn prepare_child_plan(
    strategy: &mut RuntimeToolLaunchStrategy,
    prepared: &PreparedRuntimeLaunch,
    overlay_home: &Path,
    runtime_args: &[std::ffi::OsString],
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
    preference_context: &crate::ModelPreferenceContext,
) -> Result<prodex_runtime_launch::ChildProcessPlan> {
    let mut child = strategy.build_child_plan(prepared, overlay_home, runtime_args)?;
    strategy.finalize_child_plan(&mut child, overlay_home, runtime_proxy);
    if prepared.managed
        && !child
            .extra_env
            .iter()
            .any(|(key, _)| key == "CODEX_SQLITE_HOME")
    {
        child.extra_env.push((
            "CODEX_SQLITE_HOME".into(),
            prepared.paths.shared_codex_root.as_os_str().to_os_string(),
        ));
    }
    if !strategy.args.dry_run
        && strategy.desktop_command.is_none()
        && !prodex_cli::is_codex_command_server_subcommand(&strategy.codex_args)
    {
        crate::runtime_thread_index::repair_dirty_thread_index(&prepared.paths, &child);
    }
    if !strategy.args.dry_run
        && !prodex_cli::is_codex_command_server_subcommand(&strategy.codex_args)
        // Desktop's unmanaged overlay deliberately shares the state database through a
        // symlink; app-server startup may localize that path. Managed Desktop keeps the
        // historical prelaunch repair with its explicit shared SQLite home.
        && (strategy.desktop_command.is_none() || prepared.managed)
    {
        strategy.model_preference_sync = match crate::ModelPreferenceSync::start_with_scope(
            &prepared.paths,
            &child,
            preference_context.logical_scope.clone(),
        ) {
            Ok(sync) => Some(sync),
            Err(_error) => {
                crate::print_launch_status(
                    "model preference synchronization unavailable; continuing",
                );
                None
            }
        };
    }
    Ok(child)
}

#[cfg(unix)]
fn prepare_session_app_server_companion(
    strategy: &RuntimeToolLaunchStrategy,
    overlay_home: &Path,
    runtime_args: &mut Vec<std::ffi::OsString>,
) -> Result<Option<(prodex_runtime_launch::ChildProcessPlan, PathBuf)>> {
    let companion =
        super::build_session_app_server_companion(strategy, overlay_home, runtime_args)?;
    if let Some((_, socket)) = companion.as_ref() {
        runtime_args.extend([
            std::ffi::OsString::from("--remote"),
            std::ffi::OsString::from(format!("unix://{}", socket.display())),
        ]);
    }
    Ok(companion)
}

#[cfg(unix)]
fn attach_session_app_server_companion(
    strategy: &RuntimeToolLaunchStrategy,
    prepared: &PreparedRuntimeLaunch,
    overlay_home: &Path,
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
    plan: RuntimeLaunchPlan,
    companion: Option<(prodex_runtime_launch::ChildProcessPlan, PathBuf)>,
) -> RuntimeLaunchPlan {
    let Some((mut companion, socket)) = companion else {
        return plan;
    };
    strategy.finalize_child_plan(&mut companion, overlay_home, runtime_proxy);
    if prepared.managed
        && !companion
            .extra_env
            .iter()
            .any(|(key, _)| key == "CODEX_SQLITE_HOME")
    {
        companion.extra_env.push((
            "CODEX_SQLITE_HOME".into(),
            prepared.paths.shared_codex_root.as_os_str().to_os_string(),
        ));
    }
    plan.with_unix_companion(companion, socket)
}

fn resolve_optional_tool_plan(
    strategy: &RuntimeToolLaunchStrategy,
    prepared: &PreparedRuntimeLaunch,
) -> Result<prodex_optional_tools::ToolActivationPlan> {
    let tool_plan = resolve_runtime_optional_tool_plan(
        &strategy.args.selected_tool_set(),
        &strategy.args.required_tool_set(),
    )?;
    let required_presidio = strategy
        .args
        .required_tool_set()
        .contains(prodex_optional_tools::OptionalToolId::Presidio);
    if required_presidio {
        ensure_required_presidio_services_for_super_launch(&prepared.paths)?;
    } else if strategy.presidio_enabled {
        ensure_presidio_services_for_super_launch(&prepared.paths)?;
    }
    Ok(tool_plan)
}

fn prepare_overlay_home(
    strategy: &RuntimeToolLaunchStrategy,
    prepared: &PreparedRuntimeLaunch,
) -> Result<PathBuf> {
    let overlay_home = if strategy.desktop_command.is_some() {
        prepare_desktop_overlay_home(
            &prepared.paths,
            &prepared.codex_home,
            strategy.configure_prodex_overlay,
        )?
    } else if strategy.configure_prodex_overlay {
        prepare_prodex_overlay_home(&prepared.paths, &prepared.codex_home)?
    } else {
        prepare_runtime_overlay_home(&prepared.paths, &prepared.codex_home)?
    };
    let cleanup = RuntimeOverlayCleanup::new(overlay_home.clone());
    if strategy.provider_runtime_uses_local_proxy_auth() {
        write_provider_runtime_codex_auth(&overlay_home)?;
    }
    let _ = cleanup.keep();
    Ok(overlay_home)
}

pub(crate) fn prepare_prodex_overlay_home(
    paths: &AppPaths,
    base_codex_home: &Path,
) -> Result<PathBuf> {
    let sessions_are_managed = prodex_core::same_path(
        &base_codex_home.join("sessions"),
        &paths.shared_codex_root.join("sessions"),
    );
    if sessions_are_managed {
        return prodex_optional_tools::prepare_prodex_overlay_home_from_prepared_base(
            &paths.managed_profiles_root,
            base_codex_home,
        );
    }
    prodex_optional_tools::prepare_prodex_overlay_home(
        &paths.managed_profiles_root,
        base_codex_home,
    )
}

#[cfg(test)]
#[path = "overlay_tests.rs"]
mod tests;
