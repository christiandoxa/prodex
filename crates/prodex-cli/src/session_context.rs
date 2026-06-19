use clap::{Args, Subcommand, ValueEnum};
use std::path::PathBuf;

use super::RUNTIME_PROXY_DOCTOR_TAIL_BYTES;

#[derive(Subcommand, Debug)]
pub enum SessionCommands {
    /// List all shared Codex sessions.
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
    /// Include spawned subagent sessions. Default output shows only resumable parent sessions.
    #[arg(long)]
    pub include_subagents: bool,
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
    /// Include spawned subagent sessions. Default output shows only resumable parent sessions.
    #[arg(long)]
    pub include_subagents: bool,
}

#[derive(Args, Debug)]
pub struct SessionResumeArgs {
    /// Unique full or partial shared Codex session id.
    #[arg(value_name = "ID")]
    pub id: String,
}

#[derive(Args, Debug, Default)]
pub struct InfoArgs {
    /// Include token usage totals parsed from recent runtime logs.
    #[arg(long)]
    pub tokens: bool,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum, PartialEq, Eq)]
pub enum LogMode {
    /// Show the most recent token usage event and exit.
    #[default]
    Last,
    /// Follow runtime logs and print token usage events as they arrive.
    Stream,
}

#[derive(Args, Debug, Default)]
pub struct LogArgs {
    /// Output mode. Omit for the latest event, or use `stream` for live updates.
    #[arg(value_enum, default_value_t)]
    pub mode: LogMode,
    /// Emit JSON. Stream mode emits one JSON object per line.
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
    /// Bytes of runtime log tail to inspect for --runtime/--json.
    #[arg(long, default_value_t = RUNTIME_PROXY_DOCTOR_TAIL_BYTES, value_name = "BYTES")]
    pub tail_bytes: usize,
    /// Suggest policy.toml tuning snippets from recent runtime markers.
    #[arg(long, requires = "runtime")]
    pub suggest_policy: bool,
    /// Emit machine-readable JSON output. Supported together with --runtime.
    #[arg(long)]
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

#[derive(Args, Debug)]
pub struct SetupArgs {
    /// Preview planned setup/repair actions without writing files.
    #[arg(long)]
    pub dry_run: bool,
    /// Verify embedded Caveman/Super asset manifests and guidance.
    #[arg(long)]
    pub verify_assets: bool,
    /// Emit machine-readable JSON output.
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand, Debug)]
pub enum CapabilityCommands {
    /// List optional Prodex capabilities and local availability.
    List(CapabilityListArgs),
    /// Diagnose the local optimizer stack used by `prodex s` / `prodex super`.
    #[command(name = "super-doctor", visible_alias = "s-doctor")]
    SuperDoctor(SuperDoctorArgs),
}

#[derive(Args, Debug)]
pub struct CapabilityListArgs {
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct SuperDoctorArgs {
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,
    /// Exit non-zero when any Super optimizer check is not ready.
    #[arg(long)]
    pub strict: bool,
    /// Include local Presidio Analyzer/Anonymizer health checks.
    #[arg(long)]
    pub presidio: bool,
}

#[derive(Args, Debug)]
pub struct AuditArgs {
    /// Show only the most recent matching events.
    #[arg(long, default_value_t = 50, value_name = "COUNT")]
    pub tail: usize,
    /// Emit machine-readable JSON output.
    #[arg(long)]
    pub json: bool,
    /// Filter by component, for example `profile` or `runtime`.
    #[arg(long, value_name = "NAME")]
    pub component: Option<String>,
    /// Filter by action, for example `use` or `broker_start`.
    #[arg(long, value_name = "NAME")]
    pub action: Option<String>,
    /// Filter by outcome, for example `success` or `failure`.
    #[arg(long, value_name = "NAME")]
    pub outcome: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum ContextCommands {
    /// Read-only size and approximate token audit for shared Codex context roots.
    Audit(ContextAuditArgs),
    /// Deterministically compact prose context files and write .original.md backups.
    Compress(ContextCompressArgs),
    /// Compact copied command output from stdin or a file for low-token context sharing.
    #[command(name = "compact-output")]
    CompactOutput(ContextCompactOutputArgs),
}

#[derive(Args, Debug)]
pub struct ContextAuditArgs {
    /// Shared Codex root to inspect. Defaults to the resolved shared CODEX_HOME.
    #[arg(long, value_name = "PATH")]
    pub root: Option<PathBuf>,
    /// Show this many largest files in the human table. Use 0 for all.
    #[arg(long, default_value_t = 20, value_name = "COUNT")]
    pub limit: usize,
    /// Emit machine-readable JSON output.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct ContextCompressArgs {
    /// Markdown/text file or directory to compact.
    #[arg(value_name = "PATH")]
    pub path: PathBuf,
    /// Show savings without writing the file or backup.
    #[arg(long)]
    pub dry_run: bool,
    /// Emit machine-readable JSON output.
    #[arg(long)]
    pub json: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ContextCompactOutputKind {
    Auto,
    GitStatus,
    GitDiff,
    Search,
    FileList,
    Plain,
}

#[derive(Args, Debug)]
pub struct ContextCompactOutputArgs {
    /// Text file to compact. Omit to read command output from stdin.
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,
    /// Output kind to use. `auto` detects common git/search/file-list output.
    #[arg(long, value_enum, default_value = "auto")]
    pub kind: ContextCompactOutputKind,
    /// Maximum lines in the compacted output.
    #[arg(long, default_value_t = 160, value_name = "COUNT")]
    pub max_lines: usize,
    /// Lines to keep from the beginning for plain truncation.
    #[arg(long, default_value_t = 80, value_name = "COUNT")]
    pub head_lines: usize,
    /// Lines to keep from the end for plain truncation.
    #[arg(long, default_value_t = 40, value_name = "COUNT")]
    pub tail_lines: usize,
    /// Maximum characters per retained line.
    #[arg(long, default_value_t = 240, value_name = "CHARS")]
    pub max_line_chars: usize,
    /// Maximum search matches to keep per file.
    #[arg(long, default_value_t = 4, value_name = "COUNT")]
    pub max_search_matches_per_file: usize,
    /// Maximum paths to keep in file-list summaries.
    #[arg(long, default_value_t = 120, value_name = "COUNT")]
    pub max_path_entries: usize,
    /// Emit machine-readable JSON output including the compacted text.
    #[arg(long)]
    pub json: bool,
}
