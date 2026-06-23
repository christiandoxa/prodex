use clap::{ArgGroup, Args};
use std::ffi::OsString;
use std::path::PathBuf;

pub const SUPER_OPTIMIZER_PREFIXES: [&str; 3] = ["sqz", "tokensavior", "clawcompactor"];

#[derive(Args, Debug)]
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
    /// Arguments passed through to `codex`. A lone session id is normalized to `codex resume <session-id>`.
    #[arg(value_name = "CODEX_ARG", allow_hyphen_values = true)]
    pub codex_args: Vec<OsString>,
}

#[derive(Args, Debug)]
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

#[derive(Args, Debug)]
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
    /// Local memory backend selected by a higher-level launch shortcut.
    #[arg(skip)]
    pub memory_backend: SuperMemoryBackend,
    /// Arguments passed through to `codex`. A lone session id is normalized to `codex resume <session-id>`.
    #[arg(value_name = "CODEX_ARG", allow_hyphen_values = true)]
    pub codex_args: Vec<OsString>,
}

#[derive(Args, Debug)]
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
    /// Enable prodex-memory with the managed Mem0 OSS Docker backend.
    #[arg(long, conflicts_with = "no_mem0")]
    pub mem0: bool,
    /// Skip the Mem0 prompt and leave prodex-memory disabled for Super.
    #[arg(long, conflicts_with = "mem0")]
    pub no_mem0: bool,
    /// Route Codex directly to a local OpenAI-compatible /v1 endpoint.
    #[arg(
        long,
        value_name = "URL",
        value_parser = parse_super_local_url,
        conflicts_with = "provider"
    )]
    pub url: Option<String>,
    /// External provider preset to use through Codex/Super.
    #[arg(long, value_name = "PROVIDER", value_parser = parse_super_external_provider)]
    pub provider: Option<SuperExternalProvider>,
    /// Agent CLI to launch. Gemini CLI requires the Gemini provider.
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
    /// Arguments passed through to `codex` after the implied optimizer prefixes.
    #[arg(value_name = "CODEX_ARG", allow_hyphen_values = true)]
    pub codex_args: Vec<OsString>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SuperMemoryBackend {
    #[default]
    Sqlite,
    Mem0,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperCliAgent {
    Codex,
    Gemini,
    Agy,
}

#[derive(Args, Debug)]
pub struct GeminiCompatRefreshArgs {
    /// CODEX_HOME to refresh Gemini CLI compatibility surfaces into.
    #[arg(long, value_name = "PATH")]
    pub codex_home: PathBuf,
}

#[derive(Args, Debug)]
pub struct MemoryMcpArgs {
    /// SQLite store path for local Prodex memory.
    #[arg(long, value_name = "PATH")]
    pub store: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct InspectMcpArgs {}

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
    #[arg(long, default_value_t = 4)]
    pub max_clients: usize,
    /// Serve only on loopback and do not launch cloudflared.
    #[arg(long)]
    pub no_tunnel: bool,
}

#[derive(Args, Debug)]
pub struct GatewayArgs {
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

impl SuperArgs {
    pub fn presidio_preference(&self) -> Option<bool> {
        if self.presidio {
            Some(true)
        } else if self.no_presidio {
            Some(false)
        } else {
            None
        }
    }

    pub fn mem0_preference(&self) -> Option<bool> {
        if self.mem0 {
            Some(true)
        } else if self.no_mem0 {
            Some(false)
        } else {
            None
        }
    }

    pub fn into_caveman_args(self) -> CavemanArgs {
        self.into_caveman_args_with_choices(false, false)
    }

    pub fn into_caveman_args_with_presidio(self, presidio: bool) -> CavemanArgs {
        self.into_caveman_args_with_choices(presidio, false)
    }

    pub fn into_caveman_args_with_choices(self, presidio: bool, mem0: bool) -> CavemanArgs {
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

        let mut codex_args = Vec::with_capacity(
            self.codex_args.len()
                + 1
                + SUPER_OPTIMIZER_PREFIXES.len()
                + usize::from(presidio)
                + usize::from(mem0)
                + local_provider_args.len()
                + external_provider_args.len(),
        );
        codex_args.push(OsString::from("rtk"));
        codex_args.extend(SUPER_OPTIMIZER_PREFIXES.iter().map(OsString::from));
        if mem0 {
            codex_args.push(OsString::from("mem"));
        }
        if presidio {
            codex_args.push(OsString::from("presidio"));
        }
        codex_args.extend(local_provider_args);
        codex_args.extend(external_provider_args);
        codex_args.extend(self.codex_args);
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
            memory_backend: if mem0 {
                SuperMemoryBackend::Mem0
            } else {
                SuperMemoryBackend::Sqlite
            },
            codex_args,
        }
    }
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
pub const SUPER_COPILOT_DEFAULT_MODEL: &str = "gpt-5.1-codex";
const SUPER_COPILOT_DEFAULT_BASE_URL: &str = "https://api.githubcopilot.com";
pub const SUPER_COPILOT_DEFAULT_CONTEXT_WINDOW: usize = 200_000;
pub const SUPER_COPILOT_DEFAULT_AUTO_COMPACT_LIMIT: usize = 180_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperExternalProvider {
    Anthropic,
    Copilot,
    DeepSeek,
    Gemini,
}

impl SuperExternalProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::Copilot => "copilot",
            Self::DeepSeek => "deepseek",
            Self::Gemini => "gemini",
        }
    }

    pub fn model_provider_id(self) -> &'static str {
        match self {
            Self::Anthropic => SUPER_ANTHROPIC_PROVIDER_ID,
            Self::Copilot => SUPER_COPILOT_PROVIDER_ID,
            Self::DeepSeek => SUPER_DEEPSEEK_PROVIDER_ID,
            Self::Gemini => SUPER_GEMINI_PROVIDER_ID,
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::Anthropic => SUPER_ANTHROPIC_PROVIDER_NAME,
            Self::Copilot => SUPER_COPILOT_PROVIDER_NAME,
            Self::DeepSeek => SUPER_DEEPSEEK_PROVIDER_NAME,
            Self::Gemini => SUPER_GEMINI_PROVIDER_NAME,
        }
    }

    fn codex_provider_name(self) -> &'static str {
        match self {
            // Codex currently exposes no configurable remote-compaction capability flag.
            // It enables /responses/compact only for provider names OpenAI or Azure.
            Self::Gemini => "Azure",
            _ => self.display_name(),
        }
    }

    fn default_model(self) -> &'static str {
        match self {
            Self::Anthropic => SUPER_ANTHROPIC_DEFAULT_MODEL,
            Self::Copilot => SUPER_COPILOT_DEFAULT_MODEL,
            Self::DeepSeek => SUPER_DEEPSEEK_DEFAULT_MODEL,
            Self::Gemini => SUPER_GEMINI_DEFAULT_MODEL,
        }
    }

    pub fn default_base_url(self) -> &'static str {
        match self {
            Self::Anthropic => SUPER_ANTHROPIC_DEFAULT_BASE_URL,
            Self::Copilot => SUPER_COPILOT_DEFAULT_BASE_URL,
            Self::DeepSeek => SUPER_DEEPSEEK_DEFAULT_BASE_URL,
            Self::Gemini => SUPER_GEMINI_DEFAULT_BASE_URL,
        }
    }

    fn default_context_window(self) -> usize {
        match self {
            Self::Anthropic => SUPER_ANTHROPIC_DEFAULT_CONTEXT_WINDOW,
            Self::Copilot => SUPER_COPILOT_DEFAULT_CONTEXT_WINDOW,
            Self::DeepSeek => SUPER_DEEPSEEK_DEFAULT_CONTEXT_WINDOW,
            Self::Gemini => SUPER_GEMINI_DEFAULT_CONTEXT_WINDOW,
        }
    }

    fn default_auto_compact_token_limit(self) -> usize {
        match self {
            Self::Anthropic => SUPER_ANTHROPIC_DEFAULT_AUTO_COMPACT_LIMIT,
            Self::Copilot => SUPER_COPILOT_DEFAULT_AUTO_COMPACT_LIMIT,
            Self::DeepSeek => SUPER_DEEPSEEK_DEFAULT_AUTO_COMPACT_LIMIT,
            Self::Gemini => SUPER_GEMINI_DEFAULT_AUTO_COMPACT_LIMIT,
        }
    }

    fn web_search_mode(self) -> &'static str {
        match self {
            Self::Anthropic | Self::Copilot | Self::DeepSeek | Self::Gemini => "live",
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
        other => Err(format!(
            "invalid --provider: supported values are anthropic, copilot, deepseek, gemini, got {other:?}"
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
    let context_window = context_window
        .filter(|value| *value > 1)
        .unwrap_or_else(|| provider.default_context_window());
    let auto_compact_token_limit = auto_compact_token_limit
        .filter(|value| *value > 0)
        .unwrap_or_else(|| provider.default_auto_compact_token_limit())
        .min(context_window.saturating_sub(1));
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

fn super_external_provider_base_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
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
