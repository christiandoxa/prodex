use super::*;

#[test]
fn desktop_plan_persists_proxy_config_and_shares_chat_state() {
    let root = env::temp_dir()
        .canonicalize()
        .expect("temporary directory should resolve")
        .join(format!(
            "prodex-desktop-plan-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
    let base_home = root.join("base");
    create_codex_home_if_missing(&base_home).expect("base home should exist");
    std::fs::write(base_home.join("config.toml"), "model = 'gpt-5'\n")
        .expect("base config should write");
    std::fs::write(base_home.join("state_5.sqlite"), "shared chat index")
        .expect("base chat index should write");
    let paths = AppPaths {
        root: root.clone(),
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared-codex"),
        legacy_shared_codex_root: root.join("legacy-shared"),
    };
    let command =
        parse_cli_command_from(["prodex", "caveman"]).expect("caveman command should parse");
    let Commands::Caveman(args) = command else {
        panic!("expected caveman command");
    };
    let mut strategy = RuntimeToolLaunchStrategy::new(args);
    strategy.desktop_command = Some(DesktopGuiCommand {
        binary: OsString::from("desktop"),
        args: vec![OsString::from("--new-instance")],
    });
    strategy.configure_prodex_overlay = false;
    let prepared = PreparedRuntimeLaunch {
        paths,
        codex_home: base_home.clone(),
        managed: false,
        runtime_proxy: None,
    };
    let endpoint = RuntimeProxyEndpoint {
        listen_addr: "127.0.0.1:2455".parse().unwrap(),
        openai_mount_path: "/openai/v1".to_string(),
        local_model_provider_id: None,
        force_http_responses: false,
        realtime_ws_base_url: None,
        realtime_ws_model: None,
        lease_dir: root.join("leases"),
        broker_session_affinity_control: None,
        _lease: None,
        _direct_proxy: None,
        _kiro_connect_proxy: None,
    };

    let plan = strategy
        .build_plan(&prepared, Some(&endpoint))
        .expect("desktop plan should build");

    assert_eq!(plan.child.binary, OsString::from("desktop"));
    assert_eq!(plan.child.args, [OsString::from("--new-instance")]);
    let config: toml::Value = toml::from_str(
        &std::fs::read_to_string(plan.child.codex_home.join("config.toml"))
            .expect("desktop config should exist"),
    )
    .expect("desktop config should be valid");
    assert_eq!(config["model"].as_str(), Some("gpt-5"));
    assert_eq!(
        config["chatgpt_base_url"].as_str(),
        Some("http://127.0.0.1:2455/backend-api")
    );
    assert_eq!(
        config["openai_base_url"].as_str(),
        Some("http://127.0.0.1:2455/openai/v1")
    );
    assert!(config.get("approval_policy").is_none());
    let overlay_state = plan.child.codex_home.join("state_5.sqlite");
    assert!(
        std::fs::symlink_metadata(&overlay_state)
            .expect("desktop chat index metadata")
            .file_type()
            .is_symlink()
    );
    std::fs::write(&overlay_state, "desktop update").expect("desktop chat index should write");
    assert_eq!(
        std::fs::read_to_string(base_home.join("state_5.sqlite"))
            .expect("base chat index should persist"),
        "desktop update"
    );
    prodex_runtime_launch::cleanup_runtime_launch_plan(&plan);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn super_overlay_shares_profile_chat_state() {
    let root = env::temp_dir()
        .canonicalize()
        .expect("temporary directory should resolve")
        .join(format!(
            "prodex-super-chat-state-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
    let base_home = root.join("shared-codex");
    create_codex_home_if_missing(&base_home).expect("base home should exist");
    std::fs::write(base_home.join("state_5.sqlite"), "shared chat index")
        .expect("base chat index should write");
    let paths = AppPaths {
        root: root.clone(),
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: base_home.clone(),
        legacy_shared_codex_root: root.join("legacy-shared"),
    };

    let overlay =
        prepare_prodex_overlay_home(&paths, &base_home).expect("Super overlay should prepare");
    let overlay_state = overlay.join("state_5.sqlite");
    assert!(
        std::fs::symlink_metadata(&overlay_state)
            .expect("Super chat index metadata")
            .file_type()
            .is_symlink()
    );
    std::fs::write(&overlay_state, "Super update").expect("Super chat index should write");
    assert_eq!(
        std::fs::read_to_string(base_home.join("state_5.sqlite"))
            .expect("base chat index should persist"),
        "Super update"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn codex_01491_overlays_preserve_exact_thread_source_and_image_budget_child_args() {
    const IMAGE_BUDGET: &str = "features.compaction_image_budget=true";
    const THREAD_SOURCE: &str = "automated_review";

    let root = env::temp_dir()
        .canonicalize()
        .expect("temporary directory should resolve")
        .join(format!(
            "prodex-codex-01491-overlay-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
    let base_home = root.join("base");
    create_codex_home_if_missing(&base_home).expect("base home should exist");
    let prepared = PreparedRuntimeLaunch {
        paths: AppPaths {
            root: root.clone(),
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("shared-codex"),
            legacy_shared_codex_root: root.join("legacy-shared"),
        },
        codex_home: base_home.clone(),
        managed: false,
        runtime_proxy: None,
    };

    let command = parse_cli_command_from([
        "prodex",
        "caveman",
        "-c",
        IMAGE_BUDGET,
        "exec",
        "--thread-source",
        THREAD_SOURCE,
        "-",
    ])
    .expect("Caveman launch should parse");
    let Commands::Caveman(mut args) = command else {
        panic!("expected Caveman command");
    };
    args.dry_run = true;
    args.tools.clear();
    let mut strategy = RuntimeToolLaunchStrategy::new(args);
    let plan = strategy
        .build_plan(&prepared, None)
        .expect("Caveman overlay plan should build");
    assert_ne!(plan.child.codex_home, base_home);
    assert_eq!(
        plan.child.args,
        [
            OsString::from("exec"),
            OsString::from("-c"),
            OsString::from(IMAGE_BUDGET),
            OsString::from("--thread-source"),
            OsString::from(THREAD_SOURCE),
            OsString::from("-"),
        ]
    );
    prodex_runtime_launch::cleanup_runtime_launch_plan(&plan);

    let command = parse_cli_command_from([
        "prodex",
        "s",
        "--no-presidio",
        "--no-sub-agent",
        "-c",
        IMAGE_BUDGET,
        "exec",
        "--thread-source",
        THREAD_SOURCE,
        "-",
    ])
    .expect("Super launch should parse");
    let Commands::Super(args) = command else {
        panic!("expected Super command");
    };
    let mut args = args.into_runtime_tool_args_with_presidio(false);
    args.dry_run = true;
    args.tools.clear();
    let mut strategy = RuntimeToolLaunchStrategy::new(args);
    let plan = strategy
        .build_plan(&prepared, None)
        .expect("Super overlay plan should build");
    let workspace = serde_json::to_string(&env::current_dir().unwrap().to_string_lossy())
        .expect("workspace path should serialize");
    assert_eq!(
        plan.child.args,
        [
            OsString::from("--dangerously-bypass-hook-trust"),
            OsString::from("--dangerously-bypass-approvals-and-sandbox"),
            OsString::from("exec"),
            OsString::from("-c"),
            OsString::from(format!(
                "projects={{{workspace}={{trust_level=\"trusted\"}}}}"
            )),
            OsString::from("-c"),
            OsString::from("features.apps=false"),
            OsString::from("-c"),
            OsString::from(IMAGE_BUDGET),
            OsString::from("--thread-source"),
            OsString::from(THREAD_SOURCE),
            OsString::from("-"),
        ]
    );
    prodex_runtime_launch::cleanup_runtime_launch_plan(&plan);
    std::fs::remove_dir_all(root).expect("test root should be removed");
}

#[test]
fn super_dry_run_resolves_every_registered_main_agent_provider() {
    for (provider, expected) in [
        ("openai", None),
        ("prodex-anthropic", Some(SuperExternalProvider::Anthropic)),
        ("prodex-copilot", Some(SuperExternalProvider::Copilot)),
        ("prodex-deepseek", Some(SuperExternalProvider::DeepSeek)),
        ("prodex-gemini", Some(SuperExternalProvider::Gemini)),
        ("prodex-kiro", Some(SuperExternalProvider::Kiro)),
    ] {
        let command = parse_cli_command_from([
            OsString::from("prodex"),
            OsString::from("s"),
            OsString::from("--dry-run"),
            OsString::from("--no-presidio"),
            OsString::from("--no-sub-agent"),
            OsString::from("-c"),
            OsString::from(format!("model_provider=\"{provider}\"")),
            OsString::from("exec"),
            OsString::from("review"),
        ])
        .expect("Super provider override should parse");
        let Commands::Super(mut args) = command else {
            panic!("expected Super command");
        };
        args.extract_provider_overrides_from_codex_args()
            .expect("Super tail should extract");

        crate::app_commands::runtime_launch::resolve_super_dry_run_main_agent(&mut args)
            .expect("registered provider should resolve");
        let runtime_args =
            crate::app_commands::runtime_launch::resolved_super_runtime_tool_args(args, false);

        assert_eq!(runtime_args.external_provider, expected, "{provider}");
        assert_eq!(
            codex_cli_config_override_value(&runtime_args.codex_args, "model_provider").as_deref(),
            Some(provider),
            "{provider}"
        );
    }

    let Commands::Super(mut local) = parse_cli_command_from([
        "prodex",
        "s",
        "--dry-run",
        "--no-presidio",
        "--no-sub-agent",
        "--url",
        "http://127.0.0.1:8131/v1",
    ])
    .expect("local Super provider should parse") else {
        panic!("expected Super command");
    };
    crate::app_commands::runtime_launch::resolve_super_dry_run_main_agent(&mut local)
        .expect("local provider should resolve");
    let local = crate::app_commands::runtime_launch::resolved_super_runtime_tool_args(local, false);
    assert_eq!(
        codex_cli_config_override_value(&local.codex_args, "model_provider").as_deref(),
        Some(SUPER_LOCAL_PROVIDER_ID)
    );

    let Commands::Super(mut unknown) = parse_cli_command_from([
        "prodex",
        "s",
        "--dry-run",
        "--no-presidio",
        "--no-sub-agent",
        "-c",
        "model_provider=\"unknown-provider\"",
    ])
    .expect("unknown provider should parse before resolution") else {
        panic!("expected Super command");
    };
    assert!(
        crate::app_commands::runtime_launch::resolve_super_dry_run_main_agent(&mut unknown)
            .unwrap_err()
            .to_string()
            .contains("unsupported by Prodex Super")
    );
}

#[test]
fn profile_v2_keeps_remembered_model_overrides_without_rewriting_named_config() {
    let root = env::temp_dir().join(format!(
        "prodex-profile-v2-model-preference-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let base_config = root.join("config.toml");
    let profile_config = root.join("team.config.toml");
    std::fs::write(&base_config, "model = \"base-model\"\n").unwrap();
    std::fs::write(&profile_config, "model = \"profile-model\"\n").unwrap();
    let mut args = vec![
        OsString::from("-c"),
        OsString::from("model=\"remembered-model\""),
        OsString::from("-c"),
        OsString::from("model_reasoning_effort=\"max\""),
    ];
    let original = args.clone();

    crate::project_in_app_resume_model_settings(
        &root,
        &mut args,
        Some("team"),
        [
            ("model", true),
            ("model_provider", true),
            ("model_reasoning_effort", true),
        ],
    )
    .unwrap();

    assert_eq!(args, original);
    assert_eq!(
        std::fs::read_to_string(base_config).unwrap(),
        "model = \"base-model\"\n"
    );
    assert_eq!(
        std::fs::read_to_string(profile_config).unwrap(),
        "model = \"profile-model\"\n"
    );
    let _ = std::fs::remove_dir_all(root);
}
