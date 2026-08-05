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
    if let Some(sub_agent) = strategy.sub_agent.as_ref() {
        super::write_sub_agent_overlay(&overlay_home, sub_agent)?;
    }
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
    fn build_plan_cleans_overlay_when_config_preflight_fails() {
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
        std::fs::write(base_home.join("config.toml"), "mcp_servers =\n")
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

        assert!(error.to_string().contains("config.toml"));
        assert!(
            std::fs::read_dir(&prepared.paths.managed_profiles_root)
                .expect("managed profile root should exist")
                .next()
                .is_none(),
            "failed build must remove temporary overlay"
        );
        std::fs::remove_dir_all(root).expect("test root should be removed");
    }

    #[test]
    fn prodex_overlay_keeps_shared_session_access() {
        let root = env::temp_dir()
            .canonicalize()
            .expect("temporary directory should resolve")
            .join(format!(
                "prodex-runtime-tools-overlay-sessions-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ));
        let base_home = root.join("base");
        let sessions = base_home.join("sessions");
        std::fs::create_dir_all(&sessions).expect("session directory should exist");
        std::fs::write(sessions.join("parent.jsonl"), "parent session\n")
            .expect("session should exist");
        let paths = AppPaths {
            root: root.clone(),
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("shared"),
            legacy_shared_codex_root: root.join("legacy-shared"),
        };
        let overlay = super::prepare_prodex_overlay_home(&paths, &base_home)
            .expect("overlay should be prepared");

        assert_ne!(overlay, base_home);
        assert_eq!(
            std::fs::read_to_string(overlay.join("sessions/parent.jsonl"))
                .expect("parent session should remain readable"),
            "parent session\n"
        );
        std::fs::remove_dir_all(root).expect("test root should be removed");
    }

    #[test]
    fn sub_agent_overlay_isolated_and_idempotent_with_resume_surfaces() {
        let root = env::temp_dir()
            .canonicalize()
            .expect("temporary directory should resolve")
            .join(format!(
                "prodex-runtime-tools-sub-agent-overlay-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ));
        let base_home = root.join("base");
        let session_id = "00000000-0000-7000-8000-000000000042";
        std::fs::create_dir_all(base_home.join("sessions/2026/08/04"))
            .expect("session directory should exist");
        std::fs::create_dir_all(base_home.join("archived_sessions"))
            .expect("archived session directory should exist");
        std::fs::create_dir_all(base_home.join("attachments"))
            .expect("attachment directory should exist");
        std::fs::write(base_home.join("AGENTS.md"), "base instructions\n")
            .expect("base instructions should exist");
        std::fs::write(
            base_home.join("history.jsonl"),
            format!("parent history {session_id}\n"),
        )
        .expect("history should exist");
        std::fs::write(
            base_home.join("sessions/2026/08/04/parent.jsonl"),
            format!("parent session {session_id}\n"),
        )
        .expect("session should exist");
        std::fs::write(
            base_home.join("archived_sessions/old.jsonl"),
            "archived session\n",
        )
        .expect("archived session should exist");
        std::fs::write(base_home.join("attachments/input.txt"), "attachment\n")
            .expect("attachment should exist");
        let paths = AppPaths {
            root: root.clone(),
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("shared"),
            legacy_shared_codex_root: root.join("legacy-shared"),
        };
        std::fs::create_dir_all(&paths.managed_profiles_root).expect("profile root should exist");
        #[cfg(unix)]
        for path in [&root, &paths.managed_profiles_root] {
            std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o700))
                .expect("overlay parent should be private");
        }

        let overlay = super::prepare_prodex_overlay_home(&paths, &base_home)
            .expect("overlay should be prepared");
        let sub_agent = resolve_super_sub_agent_config(
            prodex_cli::SubAgentConfig::default(),
            prodex_cli::SuperLaunchTarget::Resume {
                session_id: session_id.to_string(),
            },
        )
        .expect("sub-agent should resolve");
        write_sub_agent_overlay(&overlay, &sub_agent).expect("sub-agent file should be written");
        prodex_optional_tools::activate_optional_tools_for_codex(
            &overlay,
            &prodex_optional_tools::ToolActivationPlan::default(),
            false,
        )
        .expect("overlay instructions should activate");

        let overlay_agents = std::fs::read_to_string(overlay.join("AGENTS.md"))
            .expect("overlay instructions should be readable");
        assert!(overlay_agents.contains("<!-- PRODEX SUB-AGENT BEGIN -->"));
        assert!(overlay_agents.contains("<!-- PRODEX SUB-AGENT END -->"));
        assert!(overlay_agents.contains("Never have more than 4 child sub-agents active at once."));
        assert!(!overlay_agents.contains(&format!("@{}", overlay.join("SUB_AGENTS.md").display())));
        assert_eq!(
            overlay_agents
                .lines()
                .filter(|line| line.contains("<!-- PRODEX SUB-AGENT BEGIN -->"))
                .count(),
            1
        );
        assert_eq!(
            std::fs::read_to_string(base_home.join("AGENTS.md")).unwrap(),
            "base instructions\n"
        );
        assert!(!base_home.join("SUB_AGENTS.md").exists());
        assert_eq!(
            std::fs::read_to_string(overlay.join("history.jsonl")).unwrap(),
            format!("parent history {session_id}\n")
        );
        assert_eq!(
            std::fs::read_to_string(overlay.join("sessions/2026/08/04/parent.jsonl")).unwrap(),
            format!("parent session {session_id}\n")
        );
        assert_eq!(
            std::fs::read_to_string(overlay.join("archived_sessions/old.jsonl")).unwrap(),
            "archived session\n"
        );
        assert_eq!(
            std::fs::read_to_string(overlay.join("attachments/input.txt")).unwrap(),
            "attachment\n"
        );

        let first_sub_agents = std::fs::read(overlay.join("SUB_AGENTS.md")).unwrap();
        prodex_optional_tools::activate_optional_tools_for_codex(
            &overlay,
            &prodex_optional_tools::ToolActivationPlan::default(),
            false,
        )
        .expect("repeated overlay activation should succeed");
        write_sub_agent_overlay(&overlay, &sub_agent)
            .expect("repeated sub-agent write should succeed");
        assert_eq!(
            std::fs::read(overlay.join("SUB_AGENTS.md")).unwrap(),
            first_sub_agents
        );
        let second_agents = std::fs::read_to_string(overlay.join("AGENTS.md")).unwrap();
        assert_eq!(second_agents, overlay_agents);

        std::fs::remove_dir_all(root).expect("test root should be removed");
    }

    #[test]
    fn configured_bare_uuid_build_plan_resumes_through_sub_agent_overlay() {
        let root = env::temp_dir()
            .canonicalize()
            .expect("temporary directory should resolve")
            .join(format!(
                "prodex-runtime-tools-sub-agent-resume-plan-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ));
        let base_home = root.join("base");
        let shared_home = root.join("shared");
        let profiles = root.join("profiles");
        let session_id = "00000000-0000-7000-8000-000000000042";
        std::fs::create_dir_all(base_home.join("sessions/2026/08/04"))
            .expect("session directory should exist");
        std::fs::create_dir_all(&shared_home).expect("shared home should exist");
        std::fs::create_dir_all(&profiles).expect("profile root should exist");
        std::fs::write(
            base_home.join("sessions/2026/08/04/parent.jsonl"),
            format!("parent session {session_id}\n"),
        )
        .expect("session should exist");
        #[cfg(unix)]
        for path in [&root, &profiles] {
            std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o700))
                .expect("overlay parent should be private");
        }

        let command = parse_cli_command_from([
            "prodex",
            "s",
            session_id,
            "--no-presidio",
            "--sub-agent",
            "--sub-agent-provider",
            "kiro",
            "--sub-agent-model",
            "gpt-5.6-luna",
            "--sub-agent-model-reasoning-effort",
            "max",
        ])
        .expect("configured resume should parse");
        let Commands::Super(mut args) = command else {
            panic!("expected Super command");
        };
        args.extract_super_overrides_from_codex_args()
            .expect("Super flags should extract");
        args.validate_urls().expect("Super config should validate");
        let mut sub_agent = resolve_super_sub_agent_config(
            args.sub_agent_config(),
            resolve_super_launch_target(&args.codex_args),
        )
        .expect("sub-agent should resolve");
        sub_agent.presidio_enabled = false;
        let mut runtime_args = args.into_runtime_tool_args_with_presidio(false);
        runtime_args.dry_run = true;
        runtime_args.tools.clear();
        runtime_args.required_tools.clear();
        let strategy = RuntimeToolLaunchStrategy::new_with_sub_agent(runtime_args, Some(sub_agent));
        let prepared = PreparedRuntimeLaunch {
            paths: AppPaths {
                root: root.clone(),
                state_file: root.join("state.json"),
                managed_profiles_root: profiles,
                shared_codex_root: shared_home,
                legacy_shared_codex_root: root.join("legacy-shared"),
            },
            codex_home: base_home.clone(),
            managed: false,
            runtime_proxy: None,
        };

        let plan = super::build_plan(&strategy, &prepared, None)
            .expect("configured resume launch plan should build");

        assert_ne!(plan.child.codex_home, base_home);
        assert_eq!(
            prodex_runtime_launch::codex_resume_session_id(&plan.child.args),
            Some(session_id)
        );
        assert!(plan.child.args.iter().all(|arg| {
            !matches!(
                arg.to_str(),
                Some(
                    "--presidio"
                        | "--no-presidio"
                        | "--sub-agent"
                        | "--no-sub-agent"
                        | "--sub-agent-provider"
                        | "--sub-agent-model"
                        | "--sub-agent-model-reasoning-effort"
                        | "--sub-agent-url"
                )
            )
        }));
        assert_eq!(
            std::fs::read_to_string(
                plan.child
                    .codex_home
                    .join("sessions/2026/08/04/parent.jsonl")
            )
            .expect("resumed session should remain accessible"),
            format!("parent session {session_id}\n")
        );
        let instructions = std::fs::read_to_string(plan.child.codex_home.join("SUB_AGENTS.md"))
            .expect("sub-agent instructions should exist");
        assert!(instructions.contains("- Provider: kiro"));
        assert!(instructions.contains("- Model: gpt-5.6-luna"));
        assert!(instructions.contains("- Reasoning effort: max"));
        assert!(instructions.contains("- Presidio: disabled (inherited)"));
        assert!(instructions.contains("__sub-agent-exec"));
        let launch_config: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(plan.child.codex_home.join("sub-agent-launch.json"))
                .expect("sub-agent launch config should exist"),
        )
        .expect("sub-agent launch config should parse");
        assert_eq!(launch_config["provider"], "kiro");
        assert_eq!(launch_config["model"], "gpt-5.6-luna");
        assert_eq!(launch_config["effort"], "max");
        assert_eq!(launch_config["presidio-enabled"], false);
        assert!(!instructions.contains(session_id));
        assert!(!launch_config.to_string().contains(session_id));
        assert!(
            plan.child
                .extra_env
                .iter()
                .any(|(key, value)| key == SUB_AGENT_RECURSION_MARKER && value == "1")
        );

        prodex_runtime_launch::cleanup_runtime_launch_plan(&plan);
        assert!(!plan.child.codex_home.exists());
        std::fs::remove_dir_all(root).expect("test root should be removed");
    }
}
