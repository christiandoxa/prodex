use crate::CodexRuntimeFeatureArgs;
use clap::{Args, Subcommand};
use prodex_provider_core::{ProviderId, ProviderRuntimeMetadata, provider_runtime_metadata};
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

#[path = "runtime_args/launch_args.rs"]
mod launch_args;
#[path = "runtime_args/optional_tools.rs"]
mod optional_tools;
#[path = "runtime_args/super_tail_extract.rs"]
mod super_tail_extract;
#[path = "runtime_args/super_validation.rs"]
mod super_validation;
pub(crate) use launch_args::SuperExposeArgs;
pub use launch_args::{ClaudeArgs, RunArgs, RuntimeToolArgs, SuperArgs, SuperCliAgent};
use launch_args::{parse_harness_mode, parse_runtime_base_url};
pub use optional_tools::runtime_tool_args_with_tool;

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
pub struct SubAgentExecArgs {
    /// Temporary, secret-free child launch configuration.
    #[arg(long, value_name = "PATH")]
    pub config: PathBuf,
    /// File containing the exact delegated task.
    #[arg(long, value_name = "PATH")]
    pub task_file: PathBuf,
}

#[derive(Args, Debug, Clone)]
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
    /// Explicitly publish the loopback-only server through a Cloudflare Quick Tunnel.
    #[arg(long, conflicts_with_all = ["no_tunnel", "tunnel_provider"])]
    pub tunnel: bool,
    /// Deprecated compatibility alias; tunnel access is now disabled by default.
    #[arg(
        long,
        hide = true,
        conflicts_with_all = ["tunnel", "tunnel_provider"]
    )]
    pub no_tunnel: bool,
    /// Select the tunnel provider; OpenAI publishes MCP only and keeps the browser local.
    #[arg(
        long,
        value_name = "PROVIDER",
        value_enum,
        conflicts_with_all = ["tunnel", "no_tunnel"]
    )]
    pub tunnel_provider: Option<ExposeTunnelProvider>,
    /// Existing Cloudflare config file. Defaults to cloudflared's official search locations.
    #[arg(long, value_name = "PATH", conflicts_with = "cloudflare_token_file")]
    pub cloudflare_config: Option<PathBuf>,
    /// Existing Cloudflare tunnel name or UUID. Defaults to the config's `tunnel` value.
    #[arg(long, value_name = "NAME|UUID")]
    pub cloudflare_tunnel: Option<String>,
    /// Existing Cloudflare public hostname. Defaults to the unique hostname in the config.
    #[arg(long, value_name = "HOSTNAME")]
    pub cloudflare_hostname: Option<String>,
    /// Existing Cloudflare loopback origin port. Defaults to the matching config service port.
    #[arg(long, value_name = "PORT")]
    pub cloudflare_origin_port: Option<u16>,
    /// Existing remotely-managed Cloudflare token file passed to cloudflared via --token-file.
    #[arg(long, value_name = "PATH", conflicts_with = "cloudflare_config")]
    pub cloudflare_token_file: Option<PathBuf>,
    /// Existing OpenAI Platform tunnel id used by the OpenAI Secure MCP Tunnel provider.
    #[arg(long, value_name = "ID")]
    pub openai_tunnel_id: Option<String>,
    /// Suggested display name for the ChatGPT connection.
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,
    /// Where this expose command was invoked from; set by CLI normalization.
    #[arg(skip)]
    pub invocation: ExposeInvocation,
    /// Super configuration captured by the `prodex s expose` alias.
    #[arg(skip)]
    pub super_args: Option<SuperArgs>,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExposeTunnelProvider {
    /// Start an ephemeral `trycloudflare.com` Quick Tunnel.
    #[value(name = "cloudflare-quick", alias = "cloudflare")]
    CloudflareQuick,
    /// Start a pre-created, user-managed Cloudflare Tunnel from local configuration.
    #[value(name = "cloudflare-existing", alias = "cloudflare-named")]
    CloudflareExisting,
    /// Connect MCP through the official OpenAI Secure MCP Tunnel client.
    #[value(name = "openai")]
    OpenAi,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExposeInvocation {
    #[default]
    Standalone,
    SuperAlias,
}

#[derive(Args, Debug)]
pub struct AppServerBrokerArgs {
    /// Codex profile used by the live broker child process.
    #[arg(long, value_name = "NAME", requires = "experimental_stdio_live")]
    pub profile: Option<String>,
    /// Print the broker capability contract as JSON.
    #[arg(long)]
    pub json: bool,
    /// Experimental live broker that launches Codex app-server and validates both stdio directions.
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
    /// Model-facing harness policy. Defaults to policy.toml or native.
    #[arg(
        long,
        value_name = "native|minimal|evaluated",
        value_parser = parse_harness_mode
    )]
    pub harness: Option<prodex_provider_core::HarnessMode>,
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
            .field("harness", &self.harness)
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

pub use prodex_provider_core::{
    PRODEX_ANTHROPIC_DEFAULT_AUTO_COMPACT_LIMIT as SUPER_ANTHROPIC_DEFAULT_AUTO_COMPACT_LIMIT,
    PRODEX_ANTHROPIC_DEFAULT_CONTEXT_WINDOW as SUPER_ANTHROPIC_DEFAULT_CONTEXT_WINDOW,
    PRODEX_ANTHROPIC_DEFAULT_MODEL as SUPER_ANTHROPIC_DEFAULT_MODEL,
    PRODEX_ANTHROPIC_PROVIDER_ID as SUPER_ANTHROPIC_PROVIDER_ID,
    PRODEX_COPILOT_DEFAULT_MODEL as SUPER_COPILOT_DEFAULT_MODEL,
    PRODEX_COPILOT_PROVIDER_ID as SUPER_COPILOT_PROVIDER_ID,
    PRODEX_DEEPSEEK_DEFAULT_AUTO_COMPACT_LIMIT as SUPER_DEEPSEEK_DEFAULT_AUTO_COMPACT_LIMIT,
    PRODEX_DEEPSEEK_DEFAULT_CONTEXT_WINDOW as SUPER_DEEPSEEK_DEFAULT_CONTEXT_WINDOW,
    PRODEX_DEEPSEEK_DEFAULT_MODEL as SUPER_DEEPSEEK_DEFAULT_MODEL,
    PRODEX_DEEPSEEK_PROVIDER_ID as SUPER_DEEPSEEK_PROVIDER_ID,
    PRODEX_GEMINI_DEFAULT_AUTO_COMPACT_LIMIT as SUPER_GEMINI_DEFAULT_AUTO_COMPACT_LIMIT,
    PRODEX_GEMINI_DEFAULT_BASE_URL as SUPER_GEMINI_DEFAULT_BASE_URL,
    PRODEX_GEMINI_DEFAULT_CONTEXT_WINDOW as SUPER_GEMINI_DEFAULT_CONTEXT_WINDOW,
    PRODEX_GEMINI_DEFAULT_MODEL as SUPER_GEMINI_DEFAULT_MODEL,
    PRODEX_GEMINI_PROVIDER_ID as SUPER_GEMINI_PROVIDER_ID,
    PRODEX_KIRO_DEFAULT_AUTO_COMPACT_LIMIT as SUPER_KIRO_DEFAULT_AUTO_COMPACT_LIMIT,
    PRODEX_KIRO_DEFAULT_CONTEXT_WINDOW as SUPER_KIRO_DEFAULT_CONTEXT_WINDOW,
    PRODEX_KIRO_DEFAULT_MODEL as SUPER_KIRO_DEFAULT_MODEL,
    PRODEX_KIRO_PROVIDER_ID as SUPER_KIRO_PROVIDER_ID,
    PRODEX_LOCAL_DEFAULT_AUTO_COMPACT_LIMIT as SUPER_DEFAULT_AUTO_COMPACT_LIMIT,
    PRODEX_LOCAL_DEFAULT_CONTEXT_WINDOW as SUPER_DEFAULT_CONTEXT_WINDOW,
    PRODEX_LOCAL_DEFAULT_MODEL as SUPER_DEFAULT_LOCAL_MODEL,
    PRODEX_LOCAL_PROVIDER_ID as SUPER_LOCAL_PROVIDER_ID,
};

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
        self.provider_id().label()
    }

    pub fn model_provider_id(self) -> &'static str {
        self.metadata().model_provider_id
    }

    pub const fn provider_id(self) -> ProviderId {
        match self {
            Self::Anthropic => ProviderId::Anthropic,
            Self::Copilot => ProviderId::Copilot,
            Self::DeepSeek => ProviderId::DeepSeek,
            Self::Gemini => ProviderId::Gemini,
            Self::Kiro => ProviderId::Kiro,
        }
    }

    pub const fn from_provider_id(provider: ProviderId) -> Option<Self> {
        match provider {
            ProviderId::Anthropic => Some(Self::Anthropic),
            ProviderId::Copilot => Some(Self::Copilot),
            ProviderId::DeepSeek => Some(Self::DeepSeek),
            ProviderId::Gemini => Some(Self::Gemini),
            ProviderId::Kiro => Some(Self::Kiro),
            ProviderId::OpenAi | ProviderId::Local => None,
        }
    }

    fn metadata(self) -> &'static ProviderRuntimeMetadata {
        provider_runtime_metadata(self.provider_id())
            .expect("external provider runtime metadata should exist")
    }

    fn codex_provider_name(self) -> &'static str {
        self.metadata().codex_provider_name
    }

    fn default_model(self) -> &'static str {
        self.metadata().default_model
    }

    pub fn default_base_url(self) -> &'static str {
        self.metadata()
            .default_base_url
            .expect("external provider default base URL should exist")
    }

    fn default_context_window(self) -> usize {
        self.metadata().default_context_window
    }

    fn default_auto_compact_token_limit(self) -> usize {
        self.metadata().default_auto_compact_token_limit
    }

    fn web_search_mode(self) -> &'static str {
        self.metadata().web_search_mode
    }

    fn image_generation_enabled(self) -> bool {
        self.metadata().image_generation_enabled
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
            toml_string_literal(prodex_provider_core::PRODEX_LOCAL_PROVIDER_NAME)
        ),
        format!(
            "model_providers.{SUPER_LOCAL_PROVIDER_ID}.base_url={}",
            toml_string_literal(&base_url)
        ),
        format!("model_providers.{SUPER_LOCAL_PROVIDER_ID}.wire_api=\"responses\""),
        format!("model_providers.{SUPER_LOCAL_PROVIDER_ID}.requires_openai_auth=true"),
        format!("model_providers.{SUPER_LOCAL_PROVIDER_ID}.supports_websockets=false"),
        format!("model_context_window={context_window}"),
        format!("model_auto_compact_token_limit={auto_compact_token_limit}"),
        "model_reasoning_summary=\"none\"".to_string(),
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
        format!("model_providers.{provider_id}.requires_openai_auth=true"),
        format!("model_providers.{provider_id}.supports_websockets=false"),
        format!("model_context_window={context_window}"),
        format!("model_auto_compact_token_limit={auto_compact_token_limit}"),
        "model_reasoning_summary=\"none\"".to_string(),
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
    if let Ok(mut parsed) = url::Url::parse(url) {
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

pub fn parse_super_local_url(url: &str) -> std::result::Result<String, String> {
    parse_credential_free_http_url(url, "--url")?;
    Ok(url.to_string())
}

pub(crate) fn parse_credential_free_http_url(
    url: &str,
    option: &str,
) -> std::result::Result<url::Url, String> {
    let invalid = || {
        format!(
            "invalid {option}: expected an absolute http(s) URL with host and no credentials, \
         query, or fragment"
        )
    };
    let parsed = url::Url::parse(url).map_err(|_| invalid())?;
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
#[path = "runtime_args_tests.rs"]
mod tests;
