//! Provider model fallback chains and canonical model helpers.

#[path = "chains/gemini.rs"]
mod gemini;

use self::gemini::provider_gemini_code_assist_model_allowed;
use crate::ProviderId;

pub fn provider_model_fallback_chain(provider: ProviderId, model: &str) -> Vec<String> {
    prodex_mojo_core::rich::model_fallback_chain(provider.label(), model)
        .expect("Mojo model fallback parser returned an invalid structured result")
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

#[test]
fn rich_model_fallback_matches_expected_provider_cases() {
    let cases: &[(ProviderId, &str, &[&str])] = &[
        (
            ProviderId::Anthropic,
            "",
            &["claude-sonnet-4-6", "claude-opus-4-8", "claude-haiku-4-5"],
        ),
        (
            ProviderId::Anthropic,
            "DEFAULT",
            &["claude-sonnet-4-6", "claude-opus-4-8", "claude-haiku-4-5"],
        ),
        (
            ProviderId::Anthropic,
            "best",
            &["claude-opus-4-8", "claude-sonnet-4-6"],
        ),
        (
            ProviderId::Anthropic,
            "pro",
            &["claude-sonnet-4-6", "claude-opus-4-8"],
        ),
        (
            ProviderId::Anthropic,
            "flash",
            &["claude-haiku-4-5", "claude-sonnet-4-6"],
        ),
        (
            ProviderId::Copilot,
            "",
            &["gpt-5.3-codex", "gpt-5.1-codex", "gpt-4o"],
        ),
        (
            ProviderId::Copilot,
            "auto",
            &["gpt-5.3-codex", "gpt-5.1-codex", "gpt-4o"],
        ),
        (
            ProviderId::Copilot,
            "codex",
            &["gpt-5.3-codex", "gpt-5.1-codex", "gpt-4o"],
        ),
        (
            ProviderId::Copilot,
            "gpt-5.5",
            &["gpt-5.5", "gpt-5.3-codex", "gpt-5.1-codex", "gpt-4o"],
        ),
        (
            ProviderId::Copilot,
            "gpt-5.4",
            &["gpt-5.4", "gpt-5.3-codex", "gpt-5.1-codex", "gpt-4o"],
        ),
        (
            ProviderId::Copilot,
            "gpt-5.3-codex",
            &["gpt-5.3-codex", "gpt-5.1-codex", "gpt-4o"],
        ),
        (
            ProviderId::Copilot,
            "sonnet",
            &["claude-sonnet-4-6", "gpt-5.3-codex", "gpt-5.1-codex"],
        ),
        (
            ProviderId::Copilot,
            "gemini",
            &["gemini-3.1-pro-preview", "gpt-5.3-codex", "gpt-5.1-codex"],
        ),
        (
            ProviderId::Gemini,
            "chat-compression-default",
            &[
                "gemini-3-pro-preview",
                "gemini-3-flash-preview",
                "gemini-2.5-pro",
                "gemini-2.5-flash",
            ],
        ),
        (
            ProviderId::Gemini,
            "",
            &[
                "gemini-3-pro-preview",
                "gemini-3.1-pro-preview",
                "gemini-2.5-pro",
                "gemini-3-flash-preview",
                "gemini-3.5-flash",
                "gemini-3-flash",
                "gemini-2.5-flash",
            ],
        ),
        (
            ProviderId::Gemini,
            "auto",
            &[
                "gemini-3-pro-preview",
                "gemini-3.1-pro-preview",
                "gemini-2.5-pro",
                "gemini-3-flash-preview",
                "gemini-3.5-flash",
                "gemini-3-flash",
                "gemini-2.5-flash",
            ],
        ),
        (
            ProviderId::Gemini,
            "auto-gemini-3",
            &[
                "gemini-3-pro-preview",
                "gemini-3.1-pro-preview",
                "gemini-2.5-pro",
                "gemini-3-flash-preview",
                "gemini-3.5-flash",
                "gemini-3-flash",
                "gemini-2.5-flash",
            ],
        ),
        (
            ProviderId::Gemini,
            "auto-gemini-2.5",
            &["gemini-2.5-pro", "gemini-2.5-flash"],
        ),
        (
            ProviderId::Gemini,
            "pro",
            &[
                "gemini-3-pro-preview",
                "gemini-3.1-pro-preview",
                "gemini-2.5-pro",
            ],
        ),
        (
            ProviderId::Gemini,
            "gemini-3.1-pro-preview-customtools",
            &[
                "gemini-3.1-pro-preview-customtools",
                "gemini-3.1-pro-preview",
                "gemini-3-pro-preview",
                "gemini-2.5-pro",
                "gemini-3-flash-preview",
                "gemini-3-flash",
                "gemini-3.5-flash",
                "gemini-2.5-flash",
            ],
        ),
        (
            ProviderId::Gemini,
            "gemini-3.1-pro-preview",
            &[
                "gemini-3.1-pro-preview",
                "gemini-3-pro-preview",
                "gemini-2.5-pro",
                "gemini-3-flash-preview",
                "gemini-3-flash",
                "gemini-3.5-flash",
                "gemini-2.5-flash",
            ],
        ),
        (
            ProviderId::Gemini,
            "gemini-3-pro-preview",
            &[
                "gemini-3-pro-preview",
                "gemini-3.1-pro-preview",
                "gemini-2.5-pro",
                "gemini-3-flash-preview",
                "gemini-3.5-flash",
                "gemini-3-flash",
                "gemini-2.5-flash",
            ],
        ),
        (
            ProviderId::Gemini,
            "gemini-3.5-flash",
            &[
                "gemini-3.5-flash",
                "gemini-3-flash",
                "gemini-3-flash-preview",
                "gemini-2.5-flash",
            ],
        ),
        (
            ProviderId::Gemini,
            "gemini-3-flash-preview",
            &[
                "gemini-3-flash-preview",
                "gemini-3.5-flash",
                "gemini-3-flash",
                "gemini-2.5-flash",
            ],
        ),
        (
            ProviderId::Gemini,
            "gemini-3-flash",
            &["gemini-3-flash", "gemini-3.5-flash", "gemini-2.5-flash"],
        ),
        (
            ProviderId::Gemini,
            "gemini-3.1-flash-lite",
            &[
                "gemini-3.1-flash-lite",
                "gemini-2.5-flash-lite",
                "gemini-2.5-flash",
            ],
        ),
        (
            ProviderId::Gemini,
            "gemini-2.5-flash",
            &["gemini-2.5-flash"],
        ),
        (
            ProviderId::Gemini,
            "flash",
            &[
                "gemini-3-flash-preview",
                "gemini-3.5-flash",
                "gemini-3-flash",
                "gemini-2.5-flash",
            ],
        ),
        (
            ProviderId::Gemini,
            "flash-lite",
            &["gemini-3.1-flash-lite", "gemini-2.5-flash-lite"],
        ),
        (
            ProviderId::DeepSeek,
            "",
            &["deepseek-v4-pro", "deepseek-v4-flash"],
        ),
        (
            ProviderId::DeepSeek,
            "auto",
            &["deepseek-v4-pro", "deepseek-v4-flash"],
        ),
        (
            ProviderId::DeepSeek,
            "pro",
            &["deepseek-v4-pro", "deepseek-v4-flash"],
        ),
        (
            ProviderId::DeepSeek,
            "flash",
            &["deepseek-v4-flash", "deepseek-v4-pro"],
        ),
        (ProviderId::Kiro, "", &["auto"]),
        (ProviderId::Kiro, "default", &["auto"]),
        (ProviderId::Kiro, "claude", &["auto"]),
        (ProviderId::Kiro, "sonnet", &["auto"]),
        (ProviderId::OpenAi, "custom-model", &["custom-model"]),
        (ProviderId::Local, "local-model", &["local-model"]),
        (ProviderId::Local, "", &[]),
    ];

    for (provider, model, expected) in cases {
        assert_eq!(
            provider_model_fallback_chain(*provider, model),
            expected
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>(),
            "provider={provider:?} model={model:?}"
        );
    }
}

#[test]
fn rich_model_fallback_preserves_combo_and_unrecognized_model_values() {
    let cases = [
        (
            "combo:Alpha, alpha;Beta|gamma>beta",
            vec!["Alpha", "Beta", "gamma"],
        ),
        ("combo:,,,", vec!["combo:,,,"]),
        ("combo: \u{3000}Alpha\t, Beta", vec!["Alpha", "Beta"]),
        ("COMBO:Alpha,beta", vec!["COMBO:Alpha,beta"]),
        (" \u{2003}模型-custom\u{3000} ", vec!["模型-custom"]),
    ];

    for (model, expected) in cases {
        assert_eq!(
            provider_model_fallback_chain(ProviderId::OpenAi, model),
            expected.into_iter().map(str::to_string).collect::<Vec<_>>(),
            "model={model:?}"
        );
    }
}

#[test]
fn rich_model_fallback_grows_for_large_values() {
    for length in [4_096, 4_097] {
        let long_model = "x".repeat(length);
        assert_eq!(
            provider_model_fallback_chain(ProviderId::OpenAi, &long_model),
            vec![long_model]
        );
    }

    let models = [
        "model-0", "model-1", "model-2", "model-3", "model-4", "model-5", "model-6", "model-7",
        "model-8", "model-9", "model-10", "model-11", "model-12", "model-13", "model-14",
        "model-15", "model-16", "model-17", "model-18", "model-19", "model-20", "model-21",
        "model-22", "model-23", "model-24", "model-25", "model-26", "model-27", "model-28",
        "model-29", "model-30", "model-31", "model-32",
    ];
    for count in [32, 33] {
        let expected = &models[..count];
        let combo = format!("combo:{}", expected.join(","));
        assert_eq!(
            provider_model_fallback_chain(ProviderId::OpenAi, &combo),
            expected
                .iter()
                .map(|model| (*model).to_string())
                .collect::<Vec<_>>()
        );
    }

    let models = (0..2_049)
        .map(|index| format!("model-{index}"))
        .collect::<Vec<_>>();
    let combo = format!("combo:{}", models.join(","));
    assert_eq!(
        provider_model_fallback_chain(ProviderId::OpenAi, &combo),
        models
    );
}

#[test]
fn rich_model_fallback_batch_matches_expected_values() {
    assert_eq!(
        prodex_mojo_core::rich::model_fallback_plan(
            ProviderId::Copilot.label(),
            &["codex", "gpt-5.3-codex", "custom-model", " custom-model "],
        )
        .unwrap(),
        vec!["gpt-5.3-codex", "gpt-5.1-codex", "gpt-4o", "custom-model"]
    );
}
