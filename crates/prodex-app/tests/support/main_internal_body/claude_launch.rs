use super::*;

#[test]
fn claude_command_accepts_passthrough_args() {
    let command = parse_cli_command_from([
        "prodex",
        "claude",
        "--profile",
        "main",
        "--",
        "-p",
        "--output-format",
        "json",
        "hello",
    ])
    .expect("claude command should parse");
    let Commands::Claude(args) = command else {
        panic!("expected claude command");
    };
    assert_eq!(args.profile.as_deref(), Some("main"));
    assert_eq!(
        args.claude_args,
        vec![
            OsString::from("-p"),
            OsString::from("--output-format"),
            OsString::from("json"),
            OsString::from("hello"),
        ]
    );
}

#[test]
fn claude_caveman_mode_extracts_prefix_and_preserves_passthrough_args() {
    let (launch_modes, claude_args) = runtime_proxy_claude_extract_launch_modes(&[
        OsString::from("caveman"),
        OsString::from("-p"),
        OsString::from("hello"),
    ]);
    assert!(launch_modes.caveman_mode);
    assert_eq!(
        claude_args,
        vec![OsString::from("-p"), OsString::from("hello")]
    );

    let (launch_modes, claude_args) =
        runtime_proxy_claude_extract_launch_modes(&[OsString::from("-p"), OsString::from("hi")]);
    assert!(!launch_modes.caveman_mode);
    assert_eq!(
        claude_args,
        vec![OsString::from("-p"), OsString::from("hi")]
    );
}

#[test]
fn runtime_proxy_claude_launch_args_prepend_plugin_dirs_when_present() {
    let launch_args = runtime_proxy_claude_launch_args(
        &[OsString::from("-p"), OsString::from("hello")],
        &[PathBuf::from("/tmp/prodex-caveman-plugin")],
    );
    assert_eq!(
        launch_args,
        vec![
            OsString::from("--plugin-dir"),
            OsString::from("/tmp/prodex-caveman-plugin"),
            OsString::from("-p"),
            OsString::from("hello"),
        ]
    );

    let launch_args =
        runtime_proxy_claude_launch_args(&[OsString::from("-p"), OsString::from("hello")], &[]);
    assert_eq!(
        launch_args,
        vec![OsString::from("-p"), OsString::from("hello")]
    );
}

#[test]
fn prepare_runtime_proxy_claude_caveman_plugin_dir_installs_local_plugin_bundle() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.clone(),
        state_file: temp_dir.path.join("state.json"),
        managed_profiles_root: temp_dir.path.join("profiles"),
        shared_codex_root: temp_dir.path.join(".codex"),
        legacy_shared_codex_root: temp_dir.path.join("shared"),
    };

    let plugin_dir = prepare_runtime_proxy_claude_caveman_plugin_dir(&paths)
        .expect("Claude Caveman plugin dir should prepare");
    assert!(
        plugin_dir.join(".claude-plugin/plugin.json").is_file(),
        "plugin manifest should exist"
    );
    assert!(
        plugin_dir.join("commands/caveman.toml").is_file(),
        "caveman command should exist"
    );
    assert!(
        plugin_dir.join("skills/caveman/SKILL.md").is_file(),
        "caveman skill should exist"
    );

    let activate_hook = fs::read_to_string(plugin_dir.join("hooks/caveman-activate.js"))
        .expect("activation hook should read");
    assert!(activate_hook.contains("CLAUDE_CONFIG_DIR"));
    let tracker_hook = fs::read_to_string(plugin_dir.join("hooks/caveman-mode-tracker.js"))
        .expect("tracker hook should read");
    assert!(tracker_hook.contains("getClaudeConfigDir"));
    let statusline = fs::read_to_string(plugin_dir.join("hooks/caveman-statusline.sh"))
        .expect("statusline script should read");
    assert!(statusline.contains("CLAUDE_CONFIG_DIR"));
}

#[test]
fn prepare_prodex_overlay_home_localizes_config_and_installs_plugin() {
    let _env_guard = TestEnvVarGuard::unset(PRODEX_CAVEMAN_FULL_ASSETS_ENV);
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.clone(),
        state_file: temp_dir.path.join("state.json"),
        managed_profiles_root: temp_dir.path.join("profiles"),
        shared_codex_root: temp_dir.path.join(".codex"),
        legacy_shared_codex_root: temp_dir.path.join("shared"),
    };
    create_codex_home_if_missing(&paths.shared_codex_root).expect("shared codex root");
    create_codex_home_if_missing(&paths.managed_profiles_root).expect("managed root");
    let shared_config = paths.shared_codex_root.join("config.toml");
    fs::write(
        &shared_config,
        "model = \"gpt-5\"\n[features]\nsearch_tool = true\n",
    )
    .expect("shared config should write");

    let base_home = paths.managed_profiles_root.join("main");
    create_codex_home_if_missing(&base_home).expect("base home");
    runtime_proxy_create_symlink(&shared_config, &base_home.join("config.toml"), false)
        .expect("config symlink should create");
    fs::write(base_home.join("auth.json"), "{}").expect("auth file should write");

    let overlay_home =
        prepare_prodex_overlay_home(&paths, &base_home).expect("Prodex overlay should prepare");
    let temp_config = overlay_home.join("config.toml");
    let metadata = fs::symlink_metadata(&temp_config).expect("temp config metadata");
    assert!(
        !metadata.file_type().is_symlink(),
        "temporary Prodex overlay config should be detached from the shared config symlink"
    );

    let rendered_config = fs::read_to_string(&temp_config).expect("temp config should read");
    assert!(rendered_config.contains("plugins = false"));
    assert!(!rendered_config.contains("codex_hooks"));
    assert!(!rendered_config.contains("suppress_unstable_features_warning"));
    assert!(rendered_config.contains("[[hooks.SessionStart]]"));
    assert!(rendered_config.contains("[[hooks.SessionStart.hooks]]"));
    assert!(rendered_config.contains("type = \"command\""));
    assert!(rendered_config.contains("command = \"prodex-caveman-sessionstart\""));
    assert!(rendered_config.contains("[marketplaces.prodex-caveman]"));
    assert!(rendered_config.contains("[plugins.\"caveman@prodex-caveman\"]"));
    assert!(rendered_config.contains("enabled = true"));
    let parsed_config: toml::Value =
        toml::from_str(&rendered_config).expect("temp config should parse");
    assert_eq!(
        parsed_config["hooks"]["SessionStart"][0]["hooks"][0]["type"].as_str(),
        Some("command")
    );
    assert!(
        parsed_config["hooks"]["SessionStart"][0]["hooks"][0]["command"]
            .as_str()
            .is_some_and(|command| command == "prodex-caveman-sessionstart")
    );
    let hook_script = fs::read_to_string(overlay_home.join("bin/prodex-caveman-sessionstart"))
        .expect("Caveman SessionStart script should exist");
    assert!(hook_script.contains("CAVEMAN MODE ACTIVE"));
    assert!(hook_script.contains("PRODEX SUPER OPTIMIZERS ACTIVE WHEN AVAILABLE"));
    assert!(hook_script.contains("rtk <cmd>"));
    assert!(hook_script.contains("prodex-sqz"));
    assert!(hook_script.contains("prodex-token-savior"));
    assert!(hook_script.contains("prodex-claw-compactor"));
    assert!(hook_script.contains("Presidio is opt-in only"));
    assert!(hook_script.contains(".prodex-hooks/caveman-sessionstart"));
    let hook_key = format!("{}:session_start:0:0", temp_config.display());
    let trusted_hash = parsed_config["hooks"]["state"][&hook_key]["trusted_hash"]
        .as_str()
        .expect("Caveman hook should be auto-trusted for the temporary config source");
    assert!(
        trusted_hash.starts_with("sha256:") && trusted_hash.len() == "sha256:".len() + 64,
        "trusted hook hash should use Codex canonical sha256 format"
    );

    let shared_rendered = fs::read_to_string(&shared_config).expect("shared config should read");
    assert!(
        !shared_rendered.contains("prodex-caveman"),
        "base shared config must stay unchanged"
    );
    assert!(
        !base_home.join("hooks.json").exists(),
        "base home should not gain a persistent hooks.json file"
    );
    assert!(
        !overlay_home.join("hooks.json").exists(),
        "temporary Prodex overlay home should use inline config.toml hooks"
    );

    let marketplace_path =
        overlay_home.join(".tmp/marketplaces/prodex-caveman/.agents/plugins/marketplace.json");
    let marketplace_text =
        fs::read_to_string(&marketplace_path).expect("marketplace manifest should read");
    assert!(marketplace_text.contains("\"name\": \"prodex-caveman\""));
    assert!(
        overlay_home
            .join(".tmp/marketplaces/prodex-caveman/plugins/caveman/.codex-plugin/plugin.json")
            .is_file()
    );
    assert!(
        overlay_home
            .join("plugins/cache/prodex-caveman/caveman/0.1.0/.codex-plugin/plugin.json")
            .is_file()
    );
    assert!(
        overlay_home
            .join(".tmp/marketplaces/prodex-caveman/plugins/caveman/skills/caveman/SKILL.md")
            .is_file(),
        "core Caveman skill should install by default"
    );
    assert!(
        !overlay_home
            .join(".tmp/marketplaces/prodex-caveman/plugins/caveman/skills/compress/SKILL.md")
            .exists(),
        "compress skill should not install in the default lean overlay"
    );
    assert!(
        !overlay_home
            .join("plugins/cache/prodex-caveman/caveman/0.1.0/skills/compress/SKILL.md")
            .exists(),
        "compress skill should not install in the default plugin cache"
    );
}

#[test]
fn prepare_prodex_overlay_home_can_install_full_caveman_assets() {
    let _env_guard = TestEnvVarGuard::set(PRODEX_CAVEMAN_FULL_ASSETS_ENV, "1");
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.clone(),
        state_file: temp_dir.path.join("state.json"),
        managed_profiles_root: temp_dir.path.join("profiles"),
        shared_codex_root: temp_dir.path.join(".codex"),
        legacy_shared_codex_root: temp_dir.path.join("shared"),
    };
    create_codex_home_if_missing(&paths.shared_codex_root).expect("shared codex root");
    create_codex_home_if_missing(&paths.managed_profiles_root).expect("managed root");

    let base_home = paths.managed_profiles_root.join("main");
    create_codex_home_if_missing(&base_home).expect("base home");

    let overlay_home =
        prepare_prodex_overlay_home(&paths, &base_home).expect("Prodex overlay should prepare");
    assert!(
        overlay_home
            .join(".tmp/marketplaces/prodex-caveman/plugins/caveman/skills/compress/SKILL.md")
            .is_file(),
        "compress skill should install when full assets are enabled"
    );
    assert!(
        overlay_home
            .join("plugins/cache/prodex-caveman/caveman/0.1.0/skills/compress/scripts/compress.py")
            .is_file(),
        "compress scripts should install when full assets are enabled"
    );
}
