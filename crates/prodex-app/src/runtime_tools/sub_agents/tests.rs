use super::{slot_lifecycle::sub_agent_slot_lock_error_action, *};
use prodex_cli::SubAgentConcurrencySource;
use prodex_cli::SubAgentConfig;

#[cfg(unix)]
#[test]
fn child_exit_code_preserves_signal_status() {
    use std::os::unix::process::ExitStatusExt;

    assert_eq!(
        crate::child_exit_code(&std::process::ExitStatus::from_raw(9)),
        137
    );
    assert_eq!(
        crate::child_exit_code(&std::process::ExitStatus::from_raw(7 << 8)),
        7
    );
}

fn temp_test_root(label: &str) -> PathBuf {
    env::temp_dir().join(format!(
        "prodex-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn slot_spec(root: &Path, limit: u16) -> ChildLaunchSpec {
    let slot_dir = root.join(SUB_AGENT_SLOT_DIR);
    let task_dir = root.join(SUB_AGENT_TASK_DIR);
    fs::create_dir_all(&slot_dir).unwrap();
    fs::create_dir_all(&task_dir).unwrap();
    for index in 0..limit {
        File::create(slot_dir.join(format!("slot-{index:02}.lock"))).unwrap();
    }
    ChildLaunchSpec {
        executable: env::current_exe().unwrap(),
        provider: ProviderId::OpenAi,
        model: None,
        effort: None,
        local_url: None,
        presidio_enabled: false,
        required_tools: Vec::new(),
        max_concurrency: SubAgentMaxConcurrency::new(limit, SubAgentConcurrencySource::Custom)
            .unwrap(),
        slot_dir,
        task_dir,
        task_max_bytes: SUB_AGENT_TASK_MAX_BYTES,
        recursion_marker: SUB_AGENT_RECURSION_MARKER.to_string(),
    }
}

fn exec_args(root: &Path, spec: &ChildLaunchSpec, task: &str) -> prodex_cli::SubAgentExecArgs {
    let config = root.join(SUB_AGENT_CONFIG_FILE);
    let task_file = spec.task_dir.join("task.txt");
    fs::write(&config, serde_json::to_vec(spec).unwrap()).unwrap();
    fs::write(&task_file, task).unwrap();
    prodex_cli::SubAgentExecArgs { config, task_file }
}

#[test]
fn lock_errors_use_mojo_slot_actions() {
    assert_eq!(
        sub_agent_slot_lock_error_action(&fs2::lock_contended_error(), false).unwrap(),
        SlotLockErrorAction::TryNext
    );
    assert_eq!(
        sub_agent_slot_lock_error_action(&fs2::lock_contended_error(), true).unwrap(),
        SlotLockErrorAction::BlockResize
    );
    assert_eq!(
        sub_agent_slot_lock_error_action(&io::Error::from(io::ErrorKind::PermissionDenied), false,)
            .unwrap(),
        SlotLockErrorAction::Propagate
    );
}

#[test]
fn super_launch_target_uses_canonical_normalization_and_resume_detection() {
    const SESSION_ID: &str = "00000000-0000-7000-8000-000000000042";
    let args = |values: &[&str]| values.iter().map(OsString::from).collect::<Vec<_>>();
    let resume = |session_id: &str| SuperLaunchTarget::Resume {
        session_id: session_id.to_string(),
    };

    assert_eq!(
        resolve_super_launch_target(&args(&["review"])),
        SuperLaunchTarget::Fresh
    );
    assert_eq!(
        resolve_super_launch_target(&args(&["--config", "model=fast", "exec", "review"])),
        SuperLaunchTarget::Exec
    );
    assert_eq!(
        resolve_super_launch_target(&args(&["--config", "model=fast", SESSION_ID])),
        resume(SESSION_ID)
    );
    assert_eq!(
        resolve_super_launch_target(&args(&["resume", "--model", "fast", SESSION_ID])),
        resume(SESSION_ID)
    );
    assert_eq!(
        resolve_super_launch_target(&args(&["exec", "resume", SESSION_ID, "continue"])),
        resume(SESSION_ID)
    );
    assert_eq!(
        resolve_super_launch_target(&args(&["resume", "--last", "continue"])),
        SuperLaunchTarget::Fresh
    );
    assert_eq!(
        resolve_super_launch_target(&args(&["--", SESSION_ID])),
        SuperLaunchTarget::Fresh
    );
}

#[test]
fn default_config_omits_optional_model() {
    let config = SubAgentConfig::default();
    let resolved = resolve_super_sub_agent_config(config, SuperLaunchTarget::Fresh).unwrap();
    assert_eq!(resolved.model, None);
    assert!(resolved.recursion_disabled);
    assert!(canonical_sub_agent_providers().contains(&ProviderId::OpenAi));
    assert!(canonical_sub_agent_model_choices(ProviderId::OpenAi, None).len() > 2);
}

#[test]
fn resolver_rejects_empty_custom_model_at_the_app_boundary() {
    let error = resolve_super_sub_agent_config(
        SubAgentConfig {
            model: Some(" \t".to_string()),
            ..SubAgentConfig::default()
        },
        SuperLaunchTarget::Fresh,
    )
    .unwrap_err();
    assert!(error.to_string().contains("must be nonempty"));
}

#[test]
fn effort_suggestions_fall_back_for_dynamic_models() {
    assert_eq!(
        canonical_sub_agent_efforts(ProviderId::Kiro, Some("account-only-model")),
        canonical_sub_agent_efforts(ProviderId::Kiro, None)
    );
}

#[test]
fn aliases_normalize_and_local_urls_are_typed() {
    let resolved = resolve_super_sub_agent_config(
        SubAgentConfig {
            provider: ProviderId::Local,
            model: Some("default".to_string()),
            model_reasoning_effort: Some(SubAgentReasoningEffort::XHigh),
            url: Some("http://127.0.0.1:11434/v1".to_string()),
            max_concurrency: Default::default(),
        },
        SuperLaunchTarget::Exec,
    )
    .unwrap();
    assert_eq!(resolved.model.as_deref(), Some("local"));
    assert_eq!(resolved.effort, Some(SubAgentReasoningEffort::XHigh));
    assert_eq!(resolved.url.as_deref(), Some("http://127.0.0.1:11434/v1"));
}

#[test]
fn resolver_uses_canonical_mojo_reasoning_compatibility() {
    let error = resolve_super_sub_agent_config(
        SubAgentConfig {
            provider: ProviderId::OpenAi,
            model: Some("gpt-5.6-luna".to_string()),
            model_reasoning_effort: Some(SubAgentReasoningEffort::Ultra),
            ..SubAgentConfig::default()
        },
        SuperLaunchTarget::Fresh,
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("reasoning effort ultra is unsupported for openai model gpt-5.6-luna")
    );

    let resolved = resolve_super_sub_agent_config(
        SubAgentConfig {
            provider: ProviderId::OpenAi,
            model: Some("account/model".to_string()),
            model_reasoning_effort: Some(SubAgentReasoningEffort::Ultra),
            ..SubAgentConfig::default()
        },
        SuperLaunchTarget::Fresh,
    )
    .unwrap();
    assert_eq!(resolved.model.as_deref(), Some("account/model"));
    assert_eq!(resolved.effort, Some(SubAgentReasoningEffort::Ultra));
}

#[test]
fn resolver_preserves_mojo_error_precedence_across_facts() {
    let model_error = resolve_super_sub_agent_config(
        SubAgentConfig {
            model: Some(" \t".to_string()),
            url: Some("not-a-url".to_string()),
            ..SubAgentConfig::default()
        },
        SuperLaunchTarget::Fresh,
    )
    .unwrap_err();
    assert!(
        model_error
            .to_string()
            .contains("--sub-agent-model must be nonempty")
    );

    let reasoning_error = resolve_super_sub_agent_config(
        SubAgentConfig {
            model: Some("gpt-5.6-luna".to_string()),
            model_reasoning_effort: Some(SubAgentReasoningEffort::Ultra),
            url: Some("not-a-url".to_string()),
            ..SubAgentConfig::default()
        },
        SuperLaunchTarget::Fresh,
    )
    .unwrap_err();
    assert!(
        reasoning_error
            .to_string()
            .contains("reasoning effort ultra is unsupported")
    );

    let url_error = resolve_super_sub_agent_config(
        SubAgentConfig {
            model: Some("account/model".to_string()),
            url: Some("not-a-url".to_string()),
            ..SubAgentConfig::default()
        },
        SuperLaunchTarget::Fresh,
    )
    .unwrap_err();
    assert!(
        !url_error
            .to_string()
            .contains("--sub-agent-url is only supported with the local sub-agent provider")
    );

    let provider_url_error = resolve_super_sub_agent_config(
        SubAgentConfig {
            model: Some("account/model".to_string()),
            url: Some("http://127.0.0.1:11434/v1".to_string()),
            ..SubAgentConfig::default()
        },
        SuperLaunchTarget::Fresh,
    )
    .unwrap_err();
    assert!(
        provider_url_error
            .to_string()
            .contains("--sub-agent-url is only supported with the local sub-agent provider")
    );
}

#[test]
fn local_provider_requires_endpoint() {
    let error = resolve_super_sub_agent_config(
        SubAgentConfig {
            provider: ProviderId::Local,
            ..SubAgentConfig::default()
        },
        SuperLaunchTarget::Fresh,
    )
    .unwrap_err();
    assert!(error.to_string().contains("requires --sub-agent-url"));
}

fn test_spec(provider: ProviderId) -> ChildLaunchSpec {
    ChildLaunchSpec {
        executable: PathBuf::from("/opt/Prodex Binary/prodex"),
        provider,
        model: None,
        effort: None,
        local_url: None,
        presidio_enabled: false,
        required_tools: Vec::new(),
        max_concurrency: SubAgentMaxConcurrency::default(),
        slot_dir: PathBuf::from("sub-agent-slots"),
        task_dir: PathBuf::from("sub-agent-tasks"),
        task_max_bytes: SUB_AGENT_TASK_MAX_BYTES,
        recursion_marker: SUB_AGENT_RECURSION_MARKER.to_string(),
    }
}

#[test]
fn child_argv_is_shell_free_exact_and_never_inherits_parent_uuid() {
    let task = "spaces 'apostrophe' \"quotes\"\nUnicode 任务; $(touch nope) & |";
    let mut spec = test_spec(ProviderId::Copilot);
    spec.model = Some("模型/β-🦀".to_string());
    spec.effort = Some(SubAgentReasoningEffort::XHigh);
    spec.presidio_enabled = true;
    spec.required_tools = [
        prodex_optional_tools::OptionalToolId::Rtk,
        prodex_optional_tools::OptionalToolId::Ponytail,
    ]
    .into_iter()
    .map(|tool| tool.to_string())
    .collect();
    let args = child_argv(&spec, task);
    assert_eq!(args[0], "s");
    assert_eq!(args[1], "--no-sub-agent");
    assert_eq!(
        args.iter()
            .filter(|value| **value == "--presidio" || **value == "--no-presidio")
            .count(),
        1
    );
    assert!(
        args.windows(2)
            .any(|pair| pair == ["--provider", "copilot"])
    );
    assert_eq!(
        args.windows(2)
            .filter(|pair| pair[0] == "--require-tool")
            .collect::<Vec<_>>(),
        vec![
            [OsString::from("--require-tool"), OsString::from("rtk")],
            [OsString::from("--require-tool"), OsString::from("ponytail")],
        ]
    );
    assert!(args.windows(2).any(|pair| pair == ["--model", "模型/β-🦀"]));
    assert!(
        args.windows(2)
            .any(|pair| pair == ["-c", "model_reasoning_effort=xhigh"])
    );
    assert_eq!(args[args.len() - 2], "exec");
    assert_eq!(args.last().unwrap(), task);
    assert_eq!(args.iter().filter(|value| **value == task).count(), 1);
    assert!(
        !args
            .iter()
            .any(|value| value.to_string_lossy().contains("019c"))
    );
}

#[test]
fn openai_child_argv_uses_accepted_override_and_cannot_inherit_profile_provider() {
    let args = child_argv(&test_spec(ProviderId::OpenAi), "task");
    assert!(!args.iter().any(|arg| arg == "--provider"));
    assert!(
        args.windows(2)
            .any(|pair| pair == ["-c", "model_provider=\"openai\""])
    );
    let parsed =
        prodex_cli::parse_cli_command_from(std::iter::once(OsString::from("prodex")).chain(args))
            .unwrap();
    let prodex_cli::Commands::Super(parsed) = parsed else {
        panic!("child argv must parse as Super");
    };
    assert!(parsed.provider.is_none());
    assert!(
        parsed
            .codex_args
            .windows(2)
            .any(|pair| pair == ["-c", "model_provider=\"openai\""])
    );
}

#[test]
fn local_child_argv_keeps_exact_url() {
    let mut spec = test_spec(ProviderId::Local);
    spec.local_url = Some("http://127.0.0.1:8131/v1".to_string());
    let args = child_argv(&spec, "task");
    assert!(
        args.windows(2)
            .any(|pair| { pair == ["--url", "http://127.0.0.1:8131/v1"] })
    );
    assert!(!args.iter().any(|value| value == "--provider"));
}

#[test]
fn child_config_serializes_required_tools_and_rejects_unknown_names() {
    let root = temp_test_root("sub-agent-required-tools");
    create_private_directory(&root).unwrap();
    let mut resolved =
        resolve_super_sub_agent_config(SubAgentConfig::default(), SuperLaunchTarget::Fresh)
            .unwrap();
    resolved.required_tools = [
        prodex_optional_tools::OptionalToolId::CodebaseMemoryMcp,
        prodex_optional_tools::OptionalToolId::Rtk,
    ]
    .into_iter()
    .collect();

    write_sub_agent_overlay_with_executable(
        &root,
        &resolved,
        env::temp_dir().join("Prodex Binary").join("prodex"),
    )
    .unwrap();
    let config = std::fs::read_to_string(root.join(SUB_AGENT_CONFIG_FILE)).unwrap();
    let spec: ChildLaunchSpec = serde_json::from_str(&config).unwrap();
    assert_eq!(spec.required_tools, vec!["codebase-memory-mcp", "rtk"]);
    validate_child_launch_spec(&spec).unwrap();

    let mut invalid = serde_json::to_value(&spec).unwrap();
    invalid["required-tools"] = serde_json::json!(["not-a-tool"]);
    let invalid: ChildLaunchSpec = serde_json::from_value(invalid).unwrap();
    assert!(validate_child_launch_spec(&invalid).is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn child_config_rejects_invalid_model_effort_url_and_scalar_values() {
    let mut empty_model = test_spec(ProviderId::OpenAi);
    empty_model.model = Some(" \t".to_string());
    let error = validate_child_launch_spec(&empty_model).unwrap_err();
    assert!(error.to_string().contains("model must be nonempty"));

    let mut model_and_url_error = test_spec(ProviderId::Local);
    model_and_url_error.model = Some(" \t".to_string());
    model_and_url_error.local_url = Some("not-a-url".to_string());
    let error = validate_child_launch_spec(&model_and_url_error).unwrap_err();
    assert!(error.to_string().contains("model must be nonempty"));

    let mut unsupported_effort = test_spec(ProviderId::OpenAi);
    unsupported_effort.model = Some("gpt-5.6-luna".to_string());
    unsupported_effort.effort = Some(SubAgentReasoningEffort::Ultra);
    let error = validate_child_launch_spec(&unsupported_effort).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("reasoning effort ultra is unsupported")
    );

    let mut invalid_url = test_spec(ProviderId::Local);
    invalid_url.local_url = Some("https://example.com/v1?query=value".to_string());
    let error = validate_child_launch_spec(&invalid_url).unwrap_err();
    assert!(error.to_string().contains("invalid --sub-agent-url"));

    let mut empty_tool = test_spec(ProviderId::OpenAi);
    empty_tool.required_tools = vec![String::new()];
    let error = validate_child_launch_spec(&empty_tool).unwrap_err();
    assert!(error.to_string().contains("invalid required optional tool"));

    let mut oversized_task = test_spec(ProviderId::OpenAi);
    oversized_task.task_max_bytes = SUB_AGENT_TASK_MAX_BYTES + 1;
    let error = validate_child_launch_spec(&oversized_task).unwrap_err();
    assert!(error.to_string().contains("task size policy is invalid"));
}

#[test]
fn hidden_launcher_revalidates_provider_model_before_slot_admission() {
    let root = temp_test_root("sub-agent-child-model-validation");
    let mut spec = slot_spec(&root, 1);
    spec.model = Some("gpt-5.6-luna".to_string());
    spec.effort = Some(SubAgentReasoningEffort::Ultra);
    let error = handle_sub_agent_exec(exec_args(&root, &spec, "narrow task")).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("reasoning effort ultra is unsupported")
    );
    assert!(spec.task_dir.join("task.txt").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn hidden_launcher_uses_mojo_marker_precedence_before_reading_config() {
    let _marker = crate::test_support::TestEnvVarGuard::set(SUB_AGENT_RECURSION_MARKER, "1");
    let _launcher = crate::test_support::TestEnvVarGuard::set(SUB_AGENT_LAUNCHER_MARKER, "1");
    let root = temp_test_root("sub-agent-launcher-marker-precedence");
    let error = handle_sub_agent_exec(prodex_cli::SubAgentExecArgs {
        config: root.join(SUB_AGENT_CONFIG_FILE),
        task_file: root.join("task.txt"),
    })
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("failed to open sub-agent launcher config")
    );
    assert!(!error.to_string().contains("cannot be invoked recursively"));
}

#[test]
fn hidden_launcher_rejects_oversized_config_before_parsing_or_task_access() {
    let _marker = crate::test_support::TestEnvVarGuard::unset(SUB_AGENT_RECURSION_MARKER);
    let _launcher = crate::test_support::TestEnvVarGuard::unset(SUB_AGENT_LAUNCHER_MARKER);
    let root = temp_test_root("sub-agent-oversized-config");
    fs::create_dir_all(&root).unwrap();
    let config = root.join(SUB_AGENT_CONFIG_FILE);
    fs::write(&config, vec![b'x'; SUB_AGENT_TASK_MAX_BYTES + 1]).unwrap();
    let error = handle_sub_agent_exec(prodex_cli::SubAgentExecArgs {
        config,
        task_file: root.join("task.txt"),
    })
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("sub-agent launcher config exceeds")
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn recursion_marker_and_internal_launcher_use_mojo_policy() {
    assert_eq!(
        sub_agent_recursion_decision(false, false),
        RecursionDecision::Allowed
    );
    assert_eq!(
        sub_agent_recursion_decision(true, false),
        RecursionDecision::Disabled
    );
    assert_eq!(
        sub_agent_recursion_decision(true, true),
        RecursionDecision::InternalLauncher
    );
    assert_eq!(
        SubAgentRecursionPolicy::from_decision(sub_agent_recursion_decision(true, false)),
        SubAgentRecursionPolicy::Disabled
    );
}

#[test]
fn public_recursion_policy_does_not_trust_launcher_marker_precedence() {
    let _marker = crate::test_support::TestEnvVarGuard::set(SUB_AGENT_RECURSION_MARKER, "1");
    let _launcher = crate::test_support::TestEnvVarGuard::set(SUB_AGENT_LAUNCHER_MARKER, "1");
    assert_eq!(
        sub_agent_recursion_policy(),
        SubAgentRecursionPolicy::Disabled
    );
}

#[test]
fn dry_run_redacts_endpoint_and_resume_id() {
    let session_id = "00000000-0000-7000-8000-000000000042";
    let url = "http://127.0.0.1:11434/v1";
    let model = "sk-proj-sub-agent-secret";
    let resolved = resolve_super_sub_agent_config(
        SubAgentConfig {
            provider: ProviderId::Local,
            model: Some(model.to_string()),
            url: Some(url.to_string()),
            ..SubAgentConfig::default()
        },
        SuperLaunchTarget::Resume {
            session_id: session_id.to_string(),
        },
    )
    .unwrap();
    let report = render_sub_agent_dry_run_report(&resolved).unwrap();
    let debug = format!("{resolved:?}");
    assert!(report.contains("Sub-agent local URL: configured"));
    assert!(report.contains("Sub-agent inherited required tools: none"));
    assert!(report.contains("Sub-agent launch target: resume <SESSION_UUID>"));
    assert!(report.contains("Sub-agent recursion disabled: yes"));
    assert!(!report.contains(url));
    assert!(!report.contains(model));
    assert!(!report.contains(session_id));
    assert!(
        debug.contains("target: \"resume <SESSION_UUID>\""),
        "{debug}"
    );
    assert!(!debug.contains(session_id), "{debug}");
}

#[test]
fn overlay_and_child_marker_are_scoped_to_the_resolved_launch() {
    let root = env::temp_dir().join(format!(
        "prodex-sub-agent-overlay-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(&root, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
    let mut resolved = resolve_super_sub_agent_config(
        SubAgentConfig {
            model: Some("gpt-5.4".to_string()),
            ..SubAgentConfig::default()
        },
        SuperLaunchTarget::Resume {
            session_id: "00000000-0000-7000-8000-000000000042".to_string(),
        },
    )
    .unwrap();
    resolved.presidio_enabled = true;

    let path = write_sub_agent_overlay(&root, &resolved).unwrap();
    let contents = std::fs::read_to_string(path).unwrap();
    assert!(contents.contains("--presidio"));
    assert!(!contents.contains("00000000-0000-7000-8000-000000000042"));
    let agents = std::fs::read_to_string(root.join("AGENTS.md")).unwrap();
    assert!(agents.contains(SUB_AGENT_BLOCK_BEGIN));
    assert!(agents.contains("Never have more than 4 child sub-agents active at once."));
    assert!(!agents.contains("@/") && !agents.contains("@SUB_AGENTS.md"));
    assert_eq!(
        std::fs::read_dir(root.join(SUB_AGENT_SLOT_DIR))
            .unwrap()
            .count(),
        usize::from(resolved.max_concurrency.get())
    );

    let mut child = ChildProcessPlan::new(OsString::from("codex"), root.clone());
    apply_sub_agent_recursion_marker(&mut child, Some(&resolved));
    assert_eq!(
        child
            .extra_env
            .iter()
            .find(|(name, _)| name == SUB_AGENT_RECURSION_MARKER)
            .map(|(_, value)| value.as_os_str()),
        Some(std::ffi::OsStr::new("1"))
    );
    assert!(
        child
            .extra_env
            .iter()
            .any(|(name, value)| name == SUB_AGENT_LAUNCHER_MARKER && value == "1")
    );
    assert_eq!(child.extra_env.len(), 2);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn child_config_contains_only_launch_data_and_no_parent_uuid() {
    let model = "sk-proj-synthetic-model-id";
    let session_id = "00000000-0000-7000-8000-000000000042";
    let resolved = resolve_super_sub_agent_config(
        SubAgentConfig {
            model: Some(model.to_string()),
            ..SubAgentConfig::default()
        },
        SuperLaunchTarget::Resume {
            session_id: session_id.to_string(),
        },
    )
    .unwrap();
    let root = env::temp_dir().join(format!(
        "prodex-sub-agent-config-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    create_private_directory(&root).unwrap();
    write_sub_agent_overlay_with_executable(
        &root,
        &resolved,
        PathBuf::from("/opt/Prodex Binary/prodex"),
    )
    .unwrap();
    let config = std::fs::read_to_string(root.join(SUB_AGENT_CONFIG_FILE)).unwrap();
    assert!(config.contains(model));
    assert!(!config.contains(session_id));
    for forbidden in ["api_key", "oauth", "authorization", "bearer", "cookie"] {
        assert!(!config.to_ascii_lowercase().contains(forbidden), "{config}");
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn child_config_rejects_unknown_fields() {
    let root = temp_test_root("sub-agent-config-unknown-field");
    let spec = slot_spec(&root, 1);
    let mut config = serde_json::to_value(&spec).unwrap();
    config
        .as_object_mut()
        .unwrap()
        .insert("api-key".to_string(), serde_json::json!("synthetic"));

    assert!(serde_json::from_value::<ChildLaunchSpec>(config).is_err());

    let mut config = serde_json::to_value(&spec).unwrap();
    config["max-concurrency"]
        .as_object_mut()
        .unwrap()
        .insert("unexpected".to_string(), serde_json::json!(true));
    assert!(serde_json::from_value::<ChildLaunchSpec>(config).is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[path = "tests/slot_cases.rs"]
mod slot_cases;
