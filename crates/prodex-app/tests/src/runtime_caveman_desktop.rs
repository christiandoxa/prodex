use super::*;

#[test]
fn desktop_plan_persists_proxy_config_and_shares_chat_state() {
    let root = env::temp_dir().join(format!(
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
    let mut strategy = CavemanLaunchStrategy::new(args);
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
    let root = env::temp_dir().join(format!(
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
