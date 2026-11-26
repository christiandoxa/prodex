use anyhow::Result;
use prodex_cli::{Commands, parse_cli_command_from};
use prodex_core::AppPaths;
use prodex_mojo_core::app_command_policy::AppCommandStartupPlan;
use redaction::redaction_redact_secret_like_text;
use std::env;
use std::io::{self, Write};

use crate::app_commands::runtime_launch::goal_resume::handle_runtime_goal_session_notify_if_requested;
use crate::command_dispatch;
use crate::housekeeping::schedule_prodex_auto_runtime_housekeeping;
use crate::process_resources::prepare_descriptor_headroom;
use crate::runtime_policy::ensure_runtime_policy_valid;
use crate::shared_codex_fs::create_codex_home_if_missing;
use crate::update_notice::show_update_notice_if_available;

pub fn main_entry() {
    // A low shell soft limit must not strand state loading, runtime sockets or
    // child launch. Failure is nonfatal; normal I/O still reports its real error.
    let _ = prepare_descriptor_headroom();
    let result = match handle_runtime_goal_session_notify_if_requested() {
        Ok(true) => Ok(()),
        Ok(false) => run(),
        Err(err) => Err(err),
    };
    if let Err(err) = result {
        if main_entry_error_is_broken_pipe(&err) {
            return;
        }
        let _ = writeln!(
            io::stderr().lock(),
            "Error: {}",
            main_entry_error_message(&err)
        );
        std::process::exit(main_entry_exit_code(&err));
    }
}

pub(super) fn main_entry_error_is_broken_pipe(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        cause
            .downcast_ref::<io::Error>()
            .is_some_and(|error| error.kind() == io::ErrorKind::BrokenPipe)
    })
}

pub(super) fn main_entry_exit_code(err: &anyhow::Error) -> i32 {
    err.downcast_ref::<command_dispatch::ProdexCommandExit>()
        .map_or(1, command_dispatch::ProdexCommandExit::code)
}

pub(super) fn main_entry_error_message(err: &anyhow::Error) -> String {
    redaction_redact_secret_like_text(&format!("{err:#}"))
}

fn run() -> Result<()> {
    let command = match parse_cli_command_from(env::args_os()) {
        Ok(command) => command,
        Err(err) => err.exit(),
    };
    let policy = command_dispatch::command_startup_policy(&command);
    let initialize_home = !policy.super_dry_run && !command_uses_minimal_startup(&command, &policy);
    if initialize_home {
        create_codex_home_if_missing(&AppPaths::discover()?.root)?;
        if policy.show_update_notice {
            let _ = show_update_notice_if_available(&command);
        }
    }
    validate_command_runtime_policy(&command)?;
    if initialize_home {
        schedule_prodex_auto_runtime_housekeeping(&command);
    }
    command_dispatch::execute_command(command)
}

fn command_uses_minimal_startup(command: &Commands, policy: &AppCommandStartupPlan) -> bool {
    policy.minimal_startup
        || matches!(
            command,
            Commands::Doctor(args)
                if args.install
                    && !args.quota
                    && !args.runtime
                    && !args.repair_import_auth_journals
                    && args.bundle.is_none()
        )
}

pub(super) fn validate_command_runtime_policy(command: &Commands) -> Result<()> {
    if command.launches_runtime() && !command_dispatch::command_uses_native_agy(command) {
        ensure_runtime_policy_valid()?;
    }
    Ok(())
}

#[cfg(test)]
mod minimal_startup_tests {
    use super::{Commands, command_dispatch, command_uses_minimal_startup, parse_cli_command_from};

    #[test]
    fn doctor_install_alone_uses_minimal_startup() {
        let install = parse_cli_command_from(["prodex", "doctor", "--install"]).unwrap();
        let combined =
            parse_cli_command_from(["prodex", "doctor", "--install", "--runtime"]).unwrap();
        let bridge =
            parse_cli_command_from(["prodex", "__mcp-jsonl-bridge", "mcp-server"]).unwrap();
        let ping = parse_cli_command_from(["prodex", "ping", "openai"]).unwrap();
        let antigravity =
            parse_cli_command_from(["prodex", "s", "gemini", "--cli", "agy", "--no-presidio"])
                .unwrap();

        let uses_minimal_startup = |command: &Commands| {
            command_uses_minimal_startup(
                command,
                &command_dispatch::command_startup_policy(command),
            )
        };
        assert!(uses_minimal_startup(&install));
        assert!(uses_minimal_startup(&bridge));
        assert!(uses_minimal_startup(&ping));
        assert!(uses_minimal_startup(&antigravity));
        assert!(!uses_minimal_startup(&combined));
    }
}
