use super::*;

#[test]
fn prepare_runtime_launch_skips_proxy_for_non_openai_model_provider() {
    let root = temp_dir("skip-proxy-non-openai");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let bedrock_home = root.join("bedrock-home");
    let openai_home = root.join("openai-home");
    fs::create_dir_all(&bedrock_home).unwrap();
    fs::create_dir_all(&openai_home).unwrap();
    fs::write(
        bedrock_home.join("config.toml"),
        "model_provider = 'amazon-bedrock'\n",
    )
    .unwrap();
    fs::write(
        secret_store::auth_json_path(&openai_home),
        r#"{"tokens":{"access_token":"chatgpt-token"}}"#,
    )
    .unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("bedrock".to_string()),
            profiles: BTreeMap::from([
                (
                    "bedrock".to_string(),
                    ProfileEntry {
                        codex_home: bedrock_home.clone(),
                        managed: false,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                ),
                (
                    "openai".to_string(),
                    ProfileEntry {
                        codex_home: openai_home,
                        managed: false,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                ),
            ]),
            ..AppState::default()
        },
    );

    let prepared = prepare_runtime_launch(RuntimeLaunchRequest {
        profile: Some("bedrock"),
        allow_auto_rotate: true,
        skip_quota_check: false,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: false,
        model_context_window_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: None,
        profile_v2_name: None,
    })
    .unwrap();

    assert_eq!(prepared.codex_home, bedrock_home);
    assert!(prepared.runtime_proxy.is_none());
}

#[test]
fn prepare_runtime_launch_rejects_claude_for_non_openai_model_provider() {
    let root = temp_dir("reject-claude-non-openai");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let bedrock_home = root.join("bedrock-home");
    fs::create_dir_all(&bedrock_home).unwrap();
    fs::write(
        bedrock_home.join("config.toml"),
        "model_provider = 'amazon-bedrock'\n",
    )
    .unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("bedrock".to_string()),
            profiles: BTreeMap::from([(
                "bedrock".to_string(),
                ProfileEntry {
                    codex_home: bedrock_home,
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        },
    );

    let err = match prepare_runtime_launch(RuntimeLaunchRequest {
        profile: Some("bedrock"),
        allow_auto_rotate: true,
        skip_quota_check: false,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: false,
        model_context_window_tokens: None,
        force_runtime_proxy: true,
        model_provider_override: None,
        profile_v2_name: None,
    }) {
        Ok(_) => panic!("expected Claude launch to reject non-OpenAI model providers"),
        Err(err) => err,
    };

    let message = format!("{err:#}");
    assert!(message.contains("amazon-bedrock"));
    assert!(message.contains("prodex claude"));
}

#[test]
fn prepare_runtime_launch_dry_run_uses_proxy_preview_without_recording_selection() {
    let root = temp_dir("dry-run-preview-no-selection-save");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let main_home = root.join("main-home");
    let second_home = root.join("second-home");
    fs::create_dir_all(&main_home).unwrap();
    fs::create_dir_all(&second_home).unwrap();
    fs::write(
        secret_store::auth_json_path(&main_home),
        r#"{"tokens":{"access_token":"main-token"}}"#,
    )
    .unwrap();
    fs::write(
        secret_store::auth_json_path(&second_home),
        r#"{"tokens":{"access_token":"second-token"}}"#,
    )
    .unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([
                (
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home.clone(),
                        managed: false,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: false,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                ),
            ]),
            ..AppState::default()
        },
    );

    let prepared = prepare_runtime_launch_dry_run(RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: true,
        skip_quota_check: false,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: false,
        model_context_window_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: None,
        profile_v2_name: None,
    })
    .unwrap();

    assert_eq!(prepared.codex_home, main_home);
    assert_eq!(
        prepared
            .runtime_proxy
            .as_ref()
            .expect("runtime proxy preview should exist")
            .listen_addr
            .port(),
        0
    );
    let paths = AppPaths::discover().unwrap();
    let state = AppState::load(&paths).unwrap();
    assert!(
        state.last_run_selected_at.is_empty(),
        "dry-run must not record launch selection"
    );
}

#[test]
fn prepare_runtime_launch_allows_profileless_local_home_when_no_profiles_exist() {
    let root = temp_dir("profileless-local-home");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let paths = AppPaths::discover().unwrap();

    let prepared = prepare_runtime_launch(RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: true,
        skip_quota_check: true,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: false,
        model_context_window_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: Some("prodex-local"),
        profile_v2_name: None,
    })
    .unwrap();

    assert_eq!(prepared.codex_home, paths.shared_codex_root);
    assert!(prepared.codex_home.is_dir());
    assert!(!prepared.managed);
    assert!(prepared.runtime_proxy.is_none());
    assert!(
        !paths.state_file.exists(),
        "profileless local launch should not persist synthetic profile selection"
    );
}

#[test]
fn prepare_runtime_launch_profile_v2_config_enables_profileless_local_rewrite_proxy() {
    let root = temp_dir("profile-v2-profileless-local-smart-context-proxy");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let paths = AppPaths::discover().unwrap();
    fs::create_dir_all(&paths.shared_codex_root).unwrap();
    fs::write(
        paths.shared_codex_root.join("local.config.toml"),
        "model_provider = 'prodex-local'\n[model_providers.prodex-local]\nbase_url = 'http://127.0.0.1:8131/v1'\n",
    )
    .unwrap();

    let prepared = prepare_runtime_launch(RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: true,
        skip_quota_check: true,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: true,
        model_context_window_tokens: Some(65_536),
        force_runtime_proxy: false,
        model_provider_override: None,
        profile_v2_name: Some("local"),
    })
    .unwrap();

    assert_eq!(prepared.codex_home, paths.shared_codex_root);
    assert!(!prepared.managed);
    let runtime_proxy = prepared
        .runtime_proxy
        .as_ref()
        .expect("profile-v2 prodex-local should use local rewrite proxy");
    assert_eq!(
        runtime_proxy.local_model_provider_id.as_deref(),
        Some(SUPER_LOCAL_PROVIDER_ID)
    );
    assert_eq!(
        runtime_proxy.openai_mount_path,
        RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH
    );
}

#[test]
fn prepare_runtime_launch_enables_local_rewrite_proxy_for_prodex_local_smart_context() {
    let root = temp_dir("profileless-local-smart-context-proxy");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let paths = AppPaths::discover().unwrap();

    let prepared = prepare_runtime_launch(RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: true,
        skip_quota_check: true,
        base_url: Some("http://127.0.0.1:8131/v1"),
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: true,
        model_context_window_tokens: Some(65_536),
        force_runtime_proxy: false,
        model_provider_override: Some(SUPER_LOCAL_PROVIDER_ID),
        profile_v2_name: None,
    })
    .unwrap();

    assert_eq!(prepared.codex_home, paths.shared_codex_root);
    assert!(prepared.codex_home.is_dir());
    assert!(!prepared.managed);
    let runtime_proxy = prepared
        .runtime_proxy
        .as_ref()
        .expect("prodex-local Smart Context should use local rewrite proxy");
    assert_eq!(
        runtime_proxy.local_model_provider_id.as_deref(),
        Some(SUPER_LOCAL_PROVIDER_ID)
    );
    assert_eq!(
        runtime_proxy.openai_mount_path,
        RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH
    );
    assert!(
        !paths.state_file.exists(),
        "profileless local proxy launch should not persist synthetic profile selection"
    );
}

#[test]
fn prepare_runtime_launch_profileless_local_flag_preserves_existing_profiles() {
    let root = temp_dir("profileless-local-preserve-profile");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let main_home = root.join("main-home");
    fs::create_dir_all(&main_home).unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home.clone(),
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        },
    );

    let prepared = prepare_runtime_launch_dry_run(RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: true,
        skip_quota_check: true,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: false,
        model_context_window_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: Some("prodex-local"),
        profile_v2_name: None,
    })
    .unwrap();

    assert_eq!(prepared.codex_home, main_home);
    assert!(!prepared.managed);
    assert!(prepared.runtime_proxy.is_none());
}

#[test]
fn prepare_runtime_launch_uses_profile_v2_model_provider_overlay() {
    let root = temp_dir("profile-v2-provider-overlay");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let main_home = root.join("main-home");
    fs::create_dir_all(&main_home).unwrap();
    fs::write(main_home.join("config.toml"), "model_provider = 'openai'\n").unwrap();
    fs::write(
        main_home.join("bedrock.config.toml"),
        "model_provider = 'amazon-bedrock'\n",
    )
    .unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home.clone(),
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        },
    );

    let prepared = prepare_runtime_launch(RuntimeLaunchRequest {
        profile: Some("main"),
        allow_auto_rotate: true,
        skip_quota_check: true,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: false,
        model_context_window_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: None,
        profile_v2_name: Some("bedrock"),
    })
    .unwrap();

    assert_eq!(prepared.codex_home, main_home);
    assert!(prepared.runtime_proxy.is_none());
}

#[test]
fn prepare_runtime_launch_explicit_profile_keeps_profile_home_with_local_override() {
    let root = temp_dir("explicit-profile-local-override");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let main_home = root.join("main-home");
    fs::create_dir_all(&main_home).unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home.clone(),
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        },
    );

    let prepared = prepare_runtime_launch(RuntimeLaunchRequest {
        profile: Some("main"),
        allow_auto_rotate: true,
        skip_quota_check: true,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: false,
        model_context_window_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: Some(SUPER_LOCAL_PROVIDER_ID),
        profile_v2_name: None,
    })
    .unwrap();

    assert_eq!(prepared.codex_home, main_home);
    assert!(!prepared.managed);
    assert!(prepared.runtime_proxy.is_none());
}

#[test]
fn prepare_runtime_launch_dry_run_skips_proxy_for_non_openai_model_provider() {
    let root = temp_dir("dry-run-skip-proxy-non-openai");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let bedrock_home = root.join("bedrock-home");
    fs::create_dir_all(&bedrock_home).unwrap();
    fs::write(
        bedrock_home.join("config.toml"),
        "model_provider = 'amazon-bedrock'\n",
    )
    .unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("bedrock".to_string()),
            profiles: BTreeMap::from([(
                "bedrock".to_string(),
                ProfileEntry {
                    codex_home: bedrock_home.clone(),
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        },
    );

    let prepared = prepare_runtime_launch_dry_run(RuntimeLaunchRequest {
        profile: Some("bedrock"),
        allow_auto_rotate: true,
        skip_quota_check: false,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: false,
        model_context_window_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: None,
        profile_v2_name: None,
    })
    .unwrap();

    assert_eq!(prepared.codex_home, bedrock_home);
    assert!(prepared.runtime_proxy.is_none());
    let paths = AppPaths::discover().unwrap();
    let state = AppState::load(&paths).unwrap();
    assert!(
        state.last_run_selected_at.is_empty(),
        "dry-run must not record launch selection"
    );
}

#[test]
fn prepare_runtime_launch_dry_run_previews_local_rewrite_proxy_for_prodex_local_smart_context() {
    let root = temp_dir("dry-run-local-smart-context-proxy");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());

    let prepared = prepare_runtime_launch_dry_run(RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: true,
        skip_quota_check: true,
        base_url: Some("http://127.0.0.1:8131/v1"),
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: true,
        model_context_window_tokens: Some(65_536),
        force_runtime_proxy: false,
        model_provider_override: Some(SUPER_LOCAL_PROVIDER_ID),
        profile_v2_name: None,
    })
    .unwrap();

    let runtime_proxy = prepared
        .runtime_proxy
        .as_ref()
        .expect("dry-run should preview local rewrite proxy");
    assert_eq!(runtime_proxy.listen_addr.port(), 0);
    assert_eq!(
        runtime_proxy.local_model_provider_id.as_deref(),
        Some(SUPER_LOCAL_PROVIDER_ID)
    );
    assert_eq!(
        runtime_proxy.openai_mount_path,
        RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH
    );
    let paths = AppPaths::discover().unwrap();
    assert!(
        !paths.state_file.exists(),
        "dry-run local proxy preview must not persist synthetic profile selection"
    );
}

#[test]
fn prepare_runtime_launch_rejects_force_proxy_for_profileless_local_home() {
    let root = temp_dir("profileless-local-force-proxy");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());

    let err = match prepare_runtime_launch(RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: true,
        skip_quota_check: true,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: false,
        model_context_window_tokens: None,
        force_runtime_proxy: true,
        model_provider_override: Some(SUPER_LOCAL_PROVIDER_ID),
        profile_v2_name: None,
    }) {
        Ok(_) => panic!("expected forced proxy launch to reject profileless local provider"),
        Err(err) => err,
    };

    let message = format!("{err:#}");
    assert!(message.contains(SUPER_LOCAL_PROVIDER_ID));
    assert!(message.contains("prodex claude"));
}

#[test]
fn no_ready_runtime_profiles_returns_error_for_blocked_report() {
    let report = RunProfileProbeReport {
        name: "main".to_string(),
        order_index: 0,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Ok(UsageResponse {
            email: None,
            plan_type: None,
            rate_limit: None,
            code_review_rate_limit: None,
            additional_rate_limits: Vec::new(),
        }),
    };

    let err = handle_no_ready_runtime_profiles(
        &report,
        "main",
        &RuntimeLaunchRequest {
            profile: None,
            allow_auto_rotate: true,
            skip_quota_check: false,
            base_url: None,
            upstream_no_proxy: false,
            include_code_review: false,
            smart_context_enabled: false,
            model_context_window_tokens: None,
            force_runtime_proxy: false,
            model_provider_override: None,
            profile_v2_name: None,
        },
    )
    .expect_err("blocked preflight should return an error instead of exiting");

    let message = format!("{err:#}");
    assert!(message.contains("quota preflight blocked profile 'main'"));
    assert!(message.contains("no ready profile"));
    assert_eq!(
        err.downcast_ref::<crate::command_dispatch::ProdexCommandExit>()
            .expect("blocked preflight should carry an explicit exit code")
            .code(),
        2
    );
}

#[test]
fn no_ready_runtime_profiles_continues_when_probe_failed() {
    let report = RunProfileProbeReport {
        name: "main".to_string(),
        order_index: 0,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Err("network down".to_string()),
    };

    handle_no_ready_runtime_profiles(
        &report,
        "main",
        &RuntimeLaunchRequest {
            profile: None,
            allow_auto_rotate: true,
            skip_quota_check: false,
            base_url: None,
            upstream_no_proxy: false,
            include_code_review: false,
            smart_context_enabled: false,
            model_context_window_tokens: None,
            force_runtime_proxy: false,
            model_provider_override: None,
            profile_v2_name: None,
        },
    )
    .expect("probe failure should still continue without quota gate");
}

#[test]
fn run_command_strategy_keeps_smart_context_autopilot_disabled() {
    let strategy = RunCommandStrategy::new(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        skip_quota_check: false,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_args: vec![OsString::from("exec"), OsString::from("hello")],
    });

    assert!(!strategy.runtime_request().smart_context_enabled);
}

#[test]
fn run_command_strategy_carries_profile_v2_name() {
    let strategy = RunCommandStrategy::new(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        skip_quota_check: false,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_args: vec![
            OsString::from("exec"),
            OsString::from("--profile-v2"),
            OsString::from("bedrock"),
            OsString::from("hello"),
        ],
    });

    assert_eq!(strategy.runtime_request().profile_v2_name, Some("bedrock"));
}

#[test]
fn run_launch_route_sends_command_server_subcommands_directly() {
    let mcp_args = test_run_args(vec![
        OsString::from("mcp-server"),
        OsString::from("--stdio"),
    ]);
    let app_args = test_run_args(vec![
        OsString::from("app-server"),
        OsString::from("--stdio"),
    ]);

    assert_eq!(
        run_launch_route(&mcp_args),
        RunLaunchRoute::CodexCommandServerDirectPassthrough
    );
    assert_eq!(
        run_launch_route(&app_args),
        RunLaunchRoute::CodexCommandServerDirectPassthrough
    );
}

#[test]
fn run_launch_route_keeps_remote_control_managed() {
    let args = test_run_args(vec![OsString::from("remote-control")]);

    assert_eq!(run_launch_route(&args), RunLaunchRoute::ManagedRuntime);
}

#[test]
fn run_launch_route_keeps_explicit_dry_run_managed_for_command_server() {
    let mut args = test_run_args(vec![OsString::from("mcp-server")]);
    args.dry_run = true;

    assert_eq!(run_launch_route(&args), RunLaunchRoute::ManagedRuntime);
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
    assert_eq!(
        runtime_launch_config_model_context_window_tokens_with_profile_v2(&root, Some("local")),
        Some(65_536)
    );
    assert_eq!(
        runtime_launch_config_model_context_window_tokens_with_profile_v2(&root, Some("missing")),
        Some(8192)
    );
}

fn write_state(root: &Path, state: AppState) {
    fs::create_dir_all(root).unwrap();
    let paths = AppPaths::discover().unwrap();
    state.save(&paths).unwrap();
}

fn test_run_args(codex_args: Vec<OsString>) -> RunArgs {
    RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        skip_quota_check: false,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_args,
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!(
        "prodex-runtime-launch-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    dir
}
