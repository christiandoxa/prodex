use super::*;

#[test]
fn run_strategy_projects_automatic_model_preferences_for_in_app_resume() {
    let root = temp_dir("run-in-app-resume-model-preference");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_codex_home = root.join("shared-codex-home");
    let _shared_env = TestEnvVarGuard::set(
        "PRODEX_SHARED_CODEX_HOME",
        shared_codex_home.to_str().unwrap(),
    );
    let paths = AppPaths::discover().unwrap();
    let profile_home = root.join("profile-home");
    fs::create_dir_all(&profile_home).unwrap();
    fs::write(
        profile_home.join("config.toml"),
        "model_provider = \"openai\"\n",
    )
    .unwrap();
    write_runtime_launch_auth(
        secret_store::auth_json_path(&profile_home),
        r#"{"tokens":{"access_token":"profile-token"}}"#,
    )
    .unwrap();
    let scope = model_preference_scope(&profile_home, &[]).unwrap();
    let child = ChildProcessPlan::new("codex".into(), profile_home.clone());
    let mut sync = ModelPreferenceSync::start_with_scope(&paths, &child, scope).unwrap();
    fs::write(
        profile_home.join("config.toml"),
        concat!(
            "model_provider = \"openai\"\n",
            "model = \"remembered-model\"\n",
            "model_reasoning_effort = \"max\"\n"
        ),
    )
    .unwrap();
    assert!(sync.finish().is_none());
    write_state(
        &root,
        AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: profile_home,
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        },
    );

    let mut strategy = RunCommandStrategy::new(RunArgs {
        profile: Some("main".to_string()),
        auto_rotate: false,
        no_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: true,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: true,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: Vec::new(),
    })
    .unwrap();
    let prepared = prepare_runtime_launch(strategy.runtime_request()).unwrap();
    let plan = strategy
        .build_plan(&prepared, prepared.runtime_proxy.as_ref())
        .unwrap();

    for key in ["model", "model_provider", "model_reasoning_effort"] {
        assert!(
            codex_cli_config_override_value(&plan.child.args, key).is_none(),
            "{key} must not prevent Codex from restoring the selected thread model"
        );
    }
    let config: toml::Value =
        toml::from_str(&fs::read_to_string(plan.child.codex_home.join("config.toml")).unwrap())
            .unwrap();
    assert_eq!(
        config.get("model").and_then(toml::Value::as_str),
        Some("remembered-model")
    );
    assert_eq!(
        config
            .get("model_reasoning_effort")
            .and_then(toml::Value::as_str),
        Some("max")
    );
}

#[test]
fn runtime_launch_parses_model_context_window_override() {
    assert_eq!(
        runtime_launch_cli_model_context_window_tokens(&[
            OsString::from("-c"),
            OsString::from("model_context_window=65536"),
        ]),
        Some(65_536)
    );
}

#[test]
fn runtime_launch_reads_profile_v2_model_context_window_overlay() {
    let root = temp_dir("profile-v2-context-window");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("config.toml"), "model_context_window = 8192\n").unwrap();
    fs::write(
        root.join("local.config.toml"),
        "model_context_window = 65536\n",
    )
    .unwrap();

    assert!(
        codex_profile_v2_config_path(&root, "local")
            .unwrap()
            .exists()
    );
    assert_eq!(
        runtime_launch_config_model_context_window_tokens(&root),
        Some(8192)
    );
    let local_context =
        runtime_launch_config_model_context_window_tokens_with_profile_v2(&root, Some("local"))
            .unwrap();
    let fallback_context =
        runtime_launch_config_model_context_window_tokens_with_profile_v2(&root, Some("missing"))
            .unwrap();
    assert_eq!(local_context, Some(65_536));
    assert_eq!(fallback_context, Some(8192));
}
