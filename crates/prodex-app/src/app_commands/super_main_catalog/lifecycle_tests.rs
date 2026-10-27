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

#[test]
fn terra_main_and_subagent_preserve_account_efforts_without_changing_api_defaults_04394() {
    let fixture = Fixture::new("terra-efforts", chatgpt());
    fs::write(
        fixture.home.join("models_cache.json"),
        json!({"models":[{
            "slug":"gpt-6.1-terra", "visibility":"list", "supported_in_api":true,
            "context_window":500000, "max_context_window":872000,
            "supported_reasoning_levels":[{"effort":"low"},{"effort":"max"},{"effort":"ultra"}],
            "default_reasoning_level":"low"
        }]})
        .to_string(),
    )
    .unwrap();
    let before = fs::read(fixture.home.join("models_cache.json")).unwrap();
    let main = main_model_choices(ProviderId::OpenAi, None);
    assert_eq!(ids(&main), ["gpt-6.1-terra"]);
    let sub = crate::effective_sub_agent_efforts(ProviderId::OpenAi, Some("terra"));
    assert_eq!(
        main[1].efforts.as_ref().unwrap(),
        &sub.iter()
            .map(|effort| effort.as_str().to_string())
            .collect::<Vec<_>>()
    );
    let resolution = crate::effective_provider_model_reasoning_resolution(
        ProviderId::OpenAi,
        Some("gpt-6.1-terra"),
        Some("ultra"),
    )
    .unwrap();
    assert_eq!(
        resolution.default_reasoning_effort,
        Some(prodex_provider_core::ProviderReasoningEffort::Low)
    );
    assert_eq!(
        resolution.selected_reasoning_effort,
        Some(prodex_provider_core::ProviderReasoningEffort::Ultra)
    );
    assert_eq!(
        before,
        fs::read(fixture.home.join("models_cache.json")).unwrap()
    );
    secret_store::write_private_file_atomic(
        &fixture.home.join("auth.json"),
        json!({"auth_mode":"apikey","OPENAI_API_KEY":"synthetic-api-key"})
            .to_string()
            .as_bytes(),
    )
    .unwrap();
    assert_eq!(
        crate::effective_provider_model_reasoning_resolution(
            ProviderId::OpenAi,
            Some("gpt-6.1-terra"),
            Some("ultra")
        ),
        Err(prodex_provider_core::ProviderModelReasoningError::UnsupportedEffort)
    );
    assert!(
        crate::effective_provider_model_reasoning_resolution(
            ProviderId::OpenAi,
            Some("gpt-6.1-terra"),
            Some("max")
        )
        .is_ok()
    );
}

#[test]
fn active_openai_account_catalog_precedes_unrelated_shared_cache_04394() {
    use crate::AppStateIoExt;
    let fixture = Fixture::new("active-account", chatgpt());
    fixture.cache(&["shared-account-only"]);
    let paths = crate::AppPaths::discover().unwrap();
    let profile_home = paths.managed_profiles_root.join("synthetic-account");
    secret_store::ensure_private_directory(&paths.root).unwrap();
    secret_store::ensure_private_directory(&paths.managed_profiles_root).unwrap();
    secret_store::ensure_private_directory(&profile_home).unwrap();
    secret_store::write_private_file_atomic(
        &profile_home.join("auth.json"),
        chatgpt().to_string().as_bytes(),
    )
    .unwrap();
    fs::write(profile_home.join("models_cache.json"), json!({"models":[{
        "slug":"gpt-6.1-terra","visibility":"list","supported_reasoning_levels":[{"effort":"max"}]
    }]}).to_string()).unwrap();
    let mut state = crate::AppState {
        active_profile: Some("synthetic-account".into()),
        ..crate::AppState::default()
    };
    state.profiles.insert(
        "synthetic-account".into(),
        crate::ProfileEntry {
            codex_home: profile_home.clone(),
            managed: true,
            email: None,
            provider: crate::ProfileProvider::Openai,
        },
    );
    state.save(&paths).unwrap();
    let catalog = effective_provider_model_catalog(ProviderId::OpenAi);
    assert_eq!(catalog.model_ids(), ["gpt-6.1-terra"]);
    assert_eq!(
        ids(&main_model_choices(ProviderId::OpenAi, None)),
        ["gpt-6.1-terra"]
    );
    // An unreadable active snapshot must retain the ordinary shared-cache fallback.
    fs::write(profile_home.join("models_cache.json"), "{").unwrap();
    assert_eq!(
        effective_provider_model_catalog(ProviderId::OpenAi).model_ids(),
        ["shared-account-only"]
    );
}

#[test]
fn main_launch_uses_account_default_effort_and_keeps_explicit_override_04394() {
    let fixture = Fixture::new("launch-default", chatgpt());
    fs::write(fixture.home.join("models_cache.json"), json!({"models":[{
        "slug":"gpt-6.1-terra", "visibility":"list", "supported_in_api":true,
        "supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"max"},{"effort":"ultra"}],
        "default_reasoning_level":"low"
    }]}).to_string()).unwrap();
    let crate::Commands::Super(args) =
        crate::parse_cli_command_from(["prodex", "s", "--model", "gpt-6.1-terra"]).unwrap()
    else {
        panic!("expected Super command");
    };
    assert_eq!(
        super::super::resolve_main_model_and_effort(&args, ProviderId::OpenAi, false).unwrap(),
        (Some("gpt-6.1-terra".into()), Some("low".into()))
    );
    assert_eq!(
        crate::effective_provider_model_reasoning_resolution(
            ProviderId::OpenAi,
            Some("gpt-6.1-terra"),
            None
        )
        .unwrap()
        .default_reasoning_effort,
        Some(prodex_provider_core::ProviderReasoningEffort::Low)
    );
    let crate::Commands::Super(explicit) = crate::parse_cli_command_from([
        "prodex",
        "s",
        "--model",
        "gpt-6.1-terra",
        "-c",
        "model_reasoning_effort=\"max\"",
    ])
    .unwrap() else {
        panic!("expected Super command");
    };
    assert_eq!(
        super::super::resolve_main_model_and_effort(&explicit, ProviderId::OpenAi, false).unwrap(),
        (Some("gpt-6.1-terra".into()), Some("max".into()))
    );
    assert_eq!(
        super::super::prompt_openai_ping_model_and_effort(Some("gpt-6.1-terra"), Some("ultra"))
            .unwrap(),
        (Some("gpt-6.1-terra".into()), Some("ultra".into()))
    );
    secret_store::write_private_file_atomic(
        &fixture.home.join("auth.json"),
        json!({"auth_mode":"apikey","OPENAI_API_KEY":"synthetic-api-key"})
            .to_string()
            .as_bytes(),
    )
    .unwrap();
    assert_eq!(
        super::super::resolve_main_model_and_effort(&args, ProviderId::OpenAi, false).unwrap(),
        (Some("gpt-6.1-terra".into()), Some("medium".into()))
    );
}
