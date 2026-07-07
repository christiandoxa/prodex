use super::{
    COPILOT_TEXT_ENDPOINTS, CORE_TEXT_ENDPOINTS, GEMINI_ENDPOINTS, OPENAI_ENDPOINTS,
    PRODEX_GEMINI_CHAT_COMPRESSION_MODEL, PRODEX_KIRO_DEFAULT_MODEL, ProviderId, ProviderModelCost,
    ProviderModelSpec,
};

const OPENAI_CONTEXT_WINDOW_TOKENS: u64 = 400_000;
const OPENAI_CODEX_SPARK_CONTEXT_WINDOW_TOKENS: u64 = 128_000;
const ANTHROPIC_CONTEXT_WINDOW_TOKENS: u64 = 200_000;
const COPILOT_OPENAI_CONTEXT_WINDOW_TOKENS: u64 = 400_000;
const COPILOT_EXTENDED_CONTEXT_WINDOW_TOKENS: u64 = 1_000_000;
const COPILOT_ANTHROPIC_CONTEXT_WINDOW_TOKENS: u64 = 200_000;
const COPILOT_GEMINI_CONTEXT_WINDOW_TOKENS: u64 = 1_048_576;
const DEEPSEEK_CONTEXT_WINDOW_TOKENS: u64 = 128_000;
const GEMINI_CONTEXT_WINDOW_TOKENS: u64 = 1_048_576;
const KIRO_CONTEXT_WINDOW_TOKENS: u64 = 200_000;

macro_rules! model {
    ($provider:expr, $owned_by:expr, $id:expr, $display:expr, $description:expr, $ctx:expr, $in_cost:expr, $out_cost:expr, $endpoints:expr, [$($alias:expr),* $(,)?]) => {
        ProviderModelSpec {
            id: $id,
            display_name: $display,
            description: $description,
            provider: $provider,
            owned_by: $owned_by,
            context_window_tokens: $ctx,
            input_cost_per_million_microusd: $in_cost,
            output_cost_per_million_microusd: $out_cost,
            endpoints: $endpoints,
            aliases: &[$($alias),*],
        }
    };
}

const OPENAI_MODELS: &[ProviderModelSpec] = &[
    model!(
        ProviderId::OpenAi,
        "openai",
        "gpt-5.4",
        "GPT-5.4",
        "Frontier agentic coding model routed through OpenAI Responses.",
        Some(OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        OPENAI_ENDPOINTS,
        ["opus", "best", "default"]
    ),
    model!(
        ProviderId::OpenAi,
        "openai",
        "gpt-5.4-mini",
        "GPT-5.4 Mini",
        "Smaller frontier agentic coding model routed through OpenAI Responses.",
        Some(OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        OPENAI_ENDPOINTS,
        ["haiku", "mini"]
    ),
    model!(
        ProviderId::OpenAi,
        "openai",
        "gpt-5.3-codex",
        "GPT-5.3 Codex",
        "Codex-optimized agentic coding model routed through OpenAI Responses.",
        Some(OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        OPENAI_ENDPOINTS,
        ["codex", "sonnet"]
    ),
    model!(
        ProviderId::OpenAi,
        "openai",
        "gpt-5.3-codex-spark",
        "GPT-5.3 Codex Spark",
        "Spark-tier Codex-optimized coding model routed through OpenAI Responses.",
        Some(OPENAI_CODEX_SPARK_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        OPENAI_ENDPOINTS,
        ["spark"]
    ),
    model!(
        ProviderId::OpenAi,
        "openai",
        "gpt-5.2",
        "GPT-5.2",
        "General frontier model routed through OpenAI Responses.",
        Some(OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        OPENAI_ENDPOINTS,
        []
    ),
];

const ANTHROPIC_MODELS: &[ProviderModelSpec] = &[
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "auto",
        "Anthropic Auto",
        "Anthropic alias routed through provider fallback.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        ["default"]
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "opus",
        "Claude Opus",
        "Anthropic Opus alias routed through provider fallback.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        ["best"]
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "sonnet",
        "Claude Sonnet",
        "Anthropic Sonnet alias routed through provider fallback.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        ["pro"]
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "haiku",
        "Claude Haiku",
        "Anthropic Haiku alias routed through provider fallback.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        ["flash"]
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "claude-opus-4-8",
        "Claude Opus 4.8",
        "Anthropic model exposed through Prodex.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "claude-sonnet-4-6",
        "Claude Sonnet 4.6",
        "Anthropic model exposed through Prodex.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "claude-haiku-4-5",
        "Claude Haiku 4.5",
        "Anthropic model exposed through Prodex.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "claude-opus-4-6",
        "Claude Opus 4.6",
        "Anthropic compatibility model exposed through Prodex.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Anthropic,
        "anthropic",
        "claude-opus-4-20250514",
        "Claude Opus 4",
        "Anthropic compatibility model exposed through Prodex.",
        Some(ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
];

const COPILOT_MODELS: &[ProviderModelSpec] = &[
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "auto",
        "Copilot Auto",
        "GitHub Copilot alias routed through provider fallback.",
        Some(COPILOT_EXTENDED_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        ["default"]
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "codex",
        "Copilot Codex",
        "GitHub Copilot Codex alias routed through provider fallback.",
        Some(COPILOT_EXTENDED_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        ["pro"]
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "gpt-5.3-codex",
        "GPT-5.3 Codex",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_EXTENDED_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "gpt-5.5",
        "GPT-5.5",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_EXTENDED_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        ["best"]
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "gpt-5.4",
        "GPT-5.4",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_EXTENDED_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "gpt-5.4-mini",
        "GPT-5.4 Mini",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        ["mini"]
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "gpt-5.4-nano",
        "GPT-5.4 Nano",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        ["nano"]
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "gpt-5-mini",
        "GPT-5 Mini",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "claude-sonnet-4-6",
        "Claude Sonnet 4.6",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_EXTENDED_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        ["claude", "sonnet"]
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "claude-opus-4-8",
        "Claude Opus 4.8",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_EXTENDED_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        ["opus"]
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "claude-opus-4-7",
        "Claude Opus 4.7",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_EXTENDED_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "claude-opus-4-6-fast",
        "Claude Opus 4.6 Fast",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_EXTENDED_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "claude-opus-4-6",
        "Claude Opus 4.6",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_EXTENDED_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "claude-opus-4-5",
        "Claude Opus 4.5",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "claude-fable-5",
        "Claude Fable 5",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_EXTENDED_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "claude-sonnet-4-5",
        "Claude Sonnet 4.5",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "claude-haiku-4-5",
        "Claude Haiku 4.5",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_ANTHROPIC_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        ["haiku"]
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "gemini-3.1-pro-preview",
        "Gemini 3.1 Pro Preview",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        ["gemini"]
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "gemini-2.5-pro",
        "Gemini 2.5 Pro",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "gemini-3-flash",
        "Gemini 3 Flash",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        ["flash"]
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "gemini-3.5-flash",
        "Gemini 3.5 Flash",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "mai-code-1-flash",
        "MAI-Code-1-Flash",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "raptor-mini",
        "Raptor Mini",
        "GitHub Copilot model exposed through Prodex.",
        Some(COPILOT_OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "gpt-5.1-codex",
        "GPT-5.1 Codex",
        "Legacy GitHub Copilot model kept for compatibility.",
        Some(COPILOT_OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Copilot,
        "github-copilot",
        "gpt-4o",
        "GPT-4o",
        "Legacy GitHub Copilot model kept as a safe fallback.",
        Some(COPILOT_OPENAI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        COPILOT_TEXT_ENDPOINTS,
        []
    ),
];
const DEEPSEEK_MODELS: &[ProviderModelSpec] = &[
    model!(
        ProviderId::DeepSeek,
        "deepseek",
        "deepseek-v4-pro",
        "DeepSeek V4 Pro",
        "DeepSeek Pro alias routed through provider fallback.",
        Some(DEEPSEEK_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        ["pro", "auto"]
    ),
    model!(
        ProviderId::DeepSeek,
        "deepseek",
        "deepseek-v4-flash",
        "DeepSeek V4 Flash",
        "DeepSeek Flash alias routed through provider fallback.",
        Some(DEEPSEEK_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        ["flash"]
    ),
    model!(
        ProviderId::DeepSeek,
        "deepseek",
        "deepseek-chat",
        "DeepSeek Chat",
        "DeepSeek chat-completions model.",
        Some(DEEPSEEK_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::DeepSeek,
        "deepseek",
        "deepseek-reasoner",
        "DeepSeek Reasoner",
        "DeepSeek reasoning model.",
        Some(DEEPSEEK_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        CORE_TEXT_ENDPOINTS,
        []
    ),
];

const GEMINI_MODELS: &[ProviderModelSpec] = &[
    model!(
        ProviderId::Gemini,
        "google",
        "auto",
        "Gemini Auto",
        "Gemini CLI-style auto routing through preview, pro, and flash fallback models.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        ["default"]
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "auto-gemini-3",
        "Gemini 3 Auto",
        "Gemini CLI preview auto alias routed through Gemini 3 and stable fallback models.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "auto-gemini-2.5",
        "Gemini 2.5 Auto",
        "Gemini CLI stable auto alias routed through Gemini 2.5 Pro and Flash models.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "pro",
        "Gemini Pro",
        "Gemini Pro alias routed through preview and stable Pro models.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "flash",
        "Gemini Flash",
        "Gemini Flash alias routed through preview and stable Flash models.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "flash-lite",
        "Gemini Flash Lite",
        "Gemini Flash-Lite alias routed through Flash-Lite models.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        PRODEX_GEMINI_CHAT_COMPRESSION_MODEL,
        "Gemini Chat Compression",
        "Gemini CLI semantic compaction alias routed through chat-compression-default.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-3.1-pro-preview",
        "Gemini 3.1 Pro Preview",
        "Gemini CLI preview Pro model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-3.1-pro-preview-customtools",
        "Gemini 3.1 Pro Preview Custom Tools",
        "Gemini CLI preview Pro variant for custom tools routed through Prodex.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-3-pro-preview",
        "Gemini 3 Pro Preview",
        "Gemini CLI preview Pro model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-3-flash-preview",
        "Gemini 3 Flash Preview",
        "Gemini CLI preview Flash model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-3-flash",
        "Gemini 3 Flash",
        "Gemini CLI secondary Flash model name routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-3.5-flash",
        "Gemini 3.5 Flash",
        "Gemini CLI Flash model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-3.1-flash-lite",
        "Gemini 3.1 Flash Lite",
        "Gemini CLI Flash-Lite model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-2.5-pro",
        "Gemini 2.5 Pro",
        "Gemini CLI stable Pro model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-2.5-flash",
        "Gemini 2.5 Flash",
        "Gemini CLI stable Flash model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemini-2.5-flash-lite",
        "Gemini 2.5 Flash Lite",
        "Gemini CLI Flash-Lite model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemma-4-31b-it",
        "Gemma 4 31B IT",
        "Gemini CLI Gemma model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
    model!(
        ProviderId::Gemini,
        "google",
        "gemma-4-26b-a4b-it",
        "Gemma 4 26B A4B IT",
        "Gemini CLI Gemma model routed through the Prodex Responses adapter.",
        Some(GEMINI_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        GEMINI_ENDPOINTS,
        []
    ),
];

const LOCAL_MODELS: &[ProviderModelSpec] = &[model!(
    ProviderId::Local,
    "local",
    "local",
    "Local OpenAI Compatible",
    "Local OpenAI-compatible server default model.",
    None,
    None,
    None,
    OPENAI_ENDPOINTS,
    ["default"]
)];

const KIRO_MODELS: &[ProviderModelSpec] = &[
    model!(
        ProviderId::Kiro,
        "kiro-cli",
        "auto",
        "Kiro Auto",
        "Kiro alias routed through the imported runtime model selection.",
        Some(KIRO_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        super::KIRO_ENDPOINTS,
        ["default"]
    ),
    model!(
        ProviderId::Kiro,
        "kiro-cli",
        PRODEX_KIRO_DEFAULT_MODEL,
        "Claude Sonnet 4",
        "Kiro imported runtime compatibility model exposed through Prodex.",
        Some(KIRO_CONTEXT_WINDOW_TOKENS),
        None,
        None,
        super::KIRO_ENDPOINTS,
        ["sonnet", "claude"]
    ),
];

pub fn provider_model_catalog(provider: ProviderId) -> &'static [ProviderModelSpec] {
    match provider {
        ProviderId::OpenAi => OPENAI_MODELS,
        ProviderId::Anthropic => ANTHROPIC_MODELS,
        ProviderId::Copilot => COPILOT_MODELS,
        ProviderId::DeepSeek => DEEPSEEK_MODELS,
        ProviderId::Gemini => GEMINI_MODELS,
        ProviderId::Kiro => KIRO_MODELS,
        ProviderId::Local => LOCAL_MODELS,
    }
}

pub fn provider_model_spec(
    provider: ProviderId,
    model: &str,
) -> Option<&'static ProviderModelSpec> {
    let model = model.trim();
    provider_model_catalog(provider)
        .iter()
        .find(|spec| spec.matches_id_or_alias(model))
}

pub fn provider_model_cost(provider: ProviderId, model: &str) -> ProviderModelCost {
    provider_model_spec(provider, model)
        .map(|spec| spec.cost())
        .unwrap_or_default()
}
