use super::*;

#[derive(Debug)]
pub(crate) struct ProdexCommandExit {
    code: i32,
    message: String,
}

impl ProdexCommandExit {
    pub(crate) fn code(&self) -> i32 {
        self.code
    }
}

impl std::fmt::Display for ProdexCommandExit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ProdexCommandExit {}

pub(crate) fn command_exit_error(code: i32, message: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(ProdexCommandExit {
        code,
        message: message.into(),
    })
}

pub(crate) fn command_should_show_update_notice(command: &Commands) -> bool {
    !command_is_super_dry_run(command)
        && !matches!(
            command,
            Commands::RuntimeBroker(_)
                | Commands::Update(_)
                | Commands::Capability(_)
                | Commands::GeminiCompatRefresh(_)
                | Commands::McpJsonlBridge(_)
                | Commands::SubAgentExec(_)
        )
}

pub(crate) fn command_is_super_dry_run(command: &Commands) -> bool {
    matches!(
        command,
        Commands::Super(args)
            if args.dry_run || prodex_dry_run_requested(&args.codex_args)
    )
}

pub(crate) fn execute_command(command: Commands) -> Result<()> {
    let _insecure_file_access =
        profile_command_requests_insecure(&command).then(secret_store::allow_insecure_file_access);
    if command_runs_profile_lifecycle_recovery(&command) {
        recover_pending_profile_lifecycle()?;
    }
    match command {
        Commands::Profile(command) => execute_profile_command(command),
        Commands::UseProfile(args) => handle_set_active_profile(args),
        Commands::Current => handle_current_profile(),
        Commands::Info(args) => handle_info(args),
        Commands::Status(args) => handle_status(args),
        Commands::Log(args) => handle_log(args),
        Commands::Session(command) => handle_session(command),
        Commands::Doctor(args) => handle_doctor(args),
        Commands::Setup(args) => handle_setup(args),
        Commands::Capability(command) => handle_capability(command),
        Commands::Audit(args) => handle_audit(args),
        Commands::AppServerBroker(args) => handle_app_server_broker(args),
        Commands::Context(command) => execute_context_command(command),
        Commands::Cleanup(args) => handle_cleanup(args),
        Commands::Presidio(command) => handle_presidio(command),
        Commands::Login(args) => handle_codex_login(args),
        Commands::Logout(args) => handle_codex_logout(args),
        Commands::Update(args) => handle_prodex_update(args),
        Commands::Quota(args) => handle_quota(args),
        Commands::Redeem(args) => handle_redeem(args),
        Commands::Ping(command) => handle_ping(command),
        Commands::Gui(args) => handle_gui(args),
        Commands::Dashboard(args) => handle_dashboard(args),
        Commands::Run(args) => app_commands::runtime_launch::handle_run(args),
        Commands::Caveman(mut args) => {
            args.require_tool(prodex_optional_tools::OptionalToolId::Caveman);
            execute_tool_launch(args)
        }
        Commands::Rtk(args) => {
            execute_optional_tool_alias(args, prodex_optional_tools::OptionalToolId::Rtk)
        }
        Commands::Playwright(args) => {
            execute_optional_tool_alias(args, prodex_optional_tools::OptionalToolId::PlaywrightMcp)
        }
        Commands::Ponytail(args) => {
            execute_optional_tool_alias(args, prodex_optional_tools::OptionalToolId::Ponytail)
        }
        Commands::Super(args) => execute_super(args),
        Commands::Expose(args) => handle_expose(args),
        Commands::Gateway(args) => handle_gateway(args),
        Commands::Claude(args) => handle_claude(args),
        Commands::RuntimeBroker(args) => handle_runtime_broker(args),
        Commands::GeminiCompatRefresh(args) => handle_gemini_compat_refresh(args),
        Commands::McpJsonlBridge(args) => handle_mcp_jsonl_bridge(args),
        Commands::SubAgentExec(args) => handle_sub_agent_exec(args),
    }
}

fn command_runs_profile_lifecycle_recovery(command: &Commands) -> bool {
    !command_is_super_dry_run(command)
        && !matches!(
            command,
            Commands::Profile(ProfileCommands::Remove(_))
                | Commands::Cleanup(_)
                | Commands::Doctor(_)
                | Commands::Capability(_)
                | Commands::McpJsonlBridge(_)
                | Commands::SubAgentExec(_)
        )
        && !matches!(command, Commands::Gateway(args) if args.command.is_some())
}

fn profile_command_requests_insecure(command: &Commands) -> bool {
    match command {
        Commands::Profile(ProfileCommands::Add(args)) => args.insecure,
        Commands::Profile(ProfileCommands::Import(args)) => args.insecure,
        Commands::Profile(ProfileCommands::ImportCurrent(args)) => args.insecure,
        _ => false,
    }
}

fn execute_profile_command(command: ProfileCommands) -> Result<()> {
    match command {
        ProfileCommands::Add(args) => handle_add_profile(args),
        ProfileCommands::Export(args) => handle_export_profiles(args),
        ProfileCommands::Import(args) => handle_import_profiles(args),
        ProfileCommands::ImportCurrent(args) => handle_import_current_profile(args),
        ProfileCommands::List => handle_list_profiles(),
        ProfileCommands::Remove(args) => handle_remove_profile(args),
        ProfileCommands::Use(args) => handle_set_active_profile(args),
    }
}

fn execute_context_command(command: ContextCommands) -> Result<()> {
    match command {
        ContextCommands::Audit(args) => handle_context_audit(args),
        ContextCommands::Export(args) => handle_context_export(args),
        ContextCommands::Compress(args) => handle_context_compress(args),
        ContextCommands::ReplayReport(args) => handle_context_replay_report(args),
        ContextCommands::CompactOutput(args) => handle_context_compact_output(args),
    }
}

fn execute_optional_tool_alias(
    mut args: RuntimeToolArgs,
    tool: prodex_optional_tools::OptionalToolId,
) -> Result<()> {
    args.select_tool(prodex_optional_tools::OptionalToolId::Caveman);
    args.select_tool(tool);
    execute_tool_launch(args)
}

fn execute_tool_launch(args: RuntimeToolArgs) -> Result<()> {
    if args.dry_run || prodex_dry_run_requested(&args.codex_args) {
        return handle_runtime_tools_dry_run(args);
    }
    handle_runtime_tools(args)
}

fn execute_super(mut args: SuperArgs) -> Result<()> {
    args.extract_provider_overrides_from_codex_args()
        .map_err(anyhow::Error::msg)?;
    crate::runtime_gemini_cli::validate_super_native_cli_capability_args(&args)?;
    args.validate_urls().map_err(anyhow::Error::msg)?;
    if args.codex_args.first().is_some_and(|arg| arg == "gui") {
        args.codex_args.remove(0);
        if !args.codex_args.is_empty() {
            bail!("`prodex s gui` does not accept Codex CLI arguments")
        }
        return handle_super_gui(args);
    }
    if args.dry_run || prodex_dry_run_requested(&args.codex_args) {
        let use_presidio = match args.presidio_preference() {
            Some(use_presidio) => use_presidio,
            None => stored_presidio_preference()?.unwrap_or(false),
        };
        let mut sub_agent = resolve_super_sub_agent(&args, false)?;
        if let Some(sub_agent) = sub_agent.as_mut() {
            sub_agent.presidio_enabled = use_presidio;
        }
        if matches!(args.cli, Some(agent) if agent != SuperCliAgent::Codex) {
            return crate::runtime_gemini_cli::handle_super_native_cli_dry_run(
                args,
                sub_agent.as_ref(),
            );
        }
        return handle_super_runtime_tools_dry_run(args, use_presidio, sub_agent.as_ref());
    }
    handle_super(args)
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::execute_command;
    use super::{
        command_is_super_dry_run, command_runs_profile_lifecycle_recovery,
        command_should_show_update_notice, parse_cli_command_from,
    };

    #[test]
    fn super_dry_run_skips_startup_side_effects() {
        let native = parse_cli_command_from(["prodex", "super", "--cli", "gemini", "--dry-run"])
            .expect("native dry-run should parse");
        let codex = parse_cli_command_from(["prodex", "super", "--dry-run"])
            .expect("Codex dry-run should parse");
        let native_tail = parse_cli_command_from([
            "prodex",
            "super",
            "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9",
            "--cli",
            "gemini",
            "--dry-run",
        ])
        .expect("native tail dry-run should parse");

        assert!(command_is_super_dry_run(&native));
        assert!(command_is_super_dry_run(&native_tail));
        assert!(!command_should_show_update_notice(&native));
        assert!(!crate::housekeeping::command_runs_auto_runtime_housekeeping(&native));
        assert!(command_is_super_dry_run(&codex));
        assert!(!command_should_show_update_notice(&codex));
        assert!(!crate::housekeeping::command_runs_auto_runtime_housekeeping(&codex));
    }

    #[test]
    #[cfg(unix)]
    fn command_dispatch_does_not_repeat_native_preflight() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::time::{SystemTime, UNIX_EPOCH};

        let root = std::env::temp_dir().join(format!(
            "prodex-native-preflight-count-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        ));
        fs::create_dir_all(&root).unwrap();
        let agy = root.join("agy");
        let marker = root.join("preflight-count");
        fs::write(
            &agy,
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then printf x >> \"$PRODEX_AGY_PREFLIGHT_MARKER\"; fi\nexit 0\n",
        )
        .unwrap();
        fs::set_permissions(&agy, fs::Permissions::from_mode(0o755)).unwrap();

        let _lock = crate::test_support::TestEnvVarGuard::lock();
        let _home = crate::test_support::TestEnvVarGuard::set(
            "PRODEX_HOME",
            root.to_str().expect("test root should be UTF-8"),
        );
        let _agy = crate::test_support::TestEnvVarGuard::set(
            "PRODEX_AGY_BIN",
            agy.to_str().expect("test binary should be UTF-8"),
        );
        let _marker = crate::test_support::TestEnvVarGuard::set(
            "PRODEX_AGY_PREFLIGHT_MARKER",
            marker.to_str().expect("test marker should be UTF-8"),
        );
        let crate::Commands::Super(args) = parse_cli_command_from([
            "prodex",
            "super",
            "--cli",
            "agy",
            "--provider",
            "gemini",
            "gui",
            "extra",
        ])
        .expect("native Super command should parse") else {
            panic!("expected Super command");
        };

        crate::runtime_gemini_cli::validate_super_native_cli_preflight(&args)
            .expect("native preflight should succeed");
        let error = execute_command(crate::Commands::Super(args))
            .expect_err("extra GUI argument should fail before launch");
        assert!(
            error
                .to_string()
                .contains("does not accept Codex CLI arguments")
        );
        assert_eq!(fs::read_to_string(&marker).unwrap().len(), 1);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mcp_bridge_dispatches_without_profile_recovery() {
        let command =
            parse_cli_command_from(["prodex", "__mcp-jsonl-bridge", "codebase-memory-mcp"])
                .unwrap();
        assert!(!command_runs_profile_lifecycle_recovery(&command));
    }

    #[test]
    fn gateway_catalog_commands_skip_profile_recovery() {
        let command = parse_cli_command_from(["prodex", "gateway", "providers"]).unwrap();

        assert!(!command_runs_profile_lifecycle_recovery(&command));
    }

    #[test]
    fn capability_commands_skip_startup_notifications() {
        let command = parse_cli_command_from(["prodex", "s", "--no-presidio", "doctor"])
            .expect("Super doctor should parse");

        assert!(!command_should_show_update_notice(&command));
        assert!(!crate::housekeeping::command_runs_auto_runtime_housekeeping(&command));
    }
}
