use super::{catalog_model, main_model_choices, openai_main_model_choices};
#[cfg(feature = "mojo-core")]
use super::{main_model_choices_from_catalog, main_model_choices_from_catalog_rust};
use serde_json::json;

#[cfg(feature = "mojo-core")]
#[test]
fn dynamic_catalog_mojo_matches_the_test_only_rust_owner() {
    let entries = vec![
        catalog_model("gpt-5.6-luna", "Luna", 3, &[" low ", "LOW", "medium"]),
        catalog_model("future/model:alpha", "Future", 1, &["max"]),
        catalog_model("GPT-5.6-LUNA", "Duplicate", 2, &["high"]),
        json!({
            "id": "fallback-id",
            "display_name": " ",
            "priority": 4,
            "visibility": "list",
            "supported_reasoning_levels": [{"effort": "xhigh"}]
        }),
        json!({"slug": "hidden", "visibility": "hide", "priority": 0}),
        json!({"slug": "unsupported", "supported_in_api": false, "priority": 0}),
    ];
    assert_eq!(
        main_model_choices_from_catalog(entries.clone()),
        main_model_choices_from_catalog_rust(entries),
    );
}

#[cfg(feature = "mojo-core")]
#[test]
fn dynamic_catalog_bounds_match_the_mojo_input_contract() {
    for (field, value) in [
        ("slug", json!("x".repeat(4_097))),
        ("display_name", json!("x".repeat(65_537))),
        (
            "supported_reasoning_levels",
            json!([{"effort": "x".repeat(65_537)}]),
        ),
    ] {
        let mut entry = catalog_model("model", "Model", 0, &["medium"]);
        entry[field] = value;
        let entries = vec![entry];
        assert!(
            main_model_choices_from_catalog(entries.clone()).is_none(),
            "Mojo accepted oversized {field}"
        );
        assert!(
            main_model_choices_from_catalog_rust(entries).is_none(),
            "Rust oracle accepted oversized {field}"
        );
    }
}

#[test]
fn openai_main_picker_reads_the_active_models_cache() {
    let root =
        crate::test_temp_root().join(format!("prodex-main-model-cache-{}", std::process::id()));
    let codex_home = root.join("codex");
    std::fs::create_dir_all(&codex_home).unwrap();
    let _env_lock = crate::test_support::TestEnvVarGuard::lock();
    let _prodex_home = crate::test_support::TestEnvVarGuard::set(
        "PRODEX_HOME",
        root.join("prodex").to_str().unwrap(),
    );
    let _codex_home = crate::test_support::TestEnvVarGuard::set(
        "PRODEX_SHARED_CODEX_HOME",
        codex_home.to_str().unwrap(),
    );
    std::fs::write(
        codex_home.join("models_cache.json"),
        json!({
            "client_version": "0.150.1",
            "models": [
                catalog_model("gpt-5.6-terra", "GPT-5.6 Terra", 2, &["high"]),
                catalog_model("gpt-5.6-sol", "GPT-5.6 Sol", 1, &["max"]),
                catalog_model("gpt-5.6-luna", "GPT-5.6 Luna", 3, &["medium"]),
            ]
        })
        .to_string(),
    )
    .unwrap();

    let choices = openai_main_model_choices().unwrap();
    let models = choices
        .iter()
        .filter_map(|choice| match &choice.choice {
            prodex_provider_core::ProviderModelChoice::Model(model) => Some(model.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(models, ["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"]);
    drop(_codex_home);
    drop(_prodex_home);
    drop(_env_lock);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn invalid_openai_model_cache_falls_back_to_static_catalog() {
    let root = crate::test_temp_root().join(format!(
        "prodex-main-model-cache-fallback-{}",
        std::process::id()
    ));
    let codex_home = root.join("codex");
    std::fs::create_dir_all(&codex_home).unwrap();
    let _env_lock = crate::test_support::TestEnvVarGuard::lock();
    let _prodex_home = crate::test_support::TestEnvVarGuard::set(
        "PRODEX_HOME",
        root.join("prodex").to_str().unwrap(),
    );
    let _codex_home = crate::test_support::TestEnvVarGuard::set(
        "PRODEX_SHARED_CODEX_HOME",
        codex_home.to_str().unwrap(),
    );
    let expected = prodex_provider_core::resolve_provider_model_choices(
        prodex_provider_core::ProviderId::OpenAi,
        &[],
        None,
    );

    for (name, contents) in [
        ("malformed", "{"),
        ("missing models", "{}"),
        ("empty models", r#"{"models":[]}"#),
        ("invalid models", r#"{"models":{}}"#),
    ] {
        std::fs::write(codex_home.join("models_cache.json"), contents).unwrap();
        assert!(openai_main_model_choices().is_none(), "{name} cache");
        assert_eq!(
            main_model_choices(prodex_provider_core::ProviderId::OpenAi, None)
                .into_iter()
                .map(|choice| choice.choice)
                .collect::<Vec<_>>(),
            expected,
            "{name} cache fallback",
        );
    }

    std::fs::remove_file(codex_home.join("models_cache.json")).unwrap();
    assert!(openai_main_model_choices().is_none(), "missing cache");
    assert_eq!(
        main_model_choices(prodex_provider_core::ProviderId::OpenAi, None)
            .into_iter()
            .map(|choice| choice.choice)
            .collect::<Vec<_>>(),
        expected,
    );
    drop(_codex_home);
    drop(_prodex_home);
    drop(_env_lock);
    let _ = std::fs::remove_dir_all(root);
}
