use super::*;

impl Commands {
    pub(super) fn execute(self) -> Result<()> {
        match self {
            Commands::Profile(command) => handle_profile_command(command),
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
