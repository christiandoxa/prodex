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
        (ProviderId::DeepSeek, false),
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

#[test]
fn deepseek_runtime_schema_keeps_fixed_efforts_and_instructions() {
    let model = render_runtime_model_catalog(RuntimeModelCatalogInput {
        provider: ProviderId::DeepSeek,
        supported_reasoning_efforts: Some(&[]),
        default_reasoning_effort: Some(ProviderReasoningEffort::Max),
        ..observation()
    })
    .unwrap();
    assert_eq!(model["default_reasoning_level"], json!("high"));
    assert_eq!(
        model["supported_reasoning_levels"],
        json!([
            {"effort":"low","description":"DeepSeek low reasoning effort"},
            {"effort":"medium","description":"DeepSeek medium reasoning effort"},
            {"effort":"high","description":"DeepSeek high reasoning effort"},
            {"effort":"xhigh","description":"DeepSeek max reasoning effort"}
        ])
    );
    let instructions = model["base_instructions"].as_str().unwrap();
    assert_eq!(instructions.len(), 430);
    assert_eq!(instructions.lines().count(), 5);
    assert!(!instructions.contains('\r'));
}

#[test]
fn runtime_dynamic_catalog_projection_preserves_complete_models_and_duplicates() {
    let models = normalize_runtime_dynamic_catalog(&json!({"models":[
        {"id":" \u{3000}Model雪 ","name":"","description":" Raw\n", "max_prompt_tokens":1, "context_window_tokens":1, "context_window":999},
        {"id":null,"slug":"ignored"},
        {"slug":"Model雪","model_name":" Name ", "max_prompt_tokens":u64::MAX},
        {"model":"last","capabilities":{"limits":{"max_prompt_tokens":345}}},
        false
    ]})).unwrap();
    assert_eq!(
        models,
        vec![
            RuntimeDynamicCatalogModel {
                slug: "Model雪".into(),
                display_name: Some("".into()),
                description: Some(" Raw\n".into()),
                context_window: None
            },
            RuntimeDynamicCatalogModel {
                slug: "Model雪".into(),
                display_name: Some(" Name ".into()),
                description: None,
                context_window: Some(u64::MAX)
            },
            RuntimeDynamicCatalogModel {
                slug: "last".into(),
                display_name: None,
                description: None,
                context_window: Some(345)
            },
        ]
    );
}

#[test]
fn runtime_dynamic_catalog_projection_preserves_exact_rejections_and_raw_limits() {
    for input in [json!(null), json!([]), json!({}), json!({"models":{}})] {
        let error = normalize_runtime_dynamic_catalog(&input).unwrap_err();
        assert!(matches!(&error, RuntimeModelCatalogError::Rejected(_)));
        assert_eq!(
            error.to_string(),
            "provider model catalog is missing models array"
        );
    }
    let limit = super::super::PROVIDER_MODEL_CATALOG_HARD_LIMIT;
    let at_limit = json!({"models":vec![Value::Null;limit]});
    assert!(
        normalize_runtime_dynamic_catalog(&at_limit)
            .unwrap()
            .is_empty()
    );
    let over_limit = json!({"models":vec![Value::Null;limit+1]});
    let error = normalize_runtime_dynamic_catalog(&over_limit).unwrap_err();
    assert_eq!(
        error.to_string(),
        format!("provider model catalog exceeds the hard limit of {limit} entries")
    );
}

#[test]
fn runtime_catalog_plan_uses_typed_metadata_saturated_budgets_and_provider_rejections() {
    let dynamic = [RuntimeDynamicCatalogModel {
        slug: "synthetic-plan".into(),
        display_name: Some("".into()),
        description: None,
        context_window: Some(u64::MAX),
    }];
    let input = RuntimeModelCatalogPlanInput {
        provider: ProviderId::Copilot,
        launch_model: "SYNTHETIC-PLAN",
        context_window: 1,
        auto_compact_token_limit: 0,
        dynamic_models: &dynamic,
    };
    let models = plan_runtime_model_catalog(input).unwrap();
    let first = &models[0];
    assert_eq!(first.slug, "SYNTHETIC-PLAN");
    assert_eq!(first.display_name, "");
    assert_eq!(first.priority, 1);
    assert_eq!(first.context_window, u64::MAX);
    assert_eq!(first.auto_compact_token_limit, 184_467_440_737_095_516_u64);
    let oversized = [RuntimeDynamicCatalogModel {
        slug: "x".repeat(4097),
        ..dynamic[0].clone()
    }];
    assert!(matches!(
        plan_runtime_model_catalog(RuntimeModelCatalogPlanInput {
            dynamic_models: &oversized,
            ..input
        }),
        Err(RuntimeModelCatalogError::Mojo(MojoError::InvalidInput))
    ));
    assert!(matches!(
        plan_runtime_model_catalog(RuntimeModelCatalogPlanInput {
            provider: ProviderId::Gemini,
            ..input
        }),
        Err(RuntimeModelCatalogError::Mojo(MojoError::InvalidInput))
    ));
}

#[test]
fn deepseek_catalog_plan_keeps_legacy_static_entries_and_unclamped_budgets() {
    let input = RuntimeModelCatalogPlanInput {
        provider: ProviderId::DeepSeek,
        launch_model: " PRO ",
        context_window: 0,
        auto_compact_token_limit: u64::MAX,
        dynamic_models: &[],
    };
    let models = plan_runtime_model_catalog(input).unwrap();
    assert_eq!(
        models
            .iter()
            .map(|model| model.slug.as_str())
            .collect::<Vec<_>>(),
        [
            "PRO",
            "auto",
            "flash",
            "deepseek-v4-pro",
            "deepseek-v4-flash",
            "deepseek-chat",
            "deepseek-reasoner"
        ]
    );
    assert_eq!(models[0].display_name, "DeepSeek Pro");
    assert_eq!(
        models[0].description,
        "Prodex DeepSeek Pro alias routed through DeepSeek V4 Pro."
    );
    assert!(
        models
            .iter()
            .all(|model| model.context_window == 0 && model.auto_compact_token_limit == u64::MAX)
    );
    let dynamic = [RuntimeDynamicCatalogModel {
        slug: "synthetic".into(),
        display_name: None,
        description: None,
        context_window: None,
    }];
    assert!(matches!(
        plan_runtime_model_catalog(RuntimeModelCatalogPlanInput {
            dynamic_models: &dynamic,
            ..input
        }),
        Err(RuntimeModelCatalogError::Mojo(MojoError::InvalidInput))
    ));
}
