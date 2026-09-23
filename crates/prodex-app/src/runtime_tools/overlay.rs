use super::session_app_server_companion_eligible;
use super::{
    AppPaths, PreparedRuntimeLaunch, RuntimeLaunchPlan, RuntimeProxyEndpoint,
    RuntimeToolLaunchStrategy, ensure_presidio_services_for_super_launch,
    ensure_required_presidio_services_for_super_launch, redaction_redact_secret_like_text,
    write_provider_runtime_codex_auth,
};
use crate::app_commands::runtime_launch::goal_resume::add_runtime_goal_session_tracking;
use anyhow::{Context, Result, bail};
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
    if let Some(message) = required_optional_tool_error(&plan, &required) {
        bail!("{message}");
    }
    Ok(plan)
}

fn required_optional_tool_error(
    plan: &prodex_optional_tools::ToolActivationPlan,
    required: &prodex_optional_tools::OptionalToolSet,
) -> Option<String> {
    plan.unavailable
        .iter()
        .find(|health| required.contains(health.id))
        .map(|unavailable| {
            format!(
                "required optional tool {} is unavailable: {}; run `prodex doctor --install`",
                unavailable.id,
                redaction_redact_secret_like_text(&unavailable.detail)
            )
        })
}

fn optional_tool_skip_messages(
    plan: &prodex_optional_tools::ToolActivationPlan,
    required: &prodex_optional_tools::OptionalToolSet,
) -> Vec<String> {
    plan.unavailable
        .iter()
        .filter(|health| {
            health.status == prodex_optional_tools::ToolHealthStatus::Invalid
                && !required.contains(health.id)
        })
        .map(|health| {
            format!(
                "{}: skipped for this launch; {}. Update when convenient (minimum supported: {}, release-qualified reference: {}).",
                health.id,
                redaction_redact_secret_like_text(&health.detail),
                prodex_optional_tools::optional_tool_minimum_supported_version(health.id),
                prodex_optional_tools::optional_tool_recommended_version(health.id),
            )
        })
        .collect()
}

fn configure_overlay_codex_home(
    codex_home: &Path,
    codex_args: &[std::ffi::OsString],
    full_access: bool,
) -> Result<()> {
    let config_path = codex_home.join("config.toml");
    if std::fs::symlink_metadata(&config_path)
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        bail!(
            "refusing to write symlinked overlay config {}",
            config_path.display()
        );
    }
    let raw = match std::fs::read_to_string(&config_path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", config_path.display()));
        }
    };
    let mut config = if raw.trim().is_empty() {
        toml::Value::Table(toml::map::Map::new())
    } else {
        toml::from_str(&raw)
            .with_context(|| format!("failed to parse {}", config_path.display()))?
    };
    for assignment in overlay_config_assignments(codex_args)? {
        let patch: toml::Value = toml::from_str(&format!("{assignment}\n"))
            .map_err(|_| anyhow::anyhow!("invalid Codex overlay config override"))?;
        merge_overlay_toml(&mut config, patch);
    }
    if full_access {
        merge_overlay_toml(
            &mut config,
            toml::from_str("approval_policy = \"never\"\nsandbox_mode = \"danger-full-access\"\n")?,
        );
    }
    let rendered = toml::to_string_pretty(&config).context("failed to render overlay config")?;
    std::fs::write(&config_path, rendered)
        .with_context(|| format!("failed to write {}", config_path.display()))
}

fn overlay_config_assignments(args: &[std::ffi::OsString]) -> Result<Vec<&str>> {
    let mut assignments = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let Some(arg) = args[index].to_str() else {
            index += 1;
            continue;
        };
        if matches!(arg, "-c" | "--config") {
            let value = args
                .get(index + 1)
                .context("Codex overlay config flag is missing its value")?
                .to_str()
                .context("Codex overlay config override must be UTF-8")?;
            assignments.push(value);
            index += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--config=") {
            assignments.push(value);
        } else if let Some(value) = arg.strip_prefix("-c")
            && !value.is_empty()
        {
            assignments.push(value);
        }
        index += 1;
    }
    Ok(assignments)
}

fn merge_overlay_toml(target: &mut toml::Value, patch: toml::Value) {
    match (target, patch) {
        (toml::Value::Table(target), toml::Value::Table(patch)) => {
            for (key, value) in patch {
                match target.get_mut(&key) {
                    Some(current) => merge_overlay_toml(current, value),
                    None => {
                        target.insert(key, value);
                    }
                }
            }
        }
        (target, patch) => *target = patch,
    }
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
    configure_overlay_codex_home(codex_home, &config_args, false)?;
    *codex_args = projected_args;
    Ok(())
}

fn project_fresh_super_config(
    strategy: &RuntimeToolLaunchStrategy,
    overlay_home: &Path,
    runtime_args: &mut Vec<std::ffi::OsString>,
) -> Result<()> {
    let mut projected_args = Vec::with_capacity(runtime_args.len());
    let mut config_args = Vec::new();
    let mut index = 0;
    while index < runtime_args.len() {
        let argument = runtime_args[index].to_string_lossy();
        match argument.as_ref() {
            "-c" | "--config" => {
                if let Some(value) = runtime_args.get(index + 1) {
                    config_args.extend([runtime_args[index].clone(), value.clone()]);
                    index += 2;
                    continue;
                }
            }
            "--enable" | "--disable" => {
                if let Some(feature) = runtime_args.get(index + 1) {
                    config_args.extend([
                        std::ffi::OsString::from("-c"),
                        std::ffi::OsString::from(format!(
                            "features.{}={}",
                            feature.to_string_lossy(),
                            argument == "--enable"
                        )),
                    ]);
                    index += 2;
                    continue;
                }
            }
            "--dangerously-bypass-hook-trust" => {
                projected_args.push(runtime_args[index].clone());
                index += 1;
                continue;
            }
            "--dangerously-bypass-approvals-and-sandbox" => {
                index += 1;
                continue;
            }
            value if value.starts_with("--config=") || value.starts_with("-c") => {
                config_args.push(runtime_args[index].clone());
                index += 1;
                continue;
            }
            _ => {}
        }
        projected_args.push(runtime_args[index].clone());
        index += 1;
    }
    if crate::codex_cli_config_override_value(runtime_args, "disable_paste_burst").is_none() {
        config_args.extend([
            std::ffi::OsString::from("-c"),
            std::ffi::OsString::from("disable_paste_burst=true"),
        ]);
    }
    configure_overlay_codex_home(overlay_home, &config_args, strategy.args.full_access)?;
    *runtime_args = projected_args;
    Ok(())
}

struct PreparedOverlayLaunch {
    cleanup: RuntimeOverlayCleanup,
    overlay_home: PathBuf,
    tool_plan: prodex_optional_tools::ToolActivationPlan,
    runtime_args: Vec<std::ffi::OsString>,
}

pub(super) fn build_plan(
    strategy: &mut RuntimeToolLaunchStrategy,
    prepared: &PreparedRuntimeLaunch,
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
) -> Result<RuntimeLaunchPlan> {
    strategy.runtime_recovery_log_target =
        runtime_proxy.and_then(RuntimeProxyEndpoint::recovery_log_target);
    let PreparedOverlayLaunch {
        cleanup,
        overlay_home,
        tool_plan,
        runtime_args,
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
    let mut child = prepare_child_plan(
        strategy,
        prepared,
        &overlay_home,
        &runtime_args,
        runtime_proxy,
    )?;
    let explicit_hook_bypass = child
        .args
        .iter()
        .any(|arg| arg == "--dangerously-bypass-hook-trust");
    let remote_launch = child
        .args
        .iter()
        .any(|arg| arg == "--remote" || arg.to_string_lossy().starts_with("--remote="));
    let legacy_hook_bypass = if strategy.args.super_mode
        && !strategy.args.dry_run
        && !explicit_hook_bypass
        && !remote_launch
    {
        let stage_started = Instant::now();
        let workspace = std::env::current_dir().context("failed to resolve Super workspace")?;
        let result = super::hook_trust::trust_super_hooks(&child, &workspace)?;
        crate::runtime_launch::emit_runtime_timing("startup.hook_trust_ms", stage_started);
        result == super::hook_trust::HookTrustPreflight::LegacyBypass
    } else {
        remote_launch && strategy.args.super_mode && !explicit_hook_bypass
    };
    if legacy_hook_bypass {
        child.args.insert(
            0,
            std::ffi::OsString::from("--dangerously-bypass-hook-trust"),
        );
    }
    #[cfg(unix)]
    let companion = prepare_session_app_server_companion(
        strategy,
        &overlay_home,
        &runtime_args,
        legacy_hook_bypass,
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
    let mut runtime_args = strategy.prepare_runtime_codex_args(&overlay_home, runtime_proxy)?;
    strategy.recovery_model =
        crate::codex_effective_config_value(&overlay_home, &runtime_args, "model")?;
    if let Some(monitor) = strategy.goal_usage_limit_monitor.as_ref() {
        add_runtime_goal_session_tracking(
            &overlay_home,
            strategy.profile_v2_name.as_deref(),
            &mut runtime_args,
            &monitor.marker_path,
        )?;
    }
    if !prodex_runtime_launch::is_codex_exec_invocation(&runtime_args)
        && !prodex_runtime_launch::codex_resume_requested(&runtime_args)
    {
        crate::project_in_app_resume_model_settings(
            &overlay_home,
            &mut runtime_args,
            strategy.profile_v2_name.as_deref(),
            [
                (
                    "model",
                    crate::runtime_launch_cli_model(&strategy.codex_args).is_none()
                        && crate::codex_cli_config_override_value(&strategy.codex_args, "model")
                            .is_none(),
                ),
                ("model_provider", strategy.model_provider_override.is_none()),
                (
                    "model_reasoning_effort",
                    crate::codex_cli_config_override_value(
                        &strategy.codex_args,
                        "model_reasoning_effort",
                    )
                    .is_none(),
                ),
            ],
        )?;
    }
    if session_app_server_companion_eligible(strategy, &runtime_args) {
        project_fresh_super_config(strategy, &overlay_home, &mut runtime_args)?;
    }
    crate::runtime_launch::emit_runtime_timing(
        "startup.provider_catalog_prepare_ms",
        stage_started,
    );
    Ok(PreparedOverlayLaunch {
        cleanup,
        overlay_home,
        tool_plan,
        runtime_args,
    })
}

fn prepare_child_plan(
    strategy: &mut RuntimeToolLaunchStrategy,
    prepared: &PreparedRuntimeLaunch,
    overlay_home: &Path,
    runtime_args: &[std::ffi::OsString],
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
) -> Result<prodex_runtime_launch::ChildProcessPlan> {
    let mut child = strategy.build_child_plan(overlay_home, runtime_args)?;
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
        && !prodex_cli::is_codex_command_server_subcommand(&strategy.codex_args)
    {
        crate::runtime_thread_index::repair_dirty_thread_index(&prepared.paths, &child);
    }
    Ok(child)
}

#[cfg(unix)]
fn prepare_session_app_server_companion(
    strategy: &RuntimeToolLaunchStrategy,
    overlay_home: &Path,
    runtime_args: &[std::ffi::OsString],
    legacy_hook_bypass: bool,
) -> Result<Option<(prodex_runtime_launch::ChildProcessPlan, PathBuf)>> {
    let companion = super::build_session_app_server_companion(
        strategy,
        overlay_home,
        runtime_args,
        legacy_hook_bypass,
    )?;
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
    let required_tools = strategy.args.required_tool_set();
    let tool_plan =
        resolve_runtime_optional_tool_plan(&strategy.args.selected_tool_set(), &required_tools)?;
    let skipped_incompatible = optional_tool_skip_messages(&tool_plan, &required_tools);
    if !skipped_incompatible.is_empty() {
        prodex_terminal_ui::print_stderr_panel("Optional Tools", &skipped_incompatible)?;
    }
    let required_presidio =
        required_tools.contains(prodex_optional_tools::OptionalToolId::Presidio);
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
    let overlay_home = prepare_prodex_overlay_home(&prepared.paths, &prepared.codex_home)?;
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
    prodex_optional_tools::prepare_prodex_overlay_home(
        &paths.managed_profiles_root,
        base_codex_home,
    )
}

#[cfg(test)]
mod overlay_tests {
    use super::*;
    use std::ffi::OsString;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_overlay(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("prodex-{name}-{}-{stamp}", std::process::id()))
    }

    #[test]
    fn optional_incompatible_tool_is_informational_unless_required() {
        let invalid_rtk = prodex_optional_tools::ToolHealth {
            id: prodex_optional_tools::OptionalToolId::Rtk,
            status: prodex_optional_tools::ToolHealthStatus::Invalid,
            source: Some(prodex_optional_tools::ToolDiscoverySource::Path),
            path: Some(PathBuf::from("/tmp/rtk")),
            version: Some("0.45.0".to_string()),
            digest: None,
            can_activate: false,
            detail: "rtk 0.45.0 is too old; Prodex requires 0.46.0 or newer".to_string(),
        };
        let plan = prodex_optional_tools::ToolActivationPlan {
            activations: Vec::new(),
            unavailable: vec![invalid_rtk],
        };

        let optional_required = prodex_optional_tools::OptionalToolSet::default();
        assert!(required_optional_tool_error(&plan, &optional_required).is_none());
        let messages = optional_tool_skip_messages(&plan, &optional_required);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("rtk: skipped for this launch"));
        assert!(messages[0].contains("Update when convenient"));
        assert!(messages[0].contains("minimum supported: 0.46.0"));

        let required = [prodex_optional_tools::OptionalToolId::Rtk]
            .into_iter()
            .collect::<prodex_optional_tools::OptionalToolSet>();
        let error = required_optional_tool_error(&plan, &required)
            .expect("required incompatible RTK must remain fatal");
        assert!(error.contains("required optional tool rtk is unavailable"));
    }

    #[test]
    fn hook_trust_bypass_stays_cli_only_in_overlay_config() {
        let root = temp_overlay("hook-trust-overlay");
        std::fs::create_dir_all(&root).unwrap();
        let args = vec![
            OsString::from("--dangerously-bypass-hook-trust"),
            OsString::from("-c"),
            OsString::from("disable_paste_burst=true"),
        ];

        configure_overlay_codex_home(&root, &args, false).unwrap();

        let rendered = std::fs::read_to_string(root.join("config.toml")).unwrap();
        let config: toml::Value = toml::from_str(&rendered).unwrap();
        assert!(config.get("bypass_hook_trust").is_none());
        assert_eq!(
            config
                .get("disable_paste_burst")
                .and_then(toml::Value::as_bool),
            Some(true)
        );
        assert!(
            args.iter()
                .any(|arg| arg == "--dangerously-bypass-hook-trust")
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
