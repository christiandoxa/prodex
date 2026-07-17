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

#[derive(Args)]
pub struct CodexPassthroughArgs {
    /// Existing profile to log into. If omitted, prodex creates or reuses a profile by workspace identity.
    #[arg(short, long, value_name = "NAME")]
    pub profile: Option<String>,
    /// Login-method flags or extra arguments for the selected provider login flow.
    #[arg(value_name = "LOGIN_ARG", allow_hyphen_values = true)]
    pub codex_args: Vec<OsString>,
}

impl fmt::Debug for CodexPassthroughArgs {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexPassthroughArgs")
            .field("profile_configured", &self.profile.is_some())
            .field("codex_args_count", &self.codex_args.len())
            .finish()
    }
}

#[derive(Args, Debug, Default)]
pub struct ProdexUpdateArgs {}

#[derive(Args)]
pub struct QuotaArgs {
    /// Inspect a single profile. If omitted, prodex uses the active profile.
    #[arg(short, long, value_name = "NAME", conflicts_with = "all")]
    pub profile: Option<String>,
    /// Show every configured profile in one aggregated view.
    #[arg(long, conflicts_with = "profile")]
    pub all: bool,
    /// Show only profiles whose auth label or compatibility matches this filter.
    ///
    /// Supported values: no-auth, chatgpt, api-key, invalid-auth, unreadable-auth,
    /// quota-compatible, non-quota-compatible, all.
    #[arg(long, value_name = "AUTH", requires = "all")]
    pub auth: Option<String>,
    /// Show only profiles for one provider in --all views.
    ///
    /// Supported values: all, openai, gemini, anthropic, claude, copilot, kiro, deepseek, local, agy.
    #[arg(long, value_name = "PROVIDER", requires = "all")]
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

#[derive(Args)]
pub struct RedeemArgs {
    /// OpenAI/Codex profile whose reset credit should be redeemed.
    #[arg(value_name = "PROFILE")]
    pub profile: String,
    /// Skip the near-reset confirmation prompt.
    #[arg(short = 'y', long)]
    pub yes: bool,
    /// Override the ChatGPT backend base URL used for the redeem request.
    #[arg(long, value_name = "URL")]
    pub base_url: Option<String>,
    /// Bypass proxy environment variables for the upstream redeem request.
    #[arg(long)]
    pub no_proxy: bool,
}

impl fmt::Debug for RedeemArgs {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RedeemArgs")
            .field("profile_configured", &!self.profile.is_empty())
            .field("yes", &self.yes)
            .field("base_url_configured", &self.base_url.is_some())
            .field("no_proxy", &self.no_proxy)
            .finish()
    }
}

#[derive(Args)]
pub struct DashboardArgs {
    /// Interface to bind. Defaults to localhost only.
    #[arg(long, default_value = "127.0.0.1", value_name = "HOST")]
    pub host: String,
    /// Port to bind. Use 0 to ask the OS for a free port.
    #[arg(long, default_value_t = 8765, value_name = "PORT")]
    pub port: u16,
    /// Override the ChatGPT backend base URL used for quota requests.
    #[arg(long, value_name = "URL")]
    pub base_url: Option<String>,
}

impl fmt::Debug for DashboardArgs {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DashboardArgs")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("base_url_configured", &self.base_url.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quota_family_debug_redacts_base_urls() {
        let sentinel = "quota-args-debug-secret-sentinel";
        let base_url = Some(format!("https://user:{sentinel}@example.test"));
        let quota = QuotaArgs {
            profile: None,
            all: false,
            auth: None,
            provider: None,
            detail: false,
            raw: false,
            watch: false,
            once: true,
            base_url: base_url.clone(),
        };
        let redeem = RedeemArgs {
            profile: sentinel.to_string(),
            yes: false,
            base_url: base_url.clone(),
            no_proxy: false,
        };
        let dashboard = DashboardArgs {
            host: "127.0.0.1".to_string(),
            port: 8765,
            base_url,
        };

        for rendered in [
            format!("{quota:?}"),
            format!("{redeem:?}"),
            format!("{dashboard:?}"),
            format!(
                "{:?}",
                CodexPassthroughArgs {
                    profile: None,
                    codex_args: vec![OsString::from(format!(
                        "--base-url=https://user:{sentinel}@example.test"
                    ))],
                }
            ),
        ] {
            assert!(!rendered.contains(sentinel), "{rendered}");
        }
        assert!(format!("{quota:?}").contains("base_url_configured: true"));
    }
}
