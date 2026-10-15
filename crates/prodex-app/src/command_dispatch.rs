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

pub(crate) fn command_uses_native_agy(command: &Commands) -> bool {
    let Commands::Super(args) = command else {
        return false;
    };
    if args.cli == Some(prodex_cli::SuperCliAgent::Agy) {
        return true;
    }
    let mut args = (**args).clone();
    args.extract_super_overrides_from_codex_args_for_native_preflight()
        .is_ok()
        && args.cli == Some(prodex_cli::SuperCliAgent::Agy)
}

fn command_startup_policy_input(
    command: &Commands,
) -> prodex_mojo_core::app_command_policy::AppCommandStartupInput {
    use prodex_mojo_core::app_command_policy::{AppCommandClass, AppCommandStartupInput};

    let command_class = match command {
        Commands::RuntimeBroker(_) => AppCommandClass::RuntimeBroker,
        Commands::Info(_) => AppCommandClass::Info,
        Commands::Log(_) => AppCommandClass::Log,
        Commands::Update(_) => AppCommandClass::Update,
        Commands::Ping(_) => AppCommandClass::Ping,
        Commands::SuperExpose(_) => AppCommandClass::SuperExpose,
        Commands::McpJsonlBridge(_) => AppCommandClass::McpBridge,
        Commands::SubAgentExec(_) => AppCommandClass::SubAgentExec,
        Commands::Profile(ProfileCommands::Remove(_)) => AppCommandClass::ProfileRemove,
        Commands::Doctor(_) => AppCommandClass::Doctor,
        Commands::Super(_) => AppCommandClass::Super,
        Commands::Profile(_)
        | Commands::UseProfile(_)
        | Commands::Current
        | Commands::Status(_)
        | Commands::Session(_)
        | Commands::Login(_)
        | Commands::Logout(_)
        | Commands::Quota(_)
        | Commands::Redeem(_)
        | Commands::Run(_)
        | Commands::Gateway(_) => AppCommandClass::Other,
    };
    let (super_direct_dry_run, super_codex_dry_run) = match command {
        Commands::Super(args) => (args.dry_run, prodex_dry_run_requested(&args.codex_args)),
        _ => (false, false),
    };
    let (super_expose_direct_dry_run, super_expose_codex_dry_run) = match command {
        Commands::SuperExpose(args) => (
            args.super_args.dry_run,
            prodex_dry_run_requested(&args.super_args.codex_args),
        ),
        _ => (false, false),
    };
    AppCommandStartupInput {
        command_class,
        super_direct_dry_run,
        super_codex_dry_run,
        super_expose_direct_dry_run,
        super_expose_codex_dry_run,
        native_agy: command_uses_native_agy(command),
    }
}

pub(crate) fn command_startup_policy(
    command: &Commands,
) -> prodex_mojo_core::app_command_policy::AppCommandStartupPlan {
    prodex_mojo_core::app_command_policy::startup_plan(command_startup_policy_input(command))
        .expect("Mojo app command startup policy returned invalid output")
}

pub(crate) fn command_should_show_update_notice(command: &Commands) -> bool {
    command_startup_policy(command).show_update_notice
}

pub(crate) fn command_is_super_dry_run(command: &Commands) -> bool {
    command_startup_policy(command).super_dry_run
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
        Commands::Login(args) => handle_codex_login(args),
        Commands::Logout(args) => handle_codex_logout(args),
        Commands::Update(args) => handle_prodex_update(args),
        Commands::Quota(args) => handle_quota(args),
        Commands::Redeem(args) => handle_redeem(args),
        Commands::Ping(command) => handle_ping(command),
        Commands::Run(args) => app_commands::runtime_launch::handle_run(args),
        Commands::Super(args) => execute_super(*args),
        Commands::Gateway(args) => handle_gateway(args),
        Commands::SuperExpose(args) => super_expose::handle_super_expose(*args),
        Commands::RuntimeBroker(args) => handle_runtime_broker(args),
        Commands::McpJsonlBridge(args) => handle_mcp_jsonl_bridge(args),
        Commands::SubAgentExec(args) => handle_sub_agent_exec(args),
    }
}

fn command_runs_profile_lifecycle_recovery(command: &Commands) -> bool {
    command_startup_policy(command).run_profile_lifecycle_recovery
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

fn execute_super(mut args: SuperArgs) -> Result<()> {
    args.extract_provider_overrides_from_codex_args()
        .map_err(anyhow::Error::msg)?;
    args.validate_urls().map_err(anyhow::Error::msg)?;
    if super_uses_native_agy(&args) {
        return handle_super_native_agy(args);
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
        return handle_super_runtime_tools_dry_run(args, use_presidio, sub_agent.as_ref());
    }
    handle_super(args)
}

#[cfg(test)]
mod tests {

    use super::{
        command_is_super_dry_run, command_runs_profile_lifecycle_recovery,
        command_should_show_update_notice, command_uses_native_agy, parse_cli_command_from,
    };

    #[test]
    fn super_dry_run_skips_startup_side_effects() {
        let native =
            parse_cli_command_from(["prodex", "super", "gemini", "--cli", "agy", "--dry-run"])
                .expect("native dry-run should parse");
        let codex = parse_cli_command_from(["prodex", "super", "--dry-run"])
            .expect("Codex dry-run should parse");
        let native_tail = parse_cli_command_from([
            "prodex",
            "super",
            "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9",
            "--provider",
            "gemini",
            "--cli",
            "agy",
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
    fn info_is_read_only_startup_surface() {
        let command = parse_cli_command_from(["prodex", "info"]).unwrap();
        assert!(!command_runs_profile_lifecycle_recovery(&command));
        assert!(!command_should_show_update_notice(&command));
        assert!(!crate::housekeeping::command_runs_auto_runtime_housekeeping(&command));
    }

    #[test]
    fn log_is_read_only_startup_surface() {
        let command = parse_cli_command_from(["prodex", "log", "last"]).unwrap();
        assert!(!command_runs_profile_lifecycle_recovery(&command));
        assert!(!command_should_show_update_notice(&command));
        assert!(!crate::housekeeping::command_runs_auto_runtime_housekeeping(&command));
    }

    #[test]
    fn mcp_bridge_dispatches_without_profile_recovery() {
        let command =
            parse_cli_command_from(["prodex", "__mcp-jsonl-bridge", "codebase-memory-mcp"])
                .unwrap();
        assert!(!command_runs_profile_lifecycle_recovery(&command));
    }

    #[test]
    fn native_antigravity_skips_profile_startup_surfaces() {
        let direct =
            parse_cli_command_from(["prodex", "s", "gemini", "--cli", "agy", "--no-presidio"])
                .expect("native Antigravity should parse");
        let tail = parse_cli_command_from([
            "prodex",
            "s",
            "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9",
            "--provider",
            "gemini",
            "--cli",
            "agy",
            "--no-presidio",
        ])
        .expect("native Antigravity tail overrides should parse");

        for command in [&direct, &tail] {
            assert!(command_uses_native_agy(command));
            assert!(!command_runs_profile_lifecycle_recovery(command));
            assert!(!command_should_show_update_notice(command));
            assert!(!crate::housekeeping::command_runs_auto_runtime_housekeeping(command));
        }
    }

    #[test]
    fn startup_policy_keeps_profile_remove_and_super_expose_boundaries() {
        let remove = parse_cli_command_from(["prodex", "profile", "remove", "work"]).unwrap();
        let remove_plan = super::command_startup_policy(&remove);
        assert!(remove_plan.show_update_notice);
        assert!(!remove_plan.run_profile_lifecycle_recovery);
        assert!(remove_plan.run_auto_housekeeping);

        let expose = parse_cli_command_from(["prodex", "__super-expose"]).unwrap();
        let expose_plan = super::command_startup_policy(&expose);
        assert!(!expose_plan.show_update_notice);
        assert!(expose_plan.run_profile_lifecycle_recovery);
        assert!(expose_plan.run_auto_housekeeping);
    }
}
