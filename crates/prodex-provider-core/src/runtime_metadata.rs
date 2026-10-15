use std::sync::OnceLock;

use crate::ProviderId;

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
pub const PRODEX_ANTHROPIC_DEFAULT_CONTEXT_WINDOW: usize = 1_000_000;
pub const PRODEX_ANTHROPIC_DEFAULT_AUTO_COMPACT_LIMIT: usize = 950_000;

pub const PRODEX_COPILOT_PROVIDER_ID: &str = "prodex-copilot";
pub const PRODEX_COPILOT_PROVIDER_NAME: &str = "GitHub Copilot";
pub const PRODEX_COPILOT_DEFAULT_BASE_URL: &str = "https://api.githubcopilot.com";
pub const PRODEX_COPILOT_DEFAULT_CONTEXT_WINDOW: usize = 1_050_000;
pub const PRODEX_COPILOT_DEFAULT_AUTO_COMPACT_LIMIT: usize = 997_500;

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

fn leaked(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn load_runtime_metadata(provider: ProviderId) -> ProviderRuntimeMetadata {
    let plan = prodex_mojo_core::provider_registry::runtime_metadata(provider as i64)
        .expect("Mojo provider runtime metadata policy failed");
    assert_eq!(
        plan.provider, provider as i64,
        "Mojo provider runtime metadata returned the wrong provider"
    );
    ProviderRuntimeMetadata {
        provider,
        model_provider_id: leaked(plan.model_provider_id),
        display_name: leaked(plan.display_name),
        codex_provider_name: leaked(plan.codex_provider_name),
        default_model: leaked(plan.default_model),
        default_base_url: plan.default_base_url.map(leaked),
        default_context_window: plan.default_context_window,
        default_auto_compact_token_limit: plan.default_auto_compact_token_limit,
        web_search_mode: leaked(plan.web_search_mode),
        image_generation_enabled: plan.image_generation_enabled,
    }
}

fn all_runtime_metadata() -> [Option<ProviderRuntimeMetadata>; 7] {
    let mut metadata = [None; 7];
    for provider in [
        ProviderId::Anthropic,
        ProviderId::Copilot,
        ProviderId::DeepSeek,
        ProviderId::Gemini,
        ProviderId::Kiro,
        ProviderId::Local,
    ] {
        metadata[provider as usize] = Some(load_runtime_metadata(provider));
    }
    metadata
}

pub fn provider_runtime_metadata(provider: ProviderId) -> Option<&'static ProviderRuntimeMetadata> {
    if provider == ProviderId::OpenAi {
        return None;
    }
    RUNTIME_METADATA
        .get_or_init(all_runtime_metadata)
        .get(provider as usize)
        .and_then(|metadata| metadata.as_ref())
}

static RUNTIME_METADATA: OnceLock<[Option<ProviderRuntimeMetadata>; 7]> = OnceLock::new();

pub fn copilot_prompt_token_limit_for_model(model: &str) -> Option<usize> {
    prodex_mojo_core::provider_constraints::provider_copilot_prompt_token_limit(model)
        .expect("Mojo Copilot prompt-token policy failed")
        .or_else(|| {
            crate::provider_catalog_entry(ProviderId::Copilot, model)
                .and_then(|entry| entry.context_window_tokens)
                .and_then(|tokens| usize::try_from(tokens).ok())
        })
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
    fn runtime_defaults_match_canonical_catalog_and_fallback_heads() {
        for provider in [ProviderId::Anthropic, ProviderId::Copilot] {
            let metadata = provider_runtime_metadata(provider).expect("runtime metadata");
            let catalog = crate::provider_catalog_entry(provider, metadata.default_model)
                .expect("default model must exist in canonical catalog");
            assert_eq!(
                catalog.context_window_tokens,
                Some(metadata.default_context_window as u64),
                "{} default context drift",
                provider.label()
            );
            let fallback = crate::provider_model_fallback_chain(provider, "");
            assert_eq!(
                fallback.first().map(String::as_str),
                Some(metadata.default_model),
                "{} default model drift",
                provider.label()
            );
            assert!(metadata.default_auto_compact_token_limit < metadata.default_context_window);
        }
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
            copilot_prompt_token_limit_for_model("gpt-6-astra"),
            Some(1_050_000)
        );
        assert_eq!(
            copilot_prompt_token_limit_for_model(" GPT-6.1-SOL "),
            Some(1_050_000)
        );
        assert_eq!(
            copilot_prompt_token_limit_for_model("gpt-5.6-sol"),
            Some(272_000)
        );
        assert_eq!(
            copilot_prompt_token_limit_for_model("gpt-5.6-terra"),
            Some(272_000)
        );
        assert_eq!(
            copilot_prompt_token_limit_for_model("gpt-5.4"),
            Some(922_000)
        );
        assert_eq!(
            copilot_prompt_token_limit_for_model(" CLAUDE-OPUS-4.8 "),
            Some(936_000)
        );
        assert_eq!(
            copilot_prompt_token_limit_for_model("gpt-5.4-NANO"),
            Some(128_000)
        );
        assert_eq!(copilot_prompt_token_limit_for_model("unknown"), None);
    }
}
