use clap::{Args, Subcommand};
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub enum ProfileCommands {
    /// Add a profile entry and optionally seed it from another CODEX_HOME.
    Add(AddProfileArgs),
    /// Export one or more profiles, including supported profile secrets.
    Export(ExportProfileArgs),
    /// Import profiles from an export bundle or supported built-in source.
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
    /// Bypass private-file permission and ACL validation for this operation.
    ///
    /// This is unsafe; use it only when the profile files are otherwise trusted.
    #[arg(long)]
    pub insecure: bool,
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
    ///
    /// In non-interactive use, set PRODEX_PROFILE_EXPORT_PASSWORD.
    #[arg(long, conflicts_with = "no_password")]
    pub password_protect: bool,
    /// Explicitly export without password protection and skip the interactive prompt.
    #[arg(long)]
    pub no_password: bool,
}

#[derive(Args, Debug)]
pub struct ImportProfileArgs {
    /// Path to a profile export bundle created by `prodex profile export`, or a built-in source such as `claude`, `copilot`, or `kiro`.
    #[arg(value_name = "PATH_OR_SOURCE")]
    pub path: PathBuf,
    /// Override the imported profile name when using a built-in source such as `claude`, `copilot`, or `kiro`.
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,
    /// Activate the imported profile immediately when using a built-in source such as `claude`, `copilot`, or `kiro`.
    #[arg(long)]
    pub activate: bool,
    /// Bypass private-file permission and ACL validation for this operation.
    ///
    /// This is unsafe; use it only when the profile files are otherwise trusted.
    #[arg(long)]
    pub insecure: bool,
}

#[derive(Args, Debug)]
pub struct ImportCurrentArgs {
    /// Name of the managed profile to create from the current shared Codex home.
    #[arg(default_value = "default")]
    pub name: String,
    /// Bypass private-file permission and ACL validation for this operation.
    ///
    /// This is unsafe; use it only when the profile files are otherwise trusted.
    #[arg(long)]
    pub insecure: bool,
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

#[derive(Args)]
pub struct CodexPassthroughArgs {
    /// Existing profile to log into. If omitted, prodex creates or reuses a profile by workspace identity.
    #[arg(short, long, value_name = "NAME")]
    pub profile: Option<String>,
    /// Optional profile name first, followed by login-method flags or provider arguments.
    #[arg(value_name = "PROFILE_OR_LOGIN_ARG", allow_hyphen_values = true)]
    pub codex_args: Vec<OsString>,
}

impl CodexPassthroughArgs {
    pub fn selected_profile(&self) -> Option<&str> {
        self.profile.as_deref().or_else(|| {
            self.codex_args
                .first()
                .and_then(|arg| arg.to_str())
                .filter(|arg| !arg.starts_with('-') && *arg != "status")
        })
    }

    pub fn into_selected_profile(mut self) -> (Option<String>, Vec<OsString>) {
        let positional_profile = if self.profile.is_none() {
            self.selected_profile().map(str::to_owned)
        } else {
            None
        };
        if positional_profile.is_some() {
            self.codex_args.remove(0);
        }
        (self.profile.or(positional_profile), self.codex_args)
    }
}

impl fmt::Debug for CodexPassthroughArgs {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexPassthroughArgs")
            .field("profile_configured", &self.selected_profile().is_some())
            .field("codex_args_count", &self.codex_args.len())
            .finish()
    }
}

#[derive(Args, Debug, Default)]
pub struct ProdexUpdateArgs {}

#[derive(Args)]
pub struct QuotaArgs {
    /// Inspect a single profile instead of the default detailed pool view.
    #[arg(short, long, value_name = "NAME", conflicts_with = "all")]
    pub profile: Option<String>,
    /// Show every configured profile in a compact aggregated view.
    #[arg(long, conflicts_with = "profile")]
    pub all: bool,
    /// Show only profiles whose auth label or compatibility matches this filter.
    ///
    /// Supported values: no-auth, chatgpt, api-key, invalid-auth, unreadable-auth,
    /// quota-compatible, non-quota-compatible, all.
    #[arg(
        long,
        value_name = "AUTH",
        conflicts_with_all = ["profile", "raw"]
    )]
    pub auth: Option<String>,
    /// Show only profiles for one provider in aggregated views.
    ///
    /// Supported values: all, openai, gemini, anthropic, claude, copilot, kiro, deepseek, local, agy.
    #[arg(
        long,
        value_name = "PROVIDER",
        conflicts_with_all = ["profile", "raw"]
    )]
    pub provider: Option<String>,
    /// Include exact reset timestamps and expanded window details.
    #[arg(long, conflicts_with = "raw")]
    pub detail: bool,
    /// Print raw usage JSON for a single profile and disable the live refresh view.
    #[arg(
        long,
        conflicts_with_all = ["all", "detail", "watch", "once", "auth", "provider"]
    )]
    pub raw: bool,
    #[arg(long, hide = true, conflicts_with_all = ["raw", "once"])]
    pub watch: bool,
    /// Render one human-readable snapshot instead of refreshing every 5 seconds.
    #[arg(long, conflicts_with_all = ["watch", "raw"])]
    pub once: bool,
    /// Override the ChatGPT backend base URL used for quota requests.
    #[arg(long, value_name = "URL")]
    pub base_url: Option<String>,
}

impl fmt::Debug for QuotaArgs {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuotaArgs")
            .field("profile_configured", &self.profile.is_some())
            .field("all", &self.all)
            .field("auth", &self.auth)
            .field("provider", &self.provider)
            .field("detail", &self.detail)
            .field("raw", &self.raw)
            .field("watch", &self.watch)
            .field("once", &self.once)
            .field("base_url_configured", &self.base_url.is_some())
            .finish()
    }
}
