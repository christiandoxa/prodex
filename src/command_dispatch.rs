use super::*;

impl Commands {
    pub(super) fn execute(self) -> Result<()> {
        match self {
            Commands::Profile(command) => command.execute(),
            Commands::UseProfile(selector) => handle_set_active_profile(selector),
            Commands::Current => handle_current_profile(),
            Commands::Info(args) => handle_info(args),
            Commands::Doctor(args) => handle_doctor(args),
            Commands::Audit(args) => handle_audit(args),
            Commands::Cleanup => handle_cleanup(),
            Commands::Login(args) => handle_codex_login(args),
            Commands::Logout(args) => handle_codex_logout(args),
            Commands::Quota(args) => handle_quota(args),
            Commands::Run(args) => handle_run(args),
            Commands::Caveman(args) => handle_caveman(args),
            Commands::Claude(args) => handle_claude(args),
            Commands::RuntimeBroker(args) => handle_runtime_broker(args),
        }
    }

    pub(super) fn should_show_update_notice(&self) -> bool {
        !matches!(self, Commands::RuntimeBroker(_))
    }
}

impl ProfileCommands {
    fn execute(self) -> Result<()> {
        match self {
            ProfileCommands::Add(args) => handle_add_profile(args),
            ProfileCommands::Export(args) => handle_export_profiles(args),
            ProfileCommands::Import(args) => handle_import_profiles(args),
            ProfileCommands::ImportCurrent(args) => handle_import_current_profile(args),
            ProfileCommands::List => handle_list_profiles(),
            ProfileCommands::Remove(args) => handle_remove_profile(args),
            ProfileCommands::Use(selector) => handle_set_active_profile(selector),
        }
    }
}
