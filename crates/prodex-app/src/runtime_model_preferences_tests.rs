use super::{
    AppPaths, ChildProcessPlan, LastModelSelection, MODEL_PREFERENCE_FILE, ModelPreferenceScope,
    ModelPreferenceSync, apply_fresh_model_preference, apply_model_preference_selection,
    capture_changed_config, catalog_supports_selection, load_latest_model_preference,
    model_preference_file_path, model_preference_model_is_compatible, model_preference_scope,
    now_nanos, read_config_snapshot, record_model_preference,
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
fn explicit_none_is_distinct_from_absent_effort() {
    let root = std::env::temp_dir().join(format!(
        "prodex-model-preferences-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let paths = paths(&root);
    let scope = ModelPreferenceScope {
        provider: "provider".to_string(),
        catalog: "catalog".to_string(),
    };
    record_model_preference(
        &paths,
        LastModelSelection {
            scope: scope.clone(),
            model: "model-none".to_string(),
            reasoning_effort: Some("none".to_string()),
            selected_at: 2,
            generation: 0,
            source: "test".to_string(),
        },
    )
    .unwrap();
    record_model_preference(
        &paths,
        LastModelSelection {
            scope,
            model: "model-unset".to_string(),
            reasoning_effort: None,
            selected_at: 3,
            generation: 0,
            source: "test".to_string(),
        },
    )
    .unwrap();

    let raw = fs::read_to_string(model_preference_file_path(&paths)).unwrap();
    assert!(raw.contains("model-unset"));
    assert!(raw.contains("\"reasoning_effort\": null"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn explicit_none_is_applied_when_catalog_advertises_it() {
    let root = std::env::temp_dir().join(format!(
        "prodex-model-preferences-none-catalog-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let catalog = root.join("catalog.json");
    fs::write(
        &catalog,
        r#"{"models":[{"slug":"model","supported_reasoning_levels":[{"effort":"none"},{"effort":"high"}]}]}"#,
    )
    .unwrap();
    let args = vec![
        OsString::from("-c"),
        OsString::from(format!(
            "model_catalog_json={}",
            crate::runtime_catalog_config::toml_string_literal(&catalog.to_string_lossy())
        )),
    ];
    let selection = LastModelSelection {
        scope: ModelPreferenceScope {
            provider: "provider".to_string(),
            catalog: "catalog".to_string(),
        },
        model: "model".to_string(),
        reasoning_effort: Some("none".to_string()),
        selected_at: 1,
        generation: 0,
        source: "test".to_string(),
    };

    let applied = apply_model_preference_selection(&root, args, &selection, true, true);

    assert_eq!(
        crate::codex_cli_config_override_value(&applied, "model_reasoning_effort").as_deref(),
        Some("none")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn older_process_exit_cannot_overwrite_newer_config_commit() {
    let root = std::env::temp_dir().join(format!(
        "prodex-model-preferences-order-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let paths = paths(&root);
    let scope = ModelPreferenceScope {
        provider: "provider".to_string(),
        catalog: "catalog".to_string(),
    };
    for (model, selected_at) in [("new", 20), ("old", 10)] {
        record_model_preference(
            &paths,
            LastModelSelection {
                scope: scope.clone(),
                model: model.to_string(),
                reasoning_effort: Some("high".to_string()),
                selected_at,
                generation: 0,
                source: "test".to_string(),
            },
        )
        .unwrap();
    }
    let selection = load_latest_model_preference(&paths, &scope)
        .unwrap()
        .unwrap();
    assert_eq!(selection.model, "new");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn fresh_preference_preserves_resume_and_merges_explicit_fields() {
    let root = std::env::temp_dir().join(format!(
        "prodex-model-preferences-precedence-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join("config.toml"), "model_provider = \"openai\"\n").unwrap();
    let paths = paths(&root);
    let scope = model_preference_scope(&home, &[]).unwrap();
    record_model_preference(
        &paths,
        LastModelSelection {
            scope,
            model: "remembered".to_string(),
            reasoning_effort: Some("max".to_string()),
            selected_at: 1,
            generation: 0,
            source: "test".to_string(),
        },
    )
    .unwrap();
    let resume = vec![OsString::from("resume"), OsString::from("thread-id")];
    assert_eq!(
        apply_fresh_model_preference(&paths, &home, resume.clone(), true, true).unwrap(),
        resume
    );
    let explicit = vec![OsString::from("-m"), OsString::from("explicit")];
    let applied = apply_fresh_model_preference(&paths, &home, explicit, true, true).unwrap();
    assert_eq!(
        crate::runtime_launch_cli_model(&applied).as_deref(),
        Some("explicit")
    );
    assert_eq!(
        crate::codex_cli_config_override_value(&applied, "model_reasoning_effort").as_deref(),
        Some("max")
    );

    let explicit_model = vec![OsString::from("-c"), OsString::from("model=explicit")];
    let applied = apply_fresh_model_preference(&paths, &home, explicit_model, true, true).unwrap();
    assert_eq!(
        crate::codex_cli_config_override_value(&applied, "model").as_deref(),
        Some("explicit")
    );
    assert_eq!(
        crate::codex_cli_config_override_value(&applied, "model_reasoning_effort").as_deref(),
        Some("max")
    );

    let explicit_effort = vec![
        OsString::from("-c"),
        OsString::from("model_reasoning_effort=high"),
    ];
    let applied = apply_fresh_model_preference(&paths, &home, explicit_effort, true, true).unwrap();
    assert_eq!(
        crate::codex_cli_config_override_value(&applied, "model").as_deref(),
        Some("remembered")
    );
    assert_eq!(
        crate::codex_cli_config_override_value(&applied, "model_reasoning_effort").as_deref(),
        Some("high")
    );

    let explicit_pair = vec![
        OsString::from("-c"),
        OsString::from("model=explicit"),
        OsString::from("-c"),
        OsString::from("model_reasoning_effort=high"),
    ];
    let applied =
        apply_fresh_model_preference(&paths, &home, explicit_pair.clone(), true, true).unwrap();
    assert_eq!(applied, explicit_pair);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn generated_catalog_scope_is_stable_across_overlay_paths_and_revisions() {
    let root = std::env::temp_dir().join(format!(
        "prodex-model-preferences-generated-catalog-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    let first_home = root.join("first");
    let second_home = root.join("second");
    fs::create_dir_all(&first_home).unwrap();
    fs::create_dir_all(&second_home).unwrap();
    fs::write(
        first_home.join("config.toml"),
        "model_provider = \"prodex-local\"\n",
    )
    .unwrap();
    fs::write(
        second_home.join("config.toml"),
        "model_provider = \"prodex-local\"\n",
    )
    .unwrap();
    let first_catalog = first_home.join("prodex-local-model-catalog.json");
    let second_catalog = second_home.join("prodex-local-model-catalog.json");
    fs::write(&first_catalog, "{\"models\":[{\"slug\":\"old\"}]}\n").unwrap();
    fs::write(&second_catalog, "{\"models\":[{\"slug\":\"new\"}]}\n").unwrap();
    let args = |catalog: &Path| {
        vec![
            OsString::from("-c"),
            OsString::from(format!(
                "model_catalog_json={}",
                crate::runtime_catalog_config::toml_string_literal(&catalog.to_string_lossy())
            )),
        ]
    };

    assert_eq!(
        model_preference_scope(&first_home, &args(&first_catalog)).unwrap(),
        model_preference_scope(&second_home, &args(&second_catalog)).unwrap()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn generated_provider_scope_matches_before_and_after_catalog_creation() {
    let root = std::env::temp_dir().join(format!(
        "prodex-model-preferences-generated-provider-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("config.toml"),
        "model_provider = \"prodex-local\"\n",
    )
    .unwrap();
    let catalog = home.join("prodex-local-model-catalog.json");
    fs::write(&catalog, "{\"models\":[{\"slug\":\"local\"}]}\n").unwrap();
    let explicit = vec![
        OsString::from("-c"),
        OsString::from(format!(
            "model_catalog_json={}",
            crate::runtime_catalog_config::toml_string_literal(&catalog.to_string_lossy())
        )),
    ];

    assert_eq!(
        model_preference_scope(&home, &[]).unwrap(),
        model_preference_scope(&home, &explicit).unwrap()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn malformed_primary_preference_recovers_from_last_good_backup() {
    let root = std::env::temp_dir().join(format!(
        "prodex-model-preferences-backup-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let paths = paths(&root);
    let scope = ModelPreferenceScope {
        provider: "provider".to_string(),
        catalog: "catalog".to_string(),
    };
    record_model_preference(
        &paths,
        LastModelSelection {
            scope: scope.clone(),
            model: "first".to_string(),
            reasoning_effort: Some("high".to_string()),
            selected_at: 1,
            generation: 0,
            source: "test".to_string(),
        },
    )
    .unwrap();
    record_model_preference(
        &paths,
        LastModelSelection {
            scope: scope.clone(),
            model: "second".to_string(),
            reasoning_effort: Some("max".to_string()),
            selected_at: 2,
            generation: 0,
            source: "test".to_string(),
        },
    )
    .unwrap();
    fs::write(model_preference_file_path(&paths), "broken").unwrap();

    assert_eq!(
        load_latest_model_preference(&paths, &scope)
            .unwrap()
            .unwrap()
            .model,
        "second"
    );
    fs::write(model_preference_file_path(&paths), "broken").unwrap();
    record_model_preference(
        &paths,
        LastModelSelection {
            scope: scope.clone(),
            model: "third".to_string(),
            reasoning_effort: Some("high".to_string()),
            selected_at: 3,
            generation: 0,
            source: "test".to_string(),
        },
    )
    .unwrap();
    assert_eq!(
        load_latest_model_preference(&paths, &scope)
            .unwrap()
            .unwrap()
            .model,
        "third"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn existing_native_config_is_migrated_without_auth_state() {
    let root = std::env::temp_dir().join(format!(
        "prodex-model-preferences-migration-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("config.toml"),
        "model_provider = \"openai\"\nmodel = \"legacy-model\"\nmodel_reasoning_effort = \"none\"\n",
    )
    .unwrap();
    let paths = paths(&root);
    let args = Vec::new();

    let applied = apply_fresh_model_preference(&paths, &home, args, true, true).unwrap();
    assert_eq!(
        crate::codex_cli_config_override_value(&applied, "model").as_deref(),
        Some("legacy-model")
    );
    assert_eq!(
        crate::codex_cli_config_override_value(&applied, "model_reasoning_effort").as_deref(),
        Some("none")
    );
    let scope = model_preference_scope(&home, &[]).unwrap();
    assert_eq!(
        load_latest_model_preference(&paths, &scope)
            .unwrap()
            .unwrap()
            .source,
        "codex-config-migration"
    );
    assert!(!fs::exists(root.join("auth.json")).unwrap());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn unrelated_config_change_is_not_recorded_as_model_selection() {
    let root = std::env::temp_dir().join(format!(
        "prodex-model-preferences-unrelated-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    let config = home.join("config.toml");
    fs::write(
        &config,
        "model_provider = \"openai\"\nmodel = \"remembered\"\n",
    )
    .unwrap();
    let paths = paths(&root);
    let scope = model_preference_scope(&home, &[]).unwrap();
    let mut previous = Some(
        read_config_snapshot(std::slice::from_ref(&config))
            .unwrap()
            .unwrap(),
    );
    fs::write(
        &config,
        "model_provider = \"openai\"\nmodel = \"remembered\"\nshow_raw_agent_reasoning = true\n",
    )
    .unwrap();

    capture_changed_config(&paths, std::slice::from_ref(&config), &scope, &mut previous).unwrap();

    assert!(
        load_latest_model_preference(&paths, &scope)
            .unwrap()
            .is_none()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn malformed_preference_file_falls_back_without_blocking_launch() {
    let root = std::env::temp_dir().join(format!(
        "prodex-model-preferences-malformed-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join("config.toml"), "model_provider = \"openai\"\n").unwrap();
    fs::write(root.join(MODEL_PREFERENCE_FILE), "not-json").unwrap();

    let paths = paths(&root);
    let args = vec![OsString::from("--help")];
    assert_eq!(
        apply_fresh_model_preference(&paths, &home, args.clone(), true, true).unwrap(),
        args
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn profile_scoped_config_snapshot_overrides_base_model_and_effort() {
    let root = std::env::temp_dir().join(format!(
        "prodex-model-preferences-profile-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let base = root.join("config.toml");
    let profile = root.join("team.config.toml");
    fs::write(
        &base,
        "model = \"base\"\nmodel_reasoning_effort = \"high\"\n",
    )
    .unwrap();
    fs::write(
        &profile,
        "model = \"profile\"\nmodel_reasoning_effort = \"none\"\n",
    )
    .unwrap();

    let snapshot = read_config_snapshot(&[base, profile]).unwrap().unwrap();

    assert_eq!(snapshot.model.as_deref(), Some("profile"));
    assert_eq!(snapshot.reasoning_effort.as_deref(), Some("none"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn catalog_revision_rejects_removed_model_or_effort() {
    let root = std::env::temp_dir().join(format!(
        "prodex-model-preferences-catalog-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let catalog = root.join("catalog.json");
    fs::write(
        &catalog,
        r#"{"models":[{"slug":"model","supported_reasoning_levels":[{"effort":"high"}]}]}"#,
    )
    .unwrap();
    let args = vec![
        OsString::from("-c"),
        OsString::from(format!("model_catalog_json=\"{}\"", catalog.display())),
    ];
    let selection = LastModelSelection {
        scope: ModelPreferenceScope {
            provider: "provider".to_string(),
            catalog: "catalog".to_string(),
        },
        model: "model".to_string(),
        reasoning_effort: Some("max".to_string()),
        selected_at: 1,
        generation: 0,
        source: "test".to_string(),
    };
    assert!(model_preference_model_is_compatible(
        &root, &args, &selection
    ));
    assert!(!catalog_supports_selection(&root, &args, &selection, "max"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn config_commit_is_captured_before_child_cleanup() {
    let root = std::env::temp_dir().join(format!(
        "prodex-model-preferences-sync-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let home = root.join("codex-home");
    fs::create_dir_all(&home).unwrap();
    let config = home.join("config.toml");
    fs::write(
        &config,
        "model_provider = \"openai\"\nmodel = \"initial\"\n",
    )
    .unwrap();
    let paths = paths(&root);
    let child = ChildProcessPlan::new(OsString::from("codex"), home.clone());
    let scope = model_preference_scope(&home, &[]).unwrap();
    let mut sync = ModelPreferenceSync::start_with_scope(&paths, &child, scope.clone()).unwrap();

    fs::write(
        &config,
        "model_provider = \"openai\"\nmodel = \"selected\"\nmodel_reasoning_effort = \"none\"\n",
    )
    .unwrap();
    assert!(sync.finish().is_none());

    let selection = load_latest_model_preference(&paths, &scope)
        .unwrap()
        .unwrap();
    assert_eq!(selection.model, "selected");
    assert_eq!(selection.reasoning_effort.as_deref(), Some("none"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn catalog_scopes_do_not_share_preferences_for_same_provider() {
    let root = std::env::temp_dir().join(format!(
        "prodex-model-preferences-scope-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let paths = paths(&root);
    let first = ModelPreferenceScope {
        provider: "same-provider".to_string(),
        catalog: "catalog-a".to_string(),
    };
    let second = ModelPreferenceScope {
        provider: first.provider.clone(),
        catalog: "catalog-b".to_string(),
    };
    let other_provider = ModelPreferenceScope {
        provider: "other-provider".to_string(),
        catalog: first.catalog.clone(),
    };
    record_model_preference(
        &paths,
        LastModelSelection {
            scope: first.clone(),
            model: "model-a".to_string(),
            reasoning_effort: None,
            selected_at: 2,
            generation: 0,
            source: "test".to_string(),
        },
    )
    .unwrap();
    record_model_preference(
        &paths,
        LastModelSelection {
            scope: second.clone(),
            model: "model-b".to_string(),
            reasoning_effort: None,
            selected_at: 1,
            generation: 0,
            source: "test".to_string(),
        },
    )
    .unwrap();
    record_model_preference(
        &paths,
        LastModelSelection {
            scope: other_provider.clone(),
            model: "model-other-provider".to_string(),
            reasoning_effort: None,
            selected_at: 3,
            generation: 0,
            source: "test".to_string(),
        },
    )
    .unwrap();

    assert_eq!(
        load_latest_model_preference(&paths, &first)
            .unwrap()
            .unwrap()
            .model,
        "model-a"
    );
    assert_eq!(
        load_latest_model_preference(&paths, &second)
            .unwrap()
            .unwrap()
            .model,
        "model-b"
    );
    assert_eq!(
        load_latest_model_preference(&paths, &other_provider)
            .unwrap()
            .unwrap()
            .model,
        "model-other-provider"
    );
    let _ = fs::remove_dir_all(root);
}
