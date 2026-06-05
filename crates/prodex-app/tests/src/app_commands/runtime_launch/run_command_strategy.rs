use super::*;

#[test]
fn run_strategy_auto_routes_gemini_resume_sessions_to_provider_bridge() {
    let root = temp_dir("auto-route-gemini-resume");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let paths = AppPaths::discover().unwrap();
    let session_id = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9";
    let sessions = paths.shared_codex_root.join("sessions/2026/06/05");
    fs::create_dir_all(&sessions).unwrap();
    fs::write(
        sessions.join("rollout.jsonl"),
        format!(
            "{{\"timestamp\":\"2026-06-05T01:00:00Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"{session_id}\",\"cwd\":\"{}\",\"model_provider\":\"prodex-gemini\"}}}}\n",
            root.display()
        ),
    )
    .unwrap();

    let strategy = RunCommandStrategy::new(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        skip_quota_check: false,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_args: vec![OsString::from(session_id)],
    })
    .unwrap();
    let request = strategy.runtime_request();
    let codex_args = strategy
        .codex_args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(request.external_provider, Some("gemini"));
    assert_eq!(
        request.model_provider_override,
        Some(SUPER_GEMINI_PROVIDER_ID)
    );
    assert_eq!(request.base_url, Some(SUPER_GEMINI_DEFAULT_BASE_URL));
    assert!(request.smart_context_enabled);
    assert!(codex_args.contains(&"model_provider=\"prodex-gemini\"".to_string()));
    assert!(codex_args.contains(&"resume".to_string()));
    assert!(codex_args.contains(&session_id.to_string()));
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
    })
    .unwrap();

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
            OsString::from("--profile"),
            OsString::from("bedrock"),
            OsString::from("hello"),
        ],
    })
    .unwrap();

    assert_eq!(strategy.runtime_request().profile_v2_name, Some("bedrock"));
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
