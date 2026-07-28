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
    !matches!(
        command,
        Commands::RuntimeBroker(_)
            | Commands::Update(_)
            | Commands::GeminiCompatRefresh(_)
            | Commands::McpJsonlBridge(_)
    )
}

pub(crate) fn execute_command(command: Commands) -> Result<()> {
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
    args.validate_urls().map_err(anyhow::Error::msg)?;
    if args.codex_args.first().is_some_and(|arg| arg == "gui") {
        args.codex_args.remove(0);
        if !args.codex_args.is_empty() {
            bail!("`prodex s gui` does not accept Codex CLI arguments")
        }
        return handle_super_gui(args);
    }
    if args.dry_run || prodex_dry_run_requested(&args.codex_args) {
        if matches!(args.cli, Some(agent) if agent != SuperCliAgent::Codex) {
            return crate::runtime_gemini_cli::handle_super_native_cli_dry_run(args);
        }
        let use_presidio = args.presidio_preference().unwrap_or(false);
        return handle_runtime_tools_dry_run(
            args.into_runtime_tool_args_with_presidio(use_presidio),
        );
    }
    handle_super(args)
}
