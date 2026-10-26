use super::{MainModelChoice, effective_provider_model_catalog, main_model_choices};
use crate::test_support::TestEnvVarGuard;
use prodex_provider_core::{ModelCatalogScope, ProviderId, ProviderModelChoice};
use serde_json::json;
use std::{fs, path::PathBuf};

struct Fixture {
    root: PathBuf,
    home: PathBuf,
    _environment: Vec<TestEnvVarGuard>,
}

impl Fixture {
    fn new(name: &str, auth: serde_json::Value) -> Self {
        let lock = TestEnvVarGuard::lock();
        let root = crate::test_support::test_temp_root().join(format!(
            "prodex-catalog-lifecycle-{name}-{}",
            std::process::id()
        ));
        let home = root.join("codex");
        fs::create_dir_all(&home).unwrap();
        secret_store::ensure_private_directory(&root).unwrap();
        secret_store::ensure_private_directory(&home).unwrap();
        let prodex = TestEnvVarGuard::set("PRODEX_HOME", root.join("prodex").to_str().unwrap());
        let shared = TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", home.to_str().unwrap());
        let backend = TestEnvVarGuard::set(crate::PRODEX_SECRET_BACKEND_ENV, "file");
        secret_store::write_private_file_atomic(
            &home.join("auth.json"),
            auth.to_string().as_bytes(),
        )
        .unwrap();
        Self {
            root,
            home,
            _environment: vec![backend, shared, prodex, lock],
        }
    }

    fn cache(&self, ids: &[&str]) {
        fs::write(
            self.home.join("models_cache.json"),
            json!({
                "models":ids.iter().enumerate().map(|(priority,id)| json!({
                    "slug":id,"display_name":id,"priority":priority,
                    "supported_in_api":true,"visibility":"list",
                    "supported_reasoning_levels":[{"effort":"max"}],
                    "default_reasoning_level":"max"
                })).collect::<Vec<_>>()
            })
            .to_string(),
        )
        .unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn chatgpt() -> serde_json::Value {
    json!({"auth_mode":"chatgpt","tokens":{"access_token":"synthetic-test-token"}})
}

fn ids(choices: &[MainModelChoice]) -> Vec<&str> {
    choices
        .iter()
        .filter_map(|choice| match &choice.choice {
            ProviderModelChoice::Model(id) => Some(id.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn chatgpt_main_and_subagent_do_not_resurrect_retired_or_absent_account_models_04393() {
    let fixture = Fixture::new("chatgpt", chatgpt());
    fixture.cache(&[
        "gpt-6.1-sol",
        "gpt-6-luna",
        "gpt-5.4",
        "gpt-5.3-codex",
        "spark",
    ]);
    let catalog = effective_provider_model_catalog(ProviderId::OpenAi);
    assert_eq!(catalog.scope, ModelCatalogScope::Chatgpt);
    assert_eq!(catalog.model_ids(), ["gpt-6.1-sol", "gpt-6-luna"]);
    for current in ["gpt-5.4", "opus", "gpt-5.6-terra", "spark"] {
        let main = main_model_choices(ProviderId::OpenAi, Some(current));
        let sub = catalog.choices(ProviderId::OpenAi, Some(current), &catalog.model_ids());
        assert_eq!(
            ids(&main),
            ["gpt-6.1-sol", "gpt-6-luna"],
            "current={current}"
        );
        assert_eq!(
            main.iter()
                .map(|choice| choice.choice.clone())
                .collect::<Vec<_>>(),
            sub
        );
        assert_eq!(
            main[1].efforts.as_deref(),
            Some(["max".to_string()].as_slice())
        );
    }
    let custom = main_model_choices(ProviderId::OpenAi, Some("tenant/custom-model"));
    assert!(ids(&custom).contains(&"tenant/custom-model"));
    // Selection reads do not rewrite the upstream cache or the user's auth.
    let cache: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.home.join("models_cache.json")).unwrap()).unwrap();
    assert_eq!(cache["models"].as_array().unwrap().len(), 5);
}

#[test]
fn api_key_models_survive_a_shared_chatgpt_cache_04393() {
    let fixture = Fixture::new(
        "api-key",
        json!({"auth_mode":"apikey","OPENAI_API_KEY":"synthetic-test-key"}),
    );
    fixture.cache(&["gpt-6.1-sol", "gpt-6-luna"]);
    let catalog = effective_provider_model_catalog(ProviderId::OpenAi);
    assert_eq!(catalog.scope, ModelCatalogScope::Provider);
    assert!(
        catalog.models.is_empty(),
        "ChatGPT entitlement cache must not restrict API-key access"
    );
    let main = main_model_choices(ProviderId::OpenAi, Some("gpt-5.4"));
    assert!(ids(&main).contains(&"gpt-5.4"));
    assert!(
        catalog
            .choices(ProviderId::OpenAi, Some("gpt-5.4"), &[])
            .contains(&ProviderModelChoice::Model("gpt-5.4".into()))
    );
}

#[test]
fn malformed_or_retired_only_chatgpt_cache_keeps_current_offline_models_04393() {
    let fixture = Fixture::new("offline", chatgpt());
    for data in ["{", r#"{"models":[]}"#, r#"{"models":[{"slug":"spark"}]}"#] {
        fs::write(fixture.home.join("models_cache.json"), data).unwrap();
        let choices = main_model_choices(ProviderId::OpenAi, Some("gpt-5.4"));
        let models = ids(&choices);
        assert!(models.contains(&"gpt-6.1-sol"));
        assert!(!models.contains(&"gpt-5.4"));
        assert!(!models.contains(&"gpt-5.3-codex-spark"));
        assert_eq!(choices.last().unwrap().choice, ProviderModelChoice::Custom);
    }
}
