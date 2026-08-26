use super::{
    SuperExternalProvider, codex_args_with_feature_overrides, parse_super_external_provider,
};
use crate::CodexRuntimeFeatureArgs;
use clap::{ArgGroup, Args};
use prodex_optional_tools::OptionalToolId;
use prodex_provider_core::ProviderId;
use std::ffi::OsString;
use std::fmt;

#[derive(Args)]
pub struct RunArgs {
    /// Starting profile for the run. If omitted, prodex uses the active profile.
    #[arg(short, long, value_name = "NAME")]
    pub profile: Option<String>,
    /// Allow eligible pre-commit rotation. This is the default behavior.
    #[arg(long, conflicts_with = "no_auto_rotate")]
    pub auto_rotate: bool,
    /// Keep the selected profile fixed and fail instead of rotating.
    #[arg(long)]
    pub no_auto_rotate: bool,
    /// Allow Prodex to redeem one earned reset credit automatically when all configured OpenAI/Codex profiles are weekly-exhausted.
    #[arg(long)]
    pub auto_redeem: bool,
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
    #[command(flatten)]
    pub codex_features: CodexRuntimeFeatureArgs,
    /// Arguments passed through to `codex`. A lone session id is normalized to `codex resume <session-id>`.
    #[arg(value_name = "CODEX_ARG", allow_hyphen_values = true)]
    pub codex_args: Vec<OsString>,
}

impl fmt::Debug for RunArgs {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RunArgs")
            .field("profile_configured", &self.profile.is_some())
            .field("auto_rotate", &self.auto_rotate)
            .field("no_auto_rotate", &self.no_auto_rotate)
            .field("auto_redeem", &self.auto_redeem)
            .field("skip_quota_check", &self.skip_quota_check)
            .field("full_access", &self.full_access)
            .field("base_url_configured", &self.base_url.is_some())
            .field("no_proxy", &self.no_proxy)
            .field("dry_run", &self.dry_run)
            .field("codex_features", &self.codex_features)
            .field("codex_args_count", &self.codex_args.len())
            .finish()
    }
}

#[derive(Args)]
pub struct ClaudeArgs {
    /// Starting profile for the run. If omitted, prodex uses the active profile.
    #[arg(short, long, value_name = "NAME")]
    pub profile: Option<String>,
    /// Allow eligible pre-commit rotation. This is the default behavior.
    #[arg(long, conflicts_with = "no_auto_rotate")]
    pub auto_rotate: bool,
    /// Keep the selected profile fixed and fail instead of rotating.
    #[arg(long)]
    pub no_auto_rotate: bool,
    /// Allow Prodex to redeem one earned reset credit automatically when all configured OpenAI/Codex profiles are weekly-exhausted.
    #[arg(long)]
    pub auto_redeem: bool,
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

impl fmt::Debug for ClaudeArgs {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClaudeArgs")
            .field("profile_configured", &self.profile.is_some())
            .field("auto_rotate", &self.auto_rotate)
            .field("no_auto_rotate", &self.no_auto_rotate)
            .field("auto_redeem", &self.auto_redeem)
            .field("skip_quota_check", &self.skip_quota_check)
            .field("base_url_configured", &self.base_url.is_some())
            .field("no_proxy", &self.no_proxy)
            .field("claude_args_count", &self.claude_args.len())
            .finish()
    }
}

#[derive(Args)]
pub struct RuntimeToolArgs {
    /// Starting profile for the run. If omitted, prodex uses the active profile.
    #[arg(short, long, value_name = "NAME")]
    pub profile: Option<String>,
    /// Allow eligible pre-commit rotation. This is the default behavior.
    #[arg(long, conflicts_with = "no_auto_rotate")]
    pub auto_rotate: bool,
    /// Keep the selected profile fixed and fail instead of rotating.
    #[arg(long)]
    pub no_auto_rotate: bool,
    /// Allow Prodex to redeem one earned reset credit automatically when all configured OpenAI/Codex profiles are weekly-exhausted.
    #[arg(long)]
    pub auto_redeem: bool,
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
    /// Apply the invocation-local workspace and hook trust required by Super/YOLO mode.
    #[arg(skip)]
    pub super_mode: bool,
    /// Add an optional tool to this launch.
    #[arg(long = "tool", value_name = "TOOL")]
    pub tools: Vec<OptionalToolId>,
    /// Require an optional tool; launch fails before the TUI if it is missing or invalid.
    #[arg(long = "require-tool", value_name = "TOOL")]
    pub required_tools: Vec<OptionalToolId>,
    /// Enable Presidio request redaction for this launch.
    #[arg(long)]
    pub presidio: bool,
    /// External provider selected by a higher-level launch shortcut.
    #[arg(skip)]
    pub external_provider: Option<SuperExternalProvider>,
    /// External provider API key supplied by a higher-level launch shortcut.
    #[arg(skip)]
    pub external_provider_api_key: Option<String>,
    #[arg(skip)]
    pub harness: Option<prodex_provider_core::HarnessMode>,
    #[command(flatten)]
    pub codex_features: CodexRuntimeFeatureArgs,
    /// Arguments passed through to `codex`. A lone session id is normalized to `codex resume <session-id>`.
    #[arg(value_name = "CODEX_ARG", allow_hyphen_values = true)]
    pub codex_args: Vec<OsString>,
}

#[derive(Args, Clone)]
#[command(group(
    ArgGroup::new("provider_or_url")
        .args(["provider", "url"])
        .multiple(false)
))]
pub struct SuperArgs {
    /// Starting profile for the run. If omitted, prodex uses the active profile.
    #[arg(short, long, value_name = "NAME")]
    pub profile: Option<String>,
    /// Allow eligible pre-commit rotation. This is the default behavior.
    #[arg(long, conflicts_with = "no_auto_rotate")]
    pub auto_rotate: bool,
    /// Keep the selected profile fixed and fail instead of rotating.
    #[arg(long)]
    pub no_auto_rotate: bool,
    /// Allow Prodex to redeem one earned reset credit automatically when all configured OpenAI/Codex profiles are weekly-exhausted.
    #[arg(long)]
    pub auto_redeem: bool,
    /// Skip the preflight quota gate before launching codex.
    #[arg(long)]
    pub skip_quota_check: bool,
    /// Compatibility flag. Super already starts Codex with launch-time full access.
    #[arg(long)]
    pub full_access: bool,
    /// Print resolved launch diagnostics without starting Codex.
    #[arg(long)]
    pub dry_run: bool,
    /// Override the upstream ChatGPT base URL used for quota preflight and the runtime proxy.
    #[arg(long, value_name = "URL", conflicts_with = "url")]
    pub base_url: Option<String>,
    /// Disable system and environment proxy settings for upstream OpenAI/quota HTTP requests.
    #[arg(long)]
    pub no_proxy: bool,
    /// Enable Presidio request-body and WebSocket text redaction without prompting. Unsupported by native Kiro and Antigravity CLIs.
    #[arg(long, conflicts_with = "no_presidio")]
    pub presidio: bool,
    /// Disable Presidio redaction and skip the interactive opt-in prompt.
    #[arg(long, conflicts_with = "presidio")]
    pub no_presidio: bool,
    /// Enable Codex sub-agent support for this Super launch.
    #[arg(long, conflicts_with = "no_sub_agent")]
    pub sub_agent: bool,
    /// Disable Codex sub-agent support for this Super launch.
    #[arg(long, conflicts_with = "sub_agent")]
    pub no_sub_agent: bool,
    /// Provider used by sub-agents. Detail flags require explicit sub-agent.
    #[arg(
        long,
        value_name = "PROVIDER",
        value_parser = crate::parse_sub_agent_provider,
        requires = "sub_agent"
    )]
    pub sub_agent_provider: Option<ProviderId>,
    /// Model used by sub-agents. Any nonempty model id is accepted.
    #[arg(
        long,
        value_name = "MODEL",
        value_parser = crate::parse_sub_agent_model,
        requires = "sub_agent"
    )]
    pub sub_agent_model: Option<String>,
    /// Reasoning effort used by the sub-agent model.
    #[arg(
        long,
        value_name = "EFFORT",
        value_parser = crate::parse_sub_agent_reasoning_effort,
        requires = "sub_agent"
    )]
    pub sub_agent_model_reasoning_effort: Option<crate::SubAgentReasoningEffort>,
    /// Local HTTP(S) endpoint used by sub-agents.
    #[arg(
        long,
        value_name = "URL",
        value_parser = crate::parse_sub_agent_url,
        requires = "sub_agent"
    )]
    pub sub_agent_url: Option<String>,
    /// Maximum number of child sub-agent processes active at once (1-64).
    #[arg(
        long,
        value_name = "VALUE",
        value_parser = crate::parse_sub_agent_max_concurrency,
        requires = "sub_agent"
    )]
    pub sub_agent_max_concurrency: Option<crate::SubAgentMaxConcurrency>,
    /// Add an optional tool to the default Super tool set.
    #[arg(long = "tool", value_name = "TOOL")]
    pub tools: Vec<OptionalToolId>,
    /// Require an optional tool; launch fails before the TUI if it is missing or invalid.
    #[arg(long = "require-tool", value_name = "TOOL")]
    pub required_tools: Vec<OptionalToolId>,
    /// Route Codex directly to a local OpenAI-compatible /v1 endpoint.
    #[arg(long, value_name = "URL", conflicts_with = "provider")]
    pub url: Option<String>,
    /// External provider preset to use through Codex/Super.
    #[arg(long, value_name = "PROVIDER", value_parser = parse_super_external_provider)]
    pub provider: Option<SuperExternalProvider>,
    /// Model-facing harness policy for local --provider or --url bridges. Defaults to native.
    #[arg(
        long,
        value_name = "native|minimal|evaluated",
        value_parser = parse_harness_mode,
        requires = "provider_or_url"
    )]
    pub harness: Option<prodex_provider_core::HarnessMode>,
    /// Agent CLI to launch. Gemini and Copilot use their matching provider; Kiro uses an imported profile through a local transport tunnel.
    #[arg(long, value_name = "CLI", value_enum)]
    pub cli: Option<SuperCliAgent>,
    /// API key for --provider. Prefer the provider-specific environment variable for shells/history.
    #[arg(long = "api-key", value_name = "KEY", requires = "provider")]
    pub api_key: Option<String>,
    /// Model id to use with Codex, --url, or --provider.
    #[arg(long = "model", visible_alias = "local-model", value_name = "MODEL")]
    pub local_model: Option<String>,
    /// Context window advertised to Codex when using --url or --provider.
    #[arg(
        long = "context-window",
        visible_alias = "local-context-window",
        value_name = "TOKENS",
        requires = "provider_or_url"
    )]
    pub local_context_window: Option<usize>,
    /// Auto-compact threshold advertised to Codex when using --url or --provider.
    #[arg(
        long = "auto-compact-token-limit",
        visible_alias = "local-auto-compact-token-limit",
        value_name = "TOKENS",
        requires = "provider_or_url"
    )]
    pub local_auto_compact_token_limit: Option<usize>,
    #[command(flatten)]
    pub codex_features: CodexRuntimeFeatureArgs,
    /// Arguments passed through to `codex` after Prodex's generated options.
    #[arg(value_name = "CODEX_ARG", allow_hyphen_values = true)]
    pub codex_args: Vec<OsString>,
}

impl fmt::Debug for SuperArgs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SuperArgs")
            .field("profile_configured", &self.profile.is_some())
            .field("auto_rotate", &self.auto_rotate)
            .field("no_auto_rotate", &self.no_auto_rotate)
            .field("auto_redeem", &self.auto_redeem)
            .field("skip_quota_check", &self.skip_quota_check)
            .field("full_access", &true)
            .field("full_access_flag_present", &self.full_access)
            .field("dry_run", &self.dry_run)
            .field("base_url_configured", &self.base_url.is_some())
            .field("no_proxy", &self.no_proxy)
            .field("presidio", &self.presidio)
            .field("no_presidio", &self.no_presidio)
            .field("sub_agent", &self.sub_agent)
            .field("no_sub_agent", &self.no_sub_agent)
            .field("sub_agent_provider", &self.sub_agent_provider)
            .field(
                "sub_agent_model_configured",
                &self.sub_agent_model.is_some(),
            )
            .field(
                "sub_agent_model_reasoning_effort",
                &self.sub_agent_model_reasoning_effort,
            )
            .field("sub_agent_max_concurrency", &self.sub_agent_max_concurrency)
            .field("sub_agent_url_configured", &self.sub_agent_url.is_some())
            .field("tools", &self.tools)
            .field("required_tools", &self.required_tools)
            .field("url_configured", &self.url.is_some())
            .field("provider", &self.provider)
            .field("harness", &self.harness)
            .field("cli", &self.cli)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("local_model", &self.local_model)
            .field("local_context_window", &self.local_context_window)
            .field(
                "local_auto_compact_token_limit",
                &self.local_auto_compact_token_limit,
            )
            .field("codex_features", &self.codex_features)
            .field("codex_args_count", &self.codex_args.len())
            .finish()
    }
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperCliAgent {
    Codex,
    Gemini,
    Copilot,
    Kiro,
    Agy,
}

#[derive(Args, Debug)]
pub(crate) struct SuperExposeArgs {
    #[command(flatten)]
    pub(crate) expose: super::ExposeArgs,
    #[command(flatten)]
    pub(crate) super_args: SuperArgs,
}

pub(super) fn parse_harness_mode(
    value: &str,
) -> Result<prodex_provider_core::HarnessMode, prodex_provider_core::ParseHarnessModeError> {
    value.parse()
}

pub(super) fn parse_runtime_base_url(url: &str) -> std::result::Result<String, String> {
    super::parse_credential_free_http_url(url, "--base-url")?;
    Ok(url.to_string())
}

impl SuperArgs {
    /// The first positional arg can look like a session ID when `trailing_var_arg=true`
    /// leaves Super flags unseen by clap. Extract the small set of Super-only flags that
    /// users commonly place after a session id so they do not leak into `codex resume`.
    pub fn extract_super_overrides_from_codex_args(&mut self) -> std::result::Result<(), String> {
        super::super_tail_extract::extract_super_overrides_from_codex_args(self)
    }

    pub fn extract_super_overrides_from_codex_args_for_native_preflight(
        &mut self,
    ) -> std::result::Result<(), String> {
        super::super_tail_extract::extract_super_overrides_from_codex_args_without_sub_agent_validation(
            self,
        )
    }

    /// Backward-compatible name retained for callers compiled against the provider-only helper.
    pub fn extract_provider_overrides_from_codex_args(
        &mut self,
    ) -> std::result::Result<(), String> {
        self.extract_super_overrides_from_codex_args()
    }

    pub fn validate_urls(&self) -> std::result::Result<(), String> {
        super::super_validation::validate_super_mode_compatibility(self)?;
        if let Some(base_url) = self.base_url.as_deref() {
            parse_runtime_base_url(base_url)?;
        }
        if let Some(url) = self.url.as_deref() {
            super::parse_super_local_url(url)?;
        }
        if let Some(url) = self.sub_agent_url.as_deref() {
            crate::parse_sub_agent_url(url)?;
        }
        Ok(())
    }

    pub fn sub_agent_preference(&self) -> crate::SubAgentPreference {
        if self.sub_agent {
            crate::SubAgentPreference::Enabled(self.sub_agent_config())
        } else if self.no_sub_agent {
            crate::SubAgentPreference::Disabled
        } else {
            crate::SubAgentPreference::Unspecified
        }
    }

    pub fn sub_agent_config(&self) -> crate::SubAgentConfig {
        let provider = self
            .sub_agent_provider
            .unwrap_or(crate::SUPER_SUB_AGENT_DEFAULT_PROVIDER);
        crate::SubAgentConfig {
            provider,
            model: self.sub_agent_model.clone(),
            model_reasoning_effort: self.sub_agent_model_reasoning_effort,
            url: self.sub_agent_url.clone(),
            max_concurrency: self.sub_agent_max_concurrency.unwrap_or_default(),
        }
    }
}

impl RunArgs {
    pub fn codex_args_with_feature_overrides(&self) -> Vec<OsString> {
        codex_args_with_feature_overrides(&self.codex_args, &self.codex_features)
    }
}
