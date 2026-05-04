use clap::{Args, Parser, Subcommand, ValueEnum};
use std::ffi::OsString;
use std::path::PathBuf;

mod help;

pub use help::RUNTIME_PROXY_DOCTOR_TAIL_BYTES;
use help::*;

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
    Cleanup,
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
        visible_alias = "s",
        about = "Alias for `prodex caveman mem --full-access`.",
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

#[derive(Subcommand, Debug)]
pub enum ProfileCommands {
    /// Add a profile entry and optionally seed it from another CODEX_HOME.
    Add(AddProfileArgs),
    /// Export one or more profiles, including their auth.json access tokens.
    Export(ExportProfileArgs),
    /// Import profiles from a bundle created by `prodex profile export`.
    Import(ImportProfileArgs),
    /// Copy the current shared Codex home into a new managed profile and activate it.
    ImportCurrent(ImportCurrentArgs),
    /// List configured profiles and show which one is active.
    List,
    /// Remove one profile entry or every profile entry and optionally delete managed homes.
    Remove(RemoveProfileArgs),
    /// Set the active profile used by commands that omit --profile.
    Use(ProfileSelector),
}

#[derive(Args, Debug)]
pub struct AddProfileArgs {
    /// Name of the profile to create.
    pub name: String,
    /// Register an existing CODEX_HOME path instead of creating a managed profile home.
    #[arg(long, value_name = "PATH")]
    pub codex_home: Option<PathBuf>,
    /// Copy initial state from another CODEX_HOME path into the new managed profile.
    #[arg(long, value_name = "PATH")]
    pub copy_from: Option<PathBuf>,
    /// Seed the new managed profile from the default shared Codex home.
    #[arg(long)]
    pub copy_current: bool,
    /// Make the new profile active after creation.
    #[arg(long)]
    pub activate: bool,
}

#[derive(Args, Debug)]
pub struct ExportProfileArgs {
    /// Export only the named profile. Repeat to export multiple profiles. Defaults to all profiles.
    #[arg(short, long, value_name = "NAME")]
    pub profile: Vec<String>,
    /// Write the export bundle to this path. Defaults to a timestamped JSON file in the current directory.
    #[arg(value_name = "PATH")]
    pub output: Option<PathBuf>,
    /// Protect the export bundle with a password.
    #[arg(long, conflicts_with = "no_password")]
    pub password_protect: bool,
    /// Export without password protection and skip the interactive prompt.
    #[arg(long)]
    pub no_password: bool,
}

#[derive(Args, Debug)]
pub struct ImportProfileArgs {
    /// Path to a profile export bundle created by `prodex profile export`, or the built-in source `copilot`.
    #[arg(value_name = "PATH_OR_SOURCE")]
    pub path: PathBuf,
    /// Override the imported profile name when using a built-in source such as `copilot`.
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,
    /// Activate the imported profile immediately when using a built-in source such as `copilot`.
    #[arg(long)]
    pub activate: bool,
}

#[derive(Args, Debug)]
pub struct ImportCurrentArgs {
    /// Name of the managed profile to create from the current shared Codex home.
    #[arg(default_value = "default")]
    pub name: String,
}

#[derive(Args, Debug)]
pub struct RemoveProfileArgs {
    /// Name of the profile to remove.
    #[arg(
        value_name = "NAME",
        required_unless_present = "all",
        conflicts_with = "all"
    )]
    pub name: Option<String>,
    /// Remove every configured profile.
    #[arg(long, conflicts_with = "name")]
    pub all: bool,
    /// Also delete the managed CODEX_HOME directory from disk.
    #[arg(long)]
    pub delete_home: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ProfileSelector {
    /// Profile name. If omitted, prodex uses the active profile.
    #[arg(short, long, value_name = "NAME")]
    pub profile: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct LogoutArgs {
    /// Profile name. If omitted, prodex uses the active profile.
    #[arg(value_name = "NAME", conflicts_with = "profile")]
    pub profile_name: Option<String>,
    /// Profile name. If omitted, prodex uses the active profile.
    #[arg(short, long, value_name = "NAME")]
    pub profile: Option<String>,
}

impl LogoutArgs {
    pub fn selected_profile(&self) -> Option<&str> {
        self.profile.as_deref().or(self.profile_name.as_deref())
    }
}

#[derive(Args, Debug)]
pub struct CodexPassthroughArgs {
    /// Existing profile to log into. If omitted, prodex creates or reuses a profile by workspace identity.
    #[arg(short, long, value_name = "NAME")]
    pub profile: Option<String>,
    /// Extra arguments passed through to `codex login` unchanged.
    #[arg(value_name = "CODEX_ARG", allow_hyphen_values = true)]
    pub codex_args: Vec<OsString>,
}

#[derive(Args, Debug)]
pub struct CodexUpdateArgs {
    /// Extra arguments passed through to `codex update` unchanged.
    #[arg(value_name = "CODEX_ARG", allow_hyphen_values = true)]
    pub codex_args: Vec<OsString>,
}

#[derive(Args, Debug)]
pub struct QuotaArgs {
    /// Inspect a single profile. If omitted, prodex uses the active profile.
    #[arg(short, long, value_name = "NAME")]
    pub profile: Option<String>,
    /// Show every configured profile in one aggregated view.
    #[arg(long)]
    pub all: bool,
    /// Show only profiles whose auth label or compatibility matches this filter.
    ///
    /// Supported values: no-auth, chatgpt, api-key, invalid-auth, unreadable-auth,
    /// quota-compatible, non-quota-compatible, all.
    #[arg(long, value_name = "AUTH", requires = "all")]
    pub auth: Option<String>,
    /// Include exact reset timestamps and expanded window details.
    #[arg(long)]
    pub detail: bool,
    /// Print raw usage JSON for a single profile and disable the live refresh view.
    #[arg(long)]
    pub raw: bool,
    #[arg(long, hide = true)]
    pub watch: bool,
    /// Render one human-readable snapshot instead of refreshing every 5 seconds.
    #[arg(long, conflicts_with = "watch")]
    pub once: bool,
    /// Override the ChatGPT backend base URL used for quota requests.
    #[arg(long, value_name = "URL")]
    pub base_url: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum SessionCommands {
    /// List all shared Codex sessions.
    List(SessionListArgs),
    /// List shared Codex sessions started from the current directory.
    Current(SessionCurrentArgs),
}

#[derive(Args, Debug)]
pub struct SessionListArgs {
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,
    /// Limit the number of sessions shown after sorting newest first.
    #[arg(long, value_name = "N")]
    pub limit: Option<usize>,
}

#[derive(Args, Debug)]
pub struct SessionCurrentArgs {
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,
    /// Limit the number of sessions shown after sorting newest first.
    #[arg(long, value_name = "N")]
    pub limit: Option<usize>,
    /// Directory used for matching sessions. Defaults to the current working directory.
    #[arg(long, value_name = "PATH", hide = true)]
    pub cwd: Option<PathBuf>,
}

#[derive(Args, Debug, Default)]
pub struct InfoArgs {
    /// Include token usage totals parsed from recent runtime logs.
    #[arg(long)]
    pub tokens: bool,
}

#[derive(Args, Debug)]
pub struct DoctorArgs {
    /// Also probe each profile's quota endpoint.
    #[arg(long)]
    pub quota: bool,
    /// Also summarize runtime proxy state and recent logs from the configured log directory.
    #[arg(long)]
    pub runtime: bool,
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

#[derive(Args, Debug)]
pub struct RunArgs {
    /// Starting profile for the run. If omitted, prodex uses the active profile.
    #[arg(short, long, value_name = "NAME")]
    pub profile: Option<String>,
    /// Explicitly enable auto-rotate. This is the default behavior.
    #[arg(long, conflicts_with = "no_auto_rotate")]
    pub auto_rotate: bool,
    /// Keep the selected profile fixed and fail instead of rotating.
    #[arg(long)]
    pub no_auto_rotate: bool,
    /// Skip the preflight quota gate before launching codex.
    #[arg(long)]
    pub skip_quota_check: bool,
    /// Start Codex with launch-time full access by passing Codex's sandbox-bypass launch flag.
    #[arg(long)]
    pub full_access: bool,
    /// Override the upstream ChatGPT base URL used for quota preflight and the runtime proxy.
    #[arg(long, value_name = "URL")]
    pub base_url: Option<String>,
    /// Disable system and environment proxy settings for upstream OpenAI/quota HTTP requests.
    #[arg(long)]
    pub no_proxy: bool,
    /// Print resolved launch diagnostics without starting Codex.
    #[arg(long)]
    pub dry_run: bool,
    /// Arguments passed through to `codex`. A lone session id is normalized to `codex resume <session-id>`.
    #[arg(value_name = "CODEX_ARG", allow_hyphen_values = true)]
    pub codex_args: Vec<OsString>,
}

#[derive(Args, Debug)]
pub struct ClaudeArgs {
    /// Starting profile for the run. If omitted, prodex uses the active profile.
    #[arg(short, long, value_name = "NAME")]
    pub profile: Option<String>,
    /// Explicitly enable auto-rotate. This is the default behavior.
    #[arg(long, conflicts_with = "no_auto_rotate")]
    pub auto_rotate: bool,
    /// Keep the selected profile fixed and fail instead of rotating.
    #[arg(long)]
    pub no_auto_rotate: bool,
    /// Skip the preflight quota gate before launching Claude Code.
    #[arg(long)]
    pub skip_quota_check: bool,
    /// Override the upstream ChatGPT base URL used for quota preflight and the runtime proxy.
    #[arg(long, value_name = "URL")]
    pub base_url: Option<String>,
    /// Disable system and environment proxy settings for upstream OpenAI/quota HTTP requests.
    #[arg(long)]
    pub no_proxy: bool,
    /// Arguments passed through to `claude` unchanged.
    #[arg(value_name = "CLAUDE_ARG", allow_hyphen_values = true)]
    pub claude_args: Vec<OsString>,
}

#[derive(Args, Debug)]
pub struct CavemanArgs {
    /// Starting profile for the run. If omitted, prodex uses the active profile.
    #[arg(short, long, value_name = "NAME")]
    pub profile: Option<String>,
    /// Explicitly enable auto-rotate. This is the default behavior.
    #[arg(long, conflicts_with = "no_auto_rotate")]
    pub auto_rotate: bool,
    /// Keep the selected profile fixed and fail instead of rotating.
    #[arg(long)]
    pub no_auto_rotate: bool,
    /// Skip the preflight quota gate before launching codex.
    #[arg(long)]
    pub skip_quota_check: bool,
    /// Start Codex with launch-time full access by passing Codex's sandbox-bypass launch flag.
    #[arg(long)]
    pub full_access: bool,
    /// Print resolved launch diagnostics without starting Codex.
    #[arg(long)]
    pub dry_run: bool,
    /// Override the upstream ChatGPT base URL used for quota preflight and the runtime proxy.
    #[arg(long, value_name = "URL")]
    pub base_url: Option<String>,
    /// Disable system and environment proxy settings for upstream OpenAI/quota HTTP requests.
    #[arg(long)]
    pub no_proxy: bool,
    /// Enable Prodex Smart Context Autopilot in the runtime proxy.
    #[arg(skip)]
    pub smart_context: bool,
    /// Arguments passed through to `codex`. A lone session id is normalized to `codex resume <session-id>`.
    #[arg(value_name = "CODEX_ARG", allow_hyphen_values = true)]
    pub codex_args: Vec<OsString>,
}

#[derive(Args, Debug)]
pub struct SuperArgs {
    /// Starting profile for the run. If omitted, prodex uses the active profile.
    #[arg(short, long, value_name = "NAME")]
    pub profile: Option<String>,
    /// Explicitly enable auto-rotate. This is the default behavior.
    #[arg(long, conflicts_with = "no_auto_rotate")]
    pub auto_rotate: bool,
    /// Keep the selected profile fixed and fail instead of rotating.
    #[arg(long)]
    pub no_auto_rotate: bool,
    /// Skip the preflight quota gate before launching codex.
    #[arg(long)]
    pub skip_quota_check: bool,
    /// Print resolved launch diagnostics without starting Codex.
    #[arg(long)]
    pub dry_run: bool,
    /// Override the upstream ChatGPT base URL used for quota preflight and the runtime proxy.
    #[arg(long, value_name = "URL", conflicts_with = "url")]
    pub base_url: Option<String>,
    /// Disable system and environment proxy settings for upstream OpenAI/quota HTTP requests.
    #[arg(long)]
    pub no_proxy: bool,
    /// Route Codex directly to a local OpenAI-compatible /v1 endpoint.
    #[arg(long, value_name = "URL", value_parser = parse_super_local_url)]
    pub url: Option<String>,
    /// Model id to use with --url.
    #[arg(
        long = "model",
        visible_alias = "local-model",
        value_name = "MODEL",
        requires = "url"
    )]
    pub local_model: Option<String>,
    /// Context window advertised to Codex when using --url.
    #[arg(
        long = "context-window",
        visible_alias = "local-context-window",
        value_name = "TOKENS",
        requires = "url"
    )]
    pub local_context_window: Option<usize>,
    /// Auto-compact threshold advertised to Codex when using --url.
    #[arg(
        long = "auto-compact-token-limit",
        visible_alias = "local-auto-compact-token-limit",
        value_name = "TOKENS",
        requires = "url"
    )]
    pub local_auto_compact_token_limit: Option<usize>,
    /// Use the full Claude-Mem Codex transcript schema instead of Prodex's slim default.
    #[arg(long, conflicts_with = "mem_super_slim")]
    pub mem_full: bool,
    /// Use Prodex's super-slim mem schema that stores prompt summaries/references instead of full prompts.
    #[arg(long, conflicts_with = "mem_full")]
    pub mem_super_slim: bool,
    /// Arguments passed through to `codex` after the implied `mem` prefix.
    #[arg(value_name = "CODEX_ARG", allow_hyphen_values = true)]
    pub codex_args: Vec<OsString>,
}

impl SuperArgs {
    pub fn into_caveman_args(self) -> CavemanArgs {
        let local_upstream_base_url = self.url.as_deref().map(super_local_provider_base_url);
        let local_provider_args = self
            .url
            .as_deref()
            .map(|url| {
                super_local_provider_codex_args(
                    url,
                    self.local_model.as_deref(),
                    self.local_context_window,
                    self.local_auto_compact_token_limit,
                )
            })
            .unwrap_or_default();
        let local_mode = self.url.is_some();
        let skip_quota_check = self.skip_quota_check || local_mode;

        let mut codex_args =
            Vec::with_capacity(self.codex_args.len() + 1 + local_provider_args.len());
        codex_args.push(OsString::from(if self.mem_super_slim {
            "mem-super-slim"
        } else if self.mem_full {
            "mem-full"
        } else {
            "mem"
        }));
        codex_args.extend(local_provider_args);
        codex_args.extend(self.codex_args);
        CavemanArgs {
            profile: self.profile,
            auto_rotate: self.auto_rotate,
            no_auto_rotate: self.no_auto_rotate,
            skip_quota_check,
            full_access: true,
            dry_run: self.dry_run,
            base_url: self.base_url.or(local_upstream_base_url),
            no_proxy: self.no_proxy,
            smart_context: true,
            codex_args,
        }
    }
}

pub const SUPER_LOCAL_PROVIDER_ID: &str = "prodex-local";
const SUPER_LOCAL_PROVIDER_NAME: &str = "Prodex Local";
const SUPER_DEFAULT_LOCAL_MODEL: &str = "unsloth/qwen3.5-35b-a3b";
const SUPER_DEFAULT_CONTEXT_WINDOW: usize = 16_384;
const SUPER_DEFAULT_AUTO_COMPACT_LIMIT: usize = 14_000;

fn super_local_provider_codex_args(
    url: &str,
    model: Option<&str>,
    context_window: Option<usize>,
    auto_compact_token_limit: Option<usize>,
) -> Vec<OsString> {
    let base_url = super_local_provider_base_url(url);
    let model = model
        .filter(|model| !model.trim().is_empty())
        .unwrap_or(SUPER_DEFAULT_LOCAL_MODEL);
    let context_window = context_window
        .filter(|value| *value > 1)
        .unwrap_or(SUPER_DEFAULT_CONTEXT_WINDOW);
    let auto_compact_token_limit = auto_compact_token_limit
        .filter(|value| *value > 0)
        .unwrap_or(SUPER_DEFAULT_AUTO_COMPACT_LIMIT)
        .min(context_window.saturating_sub(1));
    let overrides = [
        format!(
            "model_provider={}",
            toml_string_literal(SUPER_LOCAL_PROVIDER_ID)
        ),
        format!("model={}", toml_string_literal(model)),
        format!(
            "model_providers.{SUPER_LOCAL_PROVIDER_ID}.name={}",
            toml_string_literal(SUPER_LOCAL_PROVIDER_NAME)
        ),
        format!(
            "model_providers.{SUPER_LOCAL_PROVIDER_ID}.base_url={}",
            toml_string_literal(&base_url)
        ),
        format!("model_providers.{SUPER_LOCAL_PROVIDER_ID}.wire_api=\"responses\""),
        format!("model_providers.{SUPER_LOCAL_PROVIDER_ID}.supports_websockets=false"),
        format!("model_context_window={context_window}"),
        format!("model_auto_compact_token_limit={auto_compact_token_limit}"),
        "model_reasoning_summary=\"none\"".to_string(),
        "model_supports_reasoning_summaries=false".to_string(),
        "web_search=\"disabled\"".to_string(),
        "features.js_repl=false".to_string(),
        "features.image_generation=false".to_string(),
    ];

    let mut args = Vec::with_capacity(overrides.len() * 2);
    for override_entry in overrides {
        args.push(OsString::from("-c"));
        args.push(OsString::from(override_entry));
    }
    args
}

fn super_local_provider_base_url(url: &str) -> String {
    let trimmed = url.trim();
    if let Ok(mut parsed) = reqwest::Url::parse(trimmed) {
        let path = parsed.path().trim_end_matches('/');
        if path.is_empty() || path == "/" {
            parsed.set_path("/v1");
            return parsed.as_str().trim_end_matches('/').to_string();
        }
    }
    trimmed.trim_end_matches('/').to_string()
}

fn parse_super_local_url(url: &str) -> std::result::Result<String, String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("invalid --url: value cannot be empty".to_string());
    }
    if trimmed.starts_with("http:///") || trimmed.starts_with("https:///") {
        return Err("invalid --url: expected a URL host".to_string());
    }
    let parsed = reqwest::Url::parse(trimmed).map_err(|err| {
        format!(
            "invalid --url: expected an absolute http(s) URL such as http://127.0.0.1:11434 ({err})"
        )
    })?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(format!(
            "invalid --url: expected http or https scheme, got {}",
            parsed.scheme()
        ));
    }
    if parsed.host_str().is_none() {
        return Err("invalid --url: expected a URL host".to_string());
    }
    Ok(trimmed.to_string())
}

fn toml_string_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[derive(Args, Debug)]
pub struct RuntimeBrokerArgs {
    #[arg(long)]
    pub current_profile: String,
    #[arg(long)]
    pub upstream_base_url: String,
    #[arg(long, default_value_t = false)]
    pub include_code_review: bool,
    #[arg(long = "upstream-no-proxy", default_value_t = false)]
    pub upstream_no_proxy: bool,
    #[arg(long = "smart-context", default_value_t = false)]
    pub smart_context_enabled: bool,
    #[arg(long = "model-context-window-tokens", hide = true)]
    pub model_context_window_tokens: Option<u64>,
    #[arg(long)]
    pub broker_key: String,
    #[arg(long)]
    pub instance_token: String,
    #[arg(long)]
    pub admin_token: String,
    #[arg(long)]
    pub listen_addr: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CurrentCommand;

#[derive(Debug, Clone, Copy, Default)]
pub struct CleanupCommand;

#[derive(Debug, Clone, Copy, Default)]
pub struct ListProfilesCommand;

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
            | "login"
            | "logout"
            | "update"
            | "quota"
            | "run"
            | "caveman"
            | "super"
            | "s"
            | "claude"
            | "help"
            | "__runtime-broker"
    )
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
