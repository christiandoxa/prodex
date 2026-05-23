use clap::{Parser, Subcommand};
use std::ffi::OsString;

const CODEX_COMMAND_SERVER_SUBCOMMANDS: [&str; 3] = ["mcp-server", "app-server", "exec-server"];

mod cleanup;
mod help;
mod profile;
mod runtime_args;
mod session_context;

pub use cleanup::*;
pub use help::RUNTIME_PROXY_DOCTOR_TAIL_BYTES;
use help::*;
pub use presidio::*;
pub use profile::*;
pub use runtime_args::*;
pub use session_context::*;

mod presidio;

#[derive(Parser, Debug)]
#[command(
    name = "prodex",
    version,
    about = "Manage multiple Codex profiles backed by isolated CODEX_HOME directories.",
    after_help = CLI_TOP_LEVEL_AFTER_HELP
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(
        subcommand,
        about = "Add, inspect, remove, and activate managed profiles.",
        after_help = CLI_PROFILE_AFTER_HELP
    )]
    Profile(ProfileCommands),
    #[command(
        name = "use",
        about = "Set the active profile used by commands that omit --profile."
    )]
    UseProfile(ProfileSelector),
    #[command(about = "Show the active profile and its CODEX_HOME details.")]
    Current,
    #[command(
        name = "info",
        about = "Summarize version status, running processes, quota pool, and runway."
    )]
    Info(InfoArgs),
    #[command(
        subcommand,
        about = "Inspect shared Codex session metadata.",
        after_help = CLI_SESSION_AFTER_HELP
    )]
    Session(SessionCommands),
    #[command(
        about = "Inspect local state, Codex resolution, quota readiness, and runtime logs.",
        after_help = CLI_DOCTOR_AFTER_HELP
    )]
    Doctor(DoctorArgs),
    #[command(
        about = "Inspect structured enterprise audit events written to /tmp.",
        after_help = CLI_AUDIT_AFTER_HELP
    )]
    Audit(AuditArgs),
    #[command(
        subcommand,
        about = "Audit and compact token-heavy shared Codex context files.",
        after_help = CLI_CONTEXT_AFTER_HELP
    )]
    Context(ContextCommands),
    #[command(
        about = "Remove stale local runtime logs, temp homes, dead broker artifacts, and orphaned managed homes.",
        after_help = CLI_CLEANUP_AFTER_HELP
    )]
    Cleanup(CleanupArgs),
    #[command(
        subcommand,
        about = "Manage local Microsoft Presidio PII detection and anonymization."
    )]
    Presidio(PresidioCommands),
    #[command(
        trailing_var_arg = true,
        about = "Run codex login inside a selected or auto-created profile.",
        after_help = CLI_LOGIN_AFTER_HELP
    )]
    Login(CodexPassthroughArgs),
    #[command(about = "Run codex logout for the selected or active profile.")]
    Logout(LogoutArgs),
    #[command(
        trailing_var_arg = true,
        disable_help_flag = true,
        about = "Run codex update directly without Prodex profile or runtime routing."
    )]
    Update(CodexUpdateArgs),
    #[command(
        about = "Inspect live quota for one profile or the whole profile pool.",
        after_help = CLI_QUOTA_AFTER_HELP
    )]
    Quota(QuotaArgs),
    #[command(
        trailing_var_arg = true,
        about = "Run codex through prodex with quota preflight and safe auto-rotate.",
        after_help = CLI_RUN_AFTER_HELP
    )]
    Run(RunArgs),
    #[command(
        trailing_var_arg = true,
        about = "Run codex through prodex with the Caveman plugin active in a temporary overlay home.",
        after_help = CLI_CAVEMAN_AFTER_HELP
    )]
    Caveman(CavemanArgs),
    #[command(
        trailing_var_arg = true,
        about = "Shortcut for `prodex caveman rtk`.",
        after_help = CLI_CAVEMAN_AFTER_HELP
    )]
    Rtk(CavemanArgs),
    #[command(
        trailing_var_arg = true,
        about = "Shortcut for `prodex caveman sqz`.",
        after_help = CLI_CAVEMAN_AFTER_HELP
    )]
    Sqz(CavemanArgs),
    #[command(
        name = "tokensavior",
        visible_alias = "token-savior",
        trailing_var_arg = true,
        about = "Shortcut for `prodex caveman tokensavior`.",
        after_help = CLI_CAVEMAN_AFTER_HELP
    )]
    TokenSavior(CavemanArgs),
    #[command(
        name = "clawcompactor",
        visible_alias = "claw-compactor",
        trailing_var_arg = true,
        about = "Shortcut for `prodex caveman clawcompactor`.",
        after_help = CLI_CAVEMAN_AFTER_HELP
    )]
    ClawCompactor(CavemanArgs),
    #[command(
        name = "llmmin",
        visible_alias = "llm-min",
        trailing_var_arg = true,
        about = "Shortcut for `prodex caveman llmmin`.",
        after_help = CLI_CAVEMAN_AFTER_HELP
    )]
    LlmMin(CavemanArgs),
    #[command(
        trailing_var_arg = true,
        visible_alias = "s",
        about = "Alias for `prodex caveman mem rtk sqz tokensavior clawcompactor llmmin --full-access`.",
        after_help = CLI_SUPER_AFTER_HELP
    )]
    Super(SuperArgs),
    #[command(
        trailing_var_arg = true,
        about = "Run Claude Code through prodex via an Anthropic-compatible runtime proxy.",
        after_help = CLI_CLAUDE_AFTER_HELP
    )]
    Claude(ClaudeArgs),
    #[command(name = "__runtime-broker", hide = true)]
    RuntimeBroker(RuntimeBrokerArgs),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CurrentCommand;

pub fn parse_cli_command_from<I, T>(args: I) -> std::result::Result<Commands, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let raw_args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    let parse_args = if should_default_cli_invocation_to_run(&raw_args) {
        rewrite_cli_args_as_run(&raw_args)
    } else {
        raw_args
    };
    Ok(Cli::try_parse_from(parse_args)?.command)
}

pub fn should_default_cli_invocation_to_run(args: &[OsString]) -> bool {
    let Some(first_arg) = args.get(1).and_then(|arg| arg.to_str()) else {
        return true;
    };

    !matches!(
        first_arg,
        "-h" | "--help"
            | "-V"
            | "--version"
            | "profile"
            | "use"
            | "current"
            | "info"
            | "session"
            | "doctor"
            | "audit"
            | "context"
            | "cleanup"
            | "presidio"
            | "login"
            | "logout"
            | "update"
            | "quota"
            | "run"
            | "caveman"
            | "rtk"
            | "sqz"
            | "tokensavior"
            | "token-savior"
            | "clawcompactor"
            | "claw-compactor"
            | "llmmin"
            | "llm-min"
            | "super"
            | "s"
            | "claude"
            | "help"
            | "__runtime-broker"
    )
}

pub fn is_codex_command_server_subcommand(args: &[OsString]) -> bool {
    let Some(first_arg) = args.first().and_then(|arg| arg.to_str()) else {
        return false;
    };
    CODEX_COMMAND_SERVER_SUBCOMMANDS.contains(&first_arg)
}

pub fn rewrite_cli_args_as_run(args: &[OsString]) -> Vec<OsString> {
    let mut rewritten = Vec::with_capacity(args.len() + 1);
    rewritten.push(
        args.first()
            .cloned()
            .unwrap_or_else(|| OsString::from("prodex")),
    );
    rewritten.push(OsString::from("run"));
    rewritten.extend(args.iter().skip(1).cloned());
    rewritten
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
