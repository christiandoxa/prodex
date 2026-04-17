use super::*;

impl Commands {
    pub(super) fn execute(self) -> Result<()> {
        match self {
            Commands::Profile(command) => command.execute(),
            Commands::UseProfile(command) => command.execute(),
            Commands::Current => CurrentCommand.execute(),
            Commands::Info(command) => command.execute(),
            Commands::Doctor(command) => command.execute(),
            Commands::Audit(command) => command.execute(),
            Commands::Cleanup => CleanupCommand.execute(),
            Commands::Login(command) => command.execute(),
            Commands::Logout(command) => command.execute(),
            Commands::Quota(command) => command.execute(),
            Commands::Run(command) => command.execute(),
            Commands::Caveman(command) => command.execute(),
            Commands::Claude(command) => command.execute(),
            Commands::RuntimeBroker(command) => command.execute(),
        }
    }

    pub(super) fn should_show_update_notice(&self) -> bool {
        !matches!(self, Commands::RuntimeBroker(_))
    }
}

impl ProfileCommands {
    fn execute(self) -> Result<()> {
        match self {
            ProfileCommands::Add(command) => command.execute(),
            ProfileCommands::Export(command) => command.execute(),
            ProfileCommands::Import(command) => command.execute(),
            ProfileCommands::ImportCurrent(command) => command.execute(),
            ProfileCommands::List => ListProfilesCommand.execute(),
            ProfileCommands::Remove(command) => command.execute(),
            ProfileCommands::Use(command) => command.execute(),
        }
    }
}
