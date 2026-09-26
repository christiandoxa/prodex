use super::*;

#[test]
fn catalog_identity_trims_long_unicode_whitespace() {
    let query = format!("{}{}", "　".repeat(1_400), "gpt-5.6-sol");

    assert_eq!(
        provider_catalog_entry(ProviderId::OpenAi, &query).map(|entry| entry.id.as_str()),
        Some("gpt-5.6-sol")
    );
    assert_eq!(
        crate::provider_model_spec(ProviderId::OpenAi, &query).map(|model| model.id),
        Some("gpt-5.6-sol")
    );
    assert!(
        crate::provider_model_spec(ProviderId::OpenAi, &query)
            .is_some_and(|model| model.matches_id_or_alias(&query))
    );
}

#[test]
fn model_choices_keep_order_and_deduplicate_aliases_and_unicode_custom_ids() {
    let long_alias = format!("{}luna", " ".repeat(4_100));
    let configured = vec![
        "gpt-5.6-sol".to_string(),
        "GPT-5.6-SOL".to_string(),
        long_alias,
        "custom-模型-🦀".to_string(),
        "CUSTOM-模型-🦀".to_string(),
        "tail-model".to_string(),
        "TAIL-MODEL".to_string(),
        "　".to_string(),
    ];

    let choices =
        resolve_provider_model_choices(ProviderId::OpenAi, &configured, Some("current-模型-🦀"));
    let models = choices
        .iter()
        .filter_map(|choice| match choice {
            ProviderModelChoice::Model(model) => Some(model.as_str()),
            ProviderModelChoice::ProviderDefault | ProviderModelChoice::Custom => None,
        })
        .collect::<Vec<_>>();
    let count = |id: &str| models.iter().filter(|model| **model == id).count();

    assert_eq!(choices.first(), Some(&ProviderModelChoice::ProviderDefault));
    assert_eq!(
        choices[1],
        ProviderModelChoice::Model("gpt-5.6-sol".to_string())
    );
    assert_eq!(
        choices[3],
        ProviderModelChoice::Model("gpt-5.6-luna".to_string())
    );
    assert_eq!(choices.last(), Some(&ProviderModelChoice::Custom));
    assert_eq!(count("gpt-5.6-sol"), 1);
    assert_eq!(count("gpt-5.6-luna"), 1);
    assert_eq!(count("custom-模型-🦀"), 1);
    assert_eq!(count("tail-model"), 1);
    assert_eq!(count("current-模型-🦀"), 1);
    assert!(
        models.iter().position(|model| *model == "custom-模型-🦀")
            < models.iter().position(|model| *model == "tail-model")
    );
    assert!(
        models.iter().position(|model| *model == "tail-model")
            < models.iter().position(|model| *model == "current-模型-🦀")
    );
}

#[test]
fn merge_skips_catalog_aliases_and_case_insensitive_duplicates() {
    let additional = [
        "gpt-5.6-luna".to_string(),
        "account/model:custom".to_string(),
        "ACCOUNT/MODEL:CUSTOM".to_string(),
        "LUNA".to_string(),
        "模型-雪-🦀".to_string(),
        "模型-雪-🦀".to_string(),
        "　".to_string(),
        format!("{}gpt-5.6-sol", " ".repeat(4_100)),
        "tail-model".to_string(),
    ];
    let additional_refs = additional.iter().map(String::as_str).collect::<Vec<_>>();

    assert_eq!(
        merge_catalog_ids_with_mojo(ProviderId::OpenAi, &additional_refs),
        [1, 4, 8]
    );
}

#[test]
fn reasoning_resolves_long_unicode_model_ids_and_rejects_unicode_effort() {
    let query = format!("{}{}", "　".repeat(1_400), "gpt-5.6-luna");
    let resolution =
        provider_model_reasoning_resolution(ProviderId::OpenAi, Some(&query), Some("MAX")).unwrap();

    assert_eq!(resolution.model_index, Some(2));
    assert_eq!(
        resolution.default_reasoning_effort,
        Some(ProviderReasoningEffort::Medium)
    );
    assert_eq!(
        resolution.selected_reasoning_effort,
        Some(ProviderReasoningEffort::Max)
    );
    assert_eq!(
        provider_model_reasoning_resolution(
            ProviderId::OpenAi,
            Some("gpt-5.6-luna"),
            Some("推論-🦀"),
        ),
        Err(ProviderModelReasoningError::UnsupportedEffort)
    );
}
