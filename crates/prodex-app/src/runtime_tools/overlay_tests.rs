use super::super::*;

#[test]
fn build_plan_cleans_overlay_when_config_preflight_fails() {
    let root = env::temp_dir()
        .canonicalize()
        .expect("temporary directory should resolve")
        .join(format!(
            "prodex-runtime-tools-overlay-cleanup-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
    let base_home = root.join("base");
    let shared_home = root.join("shared");
    std::fs::create_dir_all(&base_home).expect("base home should exist");
    std::fs::create_dir_all(&shared_home).expect("shared home should exist");
    std::fs::write(base_home.join("config.toml"), "mcp_servers =\n")
        .expect("config should be written");

    let command =
        parse_cli_command_from(["prodex", "playwright", "exec", "hi"]).expect("playwright command");
    let Commands::Playwright(mut args) = command else {
        panic!("expected playwright command");
    };
    args.select_tool(prodex_optional_tools::OptionalToolId::PlaywrightMcp);
    args.dry_run = true;
    let mut strategy = RuntimeToolLaunchStrategy::new(args);
    let paths = AppPaths {
        root: root.clone(),
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: shared_home,
        legacy_shared_codex_root: root.join("legacy-shared"),
    };
    let prepared = PreparedRuntimeLaunch {
        paths,
        codex_home: base_home,
        managed: false,
        runtime_proxy: None,
    };

    let error = super::build_plan(&mut strategy, &prepared, None).unwrap_err();

    assert!(error.to_string().contains("config.toml"));
    assert!(
        std::fs::read_dir(&prepared.paths.managed_profiles_root)
            .expect("managed profile root should exist")
            .next()
            .is_none(),
        "failed build must remove temporary overlay"
    );
    std::fs::remove_dir_all(root).expect("test root should be removed");
}

#[test]
fn super_overlay_applies_fresh_model_preference() {
    let root = env::temp_dir()
        .canonicalize()
        .expect("temporary directory should resolve")
        .join(format!(
            "prodex-runtime-tools-overlay-model-preference-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
    let base_home = root.join("base");
    std::fs::create_dir_all(&base_home).expect("base home should exist");
    std::fs::write(
        base_home.join("config.toml"),
        "model_provider = \"openai\"\n",
    )
    .expect("config should be written");
    let paths = AppPaths {
        root: root.clone(),
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared"),
        legacy_shared_codex_root: root.join("legacy-shared"),
    };
    let scope = crate::model_preference_scope(&base_home, &[]).unwrap();
    let child = prodex_runtime_launch::ChildProcessPlan::new("codex".into(), base_home.clone());
    let mut sync = crate::ModelPreferenceSync::start_with_scope(&paths, &child, scope).unwrap();
    std::fs::write(
            base_home.join("config.toml"),
            "model_provider = \"openai\"\nmodel = \"remembered-model\"\nmodel_reasoning_effort = \"max\"\n",
        )
        .expect("selected config should be written");
    assert!(sync.finish().is_none());
    let command = parse_cli_command_from(["prodex", "s", "--no-presidio", "--no-sub-agent"])
        .expect("Super command should parse");
    let Commands::Super(args) = command else {
        panic!("expected Super command");
    };
    let mut runtime_args = args.into_runtime_tool_args_with_presidio(false);
    runtime_args.dry_run = true;
    runtime_args.tools.clear();
    runtime_args.required_tools.clear();
    let mut strategy = RuntimeToolLaunchStrategy::new(runtime_args);
    let prepared = PreparedRuntimeLaunch {
        paths,
        codex_home: base_home,
        managed: false,
        runtime_proxy: None,
    };

    let plan = super::build_plan(&mut strategy, &prepared, None).unwrap();
    for key in ["model", "model_provider", "model_reasoning_effort"] {
        assert!(
            crate::codex_cli_config_override_value(&plan.child.args, key).is_none(),
            "{key} must not prevent Codex from restoring the selected thread model"
        );
    }
    let config: toml::Value = toml::from_str(
        &std::fs::read_to_string(plan.child.codex_home.join("config.toml"))
            .expect("overlay config should be readable"),
    )
    .expect("overlay config should remain valid TOML");
    assert_eq!(
        config.get("model").and_then(toml::Value::as_str),
        Some("remembered-model")
    );
    assert_eq!(
        config.get("model_provider").and_then(toml::Value::as_str),
        Some("openai")
    );
    assert_eq!(
        config
            .get("model_reasoning_effort")
            .and_then(toml::Value::as_str),
        Some("max")
    );

    let mut mixed_args = vec![
        "-c".into(),
        "model=\"explicit-model\"".into(),
        "-c".into(),
        "model_provider=\"explicit-provider\"".into(),
        "-c".into(),
        "model_reasoning_effort=\"automatic-effort\"".into(),
    ];
    crate::project_in_app_resume_model_settings(
        &plan.child.codex_home,
        &mut mixed_args,
        None,
        [
            ("model", false),
            ("model_provider", false),
            ("model_reasoning_effort", true),
        ],
    )
    .unwrap();
    assert_eq!(
        crate::codex_cli_config_override_value(&mixed_args, "model").as_deref(),
        Some("explicit-model")
    );
    assert_eq!(
        crate::codex_cli_config_override_value(&mixed_args, "model_provider").as_deref(),
        Some("explicit-provider")
    );
    assert!(
        crate::codex_cli_config_override_value(&mixed_args, "model_reasoning_effort").is_none()
    );
    drop(plan);
    std::fs::remove_dir_all(root).expect("test root should be removed");
}

#[test]
fn prodex_overlay_keeps_shared_session_access() {
    let root = env::temp_dir()
        .canonicalize()
        .expect("temporary directory should resolve")
        .join(format!(
            "prodex-runtime-tools-overlay-sessions-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
    let base_home = root.join("base");
    let sessions = base_home.join("sessions");
    std::fs::create_dir_all(&sessions).expect("session directory should exist");
    std::fs::write(sessions.join("parent.jsonl"), "parent session\n")
        .expect("session should exist");
    let paths = AppPaths {
        root: root.clone(),
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared"),
        legacy_shared_codex_root: root.join("legacy-shared"),
    };
    let overlay =
        super::prepare_prodex_overlay_home(&paths, &base_home).expect("overlay should be prepared");

    assert_ne!(overlay, base_home);
    assert_eq!(
        std::fs::read_to_string(overlay.join("sessions/parent.jsonl"))
            .expect("parent session should remain readable"),
        "parent session\n"
    );
    std::fs::remove_dir_all(root).expect("test root should be removed");
}

#[test]
fn sub_agent_overlay_isolated_and_idempotent_with_resume_surfaces() {
    let root = env::temp_dir()
        .canonicalize()
        .expect("temporary directory should resolve")
        .join(format!(
            "prodex-runtime-tools-sub-agent-overlay-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
    let base_home = root.join("base");
    let session_id = "00000000-0000-7000-8000-000000000042";
    std::fs::create_dir_all(base_home.join("sessions/2026/08/04"))
        .expect("session directory should exist");
    std::fs::create_dir_all(base_home.join("archived_sessions"))
        .expect("archived session directory should exist");
    std::fs::create_dir_all(base_home.join("attachments"))
        .expect("attachment directory should exist");
    std::fs::write(base_home.join("AGENTS.md"), "base instructions\n")
        .expect("base instructions should exist");
    std::fs::write(
        base_home.join("history.jsonl"),
        format!("parent history {session_id}\n"),
    )
    .expect("history should exist");
    std::fs::write(
        base_home.join("sessions/2026/08/04/parent.jsonl"),
        format!("parent session {session_id}\n"),
    )
    .expect("session should exist");
    std::fs::write(
        base_home.join("archived_sessions/old.jsonl"),
        "archived session\n",
    )
    .expect("archived session should exist");
    std::fs::write(base_home.join("attachments/input.txt"), "attachment\n")
        .expect("attachment should exist");
    let paths = AppPaths {
        root: root.clone(),
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared"),
        legacy_shared_codex_root: root.join("legacy-shared"),
    };
    std::fs::create_dir_all(&paths.managed_profiles_root).expect("profile root should exist");
    #[cfg(unix)]
    for path in [&root, &paths.managed_profiles_root] {
        std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o700))
            .expect("overlay parent should be private");
    }

    let overlay =
        super::prepare_prodex_overlay_home(&paths, &base_home).expect("overlay should be prepared");
    let sub_agent = resolve_super_sub_agent_config(
        prodex_cli::SubAgentConfig::default(),
        prodex_cli::SuperLaunchTarget::Resume {
            session_id: session_id.to_string(),
        },
    )
    .expect("sub-agent should resolve");
    write_sub_agent_overlay(&overlay, &sub_agent).expect("sub-agent file should be written");
    prodex_optional_tools::activate_optional_tools_for_codex(
        &overlay,
        &prodex_optional_tools::ToolActivationPlan::default(),
        false,
    )
    .expect("overlay instructions should activate");

    let overlay_agents = std::fs::read_to_string(overlay.join("AGENTS.md"))
        .expect("overlay instructions should be readable");
    assert!(overlay_agents.contains("<!-- PRODEX SUB-AGENT BEGIN -->"));
    assert!(overlay_agents.contains("<!-- PRODEX SUB-AGENT END -->"));
    assert!(overlay_agents.contains("Never have more than 4 child sub-agents active at once."));
    assert!(!overlay_agents.contains(&format!("@{}", overlay.join("SUB_AGENTS.md").display())));
    assert_eq!(
        overlay_agents
            .lines()
            .filter(|line| line.contains("<!-- PRODEX SUB-AGENT BEGIN -->"))
            .count(),
        1
    );
    assert_eq!(
        std::fs::read_to_string(base_home.join("AGENTS.md")).unwrap(),
        "base instructions\n"
    );
    assert!(!base_home.join("SUB_AGENTS.md").exists());
    assert_eq!(
        std::fs::read_to_string(overlay.join("history.jsonl")).unwrap(),
        format!("parent history {session_id}\n")
    );
    assert_eq!(
        std::fs::read_to_string(overlay.join("sessions/2026/08/04/parent.jsonl")).unwrap(),
        format!("parent session {session_id}\n")
    );
    assert_eq!(
        std::fs::read_to_string(overlay.join("archived_sessions/old.jsonl")).unwrap(),
        "archived session\n"
    );
    assert_eq!(
        std::fs::read_to_string(overlay.join("attachments/input.txt")).unwrap(),
        "attachment\n"
    );

    let first_sub_agents = std::fs::read(overlay.join("SUB_AGENTS.md")).unwrap();
    prodex_optional_tools::activate_optional_tools_for_codex(
        &overlay,
        &prodex_optional_tools::ToolActivationPlan::default(),
        false,
    )
    .expect("repeated overlay activation should succeed");
    write_sub_agent_overlay(&overlay, &sub_agent).expect("repeated sub-agent write should succeed");
    assert_eq!(
        std::fs::read(overlay.join("SUB_AGENTS.md")).unwrap(),
        first_sub_agents
    );
    let second_agents = std::fs::read_to_string(overlay.join("AGENTS.md")).unwrap();
    assert_eq!(second_agents, overlay_agents);

    std::fs::remove_dir_all(root).expect("test root should be removed");
}

#[test]
fn configured_bare_uuid_build_plan_resumes_through_sub_agent_overlay() {
    let root = env::temp_dir()
        .canonicalize()
        .expect("temporary directory should resolve")
        .join(format!(
            "prodex-runtime-tools-sub-agent-resume-plan-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
    let base_home = root.join("base");
    let shared_home = root.join("shared");
    let profiles = root.join("profiles");
    let session_id = "00000000-0000-7000-8000-000000000042";
    std::fs::create_dir_all(base_home.join("sessions/2026/08/04"))
        .expect("session directory should exist");
    std::fs::create_dir_all(&shared_home).expect("shared home should exist");
    std::fs::create_dir_all(&profiles).expect("profile root should exist");
    std::fs::write(
        base_home.join("sessions/2026/08/04/parent.jsonl"),
        format!("parent session {session_id}\n"),
    )
    .expect("session should exist");
    #[cfg(unix)]
    for path in [&root, &profiles] {
        std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o700))
            .expect("overlay parent should be private");
    }

    let command = parse_cli_command_from([
        "prodex",
        "s",
        session_id,
        "--no-presidio",
        "--sub-agent",
        "--sub-agent-provider",
        "kiro",
        "--sub-agent-model",
        "gpt-5.6-luna",
        "--sub-agent-model-reasoning-effort",
        "max",
    ])
    .expect("configured resume should parse");
    let Commands::Super(mut args) = command else {
        panic!("expected Super command");
    };
    args.extract_super_overrides_from_codex_args()
        .expect("Super flags should extract");
    args.validate_urls().expect("Super config should validate");
    let mut sub_agent = resolve_super_sub_agent_config(
        args.sub_agent_config(),
        resolve_super_launch_target(&args.codex_args),
    )
    .expect("sub-agent should resolve");
    sub_agent.presidio_enabled = false;
    let mut runtime_args = args.into_runtime_tool_args_with_presidio(false);
    runtime_args.dry_run = true;
    runtime_args.tools.clear();
    runtime_args.required_tools.clear();
    let mut strategy = RuntimeToolLaunchStrategy::new_with_sub_agent(runtime_args, Some(sub_agent));
    let prepared = PreparedRuntimeLaunch {
        paths: AppPaths {
            root: root.clone(),
            state_file: root.join("state.json"),
            managed_profiles_root: profiles,
            shared_codex_root: shared_home,
            legacy_shared_codex_root: root.join("legacy-shared"),
        },
        codex_home: base_home.clone(),
        managed: false,
        runtime_proxy: None,
    };

    let plan = super::build_plan(&mut strategy, &prepared, None)
        .expect("configured resume launch plan should build");

    assert_ne!(plan.child.codex_home, base_home);
    assert_eq!(
        prodex_runtime_launch::codex_resume_session_id(&plan.child.args),
        Some(session_id)
    );
    assert!(plan.child.args.iter().all(|arg| {
        !matches!(
            arg.to_str(),
            Some(
                "--presidio"
                    | "--no-presidio"
                    | "--sub-agent"
                    | "--no-sub-agent"
                    | "--sub-agent-provider"
                    | "--sub-agent-model"
                    | "--sub-agent-model-reasoning-effort"
                    | "--sub-agent-url"
            )
        )
    }));
    assert_eq!(
        std::fs::read_to_string(
            plan.child
                .codex_home
                .join("sessions/2026/08/04/parent.jsonl")
        )
        .expect("resumed session should remain accessible"),
        format!("parent session {session_id}\n")
    );
    let instructions = std::fs::read_to_string(plan.child.codex_home.join("SUB_AGENTS.md"))
        .expect("sub-agent instructions should exist");
    assert!(instructions.contains("- Provider: kiro"));
    assert!(instructions.contains("- Model: gpt-5.6-luna"));
    assert!(instructions.contains("- Reasoning effort: max"));
    assert!(instructions.contains("- Presidio: disabled (inherited)"));
    assert!(instructions.contains("__sub-agent-exec"));
    let launch_config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(plan.child.codex_home.join("sub-agent-launch.json"))
            .expect("sub-agent launch config should exist"),
    )
    .expect("sub-agent launch config should parse");
    assert_eq!(launch_config["provider"], "kiro");
    assert_eq!(launch_config["model"], "gpt-5.6-luna");
    assert_eq!(launch_config["effort"], "max");
    assert_eq!(launch_config["presidio-enabled"], false);
    assert!(!instructions.contains(session_id));
    assert!(!launch_config.to_string().contains(session_id));
    assert!(
        plan.child
            .extra_env
            .iter()
            .any(|(key, value)| key == SUB_AGENT_RECURSION_MARKER && value == "1")
    );

    prodex_runtime_launch::cleanup_runtime_launch_plan(&plan);
    assert!(!plan.child.codex_home.exists());
    std::fs::remove_dir_all(root).expect("test root should be removed");
}
