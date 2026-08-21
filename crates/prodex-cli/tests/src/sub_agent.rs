use super::*;
use prodex_provider_core::ProviderId;

const SESSION_ID: &str = "00000000-0000-7000-8000-000000000042";

fn super_command(args: &[&str]) -> SuperArgs {
    let mut argv = vec!["prodex", "super"];
    argv.extend(args);
    let Commands::Super(args) = parse_cli_command_from(argv).expect("Super command should parse")
    else {
        panic!("expected Super command");
    };
    args
}

fn extract(args: &[&str]) -> SuperArgs {
    let mut args = super_command(args);
    args.extract_provider_overrides_from_codex_args()
        .expect("Super tail should extract");
    args
}

#[test]
fn sub_agent_flags_parse_split_and_equals_forms() {
    let split = super_command(&[
        "--sub-agent",
        "--sub-agent-provider",
        "GOOGLE",
        "--sub-agent-model",
        "vendor/model with spaces",
        "--sub-agent-model-reasoning-effort",
        "xhigh",
        "--sub-agent-max-concurrency",
        "16",
        "exec",
        "review",
    ]);
    assert!(split.sub_agent);
    assert!(!split.no_sub_agent);
    assert_eq!(split.sub_agent_provider, Some(ProviderId::Gemini));
    assert_eq!(
        split.sub_agent_model.as_deref(),
        Some("vendor/model with spaces")
    );
    assert_eq!(
        split.sub_agent_model_reasoning_effort,
        Some(SubAgentReasoningEffort::XHigh)
    );
    assert_eq!(split.sub_agent_url, None);
    assert_eq!(split.sub_agent_max_concurrency.unwrap().get(), 16);

    let equals = super_command(&[
        "--sub-agent",
        "--sub-agent-provider=github-copilot",
        "--sub-agent-model=arbitrary-model",
        "--sub-agent-model-reasoning-effort=max",
        "--sub-agent-max-concurrency=23",
    ]);
    assert_eq!(equals.sub_agent_provider, Some(ProviderId::Copilot));
    assert_eq!(equals.sub_agent_model.as_deref(), Some("arbitrary-model"));
    assert_eq!(
        equals.sub_agent_model_reasoning_effort,
        Some(SubAgentReasoningEffort::Max)
    );
    assert_eq!(equals.sub_agent_url, None);
    assert_eq!(equals.sub_agent_max_concurrency.unwrap().get(), 23);

    let local = super_command(&[
        "--sub-agent",
        "--sub-agent-provider=local",
        "--sub-agent-url=http://localhost:8131/v1",
    ]);
    assert_eq!(local.sub_agent_provider, Some(ProviderId::Local));
    assert_eq!(
        local.sub_agent_url.as_deref(),
        Some("http://localhost:8131/v1")
    );
}

#[test]
fn super_and_short_alias_accept_explicit_sub_agent_mode() {
    for command in ["super", "s"] {
        let parsed = parse_cli_command_from(["prodex", command, "--sub-agent"])
            .expect("explicit sub-agent mode should parse without prompting");
        let Commands::Super(args) = parsed else {
            panic!("expected Super command");
        };
        assert_eq!(
            args.sub_agent_preference(),
            SubAgentPreference::Enabled(SubAgentConfig::default())
        );
    }
}

#[test]
fn sub_agent_detail_flags_require_explicit_enable() {
    for flag in [
        "--sub-agent-provider",
        "--sub-agent-model",
        "--sub-agent-model-reasoning-effort",
        "--sub-agent-url",
        "--sub-agent-max-concurrency",
    ] {
        assert!(
            parse_cli_command_from(["prodex", "super", flag, "openai"]).is_err(),
            "{flag} should require --sub-agent"
        );
    }
    assert!(parse_cli_command_from(["prodex", "super", "--sub-agent", "--no-sub-agent"]).is_err());
    assert!(parse_cli_command_from(["prodex", "super", "--no-sub-agent", "--sub-agent"]).is_err());
}

#[test]
fn conflicting_decisions_separated_by_session_target_are_rejected() {
    let mut args = super_command(&["--sub-agent", SESSION_ID, "--no-sub-agent"]);
    let error = args
        .extract_super_overrides_from_codex_args()
        .expect_err("separated decision flags must still conflict");
    assert!(error.contains("conflicts"), "{error}");
}

#[test]
fn conflicting_decisions_separated_by_explicit_resume_are_rejected() {
    let mut args = super_command(&["--sub-agent", "resume", SESSION_ID, "--no-sub-agent"]);
    let error = args
        .extract_super_overrides_from_codex_args()
        .expect_err("explicit-resume decision flags must still conflict");
    assert!(error.contains("conflicts"), "{error}");
}

#[test]
fn sub_agent_tail_flags_parse_after_bare_uuid_and_explicit_resume() {
    for target in [
        [SESSION_ID, "--sub-agent", "--sub-agent-provider", "openai"].as_slice(),
        [
            "resume",
            SESSION_ID,
            "--sub-agent",
            "--sub-agent-provider=gemini",
        ]
        .as_slice(),
        [
            "exec",
            "resume",
            SESSION_ID,
            "--sub-agent",
            "--sub-agent-model=tail-model",
            "--sub-agent-model-reasoning-effort",
            "max",
            "--sub-agent-max-concurrency=8",
        ]
        .as_slice(),
    ] {
        let args = extract(target);
        assert!(args.sub_agent);
        assert_eq!(
            args.codex_args.first().and_then(|arg| arg.to_str()),
            target.first().copied()
        );
        assert!(
            args.codex_args
                .iter()
                .all(|arg| !arg.to_string_lossy().starts_with("--sub-agent"))
        );
    }

    let args = extract(&[
        SESSION_ID,
        "--sub-agent",
        "--sub-agent-provider",
        "local",
        "--sub-agent-model",
        "tail-model",
        "--sub-agent-url",
        "https://example.com/sub-agent",
    ]);
    assert_eq!(args.sub_agent_provider, Some(ProviderId::Local));
    assert_eq!(args.sub_agent_model.as_deref(), Some("tail-model"));
    assert_eq!(
        args.sub_agent_url.as_deref(),
        Some("https://example.com/sub-agent")
    );
}

#[test]
fn max_concurrency_parses_before_and_after_session_without_leakage() {
    for values in [
        vec![
            "--sub-agent",
            "--sub-agent-max-concurrency",
            "16",
            SESSION_ID,
        ],
        vec![SESSION_ID, "--sub-agent", "--sub-agent-max-concurrency=16"],
    ] {
        let args = extract(&values);
        let limit = args.sub_agent_max_concurrency.unwrap();
        assert_eq!(limit.get(), 16);
        assert_eq!(limit.source(), SubAgentConcurrencySource::Preset);
        assert_eq!(args.codex_args, os_args(&[SESSION_ID]));
    }
}

#[test]
fn max_concurrency_rejects_unbounded_and_unenabled_values() {
    for value in ["0", "65", "-1", "1.5", "1e2", "", "999999999999999999999"] {
        assert!(
            parse_cli_command_from([
                "prodex",
                "s",
                "--sub-agent",
                "--sub-agent-max-concurrency",
                value,
            ])
            .is_err(),
            "{value:?}"
        );
    }
    assert!(parse_cli_command_from(["prodex", "s", "--sub-agent-max-concurrency", "8"]).is_err());
    let mut args = super_command(&["--no-sub-agent", "--sub-agent-max-concurrency", "8"]);
    assert!(args.extract_super_overrides_from_codex_args().is_err());
}

#[test]
fn sub_agent_flags_before_and_after_uuid_do_not_leak_to_codex() {
    let args = extract(&[
        "--sub-agent",
        "--sub-agent-provider=openai",
        SESSION_ID,
        "--sub-agent-model",
        "模型/β-🦀",
        "--sub-agent-model-reasoning-effort=minimal",
        "--no-presidio",
        "--full-access",
    ]);
    assert!(args.sub_agent);
    assert!(args.no_presidio);
    assert!(args.full_access);
    assert_eq!(args.sub_agent_provider, Some(ProviderId::OpenAi));
    assert_eq!(args.sub_agent_model.as_deref(), Some("模型/β-🦀"));
    assert_eq!(
        args.sub_agent_model_reasoning_effort,
        Some(SubAgentReasoningEffort::Minimal)
    );
    assert_eq!(args.codex_args, os_args(&[SESSION_ID]));
}

#[test]
fn literal_double_dash_preserves_boundary_and_all_following_codex_args() {
    let mut args = super_command(&[
        SESSION_ID,
        "--sub-agent",
        "--sub-agent-provider",
        "openai",
        "--",
        "--sub-agent-model",
        "literal-model",
        "--unrelated",
        "value",
    ]);
    args.extract_provider_overrides_from_codex_args()
        .expect("literal boundary should suppress Super extraction");

    assert!(args.sub_agent);
    assert_eq!(args.sub_agent_provider, Some(ProviderId::OpenAi));
    assert_eq!(
        args.codex_args,
        os_args(&[
            SESSION_ID,
            "--",
            "--sub-agent-model",
            "literal-model",
            "--unrelated",
            "value",
        ])
    );
}

#[test]
fn split_detail_flag_does_not_consume_literal_double_dash_as_value() {
    let mut args = super_command(&[
        SESSION_ID,
        "--sub-agent",
        "--sub-agent-model",
        "--",
        "--sub-agent-provider",
        "openai",
    ]);
    let error = args
        .extract_provider_overrides_from_codex_args()
        .expect_err("a detail flag before -- should report its missing value");
    assert!(
        error.contains("--sub-agent-model requires a value"),
        "{error}"
    );
    assert_eq!(
        args.codex_args,
        os_args(&[
            SESSION_ID,
            "--sub-agent-model",
            "--",
            "--sub-agent-provider",
            "openai",
        ])
    );
}

#[test]
fn all_prodex_flags_after_uuid_are_consumed_without_codex_leakage() {
    let args = extract(&[
        SESSION_ID,
        "--sub-agent",
        "--sub-agent-provider",
        "openai",
        "--tool",
        "rtk",
        "--require-tool",
        "ponytail",
        "--full-access",
    ]);
    assert!(args.sub_agent);
    assert!(args.full_access);
    assert_eq!(args.tools.len(), 2);
    assert!(
        args.tools
            .contains(&prodex_optional_tools::OptionalToolId::Rtk)
    );
    assert!(
        args.required_tools
            .contains(&prodex_optional_tools::OptionalToolId::Ponytail)
    );
    assert_eq!(args.codex_args, os_args(&[SESSION_ID]));
}

#[test]
fn invalid_tail_values_fail_instead_of_leaking_to_codex() {
    for tail in [
        vec![SESSION_ID, "--provider", "not-a-provider"],
        vec![SESSION_ID, "--cli", "not-a-cli"],
        vec![SESSION_ID, "--tool", "not-a-tool"],
        vec![SESSION_ID, "--sub-agent-model-reasoning-effort", "extreme"],
    ] {
        let mut args = super_command(&tail);
        assert!(
            args.extract_super_overrides_from_codex_args().is_err(),
            "tail must fail: {tail:?}"
        );
    }
}

#[test]
fn sub_agent_provider_is_canonical_and_defaults_to_openai() {
    assert_eq!(
        parse_sub_agent_provider(" OpenAI ").unwrap(),
        ProviderId::OpenAi
    );
    assert_eq!(
        parse_sub_agent_provider("local-openai").unwrap(),
        ProviderId::Local
    );
    assert_eq!(SubAgentConfig::default().provider, ProviderId::OpenAi);

    let args = extract(&[SESSION_ID, "--sub-agent"]);
    assert_eq!(
        args.sub_agent_preference(),
        SubAgentPreference::Enabled(SubAgentConfig::default())
    );
}

#[test]
fn every_canonical_provider_is_accepted_from_the_shared_registry() {
    for descriptor in prodex_provider_core::provider_implementation_registry().iter() {
        let provider = descriptor.provider();
        for value in std::iter::once(descriptor.canonical_label())
            .chain(descriptor.accepted_aliases().iter().copied())
        {
            assert_eq!(parse_sub_agent_provider(value), Ok(provider));
            assert_eq!(
                parse_sub_agent_provider(&value.to_ascii_uppercase()),
                Ok(provider)
            );
        }
        let mut command = vec!["--sub-agent", "--sub-agent-provider", provider.label()];
        if provider == ProviderId::Local {
            command.extend(["--sub-agent-url", "http://127.0.0.1:8131/v1"]);
        }
        super_command(&command).validate_urls().unwrap();
    }
}

#[test]
fn sub_agent_model_accepts_arbitrary_nonempty_values_only() {
    assert_eq!(
        parse_sub_agent_model("custom/vendor/model@2026").unwrap(),
        "custom/vendor/model@2026"
    );
    assert_eq!(parse_sub_agent_model("模型/β-🦀").unwrap(), "模型/β-🦀");
    assert_eq!(
        parse_sub_agent_model("gpt-5.6-luna").unwrap(),
        "gpt-5.6-luna"
    );
    assert!(parse_sub_agent_model("").is_err());
    assert!(parse_sub_agent_model(" \t").is_err());
    assert!(
        parse_cli_command_from(["prodex", "super", "--sub-agent", "--sub-agent-model", ""])
            .is_err()
    );
}

#[test]
fn sub_agent_reasoning_effort_accepts_all_known_values_case_insensitively() {
    for (value, expected) in [
        ("none", SubAgentReasoningEffort::None),
        ("minimal", SubAgentReasoningEffort::Minimal),
        ("low", SubAgentReasoningEffort::Low),
        ("medium", SubAgentReasoningEffort::Medium),
        ("high", SubAgentReasoningEffort::High),
        ("XHIGH", SubAgentReasoningEffort::XHigh),
        ("max", SubAgentReasoningEffort::Max),
        ("ULTRA", SubAgentReasoningEffort::Ultra),
    ] {
        assert_eq!(parse_sub_agent_reasoning_effort(value).unwrap(), expected);
    }
    assert!(parse_sub_agent_reasoning_effort("extreme").is_err());
}

#[test]
fn sub_agent_url_uses_credential_free_http_contract() {
    for valid in [
        "http://127.0.0.1:8131",
        "https://example.com/sub-agent",
        "http://localhost:8131/v1/",
    ] {
        assert_eq!(parse_sub_agent_url(valid).unwrap(), valid);
    }
    for invalid in [
        "relative/path",
        "ftp://example.com",
        "http:///missing-host",
        "https://user:secret@example.com",
        "https://example.com?token=secret",
        "https://example.com/#secret",
    ] {
        let error = parse_sub_agent_url(invalid).unwrap_err();
        assert!(error.contains("absolute http(s) URL"), "{error}");
        assert!(!error.contains("secret"), "{error}");
    }
}

#[test]
fn sub_agent_url_requires_local_and_local_requires_a_resolved_url() {
    let non_local = super_command(&[
        "--sub-agent",
        "--sub-agent-provider",
        "kiro",
        "--sub-agent-url",
        "http://127.0.0.1:8131/v1",
    ]);
    assert!(
        non_local
            .validate_urls()
            .unwrap_err()
            .contains("requires --sub-agent-provider local")
    );

    let local = super_command(&["--sub-agent", "--sub-agent-provider", "local"]);
    assert!(
        local
            .validate_urls()
            .unwrap_err()
            .contains("requires --sub-agent-url")
    );

    let separated = super_command(&[
        "--sub-agent",
        "--sub-agent-provider",
        "local",
        "--url",
        "http://127.0.0.1:8131/v1",
    ]);
    assert!(
        separated
            .validate_urls()
            .unwrap_err()
            .contains("requires --sub-agent-url")
    );

    let explicit = super_command(&[
        "--sub-agent",
        "--sub-agent-provider",
        "local",
        "--url",
        "http://127.0.0.1:8131/v1",
        "--sub-agent-url",
        "http://127.0.0.1:9131/v1",
    ]);
    explicit.validate_urls().unwrap();
    assert_eq!(
        explicit.sub_agent_config().url.as_deref(),
        Some("http://127.0.0.1:9131/v1")
    );
}

#[test]
fn sub_agent_preference_and_config_are_typed() {
    let disabled = super_command(&["--no-sub-agent"]);
    assert_eq!(
        disabled.sub_agent_preference(),
        SubAgentPreference::Disabled
    );

    let unspecified = super_command(&[]);
    assert_eq!(
        unspecified.sub_agent_preference(),
        SubAgentPreference::Unspecified
    );

    let enabled = super_command(&[
        "--sub-agent",
        "--sub-agent-provider",
        "local",
        "--sub-agent-model",
        "claude-test",
        "--sub-agent-model-reasoning-effort",
        "high",
        "--sub-agent-url",
        "http://127.0.0.1:8131",
    ]);
    assert_eq!(
        enabled.sub_agent_preference(),
        SubAgentPreference::Enabled(SubAgentConfig {
            provider: ProviderId::Local,
            model: Some("claude-test".to_string()),
            model_reasoning_effort: Some(SubAgentReasoningEffort::High),
            url: Some("http://127.0.0.1:8131".to_string()),
            max_concurrency: Default::default(),
        })
    );
}

#[test]
fn desktop_frontend_rejects_sub_agents_before_launch() {
    let args = super_command(&["--sub-agent", "gui"]);
    let error = args
        .validate_urls()
        .expect_err("desktop must not silently ignore sub-agent configuration");
    assert!(error.contains("Codex Desktop"), "{error}");
}

#[test]
fn sub_agent_config_debug_redacts_endpoint_value() {
    let args = super_command(&[
        "--sub-agent",
        "--sub-agent-provider",
        "local",
        "--sub-agent-url",
        "https://example.com/private-path",
    ]);
    let rendered = format!("{:?}", args.sub_agent_config());
    assert!(rendered.contains("url_configured: true"), "{rendered}");
    assert!(!rendered.contains("private-path"), "{rendered}");
}

#[test]
fn launch_target_debug_redacts_session_uuid() {
    let target = SuperLaunchTarget::Resume {
        session_id: SESSION_ID.to_string(),
    };
    let rendered = format!("{target:?}");
    assert!(rendered.contains("resume <SESSION_UUID>"), "{rendered}");
    assert!(!rendered.contains(SESSION_ID), "{rendered}");
}

#[test]
fn super_help_documents_sub_agent_layer() {
    let help = parse_cli_command_from(["prodex", "super", "--help"])
        .expect_err("help should be returned as a clap error")
        .to_string();
    for flag in [
        "--sub-agent",
        "--no-sub-agent",
        "--sub-agent-provider",
        "--sub-agent-model",
        "--sub-agent-model-reasoning-effort",
        "--sub-agent-url",
        "--sub-agent-max-concurrency",
        "xhigh",
        "max",
    ] {
        assert!(help.contains(flag), "help omitted {flag}: {help}");
    }
}
