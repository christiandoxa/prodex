use super::*;

mod routed_command;

use routed_command::{CommandAction, RoutedCommand};

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
    fn should_show_update_notice(&self) -> bool;
}

impl CommandDispatchExt for Commands {
    fn execute(self) -> Result<()> {
        command_into_routed_command(self).execute()
    }

    fn should_show_update_notice(&self) -> bool {
        !matches!(self, Commands::RuntimeBroker(_) | Commands::Update(_))
    }
}

trait CommandExecute {
    fn execute(self) -> Result<()>;
}

impl<T> CommandAction for T
where
    T: CommandExecute + 'static,
{
    fn run(self: Box<Self>) -> Result<()> {
        (*self).execute()
    }
}

impl CommandExecute for AddProfileArgs {
    fn execute(self) -> Result<()> {
        handle_add_profile(self)
    }
}

impl CommandExecute for AuditArgs {
    fn execute(self) -> Result<()> {
        handle_audit(self)
    }
}

impl CommandExecute for CavemanArgs {
    fn execute(self) -> Result<()> {
        if self.dry_run || prodex_dry_run_requested(&self.codex_args) {
            return handle_caveman_dry_run(self);
        }
        handle_caveman(self)
    }
}

impl CommandExecute for ClaudeArgs {
    fn execute(self) -> Result<()> {
        handle_claude(self)
    }
}

impl CommandExecute for CleanupArgs {
    fn execute(self) -> Result<()> {
        handle_cleanup(self)
    }
}

impl CommandExecute for CodexPassthroughArgs {
    fn execute(self) -> Result<()> {
        handle_codex_login(self)
    }
}

impl CommandExecute for CodexUpdateArgs {
    fn execute(self) -> Result<()> {
        handle_codex_update(self)
    }
}

impl CommandExecute for ContextCommands {
    fn execute(self) -> Result<()> {
        match self {
            Self::Audit(args) => handle_context_audit(args),
            Self::Compress(args) => handle_context_compress(args),
            Self::CompactOutput(args) => handle_context_compact_output(args),
        }
    }
}

impl CommandExecute for CurrentCommand {
    fn execute(self) -> Result<()> {
        handle_current_profile()
    }
}

impl CommandExecute for DoctorArgs {
    fn execute(self) -> Result<()> {
        handle_doctor(self)
    }
}

impl CommandExecute for ExportProfileArgs {
    fn execute(self) -> Result<()> {
        handle_export_profiles(self)
    }
}

impl CommandExecute for ImportCurrentArgs {
    fn execute(self) -> Result<()> {
        handle_import_current_profile(self)
    }
}

impl CommandExecute for ImportProfileArgs {
    fn execute(self) -> Result<()> {
        handle_import_profiles(self)
    }
}

impl CommandExecute for InfoArgs {
    fn execute(self) -> Result<()> {
        handle_info(self)
    }
}

impl CommandExecute for PresidioCommands {
    fn execute(self) -> Result<()> {
        handle_presidio(self)
    }
}

impl CommandExecute for ListProfilesCommand {
    fn execute(self) -> Result<()> {
        handle_list_profiles()
    }
}

impl CommandExecute for LogoutArgs {
    fn execute(self) -> Result<()> {
        handle_codex_logout(self)
    }
}

impl CommandExecute for ProfileSelector {
    fn execute(self) -> Result<()> {
        handle_set_active_profile(self)
    }
}

impl CommandExecute for QuotaArgs {
    fn execute(self) -> Result<()> {
        handle_quota(self)
    }
}

impl CommandExecute for RemoveProfileArgs {
    fn execute(self) -> Result<()> {
        handle_remove_profile(self)
    }
}

impl CommandExecute for RunArgs {
    fn execute(self) -> Result<()> {
        handle_run(self)
    }
}

impl CommandExecute for RuntimeBrokerArgs {
    fn execute(self) -> Result<()> {
        handle_runtime_broker(self)
    }
}

impl CommandExecute for SessionCommands {
    fn execute(self) -> Result<()> {
        handle_session(self)
    }
}

impl CommandExecute for SuperArgs {
    fn execute(self) -> Result<()> {
        if self.dry_run || prodex_dry_run_requested(&self.codex_args) {
            return handle_caveman_dry_run(self.into_caveman_args());
        }
        handle_super(self)
    }
}

fn command_into_routed_command(command: Commands) -> RoutedCommand {
    match command {
        Commands::Profile(command) => profile_command_into_routed_command(command),
        Commands::UseProfile(command) => RoutedCommand::new(command),
        Commands::Current => RoutedCommand::new(CurrentCommand),
        Commands::Info(command) => RoutedCommand::new(command),
        Commands::Session(command) => RoutedCommand::new(command),
        Commands::Doctor(command) => RoutedCommand::new(command),
        Commands::Audit(command) => RoutedCommand::new(command),
        Commands::Context(command) => RoutedCommand::new(command),
        Commands::Cleanup(command) => RoutedCommand::new(command),
        Commands::Presidio(command) => RoutedCommand::new(command),
        Commands::Login(command) => RoutedCommand::new(command),
        Commands::Logout(command) => RoutedCommand::new(command),
        Commands::Update(command) => RoutedCommand::new(command),
        Commands::Quota(command) => RoutedCommand::new(command),
        Commands::Run(command) => RoutedCommand::new(command),
        Commands::Caveman(command) => RoutedCommand::new(command),
        Commands::Rtk(command) => {
            RoutedCommand::new(caveman_args_with_optimizer_prefix(command, "rtk"))
        }
        Commands::Sqz(command) => {
            RoutedCommand::new(caveman_args_with_optimizer_prefix(command, "sqz"))
        }
        Commands::TokenSavior(command) => {
            RoutedCommand::new(caveman_args_with_optimizer_prefix(command, "tokensavior"))
        }
        Commands::ClawCompactor(command) => {
            RoutedCommand::new(caveman_args_with_optimizer_prefix(command, "clawcompactor"))
        }
        Commands::Super(command) => RoutedCommand::new(command),
        Commands::Claude(command) => RoutedCommand::new(command),
        Commands::RuntimeBroker(command) => RoutedCommand::new(command),
    }
}

fn profile_command_into_routed_command(command: ProfileCommands) -> RoutedCommand {
    match command {
        ProfileCommands::Add(command) => RoutedCommand::new(command),
        ProfileCommands::Export(command) => RoutedCommand::new(command),
        ProfileCommands::Import(command) => RoutedCommand::new(command),
        ProfileCommands::ImportCurrent(command) => RoutedCommand::new(command),
        ProfileCommands::List => RoutedCommand::new(ListProfilesCommand),
        ProfileCommands::Remove(command) => RoutedCommand::new(command),
        ProfileCommands::Use(command) => RoutedCommand::new(command),
    }
}
