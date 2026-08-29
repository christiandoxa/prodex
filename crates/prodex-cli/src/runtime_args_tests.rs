use super::*;

fn super_args_from(codex_args: &[&str]) -> SuperArgs {
    let os: Vec<OsString> = codex_args.iter().map(OsString::from).collect();
    SuperArgs {
        codex_args: os,
        provider: None,
        harness: None,
        api_key: None,
        local_model: None,
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: false,
        full_access: false,
        dry_run: false,
        base_url: None,
        no_proxy: false,
        presidio: false,
        no_presidio: false,
        sub_agent: false,
        no_sub_agent: false,
        sub_agent_provider: None,
        sub_agent_model: None,
        sub_agent_model_reasoning_effort: None,
        sub_agent_url: None,
        sub_agent_max_concurrency: None,
        tools: Vec::new(),
        required_tools: Vec::new(),
        url: None,
        cli: None,
        local_context_window: None,
        local_auto_compact_token_limit: None,
        codex_features: CodexRuntimeFeatureArgs::default(),
    }
}

#[test]
fn extract_provider_flags_from_codex_args_after_session_id() {
    let mut args = super_args_from(&[
        "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9",
        "--provider",
        "deepseek",
        "--model",
        "deepseek-v4-pro",
        "--api-key",
        "sk-test",
    ]);
    args.extract_provider_overrides_from_codex_args().unwrap();
    assert_eq!(args.provider, Some(SuperExternalProvider::DeepSeek));
    assert_eq!(args.local_model.as_deref(), Some("deepseek-v4-pro"));
    assert_eq!(args.api_key.as_deref(), Some("sk-test"));
    assert_eq!(
        args.codex_args
            .iter()
            .map(|a| a.to_string_lossy())
            .collect::<Vec<_>>(),
        vec!["019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"]
    );
}

#[test]
fn extract_provider_equals_syntax_from_codex_args() {
    let mut args = super_args_from(&[
        "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9",
        "--provider=gemini",
        "--model=gemini-2.5-pro",
    ]);
    args.extract_provider_overrides_from_codex_args().unwrap();
    assert_eq!(args.provider, Some(SuperExternalProvider::Gemini));
    assert_eq!(args.local_model.as_deref(), Some("gemini-2.5-pro"));
    assert_eq!(
        args.codex_args
            .iter()
            .map(|a| a.to_string_lossy())
            .collect::<Vec<_>>(),
        vec!["019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"]
    );
}

#[test]
fn extract_provider_kiro_from_codex_args() {
    let mut args = super_args_from(&[
        "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9",
        "--provider",
        "kiro",
        "--model",
        "claude-sonnet-4",
    ]);
    args.extract_provider_overrides_from_codex_args().unwrap();
    assert_eq!(args.provider, Some(SuperExternalProvider::Kiro));
    assert_eq!(args.local_model.as_deref(), Some("claude-sonnet-4"));
    assert_eq!(
        args.codex_args
            .iter()
            .map(|a| a.to_string_lossy())
            .collect::<Vec<_>>(),
        vec!["019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"]
    );
}

#[test]
fn super_external_provider_codex_args_support_kiro() {
    let args = super_external_provider_codex_args(
        SuperExternalProvider::Kiro,
        "http://127.0.0.1:4317/v1",
        Some("claude-sonnet-4.5"),
        Some(222_222),
        Some(111_111),
    );
    let rendered = args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(rendered.contains(&format!(
        "model_provider={}",
        toml_string_literal(SUPER_KIRO_PROVIDER_ID)
    )));
    assert!(rendered.contains(&format!(
        "model={}",
        toml_string_literal("claude-sonnet-4.5")
    )));
    assert!(rendered.contains(&format!(
        "model_providers.{SUPER_KIRO_PROVIDER_ID}.name={}",
        toml_string_literal("Azure")
    )));
    assert!(rendered.contains(&format!(
        "model_providers.{SUPER_KIRO_PROVIDER_ID}.base_url={}",
        toml_string_literal("http://127.0.0.1:4317/v1")
    )));
    assert!(rendered.contains(&"model_context_window=222222".to_string()));
    assert!(rendered.contains(&"model_auto_compact_token_limit=111111".to_string()));
}

#[test]
fn super_external_provider_codex_args_support_copilot_compact() {
    let args = super_external_provider_codex_args(
        SuperExternalProvider::Copilot,
        "https://api.githubcopilot.com",
        Some("gpt-5.3-codex"),
        Some(333_333),
        Some(222_222),
    );
    let rendered = args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(rendered.contains(&format!(
        "model_provider={}",
        toml_string_literal(SUPER_COPILOT_PROVIDER_ID)
    )));
    assert!(rendered.contains(&format!(
        "model_providers.{SUPER_COPILOT_PROVIDER_ID}.name={}",
        toml_string_literal("OpenAI")
    )));
    assert!(rendered.contains(&"model_context_window=333333".to_string()));
    assert!(rendered.contains(&"model_auto_compact_token_limit=222222".to_string()));
}

#[test]
fn extract_noop_when_no_provider_flags_in_codex_args() {
    let mut args = super_args_from(&["just", "some", "codex", "args"]);
    args.extract_provider_overrides_from_codex_args().unwrap();
    assert_eq!(args.provider, None);
    assert_eq!(
        args.codex_args
            .iter()
            .map(|a| a.to_string_lossy())
            .collect::<Vec<_>>(),
        vec!["just", "some", "codex", "args"]
    );
}

#[test]
fn extract_respects_already_set_provider() {
    let mut args = super_args_from(&[
        "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9",
        "--provider",
        "deepseek",
    ]);
    // Simulate clap already setting provider
    args.provider = Some(SuperExternalProvider::DeepSeek);
    args.extract_provider_overrides_from_codex_args().unwrap();
    // Should overwrite with extracted value (same here but structurally ok)
    assert_eq!(args.provider, Some(SuperExternalProvider::DeepSeek));
    // codex_args should be cleaned of provider flags
    assert_eq!(
        args.codex_args
            .iter()
            .map(|a| a.to_string_lossy())
            .collect::<Vec<_>>(),
        vec!["019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"]
    );
}

#[test]
fn super_url_validation_rejects_secrets_without_echoing_them() {
    for (base_url, url) in [
        (
            Some("https://user:super-base-secret-sentinel@example.test"),
            None,
        ),
        (
            None,
            Some("https://example.test/v1?token=super-url-secret-sentinel"),
        ),
    ] {
        let mut args = super_args_from(&[]);
        args.base_url = base_url.map(str::to_string);
        args.url = url.map(str::to_string);

        let error = args.validate_urls().unwrap_err();

        assert!(
            error.contains("no credentials, query, or fragment"),
            "{error}"
        );
        assert!(!error.contains("secret-sentinel"), "{error}");
    }
}

#[test]
fn runtime_arg_debug_redacts_url_and_passthrough_values() {
    let sentinel = "runtime-args-debug-secret-sentinel";
    let run = RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: false,
        full_access: false,
        base_url: Some(format!("https://user:{sentinel}@example.test")),
        no_proxy: false,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![OsString::from(sentinel)],
    };
    let claude = ClaudeArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: false,
        base_url: Some(format!("https://user:{sentinel}@example.test")),
        no_proxy: false,
        claude_args: vec![OsString::from(sentinel)],
    };

    for rendered in [
        format!("{:?}", crate::Commands::Run(run)),
        format!("{:?}", crate::Commands::Claude(claude)),
    ] {
        assert!(rendered.contains("base_url_configured: true"), "{rendered}");
        assert!(!rendered.contains(sentinel), "{rendered}");
    }
}
