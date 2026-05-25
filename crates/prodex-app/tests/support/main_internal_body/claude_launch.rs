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
    assert!(!launch_modes.mem_mode);
    assert_eq!(
        claude_args,
        vec![OsString::from("-p"), OsString::from("hello")]
    );

    let (launch_modes, claude_args) =
        runtime_proxy_claude_extract_launch_modes(&[OsString::from("-p"), OsString::from("hi")]);
    assert!(!launch_modes.caveman_mode);
    assert!(!launch_modes.mem_mode);
    assert_eq!(
        claude_args,
        vec![OsString::from("-p"), OsString::from("hi")]
    );
}

#[test]
fn runtime_proxy_claude_launch_modes_extract_mem_and_caveman_prefixes() {
    let (launch_modes, claude_args) = runtime_proxy_claude_extract_launch_modes(&[
        OsString::from("caveman"),
        OsString::from("mem"),
        OsString::from("-p"),
        OsString::from("hello"),
    ]);
    assert_eq!(
        launch_modes,
        RuntimeProxyClaudeLaunchModes {
            caveman_mode: true,
            mem_mode: true,
        }
    );
    assert_eq!(
        claude_args,
        vec![OsString::from("-p"), OsString::from("hello")]
    );

    let (launch_modes, claude_args) = runtime_proxy_claude_extract_launch_modes(&[
        OsString::from("mem"),
        OsString::from("caveman"),
        OsString::from("--print"),
    ]);
    assert_eq!(
        launch_modes,
        RuntimeProxyClaudeLaunchModes {
            caveman_mode: true,
            mem_mode: true,
        }
    );
    assert_eq!(claude_args, vec![OsString::from("--print")]);
}

#[test]
fn runtime_mem_extract_mode_strips_only_leading_mem_prefix() {
    let (mem_mode, codex_args) =
        runtime_mem_extract_mode(&[OsString::from("mem"), OsString::from("exec")]);
    assert!(mem_mode);
    assert_eq!(codex_args, vec![OsString::from("exec")]);

    let (mem_mode, codex_args) =
        runtime_mem_extract_mode_with_detail(&[OsString::from("mem-full"), OsString::from("exec")]);
    assert_eq!(mem_mode, Some(RuntimeMemTranscriptMode::Full));
    assert_eq!(codex_args, vec![OsString::from("exec")]);

    let (mem_mode, codex_args) = runtime_mem_extract_mode_with_detail(&[
        OsString::from("mem"),
        OsString::from("--mem-full"),
        OsString::from("exec"),
    ]);
    assert_eq!(mem_mode, Some(RuntimeMemTranscriptMode::Full));
    assert_eq!(codex_args, vec![OsString::from("exec")]);

    let (mem_mode, codex_args) =
        runtime_mem_extract_mode(&[OsString::from("exec"), OsString::from("mem")]);
    assert!(!mem_mode);
    assert_eq!(
        codex_args,
        vec![OsString::from("exec"), OsString::from("mem")]
    );
}

#[test]
fn runtime_mem_default_schema_is_slim_and_full_schema_preserves_outputs() {
    let slim = runtime_mem_default_codex_schema().to_string();
    assert!(slim.contains("0.4-slim"));
    assert!(slim.contains("output omitted"));
    assert!(!slim.contains("\"toolResponse\":\"payload.output\""));

    let full = runtime_mem_full_codex_schema().to_string();
    assert!(full.contains("Full schema"));
    assert!(full.contains("\"toolResponse\":\"payload.output\""));
    assert!(full.contains("\"message\":\"payload.message\""));
}

#[test]
fn runtime_proxy_claude_launch_args_prepend_plugin_dirs_when_present() {
    let launch_args = runtime_proxy_claude_launch_args(
        &[OsString::from("-p"), OsString::from("hello")],
        &[
            PathBuf::from("/tmp/claude-mem-plugin"),
            PathBuf::from("/tmp/prodex-caveman-plugin"),
        ],
    );
    assert_eq!(
        launch_args,
        vec![
            OsString::from("--plugin-dir"),
            OsString::from("/tmp/claude-mem-plugin"),
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
fn runtime_mem_claude_plugin_dir_from_home_uses_marketplace_install_path() {
    let temp_dir = TestDir::new();
    let home = temp_dir.path.join("home");
    assert_eq!(
        runtime_mem_claude_plugin_dir_from_home(&home),
        home.join(".claude")
            .join("plugins")
            .join("marketplaces")
            .join("thedotmack")
            .join("plugin")
    );
}

#[test]
fn runtime_mem_transcript_watch_config_path_from_home_prefers_settings_override() {
    let temp_dir = TestDir::new();
    let home = temp_dir.path.join("home");
    let data_dir = runtime_mem_data_dir_from_home(&home);
    fs::create_dir_all(&data_dir).expect("claude-mem data dir should exist");
    fs::write(
        data_dir.join("settings.json"),
        serde_json::json!({
            "CLAUDE_MEM_TRANSCRIPTS_CONFIG_PATH": data_dir.join("custom-watch.json").display().to_string()
        })
        .to_string(),
    )
    .expect("settings should write");

    assert_eq!(
        runtime_mem_transcript_watch_config_path_from_home(&home),
        data_dir.join("custom-watch.json")
    );
}

#[test]
fn ensure_runtime_mem_prodex_observer_writes_wrapper_and_settings() {
    let temp_dir = TestDir::new();
    let home = temp_dir.path.join("home");
    let settings_path = runtime_mem_settings_path_from_home(&home);
    fs::create_dir_all(settings_path.parent().expect("settings parent"))
        .expect("settings parent should exist");
    fs::write(
        &settings_path,
        serde_json::json!({
            "CLAUDE_MEM_PROVIDER": "claude",
            "CLAUDE_CODE_PATH": "/usr/bin/claude"
        })
        .to_string(),
    )
    .expect("settings should write");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex-home"),
        state_file: temp_dir.path.join("prodex-home/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex-home/profiles"),
        shared_codex_root: temp_dir.path.join("prodex-home/.codex"),
        legacy_shared_codex_root: temp_dir.path.join("prodex-home/shared"),
    };
    let prodex_exe = temp_dir.path.join("bin/prodex");
    fs::create_dir_all(prodex_exe.parent().expect("prodex bin parent"))
        .expect("prodex bin parent should exist");
    fs::write(&prodex_exe, "").expect("prodex exe should write");

    let wrapper_path = ensure_runtime_mem_prodex_observer_for_home(&home, &paths, &prodex_exe)
        .expect("prodex observer should configure");

    assert_eq!(wrapper_path, runtime_mem_prodex_claude_wrapper_path(&paths));
    let settings: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&settings_path).expect("settings should read"))
            .expect("settings should parse");
    assert_eq!(settings["CLAUDE_MEM_PROVIDER"], serde_json::json!("claude"));
    assert_eq!(
        settings["CLAUDE_CODE_PATH"],
        serde_json::json!(wrapper_path.display().to_string())
    );

    let wrapper = fs::read_to_string(&wrapper_path).expect("wrapper should read");
    assert!(wrapper.contains(" claude --skip-quota-check -- "));
    assert!(wrapper.contains(&prodex_exe.display().to_string()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&wrapper_path)
                .expect("wrapper metadata should read")
                .permissions()
                .mode()
                & 0o111,
            0o111,
            "wrapper should be executable"
        );
    }
}

#[test]
fn ensure_runtime_mem_codex_watch_for_home_adds_prodex_watch_without_clobbering_default_watch() {
    let temp_dir = TestDir::new();
    let config_path = temp_dir.path.join("claude-mem/transcript-watch.json");
    fs::create_dir_all(config_path.parent().expect("config parent"))
        .expect("config parent should exist");
    fs::write(
        &config_path,
        serde_json::json!({
            "version": 1,
            "schemas": {
                "codex": runtime_mem_default_codex_schema(),
            },
            "watches": [{
                "name": "codex",
                "path": "~/.codex/sessions/**/*.jsonl",
                "schema": "codex",
                "startAtEnd": true,
                "context": {
                    "mode": "agents",
                    "updateOn": ["session_start", "session_end"],
                }
            }]
        })
        .to_string(),
    )
    .expect("transcript watch config should write");

    let sessions_root = temp_dir.path.join("prodex-shared/sessions");
    fs::create_dir_all(&sessions_root).expect("sessions root should exist");
    let codex_home = temp_dir.path.join("codex-home");
    fs::create_dir_all(&codex_home).expect("codex home should exist");
    runtime_proxy_create_symlink(&sessions_root, &codex_home.join("sessions"), true)
        .expect("sessions symlink should create");

    ensure_runtime_mem_codex_watch_for_home_at_path(&config_path, &codex_home)
        .expect("prodex codex watch should be added");
    ensure_runtime_mem_codex_watch_for_home_at_path(&config_path, &codex_home)
        .expect("prodex codex watch should stay deduplicated");

    let rendered: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&config_path).expect("transcript watch config should read"),
    )
    .expect("transcript watch config should parse");
    let watches = rendered["watches"]
        .as_array()
        .expect("watches should be an array");
    assert_eq!(watches.len(), 2);
    assert_eq!(watches[0]["name"], serde_json::json!("codex"));
    let prodex_watch = watches
        .iter()
        .find(|watch| {
            watch["name"]
                .as_str()
                .is_some_and(|name| name.starts_with("prodex-codex-"))
        })
        .expect("prodex watch should exist");
    assert_eq!(
        prodex_watch["path"],
        serde_json::json!(format!(
            "{}{}**{}*.jsonl",
            sessions_root.display(),
            std::path::MAIN_SEPARATOR,
            std::path::MAIN_SEPARATOR
        ))
    );
}

#[test]
fn prepare_caveman_launch_home_localizes_config_and_installs_plugin() {
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

    let caveman_home =
        prepare_caveman_launch_home(&paths, &base_home).expect("caveman home should prepare");
    let temp_config = caveman_home.join("config.toml");
    let metadata = fs::symlink_metadata(&temp_config).expect("temp config metadata");
    assert!(
        !metadata.file_type().is_symlink(),
        "temporary Caveman config should be detached from the shared config symlink"
    );

    let rendered_config = fs::read_to_string(&temp_config).expect("temp config should read");
    assert!(rendered_config.contains("plugins = true"));
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
    let hook_script = fs::read_to_string(caveman_home.join("bin/prodex-caveman-sessionstart"))
        .expect("Caveman SessionStart script should exist");
    assert!(hook_script.contains("CAVEMAN MODE ACTIVE"));
    assert!(hook_script.contains("noisy shell commands must visibly start with rtk <cmd>"));
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
        !caveman_home.join("hooks.json").exists(),
        "temporary Caveman home should use inline config.toml hooks"
    );

    let marketplace_path =
        caveman_home.join(".tmp/marketplaces/prodex-caveman/.agents/plugins/marketplace.json");
    let marketplace_text =
        fs::read_to_string(&marketplace_path).expect("marketplace manifest should read");
    assert!(marketplace_text.contains("\"name\": \"prodex-caveman\""));
    assert!(
        caveman_home
            .join(".tmp/marketplaces/prodex-caveman/plugins/caveman/.codex-plugin/plugin.json")
            .is_file()
    );
    assert!(
        caveman_home
            .join("plugins/cache/prodex-caveman/caveman/0.1.0/.codex-plugin/plugin.json")
            .is_file()
    );
    assert!(
        caveman_home
            .join(".tmp/marketplaces/prodex-caveman/plugins/caveman/skills/caveman/SKILL.md")
            .is_file(),
        "core Caveman skill should install by default"
    );
    assert!(
        !caveman_home
            .join(".tmp/marketplaces/prodex-caveman/plugins/caveman/skills/compress/SKILL.md")
            .exists(),
        "compress skill should not install in the default lean overlay"
    );
    assert!(
        !caveman_home
            .join("plugins/cache/prodex-caveman/caveman/0.1.0/skills/compress/SKILL.md")
            .exists(),
        "compress skill should not install in the default plugin cache"
    );
}

#[test]
fn trust_claude_mem_codex_plugin_hooks_updates_temporary_caveman_config_only() {
    let _env_guard = TestEnvVarGuard::unset(PRODEX_CAVEMAN_FULL_ASSETS_ENV);
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.clone(),
        state_file: temp_dir.path.join("state.json"),
        managed_profiles_root: temp_dir.path.join("profiles"),
        shared_codex_root: temp_dir.path.join(".codex"),
        legacy_shared_codex_root: temp_dir.path.join("shared"),
    };
    create_codex_home_if_missing(&paths.managed_profiles_root).expect("managed root");

    let base_home = paths.managed_profiles_root.join("main");
    create_codex_home_if_missing(&base_home).expect("base home");
    fs::write(
        base_home.join("config.toml"),
        r#"
[features]
plugin_hooks = true

[plugins."claude-mem@claude-mem-local"]
enabled = true
"#,
    )
    .expect("base config should write");

    let plugin_root = base_home
        .join("plugins/cache/claude-mem-local/claude-mem/12.7.5");
    fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("manifest dir should create");
    fs::create_dir_all(plugin_root.join("hooks")).expect("hooks dir should create");
    fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"claude-mem","version":"12.7.5","hooks":"./hooks/codex-hooks.json"}"#,
    )
    .expect("plugin manifest should write");
    fs::write(
        plugin_root.join("hooks/codex-hooks.json"),
        r#"{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "startup|resume",
        "hooks": [
          { "type": "command", "command": "node hook-a.js", "timeout": 5 },
          { "type": "command", "command": "node hook-b.js", "timeout": 60, "statusMessage": "Loading claude-mem context" }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "matcher": "ignored by Codex for this event",
        "hooks": [
          { "type": "command", "command": "node prompt.js" }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          { "type": "command", "command": "node stop.js" }
        ]
      }
    ]
  }
}"#,
    )
    .expect("hooks manifest should write");

    let caveman_home =
        prepare_caveman_launch_home(&paths, &base_home).expect("caveman home should prepare");
    prodex_caveman_assets::trust_claude_mem_codex_plugin_hooks(&caveman_home)
        .expect("Claude-Mem Codex hooks should be trusted in the temp home");

    let temp_config = caveman_home.join("config.toml");
    let rendered_config = fs::read_to_string(&temp_config).expect("temp config should read");
    let parsed_config: toml::Value =
        toml::from_str(&rendered_config).expect("temp config should parse");
    let state = parsed_config["hooks"]["state"]
        .as_table()
        .expect("hook trust state should be a TOML table");
    for suffix in [
        "session_start:0:0",
        "session_start:0:1",
        "user_prompt_submit:0:0",
        "stop:0:0",
    ] {
        let hook_key = format!("claude-mem@claude-mem-local:hooks/codex-hooks.json:{suffix}");
        let trusted_hash = state[&hook_key]["trusted_hash"]
            .as_str()
            .expect("Claude-Mem hook should be trusted");
        assert!(
            trusted_hash.starts_with("sha256:") && trusted_hash.len() == "sha256:".len() + 64,
            "trusted hook hash should use Codex canonical sha256 format"
        );
    }

    let base_rendered =
        fs::read_to_string(base_home.join("config.toml")).expect("base config should read");
    assert!(
        !base_rendered.contains("claude-mem-local:hooks/codex-hooks.json"),
        "base Codex config must stay unchanged"
    );
}

#[test]
fn prepare_caveman_launch_home_can_install_full_caveman_assets() {
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

    let caveman_home =
        prepare_caveman_launch_home(&paths, &base_home).expect("caveman home should prepare");
    assert!(
        caveman_home
            .join(".tmp/marketplaces/prodex-caveman/plugins/caveman/skills/compress/SKILL.md")
            .is_file(),
        "compress skill should install when full assets are enabled"
    );
    assert!(
        caveman_home
            .join("plugins/cache/prodex-caveman/caveman/0.1.0/skills/compress/scripts/compress.py")
            .is_file(),
        "compress scripts should install when full assets are enabled"
    );
}
