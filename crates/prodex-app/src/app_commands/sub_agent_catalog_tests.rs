use super::super_prompt::{
    SUPER_CONFIGURED_MODEL_LIMIT, configured_sub_agent_model_ids,
    configured_sub_agent_models_from_paths,
};
use crate::app_state::AppStateIoExt;
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
        email: Some("kiro@example.com".to_string()),
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
    prodex_provider_core::resolve_provider_model_choices(ProviderId::Kiro, configured, None)
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
                    {"model_id": "claude-sonnet-4.5", "model_name": "Claude Sonnet 4.5"}
                ]
            }),
        )],
    );
    let configured = configured_sub_agent_models_from_paths(&paths, ProviderId::Kiro);
    assert_eq!(configured, ["claude-sonnet-4.5"]);
    assert_eq!(
        picker_model_ids(&configured),
        ["gpt-5.6-luna", "auto", "claude-sonnet-4.5"]
    );
    assert_eq!(
        prodex_provider_core::resolve_provider_model_choices(ProviderId::Kiro, &configured, None)
            .last(),
        Some(&prodex_provider_core::ProviderModelChoice::Custom)
    );
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
                    "models": [{"id": "Claude-Sonnet-4.5"}, {"id": "claude-sonnet-4"}]
                }),
            ),
            (
                "kiro-b",
                serde_json::json!({
                    "models": [{"id": "claude-sonnet-4.5"}, {"id": "CLAUDE-SONNET-4"}]
                }),
            ),
        ],
    );
    assert_eq!(
        configured_sub_agent_models_from_paths(&paths, ProviderId::Kiro),
        ["Claude-Sonnet-4.5", "claude-sonnet-4"]
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
            "availableModels": [{"modelId": "healthy-account-model"}]
        })
        .to_string(),
    )
    .unwrap();
    crate::AppState {
        active_profile: Some("kiro-stale".to_string()),
        profiles: BTreeMap::from([
            (
                "kiro-stale".to_string(),
                kiro_profile(paths.managed_profiles_root.join("missing")),
            ),
            ("kiro-healthy".to_string(), kiro_profile(healthy_home)),
        ]),
        ..crate::AppState::default()
    }
    .save(&paths)
    .unwrap();

    assert_eq!(
        configured_sub_agent_models_from_paths(&paths, ProviderId::Kiro),
        ["healthy-account-model"]
    );
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
    assert!(configured_sub_agent_models_from_paths(&paths, ProviderId::Kiro).is_empty());
    assert_eq!(picker_model_ids(&[]), ["gpt-5.6-luna", "auto"]);
    fs::write(
        paths
            .managed_profiles_root
            .join("kiro-a")
            .join(crate::KIRO_MODEL_CATALOG_FILE),
        r#"{"models":[{"id":"secret-marker"}]} trailing"#,
    )
    .unwrap();
    assert!(configured_sub_agent_models_from_paths(&paths, ProviderId::Kiro).is_empty());
    fs::write(
        paths
            .managed_profiles_root
            .join("kiro-a")
            .join(crate::KIRO_MODEL_CATALOG_FILE),
        vec![b'x'; crate::PROVIDER_MODEL_CATALOG_MAX_BYTES as usize + 1],
    )
    .unwrap();
    assert!(configured_sub_agent_models_from_paths(&paths, ProviderId::Kiro).is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn configured_model_ids_keep_case_insensitive_limit() {
    let mut models = Vec::new();
    configured_sub_agent_model_ids(
        &serde_json::json!({
            "models": [
                {"modelId": "Model-A"},
                {"id": "model-a"},
                {"slug": "Model-B"}
            ]
        }),
        &mut models,
        SUPER_CONFIGURED_MODEL_LIMIT,
    );
    assert_eq!(models, ["Model-A", "Model-B"]);
}
