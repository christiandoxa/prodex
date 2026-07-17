use crate::{
    PRODEX_ANTHROPIC_DEFAULT_MODEL, PRODEX_COPILOT_DEFAULT_MODEL, PRODEX_GEMINI_DEFAULT_MODEL,
    PRODEX_KIRO_DEFAULT_MODEL, ProviderId,
};

pub const PRODEX_LOCAL_PROVIDER_ID: &str = "prodex-local";
pub const PRODEX_LOCAL_PROVIDER_NAME: &str = "Prodex Local";
pub const PRODEX_LOCAL_DEFAULT_MODEL: &str = "unsloth/qwen3.5-35b-a3b";
pub const PRODEX_LOCAL_DEFAULT_CONTEXT_WINDOW: usize = 16_384;
pub const PRODEX_LOCAL_DEFAULT_AUTO_COMPACT_LIMIT: usize = 14_000;

pub const PRODEX_DEEPSEEK_PROVIDER_ID: &str = "prodex-deepseek";
pub const PRODEX_DEEPSEEK_PROVIDER_NAME: &str = "DeepSeek";
pub const PRODEX_DEEPSEEK_DEFAULT_MODEL: &str = "deepseek-v4-pro";
pub const PRODEX_DEEPSEEK_DEFAULT_BASE_URL: &str = "https://api.deepseek.com";
pub const PRODEX_DEEPSEEK_DEFAULT_CONTEXT_WINDOW: usize = 1_048_576;
pub const PRODEX_DEEPSEEK_DEFAULT_AUTO_COMPACT_LIMIT: usize = 900_000;

pub const PRODEX_GEMINI_PROVIDER_ID: &str = "prodex-gemini";
pub const PRODEX_GEMINI_PROVIDER_NAME: &str = "Google Gemini";
pub const PRODEX_GEMINI_DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";
pub const PRODEX_GEMINI_DEFAULT_CONTEXT_WINDOW: usize = 1_048_576;
pub const PRODEX_GEMINI_DEFAULT_AUTO_COMPACT_LIMIT: usize = 900_000;

pub const PRODEX_ANTHROPIC_PROVIDER_ID: &str = "prodex-anthropic";
pub const PRODEX_ANTHROPIC_PROVIDER_NAME: &str = "Anthropic Claude";
pub const PRODEX_ANTHROPIC_DEFAULT_BASE_URL: &str = "https://api.anthropic.com/v1";
pub const PRODEX_ANTHROPIC_DEFAULT_CONTEXT_WINDOW: usize = 200_000;
pub const PRODEX_ANTHROPIC_DEFAULT_AUTO_COMPACT_LIMIT: usize = 180_000;

pub const PRODEX_COPILOT_PROVIDER_ID: &str = "prodex-copilot";
pub const PRODEX_COPILOT_PROVIDER_NAME: &str = "GitHub Copilot";
pub const PRODEX_COPILOT_DEFAULT_BASE_URL: &str = "https://api.githubcopilot.com";
pub const PRODEX_COPILOT_DEFAULT_CONTEXT_WINDOW: usize = 272_000;
pub const PRODEX_COPILOT_DEFAULT_AUTO_COMPACT_LIMIT: usize = 258_400;

pub const PRODEX_KIRO_PROVIDER_ID: &str = "prodex-kiro";
pub const PRODEX_KIRO_PROVIDER_NAME: &str = "Kiro";
pub const PRODEX_KIRO_DEFAULT_BASE_URL: &str = "https://kiro.dev";
pub const PRODEX_KIRO_DEFAULT_CONTEXT_WINDOW: usize = 1_000_000;
pub const PRODEX_KIRO_DEFAULT_AUTO_COMPACT_LIMIT: usize = 950_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderRuntimeMetadata {
    pub provider: ProviderId,
    pub model_provider_id: &'static str,
    pub display_name: &'static str,
    pub codex_provider_name: &'static str,
    pub default_model: &'static str,
    pub default_base_url: Option<&'static str>,
    pub default_context_window: usize,
    pub default_auto_compact_token_limit: usize,
    pub web_search_mode: &'static str,
    pub image_generation_enabled: bool,
}

pub(crate) const LOCAL_RUNTIME_METADATA: ProviderRuntimeMetadata = ProviderRuntimeMetadata {
    provider: ProviderId::Local,
    model_provider_id: PRODEX_LOCAL_PROVIDER_ID,
    display_name: PRODEX_LOCAL_PROVIDER_NAME,
    codex_provider_name: PRODEX_LOCAL_PROVIDER_NAME,
    default_model: PRODEX_LOCAL_DEFAULT_MODEL,
    default_base_url: None,
    default_context_window: PRODEX_LOCAL_DEFAULT_CONTEXT_WINDOW,
    default_auto_compact_token_limit: PRODEX_LOCAL_DEFAULT_AUTO_COMPACT_LIMIT,
    web_search_mode: "disabled",
    image_generation_enabled: false,
};

pub(crate) const DEEPSEEK_RUNTIME_METADATA: ProviderRuntimeMetadata = ProviderRuntimeMetadata {
    provider: ProviderId::DeepSeek,
    model_provider_id: PRODEX_DEEPSEEK_PROVIDER_ID,
    display_name: PRODEX_DEEPSEEK_PROVIDER_NAME,
    codex_provider_name: PRODEX_DEEPSEEK_PROVIDER_NAME,
    default_model: PRODEX_DEEPSEEK_DEFAULT_MODEL,
    default_base_url: Some(PRODEX_DEEPSEEK_DEFAULT_BASE_URL),
    default_context_window: PRODEX_DEEPSEEK_DEFAULT_CONTEXT_WINDOW,
    default_auto_compact_token_limit: PRODEX_DEEPSEEK_DEFAULT_AUTO_COMPACT_LIMIT,
    web_search_mode: "live",
    image_generation_enabled: false,
};

pub(crate) const GEMINI_RUNTIME_METADATA: ProviderRuntimeMetadata = ProviderRuntimeMetadata {
    provider: ProviderId::Gemini,
    model_provider_id: PRODEX_GEMINI_PROVIDER_ID,
    display_name: PRODEX_GEMINI_PROVIDER_NAME,
    codex_provider_name: "Azure",
    default_model: PRODEX_GEMINI_DEFAULT_MODEL,
    default_base_url: Some(PRODEX_GEMINI_DEFAULT_BASE_URL),
    default_context_window: PRODEX_GEMINI_DEFAULT_CONTEXT_WINDOW,
    default_auto_compact_token_limit: PRODEX_GEMINI_DEFAULT_AUTO_COMPACT_LIMIT,
    web_search_mode: "live",
    image_generation_enabled: true,
};

pub(crate) const ANTHROPIC_RUNTIME_METADATA: ProviderRuntimeMetadata = ProviderRuntimeMetadata {
    provider: ProviderId::Anthropic,
    model_provider_id: PRODEX_ANTHROPIC_PROVIDER_ID,
    display_name: PRODEX_ANTHROPIC_PROVIDER_NAME,
    codex_provider_name: PRODEX_ANTHROPIC_PROVIDER_NAME,
    default_model: PRODEX_ANTHROPIC_DEFAULT_MODEL,
    default_base_url: Some(PRODEX_ANTHROPIC_DEFAULT_BASE_URL),
    default_context_window: PRODEX_ANTHROPIC_DEFAULT_CONTEXT_WINDOW,
    default_auto_compact_token_limit: PRODEX_ANTHROPIC_DEFAULT_AUTO_COMPACT_LIMIT,
    web_search_mode: "live",
    image_generation_enabled: false,
};

pub(crate) const COPILOT_RUNTIME_METADATA: ProviderRuntimeMetadata = ProviderRuntimeMetadata {
    provider: ProviderId::Copilot,
    model_provider_id: PRODEX_COPILOT_PROVIDER_ID,
    display_name: PRODEX_COPILOT_PROVIDER_NAME,
    codex_provider_name: "OpenAI",
    default_model: PRODEX_COPILOT_DEFAULT_MODEL,
    default_base_url: Some(PRODEX_COPILOT_DEFAULT_BASE_URL),
    default_context_window: PRODEX_COPILOT_DEFAULT_CONTEXT_WINDOW,
    default_auto_compact_token_limit: PRODEX_COPILOT_DEFAULT_AUTO_COMPACT_LIMIT,
    web_search_mode: "live",
    image_generation_enabled: false,
};

pub(crate) const KIRO_RUNTIME_METADATA: ProviderRuntimeMetadata = ProviderRuntimeMetadata {
    provider: ProviderId::Kiro,
    model_provider_id: PRODEX_KIRO_PROVIDER_ID,
    display_name: PRODEX_KIRO_PROVIDER_NAME,
    codex_provider_name: "Azure",
    default_model: PRODEX_KIRO_DEFAULT_MODEL,
    default_base_url: Some(PRODEX_KIRO_DEFAULT_BASE_URL),
    default_context_window: PRODEX_KIRO_DEFAULT_CONTEXT_WINDOW,
    default_auto_compact_token_limit: PRODEX_KIRO_DEFAULT_AUTO_COMPACT_LIMIT,
    web_search_mode: "live",
    image_generation_enabled: false,
};

pub const fn provider_runtime_metadata(
    provider: ProviderId,
) -> Option<&'static ProviderRuntimeMetadata> {
    crate::implementation_registry::builtin_provider_runtime_metadata(provider)
}

pub fn copilot_prompt_token_limit_for_model(model: &str) -> Option<usize> {
    let model = model.trim().to_ascii_lowercase();
    match model.as_str() {
        "auto" | "codex" | "gpt-5.3-codex" | "gpt-5.1-codex" | "gpt-5.1-codex-max"
        | "gpt-5.1-codex-mini" => Some(272_000),
        "gpt-5.5" | "gpt-5.4" => Some(922_000),
        "claude-sonnet-4.6"
        | "claude-opus-4.8"
        | "claude-opus-4.7"
        | "claude-opus-4.6"
        | "gemini-3.1-pro-preview"
        | "gemini-3.5-flash" => Some(936_000),
        "gpt-5-mini" | "gpt-5.4-mini" | "gpt-5.4-nano" | "raptor-mini" => Some(128_000),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_metadata_covers_every_prodex_provider() {
        for provider in [
            ProviderId::Anthropic,
            ProviderId::Copilot,
            ProviderId::DeepSeek,
            ProviderId::Gemini,
            ProviderId::Kiro,
            ProviderId::Local,
        ] {
            let metadata = provider_runtime_metadata(provider).expect("metadata should exist");
            assert_eq!(metadata.provider, provider);
            assert!(!metadata.model_provider_id.is_empty());
            assert!(!metadata.default_model.is_empty());
        }
        assert_eq!(provider_runtime_metadata(ProviderId::OpenAi), None);
    }

    #[test]
    fn codex_provider_names_preserve_remote_compaction_compatibility() {
        assert_eq!(
            provider_runtime_metadata(ProviderId::Gemini)
                .map(|metadata| metadata.codex_provider_name),
            Some("Azure")
        );
        assert_eq!(
            provider_runtime_metadata(ProviderId::Copilot)
                .map(|metadata| metadata.codex_provider_name),
            Some("OpenAI")
        );
    }

    #[test]
    fn copilot_prompt_limits_are_model_specific() {
        assert_eq!(
            copilot_prompt_token_limit_for_model("gpt-5.3-codex"),
            Some(272_000)
        );
        assert_eq!(
            copilot_prompt_token_limit_for_model("gpt-5.4"),
            Some(922_000)
        );
    }
}
