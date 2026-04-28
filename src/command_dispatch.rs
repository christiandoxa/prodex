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

macro_rules! impl_command_action {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl CommandAction for $ty {
                fn run(self: Box<Self>) -> Result<()> {
                    <$ty>::execute(*self)
                }
            }
        )+
    };
}

impl_command_action!(
    AddProfileArgs,
    AuditArgs,
    CavemanArgs,
    ClaudeArgs,
    CleanupCommand,
    CodexPassthroughArgs,
    CurrentCommand,
    DoctorArgs,
    ExportProfileArgs,
    ImportCurrentArgs,
    ImportProfileArgs,
    InfoArgs,
    ListProfilesCommand,
    LogoutArgs,
    ProfileSelector,
    QuotaArgs,
    RemoveProfileArgs,
    RunArgs,
    RuntimeBrokerArgs,
    SuperArgs,
);

impl Commands {
    pub(super) fn execute(self) -> Result<()> {
        self.into_routed_command().execute()
    }

    pub(super) fn should_show_update_notice(&self) -> bool {
        !matches!(self, Commands::RuntimeBroker(_))
    }

    fn into_routed_command(self) -> RoutedCommand {
        match self {
            Commands::Profile(command) => command.into_routed_command(),
            Commands::UseProfile(command) => RoutedCommand::new(command),
            Commands::Current => RoutedCommand::new(CurrentCommand),
            Commands::Info(command) => RoutedCommand::new(command),
            Commands::Doctor(command) => RoutedCommand::new(command),
            Commands::Audit(command) => RoutedCommand::new(command),
            Commands::Cleanup => RoutedCommand::new(CleanupCommand),
            Commands::Login(command) => RoutedCommand::new(command),
            Commands::Logout(command) => RoutedCommand::new(command),
            Commands::Quota(command) => RoutedCommand::new(command),
            Commands::Run(command) => RoutedCommand::new(command),
            Commands::Caveman(command) => RoutedCommand::new(command),
            Commands::Super(command) => RoutedCommand::new(command),
            Commands::Claude(command) => RoutedCommand::new(command),
            Commands::RuntimeBroker(command) => RoutedCommand::new(command),
        }
    }
}

impl ProfileCommands {
    fn into_routed_command(self) -> RoutedCommand {
        match self {
            ProfileCommands::Add(command) => RoutedCommand::new(command),
            ProfileCommands::Export(command) => RoutedCommand::new(command),
            ProfileCommands::Import(command) => RoutedCommand::new(command),
            ProfileCommands::ImportCurrent(command) => RoutedCommand::new(command),
            ProfileCommands::List => RoutedCommand::new(ListProfilesCommand),
            ProfileCommands::Remove(command) => RoutedCommand::new(command),
            ProfileCommands::Use(command) => RoutedCommand::new(command),
        }
    }
}
