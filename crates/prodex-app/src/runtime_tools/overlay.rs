use super::{
    AppPaths, PreparedRuntimeLaunch, RuntimeLaunchPlan, RuntimeProxyEndpoint,
    RuntimeToolLaunchStrategy, ensure_presidio_services_for_super_launch,
    prepare_desktop_overlay_home, prepare_runtime_overlay_home, redaction_redact_secret_like_text,
    write_provider_runtime_codex_auth,
};
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

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
    let plan = prodex_optional_tools::resolve_optional_tools(&selected, &required);
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

pub(super) fn build_plan(
    strategy: &RuntimeToolLaunchStrategy,
    prepared: &PreparedRuntimeLaunch,
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
) -> Result<RuntimeLaunchPlan> {
    let tool_plan = resolve_optional_tool_plan(strategy, prepared)?;
    let overlay_home = prepare_overlay_home(strategy, prepared)?;
    let cleanup = RuntimeOverlayCleanup::new(overlay_home.clone());
    let runtime_args = strategy.prepare_runtime_codex_args(&overlay_home, runtime_proxy)?;
    prodex_optional_tools::activate_optional_tools_for_codex(
        &overlay_home,
        &tool_plan,
        strategy.presidio_enabled,
    )?;
    let mut child = strategy.build_child_plan(prepared, &overlay_home, &runtime_args)?;
    strategy.finalize_child_plan(&mut child, &overlay_home, runtime_proxy);
    Ok(RuntimeLaunchPlan::new(child).with_cleanup_path(cleanup.keep()))
}

fn resolve_optional_tool_plan(
    strategy: &RuntimeToolLaunchStrategy,
    prepared: &PreparedRuntimeLaunch,
) -> Result<prodex_optional_tools::ToolActivationPlan> {
    let tool_plan = resolve_runtime_optional_tool_plan(
        &strategy.args.selected_tool_set(),
        &strategy.args.required_tool_set(),
    )?;
    if !strategy.args.dry_run {
        crate::app_commands::runtime_launch::resume_repair::repair_resume_session_in_shared_home(
            &prepared.paths.shared_codex_root,
            &strategy.codex_args,
        )?;
    }
    if strategy.presidio_enabled {
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
        // Recheck fingerprints immediately before linking history so concurrent session updates
        // retain the same attachment-persistence behavior without rescanning every JSONL payload.
        prodex_shared_codex_fs::maintain_managed_codex_sessions(paths)?;
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
mod tests {
    use super::super::*;

    #[test]
    fn build_plan_cleans_overlay_when_optional_tool_preflight_fails() {
        let root = env::temp_dir()
            .canonicalize()
            .expect("temporary directory should resolve")
            .join(format!(
                "prodex-runtime-tools-overlay-cleanup-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ));
        let base_home = root.join("base");
        let shared_home = root.join("shared");
        std::fs::create_dir_all(&base_home).expect("base home should exist");
        std::fs::create_dir_all(&shared_home).expect("shared home should exist");
        std::fs::write(base_home.join("config.toml"), "mcp_servers = \"invalid\"\n")
            .expect("config should be written");

        let command = parse_cli_command_from(["prodex", "playwright", "exec", "hi"])
            .expect("playwright command");
        let Commands::Playwright(mut args) = command else {
            panic!("expected playwright command");
        };
        args.select_tool(prodex_optional_tools::OptionalToolId::PlaywrightMcp);
        args.dry_run = true;
        let strategy = RuntimeToolLaunchStrategy::new(args);
        let paths = AppPaths {
            root: root.clone(),
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: shared_home,
            legacy_shared_codex_root: root.join("legacy-shared"),
        };
        let prepared = PreparedRuntimeLaunch {
            paths,
            codex_home: base_home,
            managed: false,
            runtime_proxy: None,
        };

        let error = super::build_plan(&strategy, &prepared, None).unwrap_err();

        assert!(error.to_string().contains("mcp_servers"));
        assert!(
            std::fs::read_dir(&prepared.paths.managed_profiles_root)
                .expect("managed profile root should exist")
                .next()
                .is_none(),
            "failed build must remove temporary overlay"
        );
        std::fs::remove_dir_all(root).expect("test root should be removed");
    }
}
