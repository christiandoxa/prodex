use super::*;

#[derive(Parser, Debug)]
#[command(
    name = "prodex",
    version,
    about = "Manage multiple Codex profiles backed by isolated CODEX_HOME directories.",
    after_help = CLI_TOP_LEVEL_AFTER_HELP
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Commands {
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
pub(crate) enum ProfileCommands {
    /// Add a profile entry and optionally seed it from another CODEX_HOME.
    Add(AddProfileArgs),
    /// Export one or more profiles, including their auth.json access tokens.
    Export(ExportProfileArgs),
    /// Import profiles from a bundle created by `prodex profile export`.
    Import(ImportProfileArgs),
    /// Copy the current shared Prodex CODEX_HOME into a new managed profile and activate it.
    ImportCurrent(ImportCurrentArgs),
    /// List configured profiles and show which one is active.
    List,
    /// Remove one profile entry or every profile entry and optionally delete managed homes.
    Remove(RemoveProfileArgs),
    /// Set the active profile used by commands that omit --profile.
    Use(ProfileSelector),
}

#[derive(Args, Debug)]
pub(crate) struct AddProfileArgs {
    /// Name of the profile to create.
    pub(crate) name: String,
    /// Register an existing CODEX_HOME path instead of creating a managed profile home.
    #[arg(long, value_name = "PATH")]
    pub(crate) codex_home: Option<PathBuf>,
    /// Copy initial state from another CODEX_HOME path into the new managed profile.
    #[arg(long, value_name = "PATH")]
    pub(crate) copy_from: Option<PathBuf>,
    /// Seed the new managed profile from the default shared Prodex CODEX_HOME.
    #[arg(long)]
    pub(crate) copy_current: bool,
    /// Make the new profile active after creation.
    #[arg(long)]
    pub(crate) activate: bool,
}

#[derive(Args, Debug)]
pub(crate) struct ExportProfileArgs {
    /// Export only the named profile. Repeat to export multiple profiles. Defaults to all profiles.
    #[arg(short, long, value_name = "NAME")]
    pub(crate) profile: Vec<String>,
    /// Write the export bundle to this path. Defaults to a timestamped JSON file in the current directory.
    #[arg(value_name = "PATH")]
    pub(crate) output: Option<PathBuf>,
    /// Protect the export bundle with a password.
    #[arg(long, conflicts_with = "no_password")]
    pub(crate) password_protect: bool,
    /// Export without password protection and skip the interactive prompt.
    #[arg(long)]
    pub(crate) no_password: bool,
}

#[derive(Args, Debug)]
pub(crate) struct ImportProfileArgs {
    /// Path to a profile export bundle created by `prodex profile export`, or the built-in source `copilot`.
    #[arg(value_name = "PATH_OR_SOURCE")]
    pub(crate) path: PathBuf,
    /// Override the imported profile name when using a built-in source such as `copilot`.
    #[arg(long, value_name = "NAME")]
    pub(crate) name: Option<String>,
    /// Activate the imported profile immediately when using a built-in source such as `copilot`.
    #[arg(long)]
    pub(crate) activate: bool,
}

#[derive(Args, Debug)]
pub(crate) struct ImportCurrentArgs {
    /// Name of the managed profile to create from the current shared Prodex CODEX_HOME.
    #[arg(default_value = "default")]
    pub(crate) name: String,
}

#[derive(Args, Debug)]
pub(crate) struct RemoveProfileArgs {
    /// Name of the profile to remove.
    #[arg(
        value_name = "NAME",
        required_unless_present = "all",
        conflicts_with = "all"
    )]
    pub(crate) name: Option<String>,
    /// Remove every configured profile.
    #[arg(long, conflicts_with = "name")]
    pub(crate) all: bool,
    /// Also delete the managed CODEX_HOME directory from disk.
    #[arg(long)]
    pub(crate) delete_home: bool,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct ProfileSelector {
    /// Profile name. If omitted, prodex uses the active profile.
    #[arg(short, long, value_name = "NAME")]
    pub(crate) profile: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct LogoutArgs {
    /// Profile name. If omitted, prodex uses the active profile.
    #[arg(value_name = "NAME", conflicts_with = "profile")]
    pub(crate) profile_name: Option<String>,
    /// Profile name. If omitted, prodex uses the active profile.
    #[arg(short, long, value_name = "NAME")]
    pub(crate) profile: Option<String>,
}

impl LogoutArgs {
    pub(crate) fn selected_profile(&self) -> Option<&str> {
        self.profile.as_deref().or(self.profile_name.as_deref())
    }
}

#[derive(Args, Debug)]
pub(crate) struct CodexPassthroughArgs {
    /// Existing profile to log into. If omitted, prodex creates or reuses a profile by account email.
    #[arg(short, long, value_name = "NAME")]
    pub(crate) profile: Option<String>,
    /// Extra arguments passed through to `codex login` unchanged.
    #[arg(value_name = "CODEX_ARG", allow_hyphen_values = true)]
    pub(crate) codex_args: Vec<OsString>,
}

#[derive(Args, Debug)]
pub(crate) struct QuotaArgs {
    /// Inspect a single profile. If omitted, prodex uses the active profile.
    #[arg(short, long, value_name = "NAME")]
    pub(crate) profile: Option<String>,
    /// Show every configured profile in one aggregated view.
    #[arg(long)]
    pub(crate) all: bool,
    /// Include exact reset timestamps and expanded window details.
    #[arg(long)]
    pub(crate) detail: bool,
    /// Print raw usage JSON for a single profile and disable the live refresh view.
    #[arg(long)]
    pub(crate) raw: bool,
    #[arg(long, hide = true)]
    pub(crate) watch: bool,
    /// Render one human-readable snapshot instead of refreshing every 5 seconds.
    #[arg(long, conflicts_with = "watch")]
    pub(crate) once: bool,
    /// Override the ChatGPT backend base URL used for quota requests.
    #[arg(long, value_name = "URL")]
    pub(crate) base_url: Option<String>,
}

#[derive(Args, Debug, Default)]
pub(crate) struct InfoArgs {}

#[derive(Args, Debug)]
pub(crate) struct DoctorArgs {
    /// Also probe each profile's quota endpoint.
    #[arg(long)]
    pub(crate) quota: bool,
    /// Also summarize runtime proxy state and recent logs from the configured log directory.
    #[arg(long)]
    pub(crate) runtime: bool,
    /// Recover orphaned profile import auth rollback journals before reporting.
    #[arg(long)]
    pub(crate) repair_import_auth_journals: bool,
    /// Bytes of runtime log tail to inspect for --runtime/--json.
    #[arg(long, default_value_t = RUNTIME_PROXY_DOCTOR_TAIL_BYTES, value_name = "BYTES")]
    pub(crate) tail_bytes: usize,
    /// Emit machine-readable JSON output. Supported together with --runtime.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Args, Debug)]
pub(crate) struct AuditArgs {
    /// Show only the most recent matching events.
    #[arg(long, default_value_t = 50, value_name = "COUNT")]
    pub(crate) tail: usize,
    /// Emit machine-readable JSON output.
    #[arg(long)]
    pub(crate) json: bool,
    /// Filter by component, for example `profile` or `runtime`.
    #[arg(long, value_name = "NAME")]
    pub(crate) component: Option<String>,
    /// Filter by action, for example `use` or `broker_start`.
    #[arg(long, value_name = "NAME")]
    pub(crate) action: Option<String>,
    /// Filter by outcome, for example `success` or `failure`.
    #[arg(long, value_name = "NAME")]
    pub(crate) outcome: Option<String>,
}

#[derive(Args, Debug)]
pub(crate) struct RunArgs {
    /// Starting profile for the run. If omitted, prodex uses the active profile.
    #[arg(short, long, value_name = "NAME")]
    pub(crate) profile: Option<String>,
    /// Explicitly enable auto-rotate. This is the default behavior.
    #[arg(long, conflicts_with = "no_auto_rotate")]
    pub(crate) auto_rotate: bool,
    /// Keep the selected profile fixed and fail instead of rotating.
    #[arg(long)]
    pub(crate) no_auto_rotate: bool,
    /// Skip the preflight quota gate before launching codex.
    #[arg(long)]
    pub(crate) skip_quota_check: bool,
    /// Start Codex with launch-time full access by passing Codex's sandbox-bypass launch flag.
    #[arg(long)]
    pub(crate) full_access: bool,
    /// Override the upstream ChatGPT base URL used for quota preflight and the runtime proxy.
    #[arg(long, value_name = "URL")]
    pub(crate) base_url: Option<String>,
    /// Print resolved launch diagnostics without starting Codex.
    #[arg(long)]
    pub(crate) dry_run: bool,
    /// Arguments passed through to `codex`. A lone session id is normalized to `codex resume <session-id>`.
    #[arg(value_name = "CODEX_ARG", allow_hyphen_values = true)]
    pub(crate) codex_args: Vec<OsString>,
}

#[derive(Args, Debug)]
pub(crate) struct ClaudeArgs {
    /// Starting profile for the run. If omitted, prodex uses the active profile.
    #[arg(short, long, value_name = "NAME")]
    pub(crate) profile: Option<String>,
    /// Explicitly enable auto-rotate. This is the default behavior.
    #[arg(long, conflicts_with = "no_auto_rotate")]
    pub(crate) auto_rotate: bool,
    /// Keep the selected profile fixed and fail instead of rotating.
    #[arg(long)]
    pub(crate) no_auto_rotate: bool,
    /// Skip the preflight quota gate before launching Claude Code.
    #[arg(long)]
    pub(crate) skip_quota_check: bool,
    /// Override the upstream ChatGPT base URL used for quota preflight and the runtime proxy.
    #[arg(long, value_name = "URL")]
    pub(crate) base_url: Option<String>,
    /// Arguments passed through to `claude` unchanged.
    #[arg(value_name = "CLAUDE_ARG", allow_hyphen_values = true)]
    pub(crate) claude_args: Vec<OsString>,
}

#[derive(Args, Debug)]
pub(crate) struct CavemanArgs {
    /// Starting profile for the run. If omitted, prodex uses the active profile.
    #[arg(short, long, value_name = "NAME")]
    pub(crate) profile: Option<String>,
    /// Explicitly enable auto-rotate. This is the default behavior.
    #[arg(long, conflicts_with = "no_auto_rotate")]
    pub(crate) auto_rotate: bool,
    /// Keep the selected profile fixed and fail instead of rotating.
    #[arg(long)]
    pub(crate) no_auto_rotate: bool,
    /// Skip the preflight quota gate before launching codex.
    #[arg(long)]
    pub(crate) skip_quota_check: bool,
    /// Start Codex with launch-time full access by passing Codex's sandbox-bypass launch flag.
    #[arg(long)]
    pub(crate) full_access: bool,
    /// Print resolved launch diagnostics without starting Codex.
    #[arg(long)]
    pub(crate) dry_run: bool,
    /// Override the upstream ChatGPT base URL used for quota preflight and the runtime proxy.
    #[arg(long, value_name = "URL")]
    pub(crate) base_url: Option<String>,
    /// Arguments passed through to `codex`. A lone session id is normalized to `codex resume <session-id>`.
    #[arg(value_name = "CODEX_ARG", allow_hyphen_values = true)]
    pub(crate) codex_args: Vec<OsString>,
}

#[derive(Args, Debug)]
pub(crate) struct SuperArgs {
    /// Starting profile for the run. If omitted, prodex uses the active profile.
    #[arg(short, long, value_name = "NAME")]
    pub(crate) profile: Option<String>,
    /// Explicitly enable auto-rotate. This is the default behavior.
    #[arg(long, conflicts_with = "no_auto_rotate")]
    pub(crate) auto_rotate: bool,
    /// Keep the selected profile fixed and fail instead of rotating.
    #[arg(long)]
    pub(crate) no_auto_rotate: bool,
    /// Skip the preflight quota gate before launching codex.
    #[arg(long)]
    pub(crate) skip_quota_check: bool,
    /// Print resolved launch diagnostics without starting Codex.
    #[arg(long)]
    pub(crate) dry_run: bool,
    /// Override the upstream ChatGPT base URL used for quota preflight and the runtime proxy.
    #[arg(long, value_name = "URL", conflicts_with = "url")]
    pub(crate) base_url: Option<String>,
    /// Route Codex directly to a local OpenAI-compatible /v1 endpoint.
    #[arg(long, value_name = "URL", value_parser = parse_super_local_url)]
    pub(crate) url: Option<String>,
    /// Model id to use with --url.
    #[arg(
        long = "model",
        visible_alias = "local-model",
        value_name = "MODEL",
        requires = "url"
    )]
    pub(crate) local_model: Option<String>,
    /// Context window advertised to Codex when using --url.
    #[arg(
        long = "context-window",
        visible_alias = "local-context-window",
        value_name = "TOKENS",
        requires = "url"
    )]
    pub(crate) local_context_window: Option<usize>,
    /// Auto-compact threshold advertised to Codex when using --url.
    #[arg(
        long = "auto-compact-token-limit",
        visible_alias = "local-auto-compact-token-limit",
        value_name = "TOKENS",
        requires = "url"
    )]
    pub(crate) local_auto_compact_token_limit: Option<usize>,
    /// Arguments passed through to `codex` after the implied `mem` prefix.
    #[arg(value_name = "CODEX_ARG", allow_hyphen_values = true)]
    pub(crate) codex_args: Vec<OsString>,
}

impl SuperArgs {
    pub(crate) fn into_caveman_args(self) -> CavemanArgs {
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
        codex_args.push(OsString::from("mem"));
        codex_args.extend(local_provider_args);
        codex_args.extend(self.codex_args);
        CavemanArgs {
            profile: self.profile,
            auto_rotate: self.auto_rotate,
            no_auto_rotate: self.no_auto_rotate,
            skip_quota_check,
            full_access: true,
            dry_run: self.dry_run,
            base_url: self.base_url,
            codex_args,
        }
    }
}

pub(crate) const SUPER_LOCAL_PROVIDER_ID: &str = "prodex-local";
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

#[derive(Args, Debug)]
pub(crate) struct RuntimeBrokerArgs {
    #[arg(long)]
    pub(crate) current_profile: String,
    #[arg(long)]
    pub(crate) upstream_base_url: String,
    #[arg(long, default_value_t = false)]
    pub(crate) include_code_review: bool,
    #[arg(long)]
    pub(crate) broker_key: String,
    #[arg(long)]
    pub(crate) instance_token: String,
    #[arg(long)]
    pub(crate) admin_token: String,
    #[arg(long)]
    pub(crate) listen_addr: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct CurrentCommand;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct CleanupCommand;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ListProfilesCommand;

impl ProfileSelector {
    pub(crate) fn execute(self) -> Result<()> {
        handle_set_active_profile(self)
    }
}

impl AddProfileArgs {
    pub(crate) fn execute(self) -> Result<()> {
        handle_add_profile(self)
    }
}

impl ExportProfileArgs {
    pub(crate) fn execute(self) -> Result<()> {
        handle_export_profiles(self)
    }
}

impl ImportProfileArgs {
    pub(crate) fn execute(self) -> Result<()> {
        handle_import_profiles(self)
    }
}

impl ImportCurrentArgs {
    pub(crate) fn execute(self) -> Result<()> {
        handle_import_current_profile(self)
    }
}

impl RemoveProfileArgs {
    pub(crate) fn execute(self) -> Result<()> {
        handle_remove_profile(self)
    }
}

impl CodexPassthroughArgs {
    pub(crate) fn execute(self) -> Result<()> {
        handle_codex_login(self)
    }
}

impl LogoutArgs {
    pub(crate) fn execute(self) -> Result<()> {
        handle_codex_logout(self)
    }
}

impl CurrentCommand {
    pub(crate) fn execute(self) -> Result<()> {
        handle_current_profile()
    }
}

impl ListProfilesCommand {
    pub(crate) fn execute(self) -> Result<()> {
        handle_list_profiles()
    }
}

impl InfoArgs {
    pub(crate) fn execute(self) -> Result<()> {
        handle_info(self)
    }
}

impl DoctorArgs {
    pub(crate) fn execute(self) -> Result<()> {
        handle_doctor(self)
    }
}

impl AuditArgs {
    pub(crate) fn execute(self) -> Result<()> {
        handle_audit(self)
    }
}

impl CleanupCommand {
    pub(crate) fn execute(self) -> Result<()> {
        handle_cleanup()
    }
}

impl QuotaArgs {
    pub(crate) fn execute(self) -> Result<()> {
        handle_quota(self)
    }
}

impl RunArgs {
    pub(crate) fn execute(self) -> Result<()> {
        handle_run(self)
    }
}

impl CavemanArgs {
    pub(crate) fn execute(self) -> Result<()> {
        if self.dry_run || prodex_dry_run_requested(&self.codex_args) {
            return handle_caveman_dry_run(self);
        }
        handle_caveman(self)
    }
}

impl SuperArgs {
    pub(crate) fn execute(self) -> Result<()> {
        if self.dry_run || prodex_dry_run_requested(&self.codex_args) {
            return handle_caveman_dry_run(self.into_caveman_args());
        }
        handle_super(self)
    }
}

impl ClaudeArgs {
    pub(crate) fn execute(self) -> Result<()> {
        handle_claude(self)
    }
}

impl RuntimeBrokerArgs {
    pub(crate) fn execute(self) -> Result<()> {
        handle_runtime_broker(self)
    }
}
