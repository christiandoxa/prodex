use super::*;
use crate::{CodexCurrentTimeClockSource, CodexWebSearchMode, SubAgentReasoningEffort};

fn super_args_from(codex_args: &[&str]) -> SuperArgs {
    let os: Vec<OsString> = codex_args.iter().map(OsString::from).collect();
    SuperArgs {
        codex_args: os,
        provider: None,
        cli: None,
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
        local_context_window: None,
        local_auto_compact_token_limit: None,
        codex_features: CodexRuntimeFeatureArgs::default(),
    }
}

#[test]
fn super_override_scan_has_fixed_caller_boundary_values_for_every_kind() {
    const SESSION: &str = "00000000-0000-7000-8000-000000000042";
    let mut args = super_args_from(&[
        SESSION,
        "--provider=gemini",
        "--cli=agy",
        "--api-key=fake-api-key",
        "--sub-agent-provider=google",
        "--sub-agent-model=vendor/child",
        "--sub-agent-model-reasoning-effort=xhigh",
        "--sub-agent-url=http://example.com/agent",
        "--sub-agent-max-concurrency=16",
        "--model=vendor/model",
        "--profile=synthetic",
        "--base-url=https://example.com/v1",
        "--url=http://127.0.0.1:3000/v1",
        "--context-window=8192",
        "--auto-compact-token-limit=1024",
        "--tool=rtk",
        "--require-tool=ponytail",
        "--web-search=live",
        "--rollout-budget-tokens=64",
        "--rollout-budget-reminders=1,2,3",
        "--rollout-budget-sampling-weight=0.5",
        "--rollout-budget-prefill-weight=0.25",
        "--current-time-reminder-interval=120",
        "--current-time-clock-source=external",
        "--no-auto-rotate",
        "--auto-rotate",
        "--auto-redeem",
        "--skip-quota-check",
        "--dry-run",
        "--no-proxy",
        "--presidio",
        "--no-presidio",
        "--sub-agent",
        "--no-sub-agent",
        "--full-access",
        "--current-time-reminder",
        "--respect-system-proxy",
        "--no-respect-system-proxy",
    ]);

    args.extract_super_overrides_from_codex_args_for_native_preflight()
        .unwrap();

    assert_eq!(args.provider, Some(SuperExternalProvider::Gemini));
    assert_eq!(args.cli, Some(SuperCliAgent::Agy));
    assert_eq!(args.api_key.as_deref(), Some("fake-api-key"));
    assert_eq!(
        args.sub_agent_provider,
        Some(prodex_provider_core::ProviderId::Gemini)
    );
    assert_eq!(args.sub_agent_model.as_deref(), Some("vendor/child"));
    assert_eq!(
        args.sub_agent_model_reasoning_effort,
        Some(SubAgentReasoningEffort::XHigh)
    );
    assert_eq!(
        args.sub_agent_url.as_deref(),
        Some("http://example.com/agent")
    );
    assert_eq!(args.sub_agent_max_concurrency.unwrap().get(), 16);
    assert_eq!(args.local_model.as_deref(), Some("vendor/model"));
    assert_eq!(args.profile.as_deref(), Some("synthetic"));
    assert_eq!(args.base_url.as_deref(), Some("https://example.com/v1"));
    assert_eq!(args.url.as_deref(), Some("http://127.0.0.1:3000/v1"));
    assert_eq!(args.local_context_window, Some(8192));
    assert_eq!(args.local_auto_compact_token_limit, Some(1024));
    assert_eq!(
        args.tools,
        [
            prodex_optional_tools::OptionalToolId::Rtk,
            prodex_optional_tools::OptionalToolId::Ponytail,
        ]
    );
    assert_eq!(
        args.required_tools,
        [prodex_optional_tools::OptionalToolId::Ponytail]
    );
    assert_eq!(
        args.codex_features.web_search,
        Some(CodexWebSearchMode::Live)
    );
    assert_eq!(args.codex_features.rollout_budget_tokens, Some(64));
    assert_eq!(args.codex_features.rollout_budget_reminders, [1, 2, 3]);
    assert_eq!(
        args.codex_features.rollout_budget_sampling_weight,
        Some(0.5)
    );
    assert_eq!(
        args.codex_features.rollout_budget_prefill_weight,
        Some(0.25)
    );
    assert_eq!(
        args.codex_features.current_time_reminder_interval,
        Some(120)
    );
    assert_eq!(
        args.codex_features.current_time_clock_source,
        Some(CodexCurrentTimeClockSource::External)
    );
    assert!(args.auto_rotate);
    assert!(!args.no_auto_rotate);
    assert!(args.auto_redeem);
    assert!(args.skip_quota_check);
    assert!(args.dry_run);
    assert!(args.no_proxy);
    assert!(args.presidio && args.no_presidio);
    assert!(args.sub_agent && args.no_sub_agent);
    assert!(args.full_access);
    assert!(args.codex_features.current_time_reminder);
    assert!(!args.codex_features.respect_system_proxy);
    assert!(args.codex_features.no_respect_system_proxy);
    assert_eq!(args.codex_args, [OsString::from(SESSION)]);
}

#[cfg(unix)]
fn opaque_argument() -> OsString {
    use std::os::unix::ffi::OsStringExt;
    OsString::from_vec(vec![0xff])
}

#[cfg(windows)]
fn opaque_argument() -> OsString {
    use std::os::windows::ffi::OsStringExt;
    OsString::from_wide(&[0xd800])
}

#[test]
#[cfg(any(unix, windows))]
fn super_override_extraction_preserves_opaque_args_and_separator_tail() {
    const SESSION: &str = "00000000-0000-7000-8000-000000000042";
    let opaque = opaque_argument();
    let mut args = super_args_from(&[SESSION, "--dry-run"]);
    args.codex_args.insert(1, opaque.clone());
    args.codex_args
        .extend([OsString::from("--"), OsString::from("--provider=gemini")]);

    args.extract_super_overrides_from_codex_args().unwrap();

    assert!(args.dry_run);
    assert_eq!(
        args.codex_args,
        [
            OsString::from(SESSION),
            opaque,
            OsString::from("--"),
            OsString::from("--provider=gemini"),
        ]
    );
}

#[test]
fn super_override_duplicates_keep_caller_precedence_and_values() {
    const SESSION: &str = "00000000-0000-7000-8000-000000000042";
    let mut args = super_args_from(&[
        SESSION,
        "--profile=first",
        "--profile",
        "second",
        "--provider=gemini",
        "--provider",
        "deepseek",
        "--model=first",
        "--local-model",
        "last",
        "--no-auto-rotate",
        "--auto-rotate",
        "--rollout-budget-reminders=1,2",
        "--rollout-budget-reminders",
        "2,1",
    ]);

    args.extract_super_overrides_from_codex_args().unwrap();

    assert_eq!(args.profile.as_deref(), Some("first"));
    assert_eq!(args.provider, Some(SuperExternalProvider::DeepSeek));
    assert_eq!(args.local_model.as_deref(), Some("last"));
    assert!(args.auto_rotate);
    assert!(!args.no_auto_rotate);
    assert_eq!(args.codex_features.rollout_budget_reminders, [1, 2, 2, 1]);
    assert_eq!(args.codex_args, [OsString::from(SESSION)]);

    let mut args = super_args_from(&[SESSION, "--model", "--dry-run"]);
    assert_eq!(
        args.extract_super_overrides_from_codex_args().unwrap_err(),
        "--model requires a value"
    );
    assert!(!args.dry_run);
    assert_eq!(
        args.codex_args,
        [
            OsString::from(SESSION),
            OsString::from("--model"),
            OsString::from("--dry-run"),
        ]
    );
}

#[test]
fn super_override_invalid_value_keeps_prior_mutations_and_validation_order() {
    const SESSION: &str = "00000000-0000-7000-8000-000000000042";
    let mut args = super_args_from(&[
        SESSION,
        "--dry-run",
        "--sub-agent",
        "--no-sub-agent",
        "--provider",
        "invalid-provider",
        "--model=unreached",
    ]);

    let error = args.extract_super_overrides_from_codex_args().unwrap_err();

    assert!(error.starts_with("invalid --provider:"), "{error}");
    assert!(args.dry_run);
    assert!(args.sub_agent);
    assert!(args.no_sub_agent);
    assert_eq!(args.local_model, None);
    assert_eq!(
        args.codex_args,
        [
            OsString::from(SESSION),
            OsString::from("--provider"),
            OsString::from("invalid-provider"),
            OsString::from("--model=unreached"),
        ]
    );

    let mut args = super_args_from(&[SESSION, "--sub-agent", "--no-sub-agent", "--model=done"]);
    assert!(
        args.extract_super_overrides_from_codex_args()
            .unwrap_err()
            .contains("conflicts")
    );
    assert!(args.sub_agent);
    assert!(args.no_sub_agent);
    assert_eq!(args.local_model.as_deref(), Some("done"));
    assert_eq!(args.codex_args, [OsString::from(SESSION)]);

    let mut args = super_args_from(&[SESSION, "--sub-agent", "--no-sub-agent"]);
    args.extract_super_overrides_from_codex_args_for_native_preflight()
        .unwrap();
    assert!(args.sub_agent);
    assert!(args.no_sub_agent);
    assert_eq!(args.codex_args, [OsString::from(SESSION)]);
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
