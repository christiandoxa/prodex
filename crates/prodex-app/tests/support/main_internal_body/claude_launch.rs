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
    assert!(rendered_config.contains("remote_plugin = false"));
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
    assert!(hook_script.contains("Ponytail applies smallest-correct-implementation pressure"));
    assert!(hook_script.contains("rtk <cmd>"));
    assert!(hook_script.contains("prodex-sqz"));
    assert!(hook_script.contains("prodex-token-savior"));
    assert!(hook_script.contains("codebase-memory-mcp"));
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
    let codex_plugin_manifest = fs::read_to_string(
        overlay_home.join(".tmp/marketplaces/prodex-caveman/plugins/caveman/.codex-plugin/plugin.json"),
    )
    .expect("Codex plugin manifest should read");
    assert!(codex_plugin_manifest.contains("\"logoDark\": \"./assets/caveman-dark.svg\""));
    assert!(
        overlay_home
            .join(".tmp/marketplaces/prodex-caveman/plugins/caveman/assets/caveman-dark.svg")
            .is_file(),
        "Codex 0.142.2 dark-mode plugin logo should install"
    );
    assert!(
        overlay_home
            .join("plugins/cache/prodex-caveman/caveman/0.1.0/.codex-plugin/plugin.json")
            .is_file()
    );
    assert!(
        overlay_home
            .join("plugins/cache/prodex-caveman/caveman/0.1.0/assets/caveman-dark.svg")
            .is_file(),
        "Codex 0.142.2 dark-mode plugin logo should install in the cache"
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

#[cfg(unix)]
#[test]
fn prepare_prodex_overlay_home_preserves_pasted_attachments_across_profile_resume() {
    let _env_guard = TestEnvVarGuard::unset(PRODEX_CAVEMAN_FULL_ASSETS_ENV);
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.clone(),
        state_file: temp_dir.path.join("state.json"),
        managed_profiles_root: temp_dir.path.join("profiles"),
        shared_codex_root: temp_dir.path.join(".codex"),
        legacy_shared_codex_root: temp_dir.path.join("shared"),
    };
    let first_profile_home = paths.managed_profiles_root.join("first");
    let second_profile_home = paths.managed_profiles_root.join("second");

    prodex_shared_codex_fs::prepare_managed_codex_home(&paths, &first_profile_home)
        .expect("first managed profile should prepare");
    let first_overlay = prepare_prodex_overlay_home(&paths, &first_profile_home)
        .expect("first Prodex overlay should prepare");

    let attachment_id = "bbbbbbbb-cccc-4ddd-8eee-ffffffffffff";
    let overlay_pasted_text = first_overlay
        .join("attachments")
        .join(attachment_id)
        .join("pasted-text-1.txt");
    let overlay_pasted_image = first_overlay
        .join("attachments")
        .join(attachment_id)
        .join("image-1.png");
    fs::create_dir_all(overlay_pasted_text.parent().expect("attachment parent"))
        .expect("attachment parent should create");
    fs::write(&overlay_pasted_text, b"pasted text from first overlay")
        .expect("overlay pasted text should write");
    fs::write(&overlay_pasted_image, b"pasted image from first overlay")
        .expect("overlay pasted image should write");

    let session_file = first_overlay.join("sessions/2026/06/26/rollout-session.jsonl");
    fs::create_dir_all(session_file.parent().expect("session parent"))
        .expect("session parent should create");
    fs::write(
        &session_file,
        format!(
            r#"{{"timestamp":"2026-06-26T08:00:00Z","type":"response_item","payload":{{"text":"Pasted Content: {} and image {}"}}}}"#,
            overlay_pasted_text.display(),
            overlay_pasted_image.display()
        ),
    )
    .expect("session should write through overlay symlink");
    let goals_db = paths.shared_codex_root.join("goals_1.sqlite");
    let conn = rusqlite::Connection::open(&goals_db).expect("goals db should open");
    conn.execute_batch(
        r#"
        CREATE TABLE thread_goals (
            thread_id TEXT PRIMARY KEY NOT NULL,
            goal_id TEXT NOT NULL,
            objective TEXT NOT NULL,
            status TEXT NOT NULL,
            token_budget INTEGER,
            tokens_used INTEGER NOT NULL DEFAULT 0,
            time_used_seconds INTEGER NOT NULL DEFAULT 0,
            created_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL
        );
        "#,
    )
    .expect("goals schema should create");
    conn.execute(
        "INSERT INTO thread_goals (thread_id, goal_id, objective, status, created_at_ms, updated_at_ms) VALUES (?1, 'goal-1', ?2, 'paused', 1, 1)",
        rusqlite::params![
            "thread-1",
            format!(
                "pasted text file: {}. image file: {}",
                overlay_pasted_text.display(),
                overlay_pasted_image.display()
            )
        ],
    )
    .expect("goal row should insert");
    drop(conn);

    prodex_shared_codex_fs::maintain_managed_codex_sessions(&paths)
        .expect("post-exit maintenance should stabilize attachment paths");
    prodex_shared_codex_fs::prepare_managed_codex_home(&paths, &second_profile_home)
        .expect("second managed profile should prepare");
    let second_overlay = prepare_prodex_overlay_home(&paths, &second_profile_home)
        .expect("second Prodex overlay should prepare");

    let shared_pasted_text = paths
        .shared_codex_root
        .join("attachments")
        .join(attachment_id)
        .join("pasted-text-1.txt");
    let shared_pasted_image = paths
        .shared_codex_root
        .join("attachments")
        .join(attachment_id)
        .join("image-1.png");
    assert_eq!(
        fs::read(&shared_pasted_text).expect("shared pasted text should remain readable"),
        b"pasted text from first overlay"
    );
    assert_eq!(
        fs::read(
            second_overlay
                .join("attachments")
                .join(attachment_id)
                .join("image-1.png")
        )
        .expect("resumed overlay should see pasted image"),
        b"pasted image from first overlay"
    );
    assert_eq!(
        fs::read_link(second_profile_home.join("attachments"))
            .expect("second profile attachments should be shared"),
        paths.shared_codex_root.join("attachments")
    );
    assert_eq!(
        fs::read_link(second_overlay.join("attachments"))
            .expect("second overlay attachments should point at second profile"),
        second_profile_home.join("attachments")
    );

    let shared_session = paths
        .shared_codex_root
        .join("sessions/2026/06/26/rollout-session.jsonl");
    let rewritten = fs::read_to_string(&shared_session).expect("shared session should read");
    assert!(
        rewritten.contains(&shared_pasted_text.display().to_string()),
        "resume history should point at stable shared pasted text path: {rewritten}"
    );
    assert!(
        rewritten.contains(&shared_pasted_image.display().to_string()),
        "resume history should point at stable shared pasted image path: {rewritten}"
    );
    assert!(
        !rewritten.contains(&first_overlay.display().to_string()),
        "resume history must not retain first overlay path: {rewritten}"
    );

    let conn = rusqlite::Connection::open(&goals_db).expect("goals db should reopen");
    let goal_objective: String = conn
        .query_row(
            "SELECT objective FROM thread_goals WHERE thread_id = 'thread-1'",
            [],
            |row| row.get(0),
        )
        .expect("goal objective should read");
    assert!(
        goal_objective.contains(&shared_pasted_text.display().to_string()),
        "goal objective should point at shared pasted text: {goal_objective}"
    );
    assert!(
        goal_objective.contains(&shared_pasted_image.display().to_string()),
        "goal objective should point at shared pasted image: {goal_objective}"
    );
    assert!(
        !goal_objective.contains(&first_overlay.display().to_string()),
        "goal objective must not retain first overlay path: {goal_objective}"
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
