use super::{
    AppPaths, ChildProcessPlan, LastModelSelection, ModelPreferenceScope, ModelPreferenceSync,
    apply_fresh_model_preference_selection, digest_bytes, load_latest_model_preference,
    model_preference_file_path, model_preference_scope, now_nanos, record_model_preference,
    resolve_fresh_model_preference_context,
};
use std::ffi::OsString;
use std::fs;
use std::path::Path;

fn paths(root: &Path) -> AppPaths {
    AppPaths {
        root: root.to_path_buf(),
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared"),
        legacy_shared_codex_root: root.join("legacy-shared"),
    }
}

#[test]
fn governed_transport_uses_the_logical_openai_scope() {
    let root = std::env::temp_dir().join(format!(
        "prodex-model-preferences-governed-scope-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    let config = home.join("config.toml");
    fs::write(
        &config,
        "model_provider = \"openai\"\nmodel = \"initial\"\n",
    )
    .unwrap();
    let paths = paths(&root);
    let logical_scope = model_preference_scope(&home, &[]).unwrap();
    let child = ChildProcessPlan::new(OsString::from("codex"), home.clone()).with_args(vec![
        OsString::from("-c"),
        OsString::from("model_provider=\"prodex-openai-governed-http\""),
    ]);
    assert_eq!(
        logical_scope,
        model_preference_scope(&home, &child.args).unwrap()
    );
    let mut sync =
        ModelPreferenceSync::start_with_scope(&paths, &child, logical_scope.clone()).unwrap();
    fs::write(
        &config,
        "model_provider = \"openai\"\nmodel = \"selected\"\nmodel_reasoning_effort = \"max\"\n",
    )
    .unwrap();
    assert!(sync.finish().is_none());
    let selection = load_latest_model_preference(&paths, &logical_scope)
        .unwrap()
        .unwrap();
    assert_eq!(selection.model, "selected");
    assert_eq!(selection.reasoning_effort.as_deref(), Some("max"));
    let relaunched = resolve_fresh_model_preference_context(&paths, &home, &[]).unwrap();
    let relaunched_args =
        apply_fresh_model_preference_selection(&home, Vec::new(), &relaunched, true, true);
    assert_eq!(
        crate::codex_cli_config_override_value(&relaunched_args, "model").as_deref(),
        Some("selected")
    );
    assert_eq!(
        crate::codex_cli_config_override_value(&relaunched_args, "model_reasoning_effort")
            .as_deref(),
        Some("max")
    );
    assert_eq!(relaunched.remembered.unwrap().model, "selected");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn governed_transport_preference_migrates_once_to_logical_scope() {
    let root = std::env::temp_dir().join(format!(
        "prodex-model-preferences-governed-migration-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join("config.toml"), "model_provider = \"openai\"\n").unwrap();
    let paths = paths(&root);
    let logical_scope = model_preference_scope(&home, &[]).unwrap();
    let legacy_scope = ModelPreferenceScope {
        provider: digest_bytes(b"prodex-openai-governed-http"),
        catalog: digest_bytes(b"codex-default-v1\0prodex-openai-governed-http"),
    };
    record_model_preference(
        &paths,
        LastModelSelection {
            scope: legacy_scope.clone(),
            model: "legacy-model".to_string(),
            reasoning_effort: Some("high".to_string()),
            selected_at: 42,
            generation: 0,
            source: "codex-config".to_string(),
        },
    )
    .unwrap();

    let context = resolve_fresh_model_preference_context(&paths, &home, &[]).unwrap();
    assert_eq!(context.remembered.as_ref().unwrap().model, "legacy-model");
    assert_eq!(
        load_latest_model_preference(&paths, &logical_scope)
            .unwrap()
            .unwrap()
            .selected_at,
        42
    );
    assert!(
        load_latest_model_preference(&paths, &legacy_scope)
            .unwrap()
            .is_some()
    );
    let first_file = fs::read_to_string(model_preference_file_path(&paths)).unwrap();
    let second = resolve_fresh_model_preference_context(&paths, &home, &[]).unwrap();
    assert_eq!(second.remembered.unwrap().model, "legacy-model");
    assert_eq!(
        first_file,
        fs::read_to_string(model_preference_file_path(&paths)).unwrap()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn compatible_openai_profiles_share_preference_without_sharing_auth() {
    let root = std::env::temp_dir().join(format!(
        "prodex-model-preferences-profile-rotation-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    let first_home = root.join("first-home");
    let second_home = root.join("second-home");
    fs::create_dir_all(&first_home).unwrap();
    fs::create_dir_all(&second_home).unwrap();
    for home in [&first_home, &second_home] {
        fs::write(home.join("config.toml"), "model_provider = \"openai\"\n").unwrap();
    }
    let paths = paths(&root);
    let first_scope = model_preference_scope(&first_home, &[]).unwrap();
    assert_eq!(
        first_scope,
        model_preference_scope(&second_home, &[]).unwrap()
    );
    record_model_preference(
        &paths,
        LastModelSelection {
            scope: first_scope,
            model: "rotated-profile-model".to_string(),
            reasoning_effort: Some("high".to_string()),
            selected_at: 7,
            generation: 0,
            source: "test".to_string(),
        },
    )
    .unwrap();
    let context = resolve_fresh_model_preference_context(&paths, &second_home, &[]).unwrap();
    assert_eq!(context.remembered.unwrap().model, "rotated-profile-model");
    assert!(!first_home.join("auth.json").exists());
    assert!(!second_home.join("auth.json").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn concurrent_preference_writers_keep_the_newer_selection() {
    let root = std::env::temp_dir().join(format!(
        "prodex-model-preferences-concurrent-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let paths = paths(&root);
    let scope = ModelPreferenceScope {
        provider: "provider".to_string(),
        catalog: "catalog".to_string(),
    };
    std::thread::scope(|threads| {
        for (model, selected_at) in [("older", 10), ("newer", 20)] {
            let paths = paths.clone();
            let scope = scope.clone();
            threads.spawn(move || {
                record_model_preference(
                    &paths,
                    LastModelSelection {
                        scope,
                        model: model.to_string(),
                        reasoning_effort: Some("high".to_string()),
                        selected_at,
                        generation: 0,
                        source: "test".to_string(),
                    },
                )
                .unwrap();
            });
        }
    });
    assert_eq!(
        load_latest_model_preference(&paths, &scope)
            .unwrap()
            .unwrap()
            .model,
        "newer"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn removed_catalog_model_is_not_sent_to_codex_on_the_second_pass() {
    let root = std::env::temp_dir().join(format!(
        "prodex-model-preferences-removed-model-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join("config.toml"), "model_provider = \"openai\"\n").unwrap();
    let paths = paths(&root);
    let scope = model_preference_scope(&home, &[]).unwrap();
    record_model_preference(
        &paths,
        LastModelSelection {
            scope,
            model: "removed-model".to_string(),
            reasoning_effort: Some("max".to_string()),
            selected_at: 1,
            generation: 0,
            source: "test".to_string(),
        },
    )
    .unwrap();
    let context = resolve_fresh_model_preference_context(&paths, &home, &[]).unwrap();
    let first_pass =
        apply_fresh_model_preference_selection(&home, Vec::new(), &context, true, false);
    let catalog = root.join("catalog.json");
    fs::write(
        &catalog,
        r#"{"models":[{"slug":"current","supported_reasoning_levels":[{"effort":"high"}]}]}"#,
    )
    .unwrap();
    let mut second_pass = first_pass;
    second_pass.extend([
        OsString::from("-c"),
        OsString::from(format!(
            "model_catalog_json={}",
            crate::runtime_catalog_config::toml_string_literal(&catalog.to_string_lossy())
        )),
    ]);
    let second_pass =
        apply_fresh_model_preference_selection(&home, second_pass, &context, false, true);
    assert!(crate::codex_cli_config_override_value(&second_pass, "model").is_none());
    assert!(
        crate::codex_cli_config_override_value(&second_pass, "model_reasoning_effort").is_none()
    );
    let _ = fs::remove_dir_all(root);
}
