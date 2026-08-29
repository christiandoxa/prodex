use super::*;

#[test]
fn resume_provider_bridge_leaves_the_session_model_authoritative() {
    let root = temp_dir("resume-provider-model");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_codex_home = root.join("shared-codex-home");
    let _shared_env = TestEnvVarGuard::set(
        "PRODEX_SHARED_CODEX_HOME",
        shared_codex_home.to_str().unwrap(),
    );
    let paths = AppPaths::discover().unwrap();
    let session_id = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9";
    let sessions = paths.shared_codex_root.join("sessions/2026/06/05");
    fs::create_dir_all(&sessions).unwrap();
    fs::write(
        sessions.join("rollout.jsonl"),
        format!(
            "{}{}",
            session_meta_line(session_id, &root, Some("prodex-kiro")),
            r#"{"timestamp":"2026-06-05T01:01:00Z","type":"turn_context","payload":{"model":"gpt-5.6-luna","effort":"max"}}
"#
        ),
    )
    .unwrap();

    let strategy = RunCommandStrategy::new(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: false,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![OsString::from(session_id)],
    })
    .unwrap();

    assert_eq!(
        strategy.auto_external_provider,
        Some(SuperExternalProvider::Kiro)
    );
    assert_eq!(
        codex_cli_config_override_value(&strategy.codex_args, "model").as_deref(),
        Some("gpt-5.6-luna")
    );
    assert_eq!(
        codex_cli_config_override_value(&strategy.codex_args, "model_reasoning_effort").as_deref(),
        Some("max")
    );
}

#[test]
fn codex_delete_cleanup_prunes_session_and_compact_bindings() {
    let root = temp_dir("delete-prune-bindings");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let _shared_env = TestEnvVarGuard::set(
        "PRODEX_SHARED_CODEX_HOME",
        root.join("shared-codex-home").to_str().unwrap(),
    );
    let paths = AppPaths::discover().unwrap();
    let session_id = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9";
    let compact_key = prodex_runtime_store::runtime_compact_session_lineage_key(session_id);
    let now = chrono::Local::now().timestamp();
    let binding = ResponseProfileBinding {
        binding_identity: None,
        profile_name: "main".to_string(),
        bound_at: now,
    };
    write_state(
        &root,
        AppState {
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: root.join("main-home"),
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            session_profile_bindings: BTreeMap::from([
                (session_id.to_string(), binding.clone()),
                (compact_key.clone(), binding.clone()),
            ]),
            ..AppState::default()
        },
    );
    let state = AppState::load(&paths).unwrap();
    let continuations = RuntimeContinuationStore {
        session_profile_bindings: BTreeMap::from([(session_id.to_string(), binding.clone())]),
        session_id_bindings: BTreeMap::from([
            (session_id.to_string(), binding.clone()),
            (compact_key.clone(), binding),
        ]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuations_for_profiles(&paths, &continuations, &state.profiles).unwrap();
    save_runtime_continuation_journal_for_profiles(&paths, &continuations, &state.profiles, now)
        .unwrap();

    cleanup_codex_deleted_session_binding(Some(session_id)).unwrap();

    let state = AppState::load(&paths).unwrap();
    assert!(!state.session_profile_bindings.contains_key(session_id));
    assert!(!state.session_profile_bindings.contains_key(&compact_key));
    for persisted in [
        load_runtime_continuations_with_recovery(&paths, &state.profiles)
            .unwrap()
            .value,
        load_runtime_continuation_journal_with_recovery(&paths, &state.profiles)
            .unwrap()
            .value
            .continuations,
    ] {
        assert!(!persisted.session_profile_bindings.contains_key(session_id));
        assert!(!persisted.session_id_bindings.contains_key(session_id));
        assert!(!persisted.session_id_bindings.contains_key(&compact_key));
        assert_eq!(
            persisted
                .statuses
                .session_id
                .get(session_id)
                .map(|status| status.state),
            Some(RuntimeContinuationBindingLifecycle::Dead)
        );
    }
}

#[test]
fn run_strategy_resolves_codex_delete_partial_selector_before_launch() {
    let root = temp_dir("delete-partial-selector");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let _shared_env = TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", "shared-codex-home");
    let paths = AppPaths::discover().unwrap();
    let session_id = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9";
    let sessions = paths.shared_codex_root.join("sessions/2026/06/05");
    fs::create_dir_all(&sessions).unwrap();
    fs::write(
        sessions.join("rollout.jsonl"),
        session_meta_line(session_id, &root, None),
    )
    .unwrap();

    let strategy = RunCommandStrategy::new(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: true,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![OsString::from("delete"), OsString::from("019c9e3d")],
    })
    .unwrap();

    assert_eq!(strategy.delete_session_id.as_deref(), Some(session_id));
}
