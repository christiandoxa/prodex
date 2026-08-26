//! Provider model fallback chains and canonical model helpers.

#[path = "chains/gemini.rs"]
mod gemini;

use self::gemini::provider_gemini_code_assist_model_allowed;
#[cfg(any(not(feature = "mojo"), test))]
use self::gemini::provider_gemini_model_fallback_alias_chain;
#[cfg(all(feature = "mojo", not(test)))]
use crate::ProviderId;
#[cfg(any(not(feature = "mojo"), test))]
use crate::{
    PRODEX_ANTHROPIC_DEFAULT_MODEL, PRODEX_COPILOT_DEFAULT_MODEL, PRODEX_KIRO_DEFAULT_MODEL,
    ProviderId,
};

#[cfg(feature = "mojo")]
pub fn provider_model_fallback_chain(provider: ProviderId, model: &str) -> Vec<String> {
    prodex_mojo_core::rich::model_fallback_chain(provider.label(), model)
        .expect("Mojo model fallback parser returned an invalid structured result")
}

#[cfg(any(not(feature = "mojo"), test))]
fn provider_model_fallback_chain_rust(provider: ProviderId, model: &str) -> Vec<String> {
    let model = model.trim();
    if let Some(chain) = combo_chain(model) {
        return chain;
    }
    let lower = model.to_ascii_lowercase();
    let chain: &[&str] = match provider {
        ProviderId::Anthropic => match lower.as_str() {
            "" | "auto" | "default" => &[
                PRODEX_ANTHROPIC_DEFAULT_MODEL,
                "claude-opus-4-8",
                "claude-haiku-4-5",
            ],
            "opus" | "best" => &["claude-opus-4-8", "claude-sonnet-4-6"],
            "sonnet" | "pro" => &["claude-sonnet-4-6", "claude-opus-4-8"],
            "haiku" | "flash" => &["claude-haiku-4-5", "claude-sonnet-4-6"],
            _ => return non_empty_single(model),
        },
        ProviderId::Copilot => match lower.as_str() {
            "" | "auto" | "default" => &[PRODEX_COPILOT_DEFAULT_MODEL, "gpt-5.1-codex", "gpt-4o"],
            "codex" | "pro" => &["gpt-5.3-codex", "gpt-5.1-codex", "gpt-4o"],
            "gpt-5.5" => &["gpt-5.5", "gpt-5.3-codex", "gpt-5.1-codex", "gpt-4o"],
            "gpt-5.4" => &["gpt-5.4", "gpt-5.3-codex", "gpt-5.1-codex", "gpt-4o"],
            "gpt-5.3-codex" => &["gpt-5.3-codex", "gpt-5.1-codex", "gpt-4o"],
            "claude" | "sonnet" => &["claude-sonnet-4-6", "gpt-5.3-codex", "gpt-5.1-codex"],
            "gemini" => &["gemini-3.1-pro-preview", "gpt-5.3-codex", "gpt-5.1-codex"],
            _ => return non_empty_single(model),
        },
        ProviderId::Gemini => match provider_gemini_model_fallback_alias_chain(&lower) {
            Some(chain) => chain,
            None => return non_empty_single(model),
        },
        ProviderId::DeepSeek => match lower.as_str() {
            "" | "auto" => &["deepseek-v4-pro", "deepseek-v4-flash"],
            "pro" => &["deepseek-v4-pro", "deepseek-v4-flash"],
            "flash" => &["deepseek-v4-flash", "deepseek-v4-pro"],
            _ => return non_empty_single(model),
        },
        ProviderId::Kiro => match lower.as_str() {
            "" | "auto" | "default" | "claude" | "sonnet" => &[PRODEX_KIRO_DEFAULT_MODEL],
            _ => return non_empty_single(model),
        },
        ProviderId::OpenAi | ProviderId::Local => return non_empty_single(model),
    };
    dedup_chain(chain.iter().map(|value| (*value).to_string()).collect())
}

#[cfg(not(feature = "mojo"))]
pub fn provider_model_fallback_chain(provider: ProviderId, model: &str) -> Vec<String> {
    provider_model_fallback_chain_rust(provider, model)
}

pub fn provider_gemini_retain_code_assist_models(model_chain: &mut Vec<String>) {
    model_chain.retain(|model| provider_gemini_code_assist_model_allowed(model));
}

pub fn provider_canonical_model(provider: ProviderId, model: &str) -> String {
    provider_model_fallback_chain(provider, model)
        .into_iter()
        .next()
        .filter(|model| !model.trim().is_empty())
        .unwrap_or_else(|| model.to_string())
}

pub fn provider_model_allows_session_memory(model: &str) -> bool {
    matches!(
        model.trim().to_ascii_lowercase().as_str(),
        "" | "auto" | "default"
    )
}

#[cfg(all(test, feature = "mojo"))]
#[test]
fn rich_model_fallback_parser_matches_rust_oracle_for_generated_cases() {
    let providers = [
        ProviderId::OpenAi,
        ProviderId::Anthropic,
        ProviderId::Copilot,
        ProviderId::DeepSeek,
        ProviderId::Gemini,
        ProviderId::Kiro,
        ProviderId::Local,
    ];
    for model in [
        "chat-compression-default",
        "auto",
        "auto-gemini-3",
        "auto-gemini-2.5",
        "pro",
        "gemini-3.1-pro-preview-customtools",
        "gemini-3.1-pro-preview",
        "gemini-3-pro-preview",
        "gemini-3.5-flash",
        "gemini-3-flash-preview",
        "gemini-3-flash",
        "gemini-3.1-flash-lite",
        "flash",
        "flash-lite",
    ] {
        assert_eq!(
            provider_model_fallback_chain(ProviderId::Gemini, model),
            provider_model_fallback_chain_rust(ProviderId::Gemini, model),
            "Gemini fallback parser case {model}"
        );
    }
    for case in 0..20_000 {
        let provider = providers[case % providers.len()];
        let model = match case % 12 {
            0 => String::new(),
            1 => " auto ".to_string(),
            2 => "default".to_string(),
            3 => "opus".to_string(),
            4 => "sonnet".to_string(),
            5 => "codex".to_string(),
            6 => "gpt-5.5".to_string(),
            7 => "pro".to_string(),
            8 => "flash".to_string(),
            9 => "combo:Alpha, alpha;Beta|gamma>beta".to_string(),
            10 => "combo:,,,".to_string(),
            _ => format!(" \t模型-{case}\t "),
        };
        assert_eq!(
            provider_model_fallback_chain(provider, &model),
            provider_model_fallback_chain_rust(provider, &model),
            "fallback parser case {case}: provider={provider:?} model={model:?}"
        );
    }
}

#[cfg(any(not(feature = "mojo"), test))]
fn non_empty_single(model: &str) -> Vec<String> {
    if model.is_empty() {
        Vec::new()
    } else {
        vec![model.to_string()]
    }
}

#[cfg(any(not(feature = "mojo"), test))]
fn combo_chain(model: &str) -> Option<Vec<String>> {
    let chain = model.trim().strip_prefix("combo:")?;
    let models = chain
        .split([',', ';', '|', '>'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    (!models.is_empty()).then(|| dedup_chain(models))
}

#[cfg(any(not(feature = "mojo"), test))]
fn dedup_chain(models: Vec<String>) -> Vec<String> {
    let mut seen = Vec::<String>::new();
    let mut deduped = Vec::new();
    for model in models {
        let key = model.to_ascii_lowercase();
        if seen.iter().any(|value| value == &key) {
            continue;
        }
        seen.push(key);
        deduped.push(model);
    }
    deduped
}
