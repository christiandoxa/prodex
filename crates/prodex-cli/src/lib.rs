use clap::{Parser, Subcommand};
use std::ffi::OsString;

const CODEX_COMMAND_SERVER_SUBCOMMANDS: [&str; 3] = ["mcp-server", "app-server", "exec-server"];

mod cleanup;
mod help;
mod ping;
mod profile;
mod runtime_args;
mod runtime_features;
mod session_context;
mod sub_agent;
pub(crate) mod super_provider_limits;

pub use cleanup::*;
pub use help::RUNTIME_PROXY_DOCTOR_TAIL_BYTES;
use help::*;
pub use ping::*;
pub use presidio::*;
pub use profile::*;
pub use runtime_args::*;
pub use runtime_features::*;
pub use session_context::*;
pub use sub_agent::*;
pub use super_provider_limits::{
    SUPER_COPILOT_DEFAULT_AUTO_COMPACT_LIMIT, SUPER_COPILOT_DEFAULT_CONTEXT_WINDOW,
    super_copilot_prompt_token_limit_for_model,
};

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

#[derive(Parser, Debug)]
#[command(
    name = "prodex expose",
    bin_name = "prodex expose",
    after_help = CLI_EXPOSE_AFTER_HELP
)]
struct SuperExposeCli {
    #[command(flatten)]
    args: runtime_args::SuperExposeArgs,
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
    #[command(about = "Summarize version status, running processes, quota pool, and runway.")]
    Info(InfoArgs),
    #[command(
        about = "Monitor profiles, quota resets, token efficiency, and Prodex resource usage."
    )]
    Status(StatusArgs),
    #[command(
        about = "Stream Prodex runtime logs (default); use upstream for payload snapshots.",
        after_help = "Examples:\n  prodex log                 Live log stream (default)\n  prodex log stream          Explicit live log stream\n  prodex log upstream        Upstream payload view"
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
        about = "Reconcile local Prodex directories and verify optional tools.",
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
    #[command(about = "Update Prodex from the latest verified GitHub release binary.")]
    Update(ProdexUpdateArgs),
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
        subcommand,
        about = "Send lightweight prompt checks through ready profiles."
    )]
    Ping(PingCommands),
    #[command(about = "Launch Codex Desktop through Prodex on this platform.")]
    Gui(GuiArgs),
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
    Caveman(RuntimeToolArgs),
    #[command(
        trailing_var_arg = true,
        about = "Shortcut for `prodex caveman rtk`.",
        after_help = CLI_CAVEMAN_AFTER_HELP
    )]
    Rtk(RuntimeToolArgs),
    #[command(
        trailing_var_arg = true,
        about = "Shortcut for `prodex caveman playwright`.",
        after_help = CLI_CAVEMAN_AFTER_HELP
    )]
    Playwright(RuntimeToolArgs),
    #[command(
        trailing_var_arg = true,
        about = "Shortcut for `prodex caveman ponytail`.",
        after_help = CLI_CAVEMAN_AFTER_HELP
    )]
    Ponytail(RuntimeToolArgs),
    #[command(
        trailing_var_arg = true,
        visible_alias = "s",
        about = "YOLO shortcut for the Super tool stack with opt-in Presidio.",
        after_help = CLI_SUPER_AFTER_HELP
    )]
    Super(SuperArgs),
    #[command(
        about = "Expose a protected browser terminal, or `prodex s expose` as a ChatGPT MCP endpoint.",
        after_help = CLI_EXPOSE_AFTER_HELP
    )]
    Expose(ExposeArgs),
    #[command(about = "Inspect the experimental JSON-RPC app-server broker contract.")]
    AppServerBroker(AppServerBrokerArgs),
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
    #[command(name = "__mcp-jsonl-bridge", hide = true)]
    McpJsonlBridge(McpJsonlBridgeArgs),
    #[command(name = "__sub-agent-exec", hide = true)]
    SubAgentExec(SubAgentExecArgs),
}

impl Commands {
    pub fn launches_runtime(&self) -> bool {
        matches!(
            self,
            Self::Gui(_)
                | Self::Run(_)
                | Self::Caveman(_)
                | Self::Rtk(_)
                | Self::Playwright(_)
                | Self::Ponytail(_)
                | Self::Super(_)
                | Self::Expose(_)
                | Self::Gateway(GatewayArgs { command: None, .. })
                | Self::Claude(_)
                | Self::RuntimeBroker(_)
        )
    }

    pub fn process_label(&self) -> &'static str {
        match self {
            Self::Profile(_) => "profile",
            Self::UseProfile(_) => "use",
            Self::Current => "current",
            Self::Info(_) => "info",
            Self::Status(_) => "status",
            Self::Log(_) => "log",
            Self::Session(_) => "session",
            Self::Doctor(_) => "doctor",
            Self::Setup(_) => "setup",
            Self::Capability(_) => "capability",
            Self::Audit(_) => "audit",
            Self::Context(_) => "context",
            Self::Cleanup(_) => "cleanup",
            Self::Presidio(_) => "presidio",
            Self::Login(_) => "login",
            Self::Logout(_) => "logout",
            Self::Update(_) => "update",
            Self::Quota(_) => "quota",
            Self::Redeem(_) => "redeem",
            Self::Ping(_) => "ping",
            Self::Gui(_) => "gui",
            Self::Dashboard(_) => "dashboard",
            Self::Run(_) => "run",
            Self::Caveman(_) => "caveman",
            Self::Rtk(_) => "rtk",
            Self::Playwright(_) => "playwright",
            Self::Ponytail(_) => "ponytail",
            Self::Super(_) => "super",
            Self::Expose(_) => "expose",
            Self::AppServerBroker(_) => "app-server-broker",
            Self::Gateway(_) => "gateway",
            Self::Claude(_) => "claude",
            Self::RuntimeBroker(_) => "__runtime-broker",
            Self::GeminiCompatRefresh(_) => "__gemini-compat-refresh",
            Self::McpJsonlBridge(_) => "__mcp-jsonl-bridge",
            Self::SubAgentExec(_) => "__sub-agent-exec",
        }
    }
}

pub fn parse_cli_command_from<I, T>(args: I) -> std::result::Result<Commands, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let raw_args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    if let Some(command) = parse_super_expose_alias(&raw_args)? {
        return Ok(command);
    }
    let raw_args = rewrite_super_doctor_args(&raw_args);
    let raw_args = rewrite_super_provider_alias_args(&raw_args);
    let parse_args = if should_default_cli_invocation_to_run(&raw_args) {
        rewrite_cli_args_as_run(&raw_args)
    } else {
        raw_args
    };
    let command = Cli::try_parse_from(parse_args.clone())?.command;
    let mut command = rewrite_positioned_super_alias(&parse_args, command)?;
    restore_super_literal_boundary(&parse_args, &mut command);
    match &mut command {
        Commands::Caveman(args)
        | Commands::Rtk(args)
        | Commands::Playwright(args)
        | Commands::Ponytail(args) => args.translate_legacy_leading_tool_prefixes(),
        _ => {}
    }
    if let Commands::Quota(args) = &mut command
        && !args.all
        && args.profile.is_none()
        && !args.raw
    {
        args.all = true;
        args.detail = true;
    }
    Ok(command)
}

fn parse_super_expose_alias(
    args: &[OsString],
) -> std::result::Result<Option<Commands>, clap::Error> {
    if !matches!(
        args.get(1).and_then(|arg| arg.to_str()),
        Some("s" | "super")
    ) {
        return Ok(None);
    }

    let Some(expose_index) = super_expose_index(args) else {
        return Ok(None);
    };

    let mut rewritten = Vec::with_capacity(args.len().saturating_sub(2));
    rewritten.push(
        args.first()
            .cloned()
            .unwrap_or_else(|| OsString::from("prodex")),
    );
    rewritten.extend(args.iter().skip(2).take(expose_index - 2).cloned());
    rewritten.extend(args.iter().skip(expose_index + 1).cloned());
    let parsed = SuperExposeCli::try_parse_from(rewritten)?;
    let mut super_args = parsed.args.super_args;
    super_args
        .extract_super_overrides_from_codex_args()
        .map_err(|error| clap::Error::raw(clap::error::ErrorKind::InvalidValue, error))?;
    let mut expose = parsed.args.expose;
    expose.invocation = ExposeInvocation::SuperAlias;
    expose.super_args = Some(super_args);
    Ok(Some(Commands::Expose(expose)))
}

fn super_expose_index(args: &[OsString]) -> Option<usize> {
    let mut index = 2;
    while index < args.len() {
        let value = args[index].to_str()?;
        if value == "--" {
            return None;
        }
        if value == "expose" {
            return Some(index);
        }
        if super_option_takes_value(value) {
            index += 2;
        } else if value.starts_with('-') {
            index += 1;
        } else {
            return None;
        }
    }
    None
}

fn super_option_takes_value(value: &str) -> bool {
    matches!(
        value.split_once('=').map_or(value, |(name, _)| name),
        "-c" | "--config"
            | "-p"
            | "--command"
            | "--name"
            | "--cols"
            | "--rows"
            | "--max-clients"
            | "--profile"
            | "--base-url"
            | "--provider"
            | "--harness"
            | "--api-key"
            | "--sub-agent-provider"
            | "--sub-agent-model"
            | "--sub-agent-model-reasoning-effort"
            | "--sub-agent-url"
            | "--sub-agent-max-concurrency"
            | "--model"
            | "--local-model"
            | "--url"
            | "--context-window"
            | "--local-context-window"
            | "--auto-compact-token-limit"
            | "--local-auto-compact-token-limit"
            | "--cli"
            | "--tool"
            | "--require-tool"
            | "--web-search"
            | "--rollout-budget-tokens"
            | "--rollout-budget-reminders"
            | "--rollout-budget-sampling-weight"
            | "--rollout-budget-prefill-weight"
            | "--current-time-reminder-interval"
            | "--current-time-clock-source"
    )
}

fn rewrite_positioned_super_alias(
    args: &[OsString],
    command: Commands,
) -> std::result::Result<Commands, clap::Error> {
    if super_literal_boundary(args).is_some() {
        return Ok(command);
    }
    let Commands::Super(mut super_args) = command else {
        return Ok(command);
    };
    let Some(alias) = super_args.codex_args.first().and_then(|arg| arg.to_str()) else {
        return Ok(Commands::Super(super_args));
    };

    match alias {
        "doctor" => rewrite_positioned_super_command(
            args,
            &super_args.codex_args,
            &["capability", "super-doctor"],
        ),
        "expose" => rewrite_positioned_super_command(args, &super_args.codex_args, &["expose"]),
        "gemini" if super_args.provider.is_none() && super_args.url.is_none() => {
            super_args.provider = Some(SuperExternalProvider::Gemini);
            super_args.codex_args.remove(0);
            Ok(Commands::Super(super_args))
        }
        "deepseek" if super_args.provider.is_none() && super_args.url.is_none() => {
            super_args.provider = Some(SuperExternalProvider::DeepSeek);
            super_args.codex_args.remove(0);
            Ok(Commands::Super(super_args))
        }
        _ => Ok(Commands::Super(super_args)),
    }
}

fn rewrite_positioned_super_command(
    args: &[OsString],
    codex_args: &[OsString],
    replacement: &[&str],
) -> std::result::Result<Commands, clap::Error> {
    let Some(alias_index) = args
        .windows(codex_args.len())
        .rposition(|window| window == codex_args)
    else {
        return Ok(Cli::try_parse_from(args)?.command);
    };

    let mut rewritten = Vec::with_capacity(args.len() + replacement.len());
    rewritten.push(
        args.first()
            .cloned()
            .unwrap_or_else(|| OsString::from("prodex")),
    );
    rewritten.extend(replacement.iter().map(|value| OsString::from(*value)));
    rewritten.extend(
        args.iter()
            .take(alias_index)
            .skip(2)
            .filter(|arg| *arg != "--no-presidio")
            .cloned(),
    );
    rewritten.extend(args.iter().skip(alias_index + 1).cloned());
    Ok(Cli::try_parse_from(rewritten)?.command)
}

fn super_literal_boundary(args: &[OsString]) -> Option<usize> {
    matches!(
        args.get(1).and_then(|arg| arg.to_str()),
        Some("super" | "s")
    )
    .then(|| {
        args.iter()
            .enumerate()
            .skip(2)
            .find_map(|(index, arg)| (arg == "--").then_some(index))
    })
    .flatten()
}

fn restore_super_literal_boundary(args: &[OsString], command: &mut Commands) {
    let Some(boundary_index) = super_literal_boundary(args) else {
        return;
    };
    let Commands::Super(super_args) = command else {
        return;
    };
    if super_args.codex_args.contains(&OsString::from("--")) {
        return;
    }

    let suffix = &args[(boundary_index + 1)..];
    let insertion_index = super_args.codex_args.len().saturating_sub(suffix.len());
    if super_args.codex_args[insertion_index..] == *suffix {
        super_args
            .codex_args
            .insert(insertion_index, OsString::from("--"));
    }
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
            | "status"
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
            | "ping"
            | "dashboard"
            | "gui"
            | "run"
            | "caveman"
            | "rtk"
            | "playwright"
            | "ponytail"
            | "super"
            | "s"
            | "app-server-broker"
            | "expose"
            | "gateway"
            | "claude"
            | "help"
            | "__runtime-broker"
            | "__gemini-compat-refresh"
            | "__mcp-jsonl-bridge"
            | "__sub-agent-exec"
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
