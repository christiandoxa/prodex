use crate::CodexRuntimeFeatureArgs;
use clap::{ArgGroup, Args, Subcommand};
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

#[path = "runtime_args/super_tail_extract.rs"]
mod super_tail_extract;

pub const SUPER_OPTIMIZER_PREFIXES: [&str; 1] = ["ponytail"];

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
pub struct CavemanArgs {
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
    /// Enable Super optimizer tools in the Prodex overlay.
    #[arg(skip)]
    pub super_optimizer_overlay: bool,
    /// External provider selected by a higher-level launch shortcut.
    #[arg(skip)]
    pub external_provider: Option<SuperExternalProvider>,
    /// External provider API key supplied by a higher-level launch shortcut.
    #[arg(skip)]
    pub external_provider_api_key: Option<String>,
    #[command(flatten)]
    pub codex_features: CodexRuntimeFeatureArgs,
    /// Arguments passed through to `codex`. A lone session id is normalized to `codex resume <session-id>`.
    #[arg(value_name = "CODEX_ARG", allow_hyphen_values = true)]
    pub codex_args: Vec<OsString>,
}

impl fmt::Debug for CavemanArgs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CavemanArgs")
            .field("profile_configured", &self.profile.is_some())
            .field("auto_rotate", &self.auto_rotate)
            .field("no_auto_rotate", &self.no_auto_rotate)
            .field("auto_redeem", &self.auto_redeem)
            .field("skip_quota_check", &self.skip_quota_check)
            .field("full_access", &self.full_access)
            .field("dry_run", &self.dry_run)
            .field("base_url_configured", &self.base_url.is_some())
            .field("no_proxy", &self.no_proxy)
            .field("smart_context", &self.smart_context)
            .field("super_optimizer_overlay", &self.super_optimizer_overlay)
            .field("external_provider", &self.external_provider)
            .field(
                "external_provider_api_key",
                &self
                    .external_provider_api_key
                    .as_ref()
                    .map(|_| "<redacted>"),
            )
            .field("codex_features", &self.codex_features)
            .field("codex_args_count", &self.codex_args.len())
            .finish()
    }
}

#[derive(Args)]
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
    /// Print resolved launch diagnostics without starting Codex.
    #[arg(long)]
    pub dry_run: bool,
    /// Override the upstream ChatGPT base URL used for quota preflight and the runtime proxy.
    #[arg(long, value_name = "URL", conflicts_with = "url")]
    pub base_url: Option<String>,
    /// Disable system and environment proxy settings for upstream OpenAI/quota HTTP requests.
    #[arg(long)]
    pub no_proxy: bool,
    /// Enable Presidio request-body and WebSocket text redaction without prompting.
    #[arg(long, conflicts_with = "no_presidio")]
    pub presidio: bool,
    /// Disable Presidio redaction and skip the interactive opt-in prompt.
    #[arg(long, conflicts_with = "presidio")]
    pub no_presidio: bool,
    /// Route Codex directly to a local OpenAI-compatible /v1 endpoint.
    #[arg(long, value_name = "URL", conflicts_with = "provider")]
    pub url: Option<String>,
    /// External provider preset to use through Codex/Super.
    #[arg(long, value_name = "PROVIDER", value_parser = parse_super_external_provider)]
    pub provider: Option<SuperExternalProvider>,
    /// Agent CLI to launch. Gemini CLI requires the Gemini provider; Kiro CLI uses an imported Kiro profile.
    #[arg(long, value_name = "CLI", value_enum)]
    pub cli: Option<SuperCliAgent>,
    /// API key for --provider. Prefer the provider-specific environment variable for shells/history.
    #[arg(long = "api-key", value_name = "KEY", requires = "provider")]
    pub api_key: Option<String>,
    /// Model id to use with --url or --provider.
    #[arg(
        long = "model",
        visible_alias = "local-model",
        value_name = "MODEL",
        requires = "provider_or_url"
    )]
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
    /// Arguments passed through to `codex` after the implied optimizer prefixes.
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
            .field("dry_run", &self.dry_run)
            .field("base_url_configured", &self.base_url.is_some())
            .field("no_proxy", &self.no_proxy)
            .field("presidio", &self.presidio)
            .field("no_presidio", &self.no_presidio)
            .field("url_configured", &self.url.is_some())
            .field("provider", &self.provider)
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
    Kiro,
    Agy,
}

#[derive(Args, Debug)]
pub struct GeminiCompatRefreshArgs {
    /// CODEX_HOME to refresh Gemini CLI compatibility surfaces into.
    #[arg(long, value_name = "PATH")]
    pub codex_home: PathBuf,
}

#[derive(Args, Debug)]
pub struct McpJsonlBridgeArgs {
    /// JSON-lines MCP server command to bridge to Codex stdio framing.
    #[arg(value_name = "COMMAND")]
    pub command: PathBuf,
    /// Arguments passed to the JSON-lines MCP server.
    #[arg(
        value_name = "ARGS",
        trailing_var_arg = true,
        allow_hyphen_values = true
    )]
    pub args: Vec<OsString>,
}

#[derive(Args, Debug)]
pub struct ExposeArgs {
    /// Shell command to run inside the exposed PTY. Defaults to $SHELL or sh.
    #[arg(long, value_name = "COMMAND")]
    pub command: Option<String>,
    /// Initial terminal columns.
    #[arg(long, default_value_t = 100)]
    pub cols: u16,
    /// Initial terminal rows.
    #[arg(long, default_value_t = 32)]
    pub rows: u16,
    /// Maximum concurrent browser clients.
    #[arg(
        long,
        default_value_t = 4,
        value_parser = clap::value_parser!(u16).range(1..=32)
    )]
    pub max_clients: u16,
    /// Explicitly publish the loopback-only server through a cloudflared quick tunnel.
    #[arg(long, conflicts_with = "no_tunnel")]
    pub tunnel: bool,
    /// Deprecated compatibility alias; tunnel access is now disabled by default.
    #[arg(long, hide = true, conflicts_with = "tunnel")]
    pub no_tunnel: bool,
}

#[derive(Args, Debug)]
pub struct AppServerBrokerArgs {
    /// Print the broker capability contract as JSON.
    #[arg(long)]
    pub json: bool,
    /// Experimental live stdio broker that validates protocol frames and preserves input.
    #[arg(
        long,
        conflicts_with_all = [
            "experimental_stdio_passthrough_preview",
            "experimental_stdio_validate",
            "experimental_stdio_validate_passthrough"
        ]
    )]
    pub experimental_stdio_live: bool,
    /// Experimental line-by-line stdio preview that emits broker diagnostics as JSONL.
    #[arg(
        long,
        conflicts_with_all = [
            "experimental_stdio_live",
            "experimental_stdio_passthrough_preview",
            "experimental_stdio_validate",
            "experimental_stdio_validate_passthrough"
        ]
    )]
    pub experimental_stdio: bool,
    /// Experimental read-only stdio passthrough that mirrors input to stdout and emits diagnostics to stderr.
    #[arg(
        long,
        conflicts_with_all = [
            "experimental_stdio_live",
            "experimental_stdio",
            "experimental_stdio_validate",
            "experimental_stdio_validate_passthrough"
        ]
    )]
    pub experimental_stdio_passthrough_preview: bool,
    /// Experimental fail-closed stdio validation that emits diagnostics and errors on malformed frames.
    #[arg(
        long,
        conflicts_with_all = [
            "experimental_stdio_live",
            "experimental_stdio",
            "experimental_stdio_passthrough_preview",
            "experimental_stdio_validate_passthrough"
        ]
    )]
    pub experimental_stdio_validate: bool,
    /// Experimental stdio passthrough that validates each frame before forwarding it.
    #[arg(
        long,
        conflicts_with_all = [
            "experimental_stdio_live",
            "experimental_stdio",
            "experimental_stdio_passthrough_preview",
            "experimental_stdio_validate"
        ]
    )]
    pub experimental_stdio_validate_passthrough: bool,
}

#[derive(Args)]
pub struct GatewayArgs {
    #[command(subcommand)]
    pub command: Option<GatewayCommands>,
    /// Address to bind the OpenAI-compatible gateway to.
    #[arg(long, value_name = "ADDR")]
    pub listen: Option<String>,
    /// External provider preset for the gateway. Omit for an OpenAI-compatible upstream.
    #[arg(long, value_name = "PROVIDER", value_parser = parse_super_external_provider)]
    pub provider: Option<SuperExternalProvider>,
    /// Upstream base URL. Defaults to the selected provider default, policy.toml, or OPENAI_BASE_URL.
    #[arg(long = "base-url", visible_alias = "url", value_name = "URL")]
    pub base_url: Option<String>,
    /// Provider API key. Prefer provider-specific env vars for shells/history.
    #[arg(long = "api-key", value_name = "KEY")]
    pub api_key: Option<String>,
    /// Require this bearer token from gateway clients. Env fallback: PRODEX_GATEWAY_TOKEN.
    #[arg(long = "auth-token", value_name = "TOKEN")]
    pub auth_token: Option<String>,
    /// Enable Smart Context Autopilot for gateway /v1/responses and /v1/chat/completions requests.
    #[arg(long = "smart-context", default_value_t = false)]
    pub smart_context: bool,
    /// Enable Presidio request-body redaction for gateway requests.
    #[arg(long, conflicts_with = "no_presidio")]
    pub presidio: bool,
    /// Disable policy-enabled Presidio redaction for this gateway process.
    #[arg(long, conflicts_with = "presidio")]
    pub no_presidio: bool,
}

impl fmt::Debug for GatewayArgs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GatewayArgs")
            .field("command", &self.command)
            .field("listen", &self.listen)
            .field("provider", &self.provider)
            .field("base_url_configured", &self.base_url.is_some())
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field(
                "auth_token",
                &self.auth_token.as_ref().map(|_| "<redacted>"),
            )
            .field("smart_context", &self.smart_context)
            .field("presidio", &self.presidio)
            .field("no_presidio", &self.no_presidio)
            .finish()
    }
}

#[derive(Subcommand, Debug)]
pub enum GatewayCommands {
    #[command(about = "Print provider adapter contracts.")]
    Providers(GatewayProvidersArgs),
    #[command(about = "Print provider endpoint capabilities.")]
    Capabilities(GatewayProviderFilterArgs),
    #[command(about = "Print provider model catalog.")]
    Models(GatewayProviderFilterArgs),
}

#[derive(Args, Debug)]
pub struct GatewayProvidersArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct GatewayProviderFilterArgs {
    #[arg(long, value_name = "PROVIDER")]
    pub provider: String,
    #[arg(long)]
    pub json: bool,
}

impl SuperArgs {
    /// The first positional arg can look like a session ID when `trailing_var_arg=true`
    /// leaves Super flags unseen by clap. Extract the small set of Super-only flags that
    /// users commonly place after a session id so they do not leak into `codex resume`.
    pub fn extract_provider_overrides_from_codex_args(
        &mut self,
    ) -> std::result::Result<(), String> {
        super_tail_extract::extract_provider_overrides_from_codex_args(self)
    }

    pub fn validate_urls(&self) -> std::result::Result<(), String> {
        if let Some(base_url) = self.base_url.as_deref() {
            parse_runtime_base_url(base_url)?;
        }
        if let Some(url) = self.url.as_deref() {
            parse_super_local_url(url)?;
        }
        Ok(())
    }

    pub fn presidio_preference(&self) -> Option<bool> {
        if self.presidio {
            Some(true)
        } else if self.no_presidio {
            Some(false)
        } else {
            None
        }
    }

    pub fn into_caveman_args(self) -> CavemanArgs {
        self.into_caveman_args_with_presidio(false)
    }

    pub fn into_caveman_args_with_presidio(self, presidio: bool) -> CavemanArgs {
        let (prefixed_presidio, passthrough_codex_args) =
            extract_super_leading_launch_prefixes(self.codex_args);
        let presidio = presidio || prefixed_presidio;
        let local_upstream_base_url = self.url.as_deref().map(super_local_provider_base_url);
        let external_upstream_base_url = self.provider.map(|provider| {
            self.base_url
                .as_deref()
                .map(super_external_provider_base_url)
                .unwrap_or_else(|| provider.default_base_url().to_string())
        });
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
        let external_provider_args = self
            .provider
            .map(|provider| {
                super_external_provider_codex_args(
                    provider,
                    external_upstream_base_url.as_deref().unwrap_or_default(),
                    self.local_model.as_deref(),
                    self.local_context_window,
                    self.local_auto_compact_token_limit,
                )
            })
            .unwrap_or_default();
        let local_mode = self.url.is_some() || self.provider.is_some();
        let skip_quota_check = self.skip_quota_check || local_mode;

        let feature_overrides = self.codex_features.to_codex_config_args();
        let mut codex_args = Vec::new();
        codex_args.push(OsString::from("rtk"));
        codex_args.extend(SUPER_OPTIMIZER_PREFIXES.iter().map(OsString::from));
        if presidio {
            codex_args.push(OsString::from("presidio"));
        }
        codex_args.push(OsString::from("--dangerously-bypass-hook-trust"));
        codex_args.extend(local_provider_args);
        codex_args.extend(external_provider_args);
        codex_args.extend(feature_overrides);
        codex_args.extend(passthrough_codex_args);
        CavemanArgs {
            profile: self.profile,
            auto_rotate: self.auto_rotate,
            no_auto_rotate: self.no_auto_rotate,
            auto_redeem: self.auto_redeem,
            skip_quota_check,
            full_access: true,
            dry_run: self.dry_run,
            base_url: local_upstream_base_url
                .or(external_upstream_base_url)
                .or(self.base_url),
            no_proxy: self.no_proxy,
            smart_context: true,
            super_optimizer_overlay: true,
            external_provider: self.provider,
            external_provider_api_key: self.api_key,
            codex_features: CodexRuntimeFeatureArgs::default(),
            codex_args,
        }
    }
}

fn extract_super_leading_launch_prefixes(args: Vec<OsString>) -> (bool, Vec<OsString>) {
    let mut presidio = false;
    let mut consumed = 0;
    for arg in &args {
        let Some(prefix) = arg.to_str() else {
            break;
        };
        match prefix {
            "rtk" | "ponytail" => {}
            "presidio" => presidio = true,
            _ => break,
        }
        consumed += 1;
    }
    (presidio, args.into_iter().skip(consumed).collect())
}

impl RunArgs {
    pub fn codex_args_with_feature_overrides(&self) -> Vec<OsString> {
        codex_args_with_feature_overrides(&self.codex_args, &self.codex_features)
    }
}

impl CavemanArgs {
    pub fn codex_args_with_feature_overrides(&self) -> Vec<OsString> {
        codex_args_with_feature_overrides(&self.codex_args, &self.codex_features)
    }
}

fn codex_args_with_feature_overrides(
    codex_args: &[OsString],
    features: &CodexRuntimeFeatureArgs,
) -> Vec<OsString> {
    let overrides = features.to_codex_config_args();
    if overrides.is_empty() {
        return codex_args.to_vec();
    }
    let mut args = Vec::with_capacity(codex_args.len() + overrides.len());
    args.extend(codex_args.iter().cloned());
    args.extend(overrides);
    args
}

pub fn caveman_args_with_optimizer_prefix(mut args: CavemanArgs, prefix: &str) -> CavemanArgs {
    args.codex_args.insert(0, OsString::from(prefix));
    args
}

pub const SUPER_LOCAL_PROVIDER_ID: &str = "prodex-local";
const SUPER_LOCAL_PROVIDER_NAME: &str = "Prodex Local";
pub const SUPER_DEFAULT_LOCAL_MODEL: &str = "unsloth/qwen3.5-35b-a3b";
pub const SUPER_DEFAULT_CONTEXT_WINDOW: usize = 16_384;
pub const SUPER_DEFAULT_AUTO_COMPACT_LIMIT: usize = 14_000;
pub const SUPER_DEEPSEEK_PROVIDER_ID: &str = "prodex-deepseek";
const SUPER_DEEPSEEK_PROVIDER_NAME: &str = "DeepSeek";
pub const SUPER_DEEPSEEK_DEFAULT_MODEL: &str = "deepseek-v4-pro";
const SUPER_DEEPSEEK_DEFAULT_BASE_URL: &str = "https://api.deepseek.com";
pub const SUPER_DEEPSEEK_DEFAULT_CONTEXT_WINDOW: usize = 1_048_576;
pub const SUPER_DEEPSEEK_DEFAULT_AUTO_COMPACT_LIMIT: usize = 900_000;
pub const SUPER_GEMINI_PROVIDER_ID: &str = "prodex-gemini";
const SUPER_GEMINI_PROVIDER_NAME: &str = "Google Gemini";
pub const SUPER_GEMINI_DEFAULT_MODEL: &str = prodex_runtime_gemini::GEMINI_DEFAULT_MODEL;
pub const SUPER_GEMINI_DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";
pub const SUPER_GEMINI_DEFAULT_CONTEXT_WINDOW: usize =
    prodex_runtime_gemini::GEMINI_DEFAULT_CONTEXT_WINDOW;
pub const SUPER_GEMINI_DEFAULT_AUTO_COMPACT_LIMIT: usize =
    prodex_runtime_gemini::GEMINI_DEFAULT_AUTO_COMPACT_LIMIT;
pub const SUPER_ANTHROPIC_PROVIDER_ID: &str = "prodex-anthropic";
const SUPER_ANTHROPIC_PROVIDER_NAME: &str = "Anthropic Claude";
pub const SUPER_ANTHROPIC_DEFAULT_MODEL: &str = "claude-sonnet-4-6";
const SUPER_ANTHROPIC_DEFAULT_BASE_URL: &str = "https://api.anthropic.com/v1";
pub const SUPER_ANTHROPIC_DEFAULT_CONTEXT_WINDOW: usize = 200_000;
pub const SUPER_ANTHROPIC_DEFAULT_AUTO_COMPACT_LIMIT: usize = 180_000;
pub const SUPER_COPILOT_PROVIDER_ID: &str = "prodex-copilot";
const SUPER_COPILOT_PROVIDER_NAME: &str = "GitHub Copilot";
pub const SUPER_COPILOT_DEFAULT_MODEL: &str = "gpt-5.3-codex";
const SUPER_COPILOT_DEFAULT_BASE_URL: &str = "https://api.githubcopilot.com";
pub const SUPER_KIRO_PROVIDER_ID: &str = "prodex-kiro";
const SUPER_KIRO_PROVIDER_NAME: &str = "Kiro";
pub const SUPER_KIRO_DEFAULT_MODEL: &str = "auto";
const SUPER_KIRO_DEFAULT_BASE_URL: &str = "https://kiro.dev";
pub const SUPER_KIRO_DEFAULT_CONTEXT_WINDOW: usize = 1_000_000;
pub const SUPER_KIRO_DEFAULT_AUTO_COMPACT_LIMIT: usize = 950_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperExternalProvider {
    Anthropic,
    Copilot,
    DeepSeek,
    Gemini,
    Kiro,
}

impl SuperExternalProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::Copilot => "copilot",
            Self::DeepSeek => "deepseek",
            Self::Gemini => "gemini",
            Self::Kiro => "kiro",
        }
    }

    pub fn model_provider_id(self) -> &'static str {
        match self {
            Self::Anthropic => SUPER_ANTHROPIC_PROVIDER_ID,
            Self::Copilot => SUPER_COPILOT_PROVIDER_ID,
            Self::DeepSeek => SUPER_DEEPSEEK_PROVIDER_ID,
            Self::Gemini => SUPER_GEMINI_PROVIDER_ID,
            Self::Kiro => SUPER_KIRO_PROVIDER_ID,
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::Anthropic => SUPER_ANTHROPIC_PROVIDER_NAME,
            Self::Copilot => SUPER_COPILOT_PROVIDER_NAME,
            Self::DeepSeek => SUPER_DEEPSEEK_PROVIDER_NAME,
            Self::Gemini => SUPER_GEMINI_PROVIDER_NAME,
            Self::Kiro => SUPER_KIRO_PROVIDER_NAME,
        }
    }

    fn codex_provider_name(self) -> &'static str {
        match self {
            // Codex currently exposes no configurable remote-compaction capability flag.
            // It enables /responses/compact only for provider names OpenAI or Azure.
            Self::Gemini | Self::Kiro => "Azure",
            Self::Copilot => "OpenAI",
            _ => self.display_name(),
        }
    }

    fn default_model(self) -> &'static str {
        match self {
            Self::Anthropic => SUPER_ANTHROPIC_DEFAULT_MODEL,
            Self::Copilot => SUPER_COPILOT_DEFAULT_MODEL,
            Self::DeepSeek => SUPER_DEEPSEEK_DEFAULT_MODEL,
            Self::Gemini => SUPER_GEMINI_DEFAULT_MODEL,
            Self::Kiro => SUPER_KIRO_DEFAULT_MODEL,
        }
    }

    pub fn default_base_url(self) -> &'static str {
        match self {
            Self::Anthropic => SUPER_ANTHROPIC_DEFAULT_BASE_URL,
            Self::Copilot => SUPER_COPILOT_DEFAULT_BASE_URL,
            Self::DeepSeek => SUPER_DEEPSEEK_DEFAULT_BASE_URL,
            Self::Gemini => SUPER_GEMINI_DEFAULT_BASE_URL,
            Self::Kiro => SUPER_KIRO_DEFAULT_BASE_URL,
        }
    }

    fn default_context_window(self) -> usize {
        match self {
            Self::Anthropic => SUPER_ANTHROPIC_DEFAULT_CONTEXT_WINDOW,
            Self::Copilot => crate::SUPER_COPILOT_DEFAULT_CONTEXT_WINDOW,
            Self::DeepSeek => SUPER_DEEPSEEK_DEFAULT_CONTEXT_WINDOW,
            Self::Gemini => SUPER_GEMINI_DEFAULT_CONTEXT_WINDOW,
            Self::Kiro => SUPER_KIRO_DEFAULT_CONTEXT_WINDOW,
        }
    }

    fn default_auto_compact_token_limit(self) -> usize {
        match self {
            Self::Anthropic => SUPER_ANTHROPIC_DEFAULT_AUTO_COMPACT_LIMIT,
            Self::Copilot => crate::SUPER_COPILOT_DEFAULT_AUTO_COMPACT_LIMIT,
            Self::DeepSeek => SUPER_DEEPSEEK_DEFAULT_AUTO_COMPACT_LIMIT,
            Self::Gemini => SUPER_GEMINI_DEFAULT_AUTO_COMPACT_LIMIT,
            Self::Kiro => SUPER_KIRO_DEFAULT_AUTO_COMPACT_LIMIT,
        }
    }

    fn web_search_mode(self) -> &'static str {
        match self {
            Self::Anthropic | Self::Copilot | Self::DeepSeek | Self::Gemini | Self::Kiro => "live",
        }
    }

    fn image_generation_enabled(self) -> bool {
        matches!(self, Self::Gemini)
    }
}

fn parse_super_external_provider(
    value: &str,
) -> std::result::Result<SuperExternalProvider, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "anthropic" | "claude" => Ok(SuperExternalProvider::Anthropic),
        "copilot" | "github-copilot" | "github_copilot" => Ok(SuperExternalProvider::Copilot),
        "deepseek" => Ok(SuperExternalProvider::DeepSeek),
        "gemini" => Ok(SuperExternalProvider::Gemini),
        "kiro" => Ok(SuperExternalProvider::Kiro),
        other => Err(format!(
            "invalid --provider: supported values are anthropic, copilot, deepseek, gemini, kiro, got {other:?}"
        )),
    }
}

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
        "features.apps=false".to_string(),
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

pub fn super_external_provider_codex_args(
    provider: SuperExternalProvider,
    base_url: &str,
    model: Option<&str>,
    context_window: Option<usize>,
    auto_compact_token_limit: Option<usize>,
) -> Vec<OsString> {
    let provider_id = provider.model_provider_id();
    let base_url = super_external_provider_base_url(base_url);
    let model = model
        .filter(|model| !model.trim().is_empty())
        .unwrap_or_else(|| provider.default_model());
    let (context_window, auto_compact_token_limit) =
        crate::super_provider_limits::external_provider_token_limits(
            model,
            provider == SuperExternalProvider::Copilot,
            provider.default_context_window(),
            provider.default_auto_compact_token_limit(),
            context_window,
            auto_compact_token_limit,
        );
    let overrides = [
        format!("model_provider={}", toml_string_literal(provider_id)),
        format!("model={}", toml_string_literal(model)),
        format!(
            "model_providers.{provider_id}.name={}",
            toml_string_literal(provider.codex_provider_name())
        ),
        format!(
            "model_providers.{provider_id}.base_url={}",
            toml_string_literal(&base_url)
        ),
        format!("model_providers.{provider_id}.wire_api=\"responses\""),
        format!("model_providers.{provider_id}.supports_websockets=false"),
        format!("model_context_window={context_window}"),
        format!("model_auto_compact_token_limit={auto_compact_token_limit}"),
        "model_reasoning_summary=\"none\"".to_string(),
        "model_supports_reasoning_summaries=true".to_string(),
        format!("web_search=\"{}\"", provider.web_search_mode()),
        "features.apps=false".to_string(),
        "features.js_repl=false".to_string(),
        format!(
            "features.image_generation={}",
            provider.image_generation_enabled()
        ),
    ];

    let mut args = Vec::with_capacity(overrides.len() * 2);
    for override_entry in overrides {
        args.push(OsString::from("-c"));
        args.push(OsString::from(override_entry));
    }
    args
}

fn super_local_provider_base_url(url: &str) -> String {
    if let Ok(mut parsed) = reqwest::Url::parse(url) {
        let path = parsed.path().trim_end_matches('/');
        if path.is_empty() || path == "/" {
            parsed.set_path("/v1");
            return parsed.as_str().trim_end_matches('/').to_string();
        }
    }
    url.trim_end_matches('/').to_string()
}

fn super_external_provider_base_url(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

fn parse_super_local_url(url: &str) -> std::result::Result<String, String> {
    parse_credential_free_http_url(url, "--url")?;
    Ok(url.to_string())
}

fn parse_runtime_base_url(url: &str) -> std::result::Result<String, String> {
    parse_credential_free_http_url(url, "--base-url")?;
    Ok(url.to_string())
}

fn parse_credential_free_http_url(
    url: &str,
    option: &str,
) -> std::result::Result<reqwest::Url, String> {
    let invalid = || {
        format!(
            "invalid {option}: expected an absolute http(s) URL with host and no credentials, \
         query, or fragment"
        )
    };
    let parsed = reqwest::Url::parse(url).map_err(|_| invalid())?;
    if url.starts_with("http:///")
        || url.starts_with("https:///")
        || !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(invalid());
    }
    Ok(parsed)
}

fn toml_string_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[derive(Args, Debug)]
pub struct RuntimeBrokerArgs {}

#[cfg(test)]
mod tests {
    use super::*;

    fn super_args_from(codex_args: &[&str]) -> SuperArgs {
        let os: Vec<OsString> = codex_args.iter().map(OsString::from).collect();
        SuperArgs {
            codex_args: os,
            provider: None,
            api_key: None,
            local_model: None,
            profile: None,
            auto_rotate: false,
            no_auto_rotate: false,
            auto_redeem: false,
            skip_quota_check: false,
            dry_run: false,
            base_url: None,
            no_proxy: false,
            presidio: false,
            no_presidio: false,
            url: None,
            cli: None,
            local_context_window: None,
            local_auto_compact_token_limit: None,
            codex_features: CodexRuntimeFeatureArgs::default(),
        }
    }

    #[test]
    fn extract_provider_flags_from_codex_args_after_session_id() {
        let mut args = super_args_from(&[
            "019ef8ae-c7cc-75c3-8575-a8d247ad291b",
            "--provider",
            "deepseek",
            "--model",
            "deepseek-v4-pro",
            "--api-key",
            "sk-test",
        ]);
        args.extract_provider_overrides_from_codex_args().unwrap();
        assert_eq!(args.provider, Some(SuperExternalProvider::DeepSeek));
        assert_eq!(args.local_model.as_deref(), Some("deepseek-v4-pro"));
        assert_eq!(args.api_key.as_deref(), Some("sk-test"));
        assert_eq!(
            args.codex_args
                .iter()
                .map(|a| a.to_string_lossy())
                .collect::<Vec<_>>(),
            vec!["019ef8ae-c7cc-75c3-8575-a8d247ad291b"]
        );
    }

    #[test]
    fn extract_provider_equals_syntax_from_codex_args() {
        let mut args = super_args_from(&[
            "019ef8ae-c7cc-75c3-8575-a8d247ad291b",
            "--provider=gemini",
            "--model=gemini-2.5-pro",
        ]);
        args.extract_provider_overrides_from_codex_args().unwrap();
        assert_eq!(args.provider, Some(SuperExternalProvider::Gemini));
        assert_eq!(args.local_model.as_deref(), Some("gemini-2.5-pro"));
        assert_eq!(
            args.codex_args
                .iter()
                .map(|a| a.to_string_lossy())
                .collect::<Vec<_>>(),
            vec!["019ef8ae-c7cc-75c3-8575-a8d247ad291b"]
        );
    }

    #[test]
    fn extract_provider_kiro_from_codex_args() {
        let mut args = super_args_from(&[
            "019ef8ae-c7cc-75c3-8575-a8d247ad291b",
            "--provider",
            "kiro",
            "--model",
            "claude-sonnet-4",
        ]);
        args.extract_provider_overrides_from_codex_args().unwrap();
        assert_eq!(args.provider, Some(SuperExternalProvider::Kiro));
        assert_eq!(args.local_model.as_deref(), Some("claude-sonnet-4"));
        assert_eq!(
            args.codex_args
                .iter()
                .map(|a| a.to_string_lossy())
                .collect::<Vec<_>>(),
            vec!["019ef8ae-c7cc-75c3-8575-a8d247ad291b"]
        );
    }

    #[test]
    fn super_external_provider_codex_args_support_kiro() {
        let args = super_external_provider_codex_args(
            SuperExternalProvider::Kiro,
            "http://127.0.0.1:4317/v1",
            Some("claude-sonnet-4.5"),
            Some(222_222),
            Some(111_111),
        );
        let rendered = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(rendered.contains(&format!(
            "model_provider={}",
            toml_string_literal(SUPER_KIRO_PROVIDER_ID)
        )));
        assert!(rendered.contains(&format!(
            "model={}",
            toml_string_literal("claude-sonnet-4.5")
        )));
        assert!(rendered.contains(&format!(
            "model_providers.{SUPER_KIRO_PROVIDER_ID}.name={}",
            toml_string_literal("Azure")
        )));
        assert!(rendered.contains(&format!(
            "model_providers.{SUPER_KIRO_PROVIDER_ID}.base_url={}",
            toml_string_literal("http://127.0.0.1:4317/v1")
        )));
        assert!(rendered.contains(&"model_context_window=222222".to_string()));
        assert!(rendered.contains(&"model_auto_compact_token_limit=111111".to_string()));
    }

    #[test]
    fn super_external_provider_codex_args_support_copilot_compact() {
        let args = super_external_provider_codex_args(
            SuperExternalProvider::Copilot,
            "https://api.githubcopilot.com",
            Some("gpt-5.3-codex"),
            Some(333_333),
            Some(222_222),
        );
        let rendered = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(rendered.contains(&format!(
            "model_provider={}",
            toml_string_literal(SUPER_COPILOT_PROVIDER_ID)
        )));
        assert!(rendered.contains(&format!(
            "model_providers.{SUPER_COPILOT_PROVIDER_ID}.name={}",
            toml_string_literal("OpenAI")
        )));
        assert!(rendered.contains(&"model_context_window=333333".to_string()));
        assert!(rendered.contains(&"model_auto_compact_token_limit=222222".to_string()));
    }

    #[test]
    fn extract_noop_when_no_provider_flags_in_codex_args() {
        let mut args = super_args_from(&["just", "some", "codex", "args"]);
        args.extract_provider_overrides_from_codex_args().unwrap();
        assert_eq!(args.provider, None);
        assert_eq!(
            args.codex_args
                .iter()
                .map(|a| a.to_string_lossy())
                .collect::<Vec<_>>(),
            vec!["just", "some", "codex", "args"]
        );
    }

    #[test]
    fn extract_respects_already_set_provider() {
        let mut args = super_args_from(&[
            "019ef8ae-c7cc-75c3-8575-a8d247ad291b",
            "--provider",
            "deepseek",
        ]);
        // Simulate clap already setting provider
        args.provider = Some(SuperExternalProvider::DeepSeek);
        args.extract_provider_overrides_from_codex_args().unwrap();
        // Should overwrite with extracted value (same here but structurally ok)
        assert_eq!(args.provider, Some(SuperExternalProvider::DeepSeek));
        // codex_args should be cleaned of provider flags
        assert_eq!(
            args.codex_args
                .iter()
                .map(|a| a.to_string_lossy())
                .collect::<Vec<_>>(),
            vec!["019ef8ae-c7cc-75c3-8575-a8d247ad291b"]
        );
    }

    #[test]
    fn super_url_validation_rejects_secrets_without_echoing_them() {
        for (base_url, url) in [
            (
                Some("https://user:super-base-secret-sentinel@example.test"),
                None,
            ),
            (
                None,
                Some("https://example.test/v1?token=super-url-secret-sentinel"),
            ),
        ] {
            let mut args = super_args_from(&[]);
            args.base_url = base_url.map(str::to_string);
            args.url = url.map(str::to_string);

            let error = args.validate_urls().unwrap_err();

            assert!(
                error.contains("no credentials, query, or fragment"),
                "{error}"
            );
            assert!(!error.contains("secret-sentinel"), "{error}");
        }
    }

    #[test]
    fn runtime_arg_debug_redacts_url_and_passthrough_values() {
        let sentinel = "runtime-args-debug-secret-sentinel";
        let run = RunArgs {
            profile: None,
            auto_rotate: false,
            no_auto_rotate: false,
            auto_redeem: false,
            skip_quota_check: false,
            full_access: false,
            base_url: Some(format!("https://user:{sentinel}@example.test")),
            no_proxy: false,
            dry_run: false,
            codex_features: CodexRuntimeFeatureArgs::default(),
            codex_args: vec![OsString::from(sentinel)],
        };
        let claude = ClaudeArgs {
            profile: None,
            auto_rotate: false,
            no_auto_rotate: false,
            auto_redeem: false,
            skip_quota_check: false,
            base_url: Some(format!("https://user:{sentinel}@example.test")),
            no_proxy: false,
            claude_args: vec![OsString::from(sentinel)],
        };

        for rendered in [
            format!("{:?}", crate::Commands::Run(run)),
            format!("{:?}", crate::Commands::Claude(claude)),
        ] {
            assert!(rendered.contains("base_url_configured: true"), "{rendered}");
            assert!(!rendered.contains(sentinel), "{rendered}");
        }
    }
}
