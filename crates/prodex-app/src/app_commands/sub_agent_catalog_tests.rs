use super::super_main_prompt::super_sub_agent_model_choices;
use crate::app_state::AppStateIoExt;
use crate::{DynamicCatalogStatus, effective_provider_model_catalog_from_paths};
use prodex_provider_core::ProviderId;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn catalog_test_paths(root: &Path) -> crate::AppPaths {
    crate::AppPaths {
        root: root.to_path_buf(),
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared"),
        legacy_shared_codex_root: root.join("legacy"),
    }
}

fn kiro_profile(home: PathBuf) -> crate::ProfileEntry {
    crate::ProfileEntry {
        codex_home: home,
        managed: true,
        email: Some("example@example.test".to_string()),
        provider: crate::ProfileProvider::Kiro {
            auth_key: "test-key".to_string(),
            auth_kind: Some("builder-id".to_string()),
            profile_arn: None,
            profile_name: None,
            start_url: None,
            region: Some("us-east-1".to_string()),
        },
    }
}

fn copilot_profile(home: PathBuf) -> crate::ProfileEntry {
    crate::ProfileEntry {
        codex_home: home,
        managed: true,
        email: Some("copilot@example.test".to_string()),
        provider: crate::ProfileProvider::Copilot {
            host: "github.com".to_string(),
            login: "example-user".to_string(),
            api_url: "https://api.github.com".to_string(),
            access_type_sku: None,
            copilot_plan: None,
        },
    }
}

fn gemini_profile(home: PathBuf) -> crate::ProfileEntry {
    crate::ProfileEntry {
        codex_home: home,
        managed: true,
        email: Some("gemini@example.test".to_string()),
        provider: crate::ProfileProvider::Gemini {
            email: "gemini@example.test".to_string(),
            project_id: None,
        },
    }
}

fn save_kiro_catalog_state(
    root_name: &str,
    catalogs: &[(&str, serde_json::Value)],
) -> (crate::AppPaths, PathBuf) {
    let root = crate::test_support::test_temp_root().join(format!(
        "prodex-kiro-sub-agent-catalog-{root_name}-{}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("profiles")).unwrap();
    let paths = catalog_test_paths(&root);
    let mut profiles = BTreeMap::new();
    for (name, catalog) in catalogs {
        let home = paths.managed_profiles_root.join(name);
        fs::create_dir_all(&home).unwrap();
        fs::write(
            home.join(crate::KIRO_MODEL_CATALOG_FILE),
            catalog.to_string(),
        )
        .unwrap();
        profiles.insert((*name).to_string(), kiro_profile(home));
    }
    crate::AppState {
        active_profile: catalogs.first().map(|(name, _)| (*name).to_string()),
        profiles,
        ..crate::AppState::default()
    }
    .save(&paths)
    .unwrap();
    (paths, root)
}

fn picker_model_ids(configured: &[String]) -> Vec<String> {
    super_sub_agent_model_choices(ProviderId::Kiro, None, configured)
        .into_iter()
        .filter_map(|choice| match choice {
            prodex_provider_core::ProviderModelChoice::Model(model) => Some(model),
            _ => None,
        })
        .collect()
}

#[test]
fn imported_kiro_snapshot_populates_sub_agent_picker() {
    let (paths, root) = save_kiro_catalog_state(
        "single",
        &[(
            "kiro-a",
            serde_json::json!({
                "models": [
                    {"model_id": "catalog-model-a", "model_name": "Catalog Model A"},
                    {"model_id": "catalog-model-b", "model_name": "Catalog Model B"}
                ]
            }),
        )],
    );
    let configured =
        effective_provider_model_catalog_from_paths(&paths, ProviderId::Kiro).model_ids();
    assert_eq!(configured, ["catalog-model-a", "catalog-model-b"]);
    assert_eq!(
        picker_model_ids(&configured),
        ["gpt-5.6-luna", "auto", "catalog-model-a", "catalog-model-b"]
    );
    assert_eq!(
        prodex_provider_core::resolve_provider_model_choices(ProviderId::Kiro, &configured, None)
            .last(),
        Some(&prodex_provider_core::ProviderModelChoice::Custom)
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn broken_kiro_snapshot_does_not_hide_healthy_catalog_and_marks_degraded() {
    let (paths, root) = save_kiro_catalog_state(
        "broken-healthy",
        &[
            ("kiro-broken", serde_json::json!("not-a-catalog")),
            (
                "kiro-healthy",
                serde_json::json!({
                    "models": [{"id": "healthy-model"}]
                }),
            ),
        ],
    );
    let catalog = effective_provider_model_catalog_from_paths(&paths, ProviderId::Kiro);
    assert_eq!(catalog.status, DynamicCatalogStatus::Degraded);
    assert_eq!(catalog.model_ids(), ["healthy-model"]);
    assert!(picker_model_ids(&catalog.model_ids()).contains(&"healthy-model".to_string()));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn multiple_kiro_snapshots_merge_case_insensitively_in_state_order() {
    let (paths, root) = save_kiro_catalog_state(
        "multiple",
        &[
            (
                "kiro-a",
                serde_json::json!({
                    "models": [{"id": "Catalog-Model-A"}, {"id": "catalog-model-b"}]
                }),
            ),
            (
                "kiro-b",
                serde_json::json!({
                    "models": [
                        {"id": "catalog-model-a"},
                        {"id": "CATALOG-MODEL-B"},
                        {"id": "catalog-model-c"}
                    ]
                }),
            ),
        ],
    );
    assert_eq!(
        effective_provider_model_catalog_from_paths(&paths, ProviderId::Kiro).model_ids(),
        ["Catalog-Model-A", "catalog-model-b", "catalog-model-c"]
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn stale_kiro_profile_home_does_not_hide_healthy_catalog() {
    let root = crate::test_support::test_temp_root()
        .join(format!("prodex-kiro-stale-profile-{}", std::process::id()));
    let paths = catalog_test_paths(&root);
    let healthy_home = paths.managed_profiles_root.join("kiro-healthy");
    fs::create_dir_all(&healthy_home).unwrap();
    fs::write(
        healthy_home.join(crate::KIRO_MODEL_CATALOG_FILE),
        serde_json::json!({
            "availableModels": [{"modelId": "healthy-model"}]
        })
        .to_string(),
    )
    .unwrap();
    let mut profiles = BTreeMap::new();
    for index in 0..crate::SUPER_CONFIGURED_MODEL_PROFILE_LIMIT {
        profiles.insert(
            format!("kiro-000-stale-{index:03}"),
            kiro_profile(
                paths
                    .managed_profiles_root
                    .join(format!("missing-{index:03}")),
            ),
        );
    }
    profiles.insert("kiro-healthy".to_string(), kiro_profile(healthy_home));
    crate::AppState {
        active_profile: Some("kiro-000-stale-000".to_string()),
        profiles,
        ..crate::AppState::default()
    }
    .save(&paths)
    .unwrap();

    let catalog = effective_provider_model_catalog_from_paths(&paths, ProviderId::Kiro);
    assert_eq!(catalog.status, DynamicCatalogStatus::Degraded);
    assert_eq!(catalog.model_ids(), ["healthy-model"]);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn missing_or_malformed_kiro_snapshot_keeps_static_picker_safe() {
    let (paths, root) = save_kiro_catalog_state(
        "fallback",
        &[(
            "kiro-a",
            serde_json::json!({
                "models": [{"id": "dynamic-before-malformed"}]
            }),
        )],
    );
    fs::remove_file(
        paths
            .managed_profiles_root
            .join("kiro-a")
            .join(crate::KIRO_MODEL_CATALOG_FILE),
    )
    .unwrap();
    let catalog = effective_provider_model_catalog_from_paths(&paths, ProviderId::Kiro);
    assert_eq!(catalog.status, DynamicCatalogStatus::Degraded);
    assert!(catalog.models.is_empty());
    assert!(
        effective_provider_model_catalog_from_paths(&paths, ProviderId::Kiro)
            .model_ids()
            .is_empty()
    );
    assert_eq!(picker_model_ids(&[]), ["gpt-5.6-luna", "auto"]);
    fs::write(
        paths
            .managed_profiles_root
            .join("kiro-a")
            .join(crate::KIRO_MODEL_CATALOG_FILE),
        r#"{"models":[{"id":"secret-marker"}]} trailing"#,
    )
    .unwrap();
    assert!(
        effective_provider_model_catalog_from_paths(&paths, ProviderId::Kiro)
            .model_ids()
            .is_empty()
    );
    fs::write(
        paths
            .managed_profiles_root
            .join("kiro-a")
            .join(crate::KIRO_MODEL_CATALOG_FILE),
        vec![b'x'; crate::PROVIDER_MODEL_CATALOG_MAX_BYTES as usize + 1],
    )
    .unwrap();
    assert!(
        effective_provider_model_catalog_from_paths(&paths, ProviderId::Kiro)
            .model_ids()
            .is_empty()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn copilot_snapshot_augments_canonical_picker_choices() {
    let root = crate::test_support::test_temp_root().join(format!(
        "prodex-copilot-sub-agent-catalog-{}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("profiles")).unwrap();
    let paths = catalog_test_paths(&root);
    let first_home = paths.managed_profiles_root.join("copilot-a");
    let second_home = paths.managed_profiles_root.join("copilot-b");
    fs::create_dir_all(&first_home).unwrap();
    fs::create_dir_all(&second_home).unwrap();
    fs::write(
        first_home.join(crate::COPILOT_RUNTIME_MODEL_CATALOG_FILE),
        serde_json::json!({
            "models": [
                {"id": "account-model"},
                {"id": "gpt-5.6-luna"}
            ]
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        second_home.join(crate::COPILOT_RUNTIME_MODEL_CATALOG_FILE),
        serde_json::json!({
            "models": [
                {"id": "ACCOUNT-MODEL"},
                {"id": "account-model-b"}
            ]
        })
        .to_string(),
    )
    .unwrap();
    let mut profiles = BTreeMap::new();
    profiles.insert("copilot-a".to_string(), copilot_profile(first_home));
    profiles.insert("copilot-b".to_string(), copilot_profile(second_home));
    crate::AppState {
        active_profile: Some("copilot-a".to_string()),
        profiles,
        ..crate::AppState::default()
    }
    .save(&paths)
    .unwrap();

    let catalog = effective_provider_model_catalog_from_paths(&paths, ProviderId::Copilot);
    assert_eq!(catalog.status, DynamicCatalogStatus::Available);
    assert_eq!(
        catalog.model_ids(),
        ["account-model", "gpt-5.6-luna", "account-model-b"]
    );
    let choices = super_sub_agent_model_choices(ProviderId::Copilot, None, &catalog.model_ids());
    assert!(
        choices.contains(&prodex_provider_core::ProviderModelChoice::Model(
            "account-model".to_string()
        ))
    );
    assert!(
        choices.contains(&prodex_provider_core::ProviderModelChoice::Model(
            "gpt-5.6-luna".to_string()
        ))
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn gemini_snapshot_augments_canonical_picker_choices() {
    let root = crate::test_support::test_temp_root().join(format!(
        "prodex-gemini-sub-agent-catalog-{}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("profiles")).unwrap();
    let paths = catalog_test_paths(&root);
    let home = paths.managed_profiles_root.join("gemini-a");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("prodex-gemini-model-catalog.json"),
        serde_json::json!({"models": [{"slug": "account-gemini"}]}).to_string(),
    )
    .unwrap();
    let mut profiles = BTreeMap::new();
    profiles.insert("gemini-a".to_string(), gemini_profile(home));
    crate::AppState {
        active_profile: Some("gemini-a".to_string()),
        profiles,
        ..crate::AppState::default()
    }
    .save(&paths)
    .unwrap();

    let catalog = effective_provider_model_catalog_from_paths(&paths, ProviderId::Gemini);
    assert_eq!(catalog.status, DynamicCatalogStatus::Available);
    assert_eq!(catalog.model_ids(), ["account-gemini"]);
    let choices = super_sub_agent_model_choices(ProviderId::Gemini, None, &catalog.model_ids());
    assert!(
        choices.contains(&prodex_provider_core::ProviderModelChoice::Model(
            "account-gemini".to_string()
        ))
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cached_deepseek_and_local_catalogs_use_the_effective_source_path() {
    let root = crate::test_support::test_temp_root().join(format!(
        "prodex-cached-provider-sub-agent-catalog-{}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("shared")).unwrap();
    let paths = catalog_test_paths(&root);
    for (provider, file, model) in [
        (
            ProviderId::DeepSeek,
            "prodex-deepseek-model-catalog.json",
            "account-deepseek",
        ),
        (
            ProviderId::Local,
            "prodex-local-model-catalog.json",
            "account-local",
        ),
    ] {
        fs::write(
            paths.shared_codex_root.join(file),
            serde_json::json!({"models": [{"slug": model}]}).to_string(),
        )
        .unwrap();
        let catalog = effective_provider_model_catalog_from_paths(&paths, provider);
        assert_eq!(catalog.status, DynamicCatalogStatus::Available);
        assert_eq!(catalog.model_ids(), [model]);
    }
    let anthropic = effective_provider_model_catalog_from_paths(&paths, ProviderId::Anthropic);
    assert_eq!(anthropic.status, DynamicCatalogStatus::NoDynamicCatalog);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn openai_models_cache_augments_sub_agent_choices_without_network() {
    let root = crate::test_support::test_temp_root().join(format!(
        "prodex-openai-sub-agent-catalog-{}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("shared")).unwrap();
    let paths = catalog_test_paths(&root);
    fs::write(
        paths.shared_codex_root.join(crate::OPENAI_MODEL_CACHE_FILE),
        serde_json::json!({
            "client_version": "0.150.1",
            "models": [
                {"slug": "account-model", "visibility": "list"},
                {"slug": "ACCOUNT-MODEL", "visibility": "list"},
                {"slug": "gpt-5.6-sol", "visibility": "list"}
            ]
        })
        .to_string(),
    )
    .unwrap();

    let catalog = effective_provider_model_catalog_from_paths(&paths, ProviderId::OpenAi);
    assert_eq!(catalog.status, DynamicCatalogStatus::Available);
    let choices = super_sub_agent_model_choices(ProviderId::OpenAi, None, &catalog.model_ids());
    assert!(
        choices.contains(&prodex_provider_core::ProviderModelChoice::Model(
            "account-model".to_string()
        ))
    );
    assert!(
        choices.contains(&prodex_provider_core::ProviderModelChoice::Model(
            "gpt-5.6-sol".to_string()
        ))
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn provider_matrix_keeps_canonical_models_and_deduplicates_dynamic_ids() {
    for provider in [
        ProviderId::OpenAi,
        ProviderId::Anthropic,
        ProviderId::Copilot,
        ProviderId::DeepSeek,
        ProviderId::Gemini,
        ProviderId::Kiro,
        ProviderId::Local,
    ] {
        let configured = ["account-model".to_string(), "ACCOUNT-MODEL".to_string()];
        let choices = super_sub_agent_model_choices(provider, None, &configured);
        assert_eq!(
            choices.first(),
            Some(&prodex_provider_core::ProviderModelChoice::ProviderDefault),
            "{provider:?} lost provider default"
        );
        assert_eq!(
            choices.last(),
            Some(&prodex_provider_core::ProviderModelChoice::Custom),
            "{provider:?} lost custom entry"
        );
        for model in prodex_provider_core::provider_model_catalog(provider) {
            assert!(
                choices.contains(&prodex_provider_core::ProviderModelChoice::Model(
                    model.id.to_string()
                )),
                "{provider:?} lost canonical model {}",
                model.id
            );
        }
        assert_eq!(
            choices
                .iter()
                .filter(|choice| {
                    matches!(
                        choice,
                        prodex_provider_core::ProviderModelChoice::Model(model)
                            if model.eq_ignore_ascii_case("account-model")
                    )
                })
                .count(),
            1,
            "{provider:?} did not deduplicate dynamic models"
        );
    }
}

#[test]
fn dynamic_model_effort_fallback_does_not_inherit_kiro_luna_metadata() {
    let dynamic = crate::canonical_sub_agent_efforts(ProviderId::Kiro, Some("account-only-model"));
    let luna = crate::canonical_sub_agent_efforts(ProviderId::Kiro, Some("gpt-5.6-luna"));
    assert_eq!(
        dynamic,
        crate::canonical_sub_agent_efforts(ProviderId::Kiro, None)
    );
    assert_ne!(dynamic, luna);
}

#[test]
fn explicit_unknown_model_id_remains_accepted_for_sub_agents() {
    let resolved = crate::resolve_super_sub_agent_config(
        prodex_cli::SubAgentConfig {
            provider: ProviderId::Kiro,
            model: Some("account/model-a".to_string()),
            ..prodex_cli::SubAgentConfig::default()
        },
        prodex_cli::SuperLaunchTarget::Fresh,
    )
    .unwrap();
    assert_eq!(resolved.model.as_deref(), Some("account/model-a"));
}

#[test]
fn kiro_catalog_parser_preserves_normalized_metadata() {
    let models = crate::parse_kiro_model_catalog_text(
        &serde_json::json!({
            "supportedModels": [
                {"id": "supported-id", "name": "Supported", "context_window_tokens": 123},
                {"model_id": "snake-id", "model_name": "Snake", "contextWindowTokens": 456},
                {"modelId": "camel-id", "modelName": "Camel"},
                {"slug": "slug-id"},
                {"model": "model-id"}
            ]
        })
        .to_string(),
    )
    .unwrap();
    assert_eq!(
        models
            .iter()
            .filter_map(|model| model.get("id").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>(),
        [
            "supported-id",
            "snake-id",
            "camel-id",
            "slug-id",
            "model-id"
        ]
    );
    assert_eq!(models[0]["name"], "Supported");
    assert_eq!(models[0]["context_window_tokens"], 123);
    assert_eq!(models[1]["name"], "Snake");
    assert_eq!(models[1]["context_window_tokens"], 456);
    assert_eq!(models[2]["name"], "Camel");
    assert_eq!(models[3]["name"], "slug-id");
    assert_eq!(models[4]["name"], "model-id");
}
