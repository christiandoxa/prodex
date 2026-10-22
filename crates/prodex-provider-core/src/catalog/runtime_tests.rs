use super::*;

fn observation() -> RuntimeModelCatalogInput<'static> {
    RuntimeModelCatalogInput {
        provider: ProviderId::Copilot,
        slug: "synthetic",
        display_name: "Display",
        description: "Description",
        priority: 1,
        context_window: 200_000,
        auto_compact_token_limit: 190_000,
        supported_reasoning_efforts: None,
        default_reasoning_effort: None,
    }
}

#[test]
fn runtime_schema_preserves_nullable_empty_unknown_and_explicit_reasoning_metadata() {
    use ProviderReasoningEffort::*;
    let empty = render_runtime_model_catalog(RuntimeModelCatalogInput {
        supported_reasoning_efforts: Some(&[]),
        default_reasoning_effort: Some(Unknown),
        ..observation()
    })
    .unwrap();
    assert_eq!(empty["supported_reasoning_levels"], json!([]));
    assert_eq!(empty["default_reasoning_level"], json!("high"));
    let efforts = [None, Minimal, Low, Medium, High, XHigh, Max, Ultra, Unknown];
    let explicit = render_runtime_model_catalog(RuntimeModelCatalogInput {
        supported_reasoning_efforts: Some(&efforts),
        default_reasoning_effort: Some(Max),
        ..observation()
    })
    .unwrap();
    assert_eq!(explicit["default_reasoning_level"], json!("max"));
    assert_eq!(
        explicit["supported_reasoning_levels"],
        json!([
            {"effort":"none","description":"No reasoning effort"},
            {"effort":"minimal","description":"Minimal reasoning effort"},
            {"effort":"low","description":"Low reasoning effort"},
            {"effort":"medium","description":"Medium reasoning effort"},
            {"effort":"high","description":"High reasoning effort"},
            {"effort":"xhigh","description":"Extra-high reasoning effort"},
            {"effort":"max","description":"Max reasoning effort"},
            {"effort":"ultra","description":"Ultra reasoning effort"}
        ])
    );
    let defaults = render_runtime_model_catalog(observation()).unwrap();
    assert_eq!(defaults["default_reasoning_level"], json!("high"));
    assert_eq!(
        defaults["supported_reasoning_levels"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
}

#[test]
fn runtime_schema_preserves_provider_capabilities_unsigned_endpoints_and_rejections() {
    for (provider, capable) in [
        (ProviderId::Anthropic, true),
        (ProviderId::Copilot, true),
        (ProviderId::Kiro, false),
    ] {
        let model = render_runtime_model_catalog(RuntimeModelCatalogInput {
            provider,
            priority: usize::MAX,
            context_window: u64::MAX,
            auto_compact_token_limit: u64::MAX,
            ..observation()
        })
        .unwrap();
        assert_eq!(model["priority"], json!(usize::MAX));
        assert_eq!(model["context_window"], json!(u64::MAX));
        assert_eq!(model["auto_compact_token_limit"], json!(u64::MAX));
        assert_eq!(model["supports_reasoning_summaries"], json!(capable));
        assert_eq!(
            model["supports_reasoning_summary_parameter"],
            json!(capable)
        );
        assert_eq!(model["supports_search_tool"], json!(capable));
        assert_eq!(
            model["input_modalities"],
            if capable {
                json!(["text", "image"])
            } else {
                json!(["text"])
            }
        );
    }
    assert!(matches!(
        render_runtime_model_catalog(RuntimeModelCatalogInput {
            provider: ProviderId::Gemini,
            ..observation()
        }),
        Err(RuntimeModelCatalogError::Mojo(MojoError::InvalidInput))
    ));
}
