use clap::{Parser, Subcommand};
use std::ffi::OsString;

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
    after_help = cli_after_help(CliHelpKind::TopLevel)
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
        after_help = cli_after_help(CliHelpKind::Profile)
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
        after_help = cli_after_help(CliHelpKind::Session)
    )]
    Session(SessionCommands),
    #[command(
        about = "Inspect local state, Codex resolution, quota readiness, and runtime logs.",
        after_help = cli_after_help(CliHelpKind::Doctor)
    )]
    Doctor(DoctorArgs),
    #[command(
        trailing_var_arg = true,
        about = "Run provider login flows, using Prodex profiles where supported.",
        after_help = cli_after_help(CliHelpKind::Login)
    )]
    Login(CodexPassthroughArgs),
    #[command(about = "Run codex logout for the selected or active profile.")]
    Logout(LogoutArgs),
    #[command(about = "Update Prodex from the latest verified GitHub release binary.")]
    Update(ProdexUpdateArgs),
    #[command(
        about = "Inspect live quota for one profile or the whole profile pool.",
        after_help = cli_after_help(CliHelpKind::Quota)
    )]
    Quota(QuotaArgs),
    #[command(
        about = "Redeem one reset credit manually for a named OpenAI/Codex profile.",
        after_help = cli_after_help(CliHelpKind::Redeem)
    )]
    Redeem(RedeemArgs),
    #[command(
        subcommand,
        about = "Send lightweight prompt checks through ready profiles."
    )]
    Ping(PingCommands),
    #[command(
        trailing_var_arg = true,
        about = "Run codex through prodex with quota preflight and eligible pre-commit rotation.",
        after_help = cli_after_help(CliHelpKind::Run)
    )]
    Run(RunArgs),
    #[command(
        trailing_var_arg = true,
        visible_alias = "s",
        about = "YOLO shortcut for the Super tool stack with opt-in Presidio.",
        after_help = cli_after_help(CliHelpKind::Super)
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
    fn presentation_kind(&self) -> prodex_mojo_core::cli_presentation::CliCommandKind {
        use prodex_mojo_core::cli_presentation::CliCommandKind;
        match self {
            Self::Profile(_) => CliCommandKind::Profile,
            Self::UseProfile(_) => CliCommandKind::UseProfile,
            Self::Current => CliCommandKind::Current,
            Self::Info(_) => CliCommandKind::Info,
            Self::Status(_) => CliCommandKind::Status,
            Self::Log(_) => CliCommandKind::Log,
            Self::Session(_) => CliCommandKind::Session,
            Self::Doctor(_) => CliCommandKind::Doctor,
            Self::Login(_) => CliCommandKind::Login,
            Self::Logout(_) => CliCommandKind::Logout,
            Self::Update(_) => CliCommandKind::Update,
            Self::Quota(_) => CliCommandKind::Quota,
            Self::Redeem(_) => CliCommandKind::Redeem,
            Self::Ping(_) => CliCommandKind::Ping,
            Self::Run(_) => CliCommandKind::Run,
            Self::Super(_) => CliCommandKind::Super,
            Self::Gateway(_) => CliCommandKind::Gateway,
            Self::SuperExpose(_) => CliCommandKind::SuperExpose,
            Self::RuntimeBroker(_) => CliCommandKind::RuntimeBroker,
            Self::McpJsonlBridge(_) => CliCommandKind::McpJsonlBridge,
            Self::SubAgentExec(_) => CliCommandKind::SubAgentExec,
        }
    }

    pub fn launches_runtime(&self) -> bool {
        prodex_mojo_core::cli_presentation::launches_runtime(self.presentation_kind())
            .expect("compiled Mojo CLI runtime classification returned invalid output")
    }

    pub fn process_label(&self) -> &'static str {
        prodex_mojo_core::cli_presentation::process_label(self.presentation_kind())
            .expect("compiled Mojo CLI process label returned invalid output")
    }
}

pub fn parse_cli_command_from<I, T>(args: I) -> std::result::Result<Commands, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let raw_args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    let raw_args = reassemble_super_expose_alias(&raw_args)?;
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

fn reassemble_super_expose_alias(args: &[OsString]) -> Result<Vec<OsString>, clap::Error> {
    let argument_views = args.iter().map(|arg| arg.to_str()).collect::<Vec<_>>();
    let expose_index = prodex_mojo_core::launch::find_super_expose_alias_index(&argument_views)
        .map_err(|error| {
            clap::Error::raw(
                clap::error::ErrorKind::ValueValidation,
                format!("failed to scan super expose alias: {error:?}"),
            )
        })?;
    let Some(expose_index) = expose_index else {
        return Ok(args.to_vec());
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
    Ok(rewritten)
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

    let alias = prodex_mojo_core::launch::positioned_super_provider_alias(
        Some(alias),
        super_args.provider.is_some(),
        super_args.url.is_some(),
    )
    .expect("Mojo positioned Super provider-alias policy returned invalid output");

    match alias {
        Some(prodex_mojo_core::launch::SuperProviderAlias::Gemini) => {
            super_args.provider = Some(SuperExternalProvider::Gemini);
            super_args.codex_args.remove(0);
            Ok(Commands::Super(super_args))
        }
        Some(prodex_mojo_core::launch::SuperProviderAlias::DeepSeek) => {
            super_args.provider = Some(SuperExternalProvider::DeepSeek);
            super_args.codex_args.remove(0);
            Ok(Commands::Super(super_args))
        }
        _ => Ok(Commands::Super(super_args)),
    }
}

fn super_literal_boundary(args: &[OsString]) -> Option<usize> {
    let argument_views = args.iter().map(|arg| arg.to_str()).collect::<Vec<_>>();
    prodex_mojo_core::launch::super_literal_boundary(&argument_views)
        .expect("Mojo Super literal-boundary policy returned invalid output")
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
    let argument_views = args.iter().map(|arg| arg.to_str()).collect::<Vec<_>>();
    let provider = prodex_mojo_core::launch::super_compat_provider_alias(&argument_views)
        .expect("Mojo Super compatibility-alias policy returned invalid output");
    let Some(_provider) = provider else {
        return args.to_vec();
    };

    let mut rewritten = Vec::with_capacity(args.len() + 1);
    rewritten.push(
        args.first()
            .cloned()
            .unwrap_or_else(|| OsString::from("prodex")),
    );

    rewritten.push(args[1].clone());
    rewritten.push(OsString::from("--provider"));
    rewritten.extend(args.iter().skip(2).cloned());

    rewritten
}

pub fn should_default_cli_invocation_to_run(args: &[OsString]) -> bool {
    let argument_views = [
        args.first().and_then(|arg| arg.to_str()),
        args.get(1).and_then(|arg| arg.to_str()),
    ];
    prodex_mojo_core::launch::default_cli_invocation_to_run(&argument_views)
        .expect("Mojo CLI default invocation policy returned invalid output")
}

pub fn is_codex_command_server_subcommand(args: &[OsString]) -> bool {
    prodex_mojo_core::launch::codex_command_server_subcommand(
        args.first().and_then(|arg| arg.to_str()),
    )
    .expect("Mojo Codex command-server policy returned invalid output")
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
