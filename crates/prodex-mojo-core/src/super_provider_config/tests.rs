use super::*;

use super::*;

fn rust_external_provider_number(
    value: Option<&str>,
    default: u64,
) -> Result<u64, ExternalProviderNumericIssue> {
    let Some(value) = value else {
        return Ok(default);
    };
    if value.is_empty() {
        return Err(ExternalProviderNumericIssue::Empty);
    }
    if value.chars().any(char::is_whitespace) {
        return Err(ExternalProviderNumericIssue::Whitespace);
    }
    let value = value
        .parse::<u64>()
        .map_err(|_| ExternalProviderNumericIssue::InvalidUnsignedInteger)?;
    if value <= 1 {
        return Err(ExternalProviderNumericIssue::MustBeGreaterThanOne);
    }
    Ok(value)
}

fn rust_external_provider_numeric_config(
    context_window: Option<&str>,
    default_context_window: u64,
    auto_compact_token_limit: Option<&str>,
    default_auto_compact_token_limit: u64,
) -> Result<ExternalProviderNumericConfig, ExternalProviderConfigError> {
    let context_window = rust_external_provider_number(context_window, default_context_window)
        .map_err(|issue| ExternalProviderConfigError::InvalidValue {
            field: ExternalProviderNumericField::ContextWindow,
            issue,
        })?;
    let auto_compact_token_limit =
        rust_external_provider_number(auto_compact_token_limit, default_auto_compact_token_limit)
            .map_err(|issue| ExternalProviderConfigError::InvalidValue {
                field: ExternalProviderNumericField::AutoCompactTokenLimit,
                issue,
            })?
            .min(context_window.saturating_sub(1));
    Ok(ExternalProviderNumericConfig {
        context_window,
        auto_compact_token_limit,
    })
}

#[test]
fn super_provider_config_kernel_preserves_aliases_escaping_and_order() {
    assert_eq!(
        external_provider_alias(" GitHub_Copilot ").unwrap(),
        Some(ExternalProviderAliasPlan::Copilot)
    );
    assert_eq!(external_provider_alias("unknown").unwrap(), None);
    assert_eq!(
        runtime_external_provider_class("GITHUB_COPILOT").unwrap(),
        Some(RuntimeExternalProviderClass::Copilot)
    );
    assert_eq!(
        runtime_external_provider_class("gemini-oauth").unwrap(),
        Some(RuntimeExternalProviderClass::GeminiOauth)
    );
    assert_eq!(
        runtime_external_provider_class("gemini-native").unwrap(),
        Some(RuntimeExternalProviderClass::GeminiNative)
    );
    assert_eq!(runtime_external_provider_class(" gemini ").unwrap(), None);
    assert_eq!(runtime_external_provider_class("unknown").unwrap(), None);
    assert_eq!(
        external_catalog_static_model_count(ExternalCatalogProviderClass::Anthropic).unwrap(),
        9
    );
    assert_eq!(
        external_catalog_static_model_count(ExternalCatalogProviderClass::Copilot).unwrap(),
        24
    );
    assert_eq!(
        external_catalog_static_model_count(ExternalCatalogProviderClass::Kiro).unwrap(),
        2
    );
    let model =
        external_catalog_model_metadata(ExternalCatalogProviderClass::Copilot, "GPT-5.1-CODEX")
            .unwrap()
            .unwrap();
    assert_eq!(model.slug, "gpt-5.1-codex");
    assert_eq!(model.display_name, "GPT-5.1 Codex");
    assert!(
        external_catalog_model_metadata(ExternalCatalogProviderClass::Copilot, " GPT-5.1-CODEX ",)
            .unwrap()
            .is_none()
    );
    assert_eq!(deepseek_catalog_static_model_count().unwrap(), 7);
    let deepseek = deepseek_catalog_model_metadata(" DEEPSEEK-V4-PRO ")
        .unwrap()
        .unwrap();
    assert_eq!(deepseek.slug, "deepseek-v4-pro");
    assert_eq!(deepseek.display_name, "DeepSeek V4 Pro");
    assert_eq!(
        deepseek_catalog_static_models().unwrap()[6].slug,
        "deepseek-reasoner"
    );
    assert_eq!(
        profile_import_source_class("CLAUDE").unwrap(),
        Some(ProfileImportSourceClass::Claude)
    );
    assert_eq!(
        profile_import_source_class("copilot").unwrap(),
        Some(ProfileImportSourceClass::Copilot)
    );
    assert_eq!(
        profile_import_source_class("Kiro").unwrap(),
        Some(ProfileImportSourceClass::Kiro)
    );
    assert_eq!(profile_import_source_class(" claude ").unwrap(), None);
    assert_eq!(profile_import_source_class("github-copilot").unwrap(), None);
    assert!(
        runtime_openai_scalar_policy(RuntimeOpenAiScalarPolicy::ProviderName, " OpenAI ").unwrap()
    );
    assert!(
        !runtime_openai_scalar_policy(RuntimeOpenAiScalarPolicy::ProviderName, "prodex-openai")
            .unwrap()
    );
    for model in [
        "GPT-5.6-SOL",
        " gpt-5-mini ",
        "gpt-6.1-sol",
        "gpt-6-sol",
        "CODEX-AUTO-REVIEW",
    ] {
        assert!(
            runtime_openai_scalar_policy(RuntimeOpenAiScalarPolicy::LargeContextModel, model)
                .unwrap(),
            "{model}"
        );
    }
    assert!(
        !runtime_openai_scalar_policy(RuntimeOpenAiScalarPolicy::LargeContextModel, "gpt-4o")
            .unwrap()
    );
    for model in [
        "gpt-5.6-sol",
        " GPT-5.6-TERRA ",
        "gpt-6.1-sol",
        "gpt-6-luna",
    ] {
        assert!(
            runtime_openai_scalar_policy(RuntimeOpenAiScalarPolicy::PreferMaxContextModel, model,)
                .unwrap(),
            "{model}"
        );
    }
    assert!(
        !runtime_openai_scalar_policy(RuntimeOpenAiScalarPolicy::PreferMaxContextModel, "gpt-5.5")
            .unwrap()
    );
    assert_eq!(
        runtime_model_provider_class("PRODEX-GEMINI").unwrap(),
        Some(RuntimeModelProviderClass::Gemini)
    );
    assert_eq!(
        runtime_model_provider_class("prodex-copilot").unwrap(),
        Some(RuntimeModelProviderClass::Copilot)
    );
    assert_eq!(
        runtime_model_provider_class(" prodex-local ").unwrap(),
        None
    );
    assert_eq!(runtime_model_provider_class("custom").unwrap(), None);
    assert_eq!(runtime_bool_token("YeS").unwrap(), Some(true));
    assert_eq!(runtime_bool_token("OFF").unwrap(), Some(false));
    assert_eq!(runtime_bool_token(" true ").unwrap(), None);
    assert_eq!(runtime_bool_token("maybe").unwrap(), None);
    assert!(runtime_ci_truth_token("YeS").unwrap());
    assert!(runtime_ci_truth_token("TRUE").unwrap());
    assert!(!runtime_ci_truth_token("on").unwrap());
    assert!(!runtime_ci_truth_token("off").unwrap());
    assert!(!runtime_ci_truth_token("").unwrap());
    assert_eq!(
        runtime_deepseek_web_search_token("OPENAI-CHAT").unwrap(),
        Some(RuntimeDeepSeekWebSearchToken::OpenAiChat)
    );
    assert_eq!(
        runtime_deepseek_web_search_token("DISABLE").unwrap(),
        Some(RuntimeDeepSeekWebSearchToken::Off)
    );
    assert_eq!(
        runtime_deepseek_web_search_token("ANTHROPIC").unwrap(),
        Some(RuntimeDeepSeekWebSearchToken::Anthropic)
    );
    assert_eq!(runtime_deepseek_web_search_token("live").unwrap(), None);
    assert_eq!(toml_string_literal("a\\b\"c").unwrap(), "\"a\\\\b\\\"c\"");

    let entries = provider_config_entries(ProviderConfigInput {
        provider_id: "gemini",
        provider_name: "Gemini \"Bridge\"",
        base_url: "https://example.com/v1",
        model: "模型/β",
        web_search: "live",
        context_window: 1_048_576,
        auto_compact_token_limit: 900_000,
        image_generation: true,
    })
    .unwrap();
    assert_eq!(entries.len(), 14);
    assert_eq!(entries[0], "model_provider=\"gemini\"");
    assert_eq!(entries[1], "model=\"模型/β\"");
    assert_eq!(
        entries[2],
        "model_providers.gemini.name=\"Gemini \\\"Bridge\\\"\""
    );
    assert_eq!(
        entries[3],
        "model_providers.gemini.base_url=\"https://example.com/v1\""
    );
    assert_eq!(entries[4], "model_providers.gemini.wire_api=\"responses\"");
    assert_eq!(
        entries[5],
        "model_providers.gemini.requires_openai_auth=true"
    );
    assert_eq!(
        entries[6],
        "model_providers.gemini.supports_websockets=false"
    );
    assert_eq!(entries[7], "model_context_window=1048576");
    assert_eq!(entries[8], "model_auto_compact_token_limit=900000");
    assert_eq!(entries[9], "model_reasoning_summary=\"none\"");
    assert_eq!(entries[10], "web_search=\"live\"");
    assert_eq!(entries[11], "features.apps=false");
    assert_eq!(entries[12], "features.js_repl=false");
    assert_eq!(entries[13], "features.image_generation=true");
}

#[test]
fn external_provider_numeric_config_matches_rust_validation_oracle() {
    let values = [
        None,
        Some(""),
        Some("0"),
        Some("1"),
        Some("2"),
        Some("+42"),
        Some("18446744073709551615"),
        Some("18446744073709551616"),
        Some("-2"),
        Some("2_000"),
        Some(" 2"),
        Some("2\u{3000}"),
        Some("\u{0085}2"),
        Some("\u{001c}2"),
    ];
    for context_window in values {
        for auto_compact_token_limit in values {
            let expected = rust_external_provider_numeric_config(
                context_window,
                100_000,
                auto_compact_token_limit,
                90_000,
            );
            let actual = external_provider_numeric_config(
                context_window,
                100_000,
                auto_compact_token_limit,
                90_000,
            );
            assert_eq!(
                actual, expected,
                "{context_window:?}, {auto_compact_token_limit:?}"
            );
        }
    }
    assert_eq!(
        external_provider_numeric_config(None, 100_000, None, 90_000).unwrap(),
        ExternalProviderNumericConfig {
            context_window: 100_000,
            auto_compact_token_limit: 90_000,
        }
    );
    assert_eq!(
        external_provider_numeric_config(Some("10"), 100_000, None, 90_000).unwrap(),
        ExternalProviderNumericConfig {
            context_window: 10,
            auto_compact_token_limit: 9,
        }
    );
}
