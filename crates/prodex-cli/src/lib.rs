use clap::{Parser, Subcommand};
use std::ffi::OsString;

const CODEX_COMMAND_SERVER_SUBCOMMANDS: [&str; 3] = ["mcp-server", "app-server", "exec-server"];

mod cleanup;
mod help;
mod profile;
mod runtime_args;
mod runtime_features;
mod session_context;

pub use cleanup::*;
pub use help::RUNTIME_PROXY_DOCTOR_TAIL_BYTES;
use help::*;
pub use presidio::*;
pub use profile::*;
pub use runtime_args::*;
pub use runtime_features::*;
pub use session_context::*;

mod presidio;

#[derive(Parser, Debug)]
#[command(
    name = "prodex",
    version,
    about = "Manage multiple Codex account profiles with profile-local auth and shared Codex state.",
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
        name = "log",
        about = "Show the latest transcript text and token counts or stream them live."
    )]
    Log(LogArgs),
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
        about = "Reconcile optional Prodex install surfaces and verify embedded assets.",
        after_help = CLI_SETUP_AFTER_HELP
    )]
    Setup(SetupArgs),
    #[command(
        subcommand,
        about = "List Prodex capabilities and local availability.",
        after_help = CLI_CAPABILITY_AFTER_HELP
    )]
    Capability(CapabilityCommands),
    #[command(
        about = "Inspect structured local audit events from the resolved audit log.",
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
        about = "Run provider login flows, using Prodex profiles where supported.",
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
        about = "Redeem one reset credit manually for a named OpenAI/Codex profile.",
        after_help = CLI_REDEEM_AFTER_HELP
    )]
    Redeem(RedeemArgs),
    #[command(
        about = "Serve a local browser dashboard for profiles, active account, and quota usage."
    )]
    Dashboard(DashboardArgs),
    #[command(
        trailing_var_arg = true,
        about = "Run codex through prodex with quota preflight and eligible pre-commit rotation.",
        after_help = CLI_RUN_AFTER_HELP
    )]
    Run(RunArgs),
    #[command(
        trailing_var_arg = true,
        about = "Run codex through prodex with Caveman mode active in a temporary Prodex overlay home.",
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
        name = "ponytail",
        trailing_var_arg = true,
        about = "Shortcut for `prodex caveman ponytail`.",
        after_help = CLI_CAVEMAN_AFTER_HELP
    )]
    Ponytail(CavemanArgs),
    #[command(
        name = "mem",
        visible_alias = "memory",
        trailing_var_arg = true,
        about = "Shortcut for `prodex caveman mem`.",
        after_help = CLI_CAVEMAN_AFTER_HELP
    )]
    Mem(CavemanArgs),
    #[command(
        trailing_var_arg = true,
        visible_alias = "s",
        about = "Daily shortcut for Super optimizer tools, full access, and opt-in Presidio/memory prompts.",
        after_help = CLI_SUPER_AFTER_HELP
    )]
    Super(SuperArgs),
    #[command(about = "Expose a protected browser terminal through a Cloudflare quick tunnel.")]
    Expose(ExposeArgs),
    #[command(
        about = "Run a standalone OpenAI-compatible gateway backed by Prodex provider routing."
    )]
    Gateway(GatewayArgs),
    #[command(
        trailing_var_arg = true,
        about = "Run Claude Code through prodex via an Anthropic-compatible runtime proxy.",
        after_help = CLI_CLAUDE_AFTER_HELP
    )]
    Claude(ClaudeArgs),
    #[command(name = "__runtime-broker", hide = true)]
    RuntimeBroker(RuntimeBrokerArgs),
    #[command(name = "__gemini-compat-refresh", hide = true)]
    GeminiCompatRefresh(GeminiCompatRefreshArgs),
    #[command(name = "__memory-mcp", hide = true)]
    MemoryMcp(MemoryMcpArgs),
    #[command(name = "__inspect-mcp", hide = true)]
    InspectMcp(InspectMcpArgs),
    #[command(name = "__mcp-jsonl-bridge", hide = true)]
    McpJsonlBridge(McpJsonlBridgeArgs),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CurrentCommand;

pub fn parse_cli_command_from<I, T>(args: I) -> std::result::Result<Commands, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let raw_args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    let raw_args = rewrite_super_doctor_args(&raw_args);
    let raw_args = rewrite_super_expose_args(&raw_args);
    let raw_args = rewrite_super_provider_alias_args(&raw_args);
    let parse_args = if should_default_cli_invocation_to_run(&raw_args) {
        rewrite_cli_args_as_run(&raw_args)
    } else {
        raw_args
    };
    Ok(Cli::try_parse_from(parse_args)?.command)
}

fn rewrite_super_doctor_args(args: &[OsString]) -> Vec<OsString> {
    let Some(command) = args.get(1).and_then(|arg| arg.to_str()) else {
        return args.to_vec();
    };
    if command != "s" && command != "super" {
        return args.to_vec();
    }
    let Some(subcommand) = args.get(2).and_then(|arg| arg.to_str()) else {
        return args.to_vec();
    };
    if subcommand != "doctor" {
        return args.to_vec();
    }
    let mut rewritten = Vec::with_capacity(args.len() + 1);
    rewritten.push(
        args.first()
            .cloned()
            .unwrap_or_else(|| OsString::from("prodex")),
    );
    rewritten.push(OsString::from("capability"));
    rewritten.push(OsString::from("super-doctor"));
    rewritten.extend(args.iter().skip(3).cloned());
    rewritten
}

fn rewrite_super_expose_args(args: &[OsString]) -> Vec<OsString> {
    let Some(command) = args.get(1).and_then(|arg| arg.to_str()) else {
        return args.to_vec();
    };
    if command != "s" && command != "super" {
        return args.to_vec();
    }
    let Some(subcommand) = args.get(2).and_then(|arg| arg.to_str()) else {
        return args.to_vec();
    };
    if subcommand != "expose" {
        return args.to_vec();
    }
    let mut rewritten = Vec::with_capacity(args.len() - 1);
    rewritten.push(
        args.first()
            .cloned()
            .unwrap_or_else(|| OsString::from("prodex")),
    );
    rewritten.push(OsString::from("expose"));
    rewritten.extend(args.iter().skip(3).cloned());
    rewritten
}

fn rewrite_super_provider_alias_args(args: &[OsString]) -> Vec<OsString> {
    let Some(command) = args.get(1).and_then(|arg| arg.to_str()) else {
        return args.to_vec();
    };
    if command != "s" && command != "super" {
        return args.to_vec();
    }
    let Some(provider) = args.get(2).and_then(|arg| arg.to_str()) else {
        return args.to_vec();
    };
    if !matches!(provider, "gemini" | "deepseek") {
        return args.to_vec();
    }

    let mut rewritten = Vec::with_capacity(args.len() + 1);
    rewritten.extend(args.iter().take(2).cloned());
    rewritten.push(OsString::from("--provider"));
    rewritten.extend(args.iter().skip(2).cloned());
    rewritten
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
            | "log"
            | "session"
            | "doctor"
            | "setup"
            | "capability"
            | "audit"
            | "context"
            | "cleanup"
            | "presidio"
            | "login"
            | "logout"
            | "update"
            | "quota"
            | "redeem"
            | "dashboard"
            | "run"
            | "caveman"
            | "rtk"
            | "sqz"
            | "tokensavior"
            | "token-savior"
            | "clawcompactor"
            | "claw-compactor"
            | "ponytail"
            | "mem"
            | "memory"
            | "super"
            | "s"
            | "expose"
            | "gateway"
            | "claude"
            | "help"
            | "__runtime-broker"
            | "__gemini-compat-refresh"
            | "__memory-mcp"
            | "__inspect-mcp"
            | "__mcp-jsonl-bridge"
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
