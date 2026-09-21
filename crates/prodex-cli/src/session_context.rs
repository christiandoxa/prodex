use clap::{Args, Subcommand, ValueEnum};
use std::path::PathBuf;

use super::RUNTIME_PROXY_DOCTOR_TAIL_BYTES;

#[derive(Subcommand, Debug)]
pub enum SessionCommands {
    /// List shared Codex sessions.
    List(SessionListArgs),
    /// List shared Codex sessions started from the current directory.
    Current(SessionCurrentArgs),
    /// Resume a shared Codex session by unique partial or full id.
    Resume(SessionResumeArgs),
}

#[derive(Args, Debug)]
pub struct SessionListArgs {
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,
    /// Print only full session ids, one per line.
    #[arg(long, conflicts_with_all = ["json", "resume_command"])]
    pub id_only: bool,
    /// Print a resume command for each matching session.
    #[arg(long, conflicts_with_all = ["json", "id_only"])]
    pub resume_command: bool,
    /// Show only sessions attached to this profile binding.
    #[arg(long, value_name = "NAME")]
    pub profile: Option<String>,
    /// Show only sessions whose id, thread name, cwd, profile, or path contains this text.
    #[arg(long, value_name = "TEXT")]
    pub query: Option<String>,
    /// Limit the number of sessions shown after sorting newest first.
    #[arg(long, value_name = "N")]
    pub limit: Option<usize>,
    /// Include spawned subagent sessions. This is the default; kept for compatibility.
    #[arg(long, conflicts_with = "parent_only")]
    pub include_subagents: bool,
    /// Show only resumable parent sessions.
    #[arg(long)]
    pub parent_only: bool,
}

#[derive(Args, Debug)]
pub struct SessionCurrentArgs {
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,
    /// Print only full session ids, one per line.
    #[arg(long, conflicts_with_all = ["json", "resume_command"])]
    pub id_only: bool,
    /// Print a resume command for each matching session.
    #[arg(long, conflicts_with_all = ["json", "id_only"])]
    pub resume_command: bool,
    /// Show only sessions attached to this profile binding.
    #[arg(long, value_name = "NAME")]
    pub profile: Option<String>,
    /// Show only sessions whose id, thread name, cwd, profile, or path contains this text.
    #[arg(long, value_name = "TEXT")]
    pub query: Option<String>,
    /// Limit the number of sessions shown after sorting newest first.
    #[arg(long, value_name = "N")]
    pub limit: Option<usize>,
    /// Directory used for matching sessions. Defaults to the current working directory.
    #[arg(long, value_name = "PATH", hide = true)]
    pub cwd: Option<PathBuf>,
    /// Include spawned subagent sessions. This is the default; kept for compatibility.
    #[arg(long, conflicts_with = "parent_only")]
    pub include_subagents: bool,
    /// Show only resumable parent sessions.
    #[arg(long)]
    pub parent_only: bool,
}

#[derive(Args, Debug)]
pub struct SessionResumeArgs {
    /// Unique full or partial shared Codex session id.
    #[arg(value_name = "ID")]
    pub id: String,
}

#[derive(Args, Debug, Default)]
pub struct InfoArgs {
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,
    /// Include token usage totals parsed from recent runtime logs.
    #[arg(long)]
    pub tokens: bool,
}

#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Render one snapshot instead of the live dashboard.
    #[arg(long)]
    pub once: bool,
    /// Resource sampling interval in seconds.
    #[arg(long, default_value_t = 1, value_name = "SECONDS", value_parser = clap::value_parser!(u64).range(1..=60))]
    pub interval: u64,
}

impl Default for StatusArgs {
    fn default() -> Self {
        Self {
            once: false,
            interval: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, ValueEnum, PartialEq, Eq)]
pub enum LogMode {
    /// Follow redacted runtime logs.
    #[default]
    Stream,
    /// Print the latest matching runtime-log line and exit.
    Last,
    /// Follow only upstream request/response log events.
    Upstream,
}

#[derive(Args, Debug, Default)]
pub struct LogArgs {
    /// Log view. Omit for the live stream.
    #[arg(value_enum, default_value_t)]
    pub mode: LogMode,
    /// Emit one JSON object per line.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct DoctorArgs {
    /// Also probe each profile's quota endpoint.
    #[arg(long)]
    pub quota: bool,
    /// Also summarize runtime proxy state and recent logs from the configured log directory.
    #[arg(long)]
    pub runtime: bool,
    /// Also check install/runtime prerequisites without mutating local state.
    #[arg(long)]
    pub install: bool,
    /// Recover orphaned profile import auth rollback journals before reporting.
    #[arg(long)]
    pub repair_import_auth_journals: bool,
    /// Explicitly run Codex's full active and archived thread-index repair.
    #[arg(long)]
    pub repair_session_index: bool,
    /// Bytes of runtime log tail to inspect for --runtime/--json.
    #[arg(long, default_value_t = RUNTIME_PROXY_DOCTOR_TAIL_BYTES, value_name = "BYTES")]
    pub tail_bytes: usize,
    /// Suggest policy.toml tuning snippets from recent runtime markers.
    #[arg(long, requires = "runtime")]
    pub suggest_policy: bool,
    /// Emit machine-readable JSON output for --runtime diagnostics.
    #[arg(long, requires = "runtime", conflicts_with = "bundle")]
    pub json: bool,
    /// Emit a redacted diagnostic bundle as JSON. Omit PATH or use '-' for stdout.
    #[arg(
        long,
        value_name = "PATH",
        num_args = 0..=1,
        default_missing_value = "-",
        requires = "redacted"
    )]
    pub bundle: Option<PathBuf>,
    /// Required for --bundle; secret values are never emitted.
    #[arg(long, requires = "bundle")]
    pub redacted: bool,
}
