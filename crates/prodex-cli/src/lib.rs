use clap::{Parser, Subcommand};
use std::ffi::OsString;

const CODEX_COMMAND_SERVER_SUBCOMMANDS: [&str; 3] = ["mcp-server", "app-server", "exec-server"];

mod help;
mod ping;
mod presidio;
mod profile;
mod runtime_args;
mod runtime_features;
mod session_context;
mod sub_agent;
pub(crate) mod super_provider_limits;

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
        about = "Summarize Prodex version, profiles, runtime policy, logs, and secret backend."
    )]
    Info(InfoArgs),
    #[command(
        about = "Monitor profiles, quota resets, token efficiency, and Prodex resource usage."
    )]
    Status(StatusArgs),
    #[command(
        about = "Follow redacted Prodex runtime logs.",
        after_help = "Examples:
  prodex log
  prodex log last
  prodex log upstream
  prodex log --json"
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
        subcommand,
        about = "Send lightweight prompt checks through ready profiles."
    )]
    Ping(PingCommands),
    #[command(
        trailing_var_arg = true,
        about = "Run codex through prodex with quota preflight and eligible pre-commit rotation.",
        after_help = CLI_RUN_AFTER_HELP
    )]
    Run(RunArgs),
    #[command(
        trailing_var_arg = true,
        visible_alias = "s",
        about = "YOLO shortcut for the Super tool stack with opt-in Presidio.",
        after_help = CLI_SUPER_AFTER_HELP
    )]
    Super(Box<SuperArgs>),
    #[command(about = "Run a lean OpenAI-compatible provider gateway.")]
    Gateway(GatewayArgs),
    #[command(name = "__super-expose", hide = true)]
    SuperExpose(Box<SuperExposeArgs>),
    #[command(name = "__runtime-broker", hide = true)]
    RuntimeBroker(RuntimeBrokerArgs),
    #[command(name = "__mcp-jsonl-bridge", hide = true)]
    McpJsonlBridge(McpJsonlBridgeArgs),
    #[command(name = "__sub-agent-exec", hide = true)]
    SubAgentExec(SubAgentExecArgs),
}

impl Commands {
    pub fn launches_runtime(&self) -> bool {
        matches!(
            self,
            Self::Run(_)
                | Self::Super(_)
                | Self::Gateway(_)
                | Self::SuperExpose(_)
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
            Self::Login(_) => "login",
            Self::Logout(_) => "logout",
            Self::Update(_) => "update",
            Self::Quota(_) => "quota",
            Self::Ping(_) => "ping",
            Self::Run(_) => "run",
            Self::Super(_) => "super",
            Self::Gateway(_) => "gateway",
            Self::SuperExpose(_) => "super-expose",
            Self::RuntimeBroker(_) => "__runtime-broker",
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
    let raw_args = rewrite_super_expose_alias(&raw_args);
    let raw_args = rewrite_super_compat_args(&raw_args);
    let parse_args = if should_default_cli_invocation_to_run(&raw_args) {
        rewrite_cli_args_as_run(&raw_args)
    } else {
        raw_args
    };
    let command = Cli::try_parse_from(parse_args.clone())?.command;
    let mut command = rewrite_positioned_super_alias(&parse_args, command)?;
    restore_super_literal_boundary(&parse_args, &mut command);
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

fn rewrite_super_expose_alias(args: &[OsString]) -> Vec<OsString> {
    if !matches!(
        args.get(1).and_then(|arg| arg.to_str()),
        Some("super" | "s")
    ) {
        return args.to_vec();
    }

    let mut index = 2;
    let mut expose_index = None;
    while index < args.len() {
        let Some(value) = args[index].to_str() else {
            break;
        };
        if value == "--" {
            break;
        }
        if value == "expose" {
            expose_index = Some(index);
            break;
        }
        if !value.starts_with('-') {
            break;
        }
        if super_option_takes_value(value) && !value.contains('=') {
            index = index.saturating_add(2);
        } else {
            index = index.saturating_add(1);
        }
    }
    let Some(expose_index) = expose_index else {
        return args.to_vec();
    };

    let mut rewritten = Vec::with_capacity(args.len().saturating_sub(1));
    rewritten.push(args[0].clone());
    rewritten.push(OsString::from("__super-expose"));
    rewritten.extend(
        args.iter()
            .enumerate()
            .skip(1)
            .filter(|(position, _)| *position != 1 && *position != expose_index)
            .map(|(_, arg)| arg.clone()),
    );
    rewritten
}

fn super_option_takes_value(value: &str) -> bool {
    matches!(
        value,
        "-p" | "--profile"
            | "--base-url"
            | "--sub-agent-provider"
            | "--sub-agent-model"
            | "--sub-agent-model-reasoning-effort"
            | "--sub-agent-url"
            | "--sub-agent-max-concurrency"
            | "--tool"
            | "--require-tool"
            | "--url"
            | "--provider"
            | "--api-key"
            | "--model"
            | "--local-model"
            | "--context-window"
            | "--local-context-window"
            | "--auto-compact-token-limit"
            | "--local-auto-compact-token-limit"
            | "-c"
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

fn rewrite_super_compat_args(args: &[OsString]) -> Vec<OsString> {
    let Some(command) = args.get(1).and_then(|arg| arg.to_str()) else {
        return args.to_vec();
    };
    if !matches!(command, "s" | "super") {
        return args.to_vec();
    }
    let Some(subcommand) = args.get(2).and_then(|arg| arg.to_str()) else {
        return args.to_vec();
    };

    let mut rewritten = Vec::with_capacity(args.len() + 1);
    rewritten.push(
        args.first()
            .cloned()
            .unwrap_or_else(|| OsString::from("prodex")),
    );

    match subcommand {
        "gemini" | "deepseek" => {
            rewritten.push(args[1].clone());
            rewritten.push(OsString::from("--provider"));
            rewritten.extend(args.iter().skip(2).cloned());
        }
        _ => return args.to_vec(),
    }

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
            | "login"
            | "logout"
            | "update"
            | "quota"
            | "ping"
            | "run"
            | "super"
            | "s"
            | "gateway"
            | "claude"
            | "help"
            | "__super-expose"
            | "__runtime-broker"
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
