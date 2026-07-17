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

pub(crate) trait CommandDispatchExt {
    fn execute(self) -> Result<()>;
    fn requires_valid_runtime_policy(&self) -> bool;
    fn should_show_update_notice(&self) -> bool;
}

impl CommandDispatchExt for Commands {
    fn execute(self) -> Result<()> {
        execute_command(self)
    }

    fn requires_valid_runtime_policy(&self) -> bool {
        matches!(
            self,
            Commands::Run(_)
                | Commands::Caveman(_)
                | Commands::Rtk(_)
                | Commands::Playwright(_)
                | Commands::Ponytail(_)
                | Commands::Super(_)
                | Commands::Expose(_)
                | Commands::Gateway(_)
                | Commands::Claude(_)
                | Commands::RuntimeBroker(_)
        )
    }

    fn should_show_update_notice(&self) -> bool {
        !matches!(
            self,
            Commands::RuntimeBroker(_)
                | Commands::Update(_)
                | Commands::GeminiCompatRefresh(_)
                | Commands::McpJsonlBridge(_)
        )
    }
}

fn execute_command(command: Commands) -> Result<()> {
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
        Commands::Dashboard(args) => handle_dashboard(args),
        Commands::Run(args) => handle_run(args),
        Commands::Caveman(args) => execute_caveman(args),
        Commands::Rtk(args) => execute_caveman(caveman_args_with_optimizer_prefix(args, "rtk")),
        Commands::Playwright(args) => {
            execute_caveman(caveman_args_with_optimizer_prefix(args, "playwright"))
        }
        Commands::Ponytail(args) => {
            execute_caveman(caveman_args_with_optimizer_prefix(args, "ponytail"))
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

fn execute_caveman(args: CavemanArgs) -> Result<()> {
    if args.dry_run || prodex_dry_run_requested(&args.codex_args) {
        return handle_caveman_dry_run(args);
    }
    handle_caveman(args)
}

fn execute_super(mut args: SuperArgs) -> Result<()> {
    args.extract_provider_overrides_from_codex_args()
        .map_err(anyhow::Error::msg)?;
    args.validate_urls().map_err(anyhow::Error::msg)?;
    if args.dry_run || prodex_dry_run_requested(&args.codex_args) {
        if matches!(
            args.cli,
            Some(
                SuperCliAgent::Gemini
                    | SuperCliAgent::Copilot
                    | SuperCliAgent::Kiro
                    | SuperCliAgent::Agy
            )
        ) {
            bail!("--dry-run is not supported with native external agent CLIs")
        }
        let use_presidio = args.presidio_preference().unwrap_or(false);
        return handle_caveman_dry_run(args.into_caveman_args_with_presidio(use_presidio));
    }
    handle_super(args)
}
