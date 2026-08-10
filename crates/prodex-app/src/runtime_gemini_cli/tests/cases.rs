#![cfg(test)]

use super::{
    PRODEX_COPILOT_PROXY_API_KEY, PreparedRuntimeLaunch, RuntimeProxyEndpoint,
    SUPER_COPILOT_PROVIDER_ID, SuperArgs, SuperCliAgent, SuperExternalProvider,
    SuperNativeCliLaunchStrategy, build_super_gemini_child, runtime_super_copilot_cli_env,
    runtime_super_native_cli_launch_args, super_native_cli_dry_run_report,
    validate_super_native_cli_preflight,
};
#[cfg(unix)]
use crate::TestEnvVarGuard;
use crate::{AppPaths, RuntimeLaunchStrategy};
use prodex_cli::CodexRuntimeFeatureArgs;
use std::ffi::OsString;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn native_cli_super_args() -> SuperArgs {
    SuperArgs {
        profile: Some("kiro-main".to_string()),
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
        provider: None,
        harness: None,
        cli: None,
        api_key: None,
        local_model: None,
        local_context_window: None,
        local_auto_compact_token_limit: None,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: Vec::new(),
    }
}

#[test]
fn native_gemini_cli_defaults_to_yolo_and_forwards_model() {
    assert_eq!(
        runtime_super_native_cli_launch_args(
            SuperCliAgent::Gemini,
            &[OsString::from("review")],
            Some("gemini-test"),
        ),
        vec![
            OsString::from("--model"),
            OsString::from("gemini-test"),
            OsString::from("--yolo"),
            OsString::from("review"),
        ]
    );
}

#[test]
fn native_gemini_cli_keeps_explicit_approval_mode() {
    let args = [OsString::from("--approval-mode"), OsString::from("plan")];
    assert_eq!(
        runtime_super_native_cli_launch_args(SuperCliAgent::Gemini, &args, None),
        args
    );
}

#[test]
fn native_gemini_cli_keeps_equals_approval_mode() {
    let args = [OsString::from("--approval-mode=plan")];
    assert_eq!(
        runtime_super_native_cli_launch_args(SuperCliAgent::Gemini, &args, None),
        args
    );
}

#[test]
fn native_cli_keeps_equals_model_flag() {
    let args = [OsString::from("--model=existing-model")];
    assert_eq!(
        runtime_super_native_cli_launch_args(SuperCliAgent::Copilot, &args, Some("ignored-model"),),
        args
    );
}

#[test]
fn native_agy_defaults_to_dangerously_skip_permissions() {
    assert_eq!(
        runtime_super_native_cli_launch_args(
            SuperCliAgent::Agy,
            &[OsString::from("--continue")],
            None,
        ),
        vec![
            OsString::from("--dangerously-skip-permissions"),
            OsString::from("--continue"),
        ]
    );
}

#[test]
#[cfg(unix)]
fn native_agy_preflight_rejects_missing_capability() {
    let missing = std::env::temp_dir().join(format!(
        "prodex-missing-agy-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _agy = TestEnvVarGuard::set("PRODEX_AGY_BIN", missing.to_str().unwrap());
    let mut args = native_cli_super_args();
    args.cli = Some(SuperCliAgent::Agy);
    args.provider = Some(SuperExternalProvider::Gemini);

    let error = validate_super_native_cli_preflight(&args).unwrap_err();

    assert!(error.to_string().contains("Antigravity CLI capability"));
}

#[test]
fn native_sub_agent_is_rejected_during_preflight_before_native_probe() {
    for (agent, provider) in [
        (SuperCliAgent::Gemini, Some(SuperExternalProvider::Gemini)),
        (SuperCliAgent::Copilot, Some(SuperExternalProvider::Copilot)),
        (SuperCliAgent::Kiro, None),
        (SuperCliAgent::Agy, Some(SuperExternalProvider::Gemini)),
    ] {
        let mut args = native_cli_super_args();
        args.cli = Some(agent);
        args.provider = provider;
        args.sub_agent = true;

        let error = validate_super_native_cli_preflight(&args).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("supported only on the Codex Super bridge"),
            "{agent:?}: {error}"
        );
    }
}

#[test]
fn native_agy_rejects_codex_tools_presidio_and_resume_options() {
    let mut cases = Vec::new();

    let mut feature = native_cli_super_args();
    feature.codex_features.current_time_reminder = true;
    cases.push((feature, "--current-time-reminder"));

    let mut tool = native_cli_super_args();
    tool.tools.push(prodex_optional_tools::OptionalToolId::Rtk);
    cases.push((tool, "--tool"));

    let mut required_tool = native_cli_super_args();
    required_tool
        .required_tools
        .push(prodex_optional_tools::OptionalToolId::Ponytail);
    cases.push((required_tool, "--require-tool"));

    let mut presidio = native_cli_super_args();
    presidio.presidio = true;
    cases.push((presidio, "--presidio"));

    let mut resume = native_cli_super_args();
    resume.codex_args = vec![OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9")];
    cases.push((resume, "Codex session resume"));

    for (mut args, option) in cases {
        args.cli = Some(SuperCliAgent::Agy);
        args.provider = Some(SuperExternalProvider::Gemini);
        let error = validate_super_native_cli_preflight(&args).unwrap_err();
        let message = error.to_string();
        assert!(message.contains(option), "{message}");
        assert!(message.contains("Antigravity (agy)"), "{message}");
    }
}

#[test]
fn native_agy_rejects_sub_agent_detail_extracted_after_positional() {
    let mut args = native_cli_super_args();
    args.cli = Some(SuperCliAgent::Agy);
    args.provider = Some(SuperExternalProvider::Gemini);
    args.codex_args = vec![
        OsString::from("prompt"),
        OsString::from("--sub-agent-model"),
        OsString::from("custom-model"),
    ];

    let error = validate_super_native_cli_preflight(&args).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("--sub-agent-model"), "{message}");
    assert!(message.contains("Antigravity (agy)"), "{message}");
}

#[test]
fn native_frontends_reject_codex_features_and_unsupported_optional_tools() {
    for (agent, provider) in [
        (SuperCliAgent::Gemini, Some(SuperExternalProvider::Gemini)),
        (SuperCliAgent::Copilot, Some(SuperExternalProvider::Copilot)),
        (SuperCliAgent::Kiro, None),
    ] {
        let mut feature = native_cli_super_args();
        feature.cli = Some(agent);
        feature.provider = provider;
        feature.codex_features.current_time_reminder = true;
        let error = validate_super_native_cli_preflight(&feature).unwrap_err();
        assert!(error.to_string().contains("--current-time-reminder"));
    }

    for (agent, provider) in [
        (SuperCliAgent::Gemini, Some(SuperExternalProvider::Gemini)),
        (SuperCliAgent::Copilot, Some(SuperExternalProvider::Copilot)),
        (SuperCliAgent::Kiro, None),
        (SuperCliAgent::Agy, Some(SuperExternalProvider::Gemini)),
    ] {
        let mut tool = native_cli_super_args();
        tool.cli = Some(agent);
        tool.provider = provider;
        tool.tools.push(prodex_optional_tools::OptionalToolId::Rtk);
        let error = validate_super_native_cli_preflight(&tool).unwrap_err();
        assert!(error.to_string().contains("--tool rtk"));

        let mut required_tool = native_cli_super_args();
        required_tool.cli = Some(agent);
        required_tool.provider = provider;
        required_tool
            .required_tools
            .push(prodex_optional_tools::OptionalToolId::Ponytail);
        let error = validate_super_native_cli_preflight(&required_tool).unwrap_err();
        assert!(error.to_string().contains("--require-tool ponytail"));
    }

    for (agent, provider) in [
        (SuperCliAgent::Gemini, Some(SuperExternalProvider::Gemini)),
        (SuperCliAgent::Kiro, None),
    ] {
        let mut tool = native_cli_super_args();
        tool.cli = Some(agent);
        tool.provider = provider;
        tool.tools
            .push(prodex_optional_tools::OptionalToolId::Presidio);
        let error = validate_super_native_cli_preflight(&tool).unwrap_err();
        assert!(error.to_string().contains("--tool presidio"));
    }

    let mut copilot_tool = native_cli_super_args();
    copilot_tool.cli = Some(SuperCliAgent::Copilot);
    copilot_tool.provider = Some(SuperExternalProvider::Copilot);
    copilot_tool
        .tools
        .push(prodex_optional_tools::OptionalToolId::Presidio);
    validate_super_native_cli_preflight(&copilot_tool).unwrap();

    let mut copilot_required = copilot_tool;
    copilot_required
        .required_tools
        .push(prodex_optional_tools::OptionalToolId::Presidio);
    validate_super_native_cli_preflight(&copilot_required).unwrap();
}

#[test]
#[cfg(unix)]
fn native_agy_capability_rejection_precedes_binary_probe_and_overlay_side_effects() {
    use std::os::unix::fs::PermissionsExt;

    let root = std::env::temp_dir().join(format!(
        "prodex-agy-capability-preflight-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let binary = root.join("agy");
    let marker = root.join("spawned.marker");
    std::fs::write(
        &binary,
        "#!/bin/sh\nprintf spawned > \"$PRODEX_AGY_PREFLIGHT_MARKER\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _agy = TestEnvVarGuard::set("PRODEX_AGY_BIN", binary.to_str().unwrap());
    let _marker = TestEnvVarGuard::set("PRODEX_AGY_PREFLIGHT_MARKER", marker.to_str().unwrap());

    let mut args = native_cli_super_args();
    args.cli = Some(SuperCliAgent::Agy);
    args.provider = Some(SuperExternalProvider::Gemini);
    args.required_tools
        .push(prodex_optional_tools::OptionalToolId::Rtk);

    let error = validate_super_native_cli_preflight(&args).unwrap_err();
    assert!(error.to_string().contains("--require-tool"));
    assert!(
        !marker.exists(),
        "unsupported options must fail before probing agy"
    );
    assert_eq!(
        std::fs::read_dir(&root).unwrap().count(),
        1,
        "preflight must not create an overlay or install tools"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_tail_sub_agent_is_rejected_before_side_effects() {
    let mut args = native_cli_super_args();
    args.provider = Some(SuperExternalProvider::Gemini);
    args.codex_args = vec![
        OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        OsString::from("--cli"),
        OsString::from("gemini"),
        OsString::from("--sub-agent"),
    ];

    let error = validate_super_native_cli_preflight(&args).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("supported only on the Codex Super bridge"),
        "{error}"
    );
}

#[test]
fn native_tail_feature_rejection_names_option_and_selected_frontend() {
    let mut args = native_cli_super_args();
    args.provider = Some(SuperExternalProvider::Gemini);
    args.codex_args = vec![
        OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        OsString::from("--cli=gemini"),
        OsString::from("--web-search=live"),
    ];

    let error = validate_super_native_cli_preflight(&args).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("--web-search"), "{message}");
    assert!(message.contains("Gemini"), "{message}");
}

#[test]
fn native_unsupported_harness_and_api_key_name_selected_frontend() {
    let mut harness = native_cli_super_args();
    harness.cli = Some(SuperCliAgent::Gemini);
    harness.provider = Some(SuperExternalProvider::Gemini);
    harness.harness = Some(prodex_provider_core::HarnessMode::Minimal);
    let error = validate_super_native_cli_preflight(&harness).unwrap_err();
    assert!(error.to_string().contains("--harness"));
    assert!(error.to_string().contains("Gemini"));

    let mut api_key = native_cli_super_args();
    api_key.cli = Some(SuperCliAgent::Kiro);
    api_key.api_key = Some("test-key".to_string());
    let error = validate_super_native_cli_preflight(&api_key).unwrap_err();
    assert!(error.to_string().contains("--api-key"));
    assert!(error.to_string().contains("Kiro"));
}

#[test]
fn native_copilot_cli_forwards_model_without_google_flags() {
    assert_eq!(
        runtime_super_native_cli_launch_args(
            SuperCliAgent::Copilot,
            &[OsString::from("--prompt"), OsString::from("review")],
            Some("gpt-test"),
        ),
        vec![
            OsString::from("--model"),
            OsString::from("gpt-test"),
            OsString::from("--prompt"),
            OsString::from("review"),
        ]
    );
}

#[test]
fn native_cli_dry_run_is_redacted_and_does_not_resolve_credentials() {
    let mut args = native_cli_super_args();
    args.cli = Some(SuperCliAgent::Copilot);
    args.provider = Some(SuperExternalProvider::Copilot);
    args.api_key = Some("secret-provider-key".to_string());
    args.profile = Some("private@example.com".to_string());
    args.local_model = Some("gpt-test".to_string());
    args.codex_args = vec![OsString::from("--prompt"), OsString::from("review")];

    let report = super_native_cli_dry_run_report(&args, None).unwrap();

    assert!(report.contains("Provider: copilot"));
    assert!(report.contains("Model: gpt-test"));
    assert!(report.contains("would use local provider bridge"));
    assert!(report.contains("--prompt"));
    assert!(report.contains("Profile: <configured>"));
    assert!(!report.contains("private@example.com"));
    assert!(!report.contains("secret-provider-key"));

    args.local_model = Some("sk-proj-native-secret".to_string());
    let report = super_native_cli_dry_run_report(&args, None).unwrap();
    assert!(!report.contains("sk-proj-native-secret"));
}

#[test]
#[cfg(unix)]
fn native_gemini_cli_dry_run_does_not_probe_optional_tools() {
    use std::os::unix::fs::PermissionsExt;

    let root = std::env::temp_dir().join(format!(
        "prodex-native-cli-dry-run-passive-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let bin = root.join("bin");
    let marker = root.join("optional-tool-probe.marker");
    std::fs::create_dir_all(&bin).unwrap();
    for command in ["rtk", "codebase-memory-mcp", "node", "npx"] {
        let path = bin.join(command);
        std::fs::write(
            &path,
            "#!/bin/sh\nprintf x >> \"$PRODEX_OPTIONAL_TOOL_PROBE_MARKER\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let root_text = root.to_str().unwrap();
    let _optimizers_home =
        TestEnvVarGuard::set(prodex_optional_tools::PRODEX_OPTIMIZERS_HOME_ENV, root_text);
    let _xdg_data_home = TestEnvVarGuard::set("XDG_DATA_HOME", root_text);
    let _home = TestEnvVarGuard::set("HOME", root_text);
    let _path = TestEnvVarGuard::set("PATH", bin.to_str().unwrap());
    let _marker = TestEnvVarGuard::set(
        "PRODEX_OPTIONAL_TOOL_PROBE_MARKER",
        marker.to_str().unwrap(),
    );

    let mut args = native_cli_super_args();
    args.profile = None;
    args.dry_run = true;
    args.cli = Some(SuperCliAgent::Gemini);
    args.provider = Some(SuperExternalProvider::Gemini);
    for codex_args in [
        vec![OsString::from("--version")],
        vec![OsString::from("--help")],
        vec![OsString::from("review")],
    ] {
        args.codex_args = codex_args;
        let report = super_native_cli_dry_run_report(&args, None).unwrap();
        assert!(report.contains("Provider: gemini"));
        assert!(report.contains("native Gemini CLI owns transport and authentication"));
    }

    let mut required_tool = args.clone();
    required_tool
        .required_tools
        .push(prodex_optional_tools::OptionalToolId::Rtk);
    let error = super_native_cli_dry_run_report(&required_tool, None).unwrap_err();
    assert!(error.to_string().contains("--require-tool rtk"));

    assert!(!marker.exists(), "dry-run must not execute optional tools");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_gemini_child_uses_cli_owned_auth_without_oauth_injection() {
    let mut args = native_cli_super_args();
    args.profile = None;
    let child = build_super_gemini_child(
        &args,
        Path::new("/synthetic/native-gemini"),
        vec![OsString::from("review")],
    )
    .unwrap();

    assert!(child.removed_env.iter().any(|key| key == "OPENAI_API_KEY"));
    assert!(!child.removed_env.iter().any(|key| key == "GEMINI_API_KEY"));
    assert!(child.extra_env.is_empty());
    assert!(
        !child
            .extra_env
            .iter()
            .any(|(key, _)| key == "GOOGLE_CLOUD_ACCESS_TOKEN")
    );
}

#[test]
fn native_gemini_explicit_api_key_overrides_inherited_provider_keys_safely() {
    let mut args = native_cli_super_args();
    args.profile = None;
    args.api_key = Some("synthetic-native-gemini-key".to_string());
    let child =
        build_super_gemini_child(&args, Path::new("/synthetic/native-gemini"), Vec::new()).unwrap();

    for key in super::NATIVE_GEMINI_AUTH_ENV_KEYS {
        assert!(child.removed_env.iter().any(|removed| removed == key));
    }
    assert_eq!(
        child
            .extra_env
            .iter()
            .find(|(key, _)| key == "GEMINI_API_KEY")
            .map(|(_, value)| value.as_os_str()),
        Some(std::ffi::OsStr::new("synthetic-native-gemini-key"))
    );
    assert!(!format!("{child:?}").contains("synthetic-native-gemini-key"));
}

#[test]
fn native_gemini_preflight_accepts_cli_owned_auth_but_rejects_prodex_profile_and_presidio() {
    let mut supported = native_cli_super_args();
    supported.profile = None;
    supported.cli = Some(SuperCliAgent::Gemini);
    supported.provider = Some(SuperExternalProvider::Gemini);
    supported.api_key = Some("synthetic-native-gemini-key".to_string());
    validate_super_native_cli_preflight(&supported).unwrap();

    let mut profile = supported.clone();
    profile.profile = Some("legacy-gemini".to_string());
    let error = validate_super_native_cli_preflight(&profile).unwrap_err();
    assert!(error.to_string().contains("--profile is unsupported"));

    let mut presidio = supported;
    presidio.presidio = true;
    let error = validate_super_native_cli_preflight(&presidio).unwrap_err();
    assert!(error.to_string().contains("--presidio"));
}

#[test]
fn native_copilot_cli_uses_local_responses_provider_contract() {
    let endpoint = RuntimeProxyEndpoint {
        listen_addr: "127.0.0.1:48123".parse().unwrap(),
        openai_mount_path: "/v1".to_string(),
        local_model_provider_id: Some(SUPER_COPILOT_PROVIDER_ID.to_string()),
        force_http_responses: false,
        realtime_ws_base_url: None,
        realtime_ws_model: None,
        lease_dir: std::env::temp_dir(),
        broker_session_affinity_control: None,
        _lease: None,
        _direct_proxy: None,
        _kiro_connect_proxy: None,
    };
    let env = runtime_super_copilot_cli_env(&endpoint, "gpt-test");
    let value = |key: &str| {
        env.iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.to_string_lossy().into_owned())
    };

    assert_eq!(
        value("COPILOT_PROVIDER_BASE_URL").as_deref(),
        Some("http://127.0.0.1:48123/v1")
    );
    assert_eq!(value("COPILOT_PROVIDER_TYPE").as_deref(), Some("openai"));
    assert_eq!(
        value("COPILOT_PROVIDER_WIRE_API").as_deref(),
        Some("responses")
    );
    assert_eq!(value("COPILOT_PROVIDER_TRANSPORT").as_deref(), Some("http"));
    assert_eq!(value("COPILOT_MODEL").as_deref(), Some("gpt-test"));
    assert_eq!(
        value("COPILOT_PROVIDER_API_KEY").as_deref(),
        Some(PRODEX_COPILOT_PROXY_API_KEY)
    );
}

#[test]
fn native_kiro_cli_injects_chat_model_when_needed() {
    assert_eq!(
        runtime_super_native_cli_launch_args(
            SuperCliAgent::Kiro,
            &[OsString::from("review this repo")],
            Some("claude-4-sonnet"),
        ),
        vec![
            OsString::from("chat"),
            OsString::from("--model"),
            OsString::from("claude-4-sonnet"),
            OsString::from("review this repo"),
        ]
    );
}

#[test]
fn native_kiro_cli_keeps_explicit_model_flag() {
    let args = [OsString::from("--model"), OsString::from("existing-model")];
    assert_eq!(
        runtime_super_native_cli_launch_args(SuperCliAgent::Kiro, &args, Some("ignored")),
        args
    );
}

#[test]
fn native_kiro_cli_keeps_equals_model_flag() {
    let args = [OsString::from("--model=existing-model")];
    assert_eq!(
        runtime_super_native_cli_launch_args(SuperCliAgent::Kiro, &args, Some("ignored")),
        args
    );
}

#[test]
fn native_kiro_cli_adds_model_to_explicit_chat_subcommand() {
    assert_eq!(
        runtime_super_native_cli_launch_args(
            SuperCliAgent::Kiro,
            &[OsString::from("chat"), OsString::from("review this repo")],
            Some("claude-4-sonnet"),
        ),
        vec![
            OsString::from("chat"),
            OsString::from("--model"),
            OsString::from("claude-4-sonnet"),
            OsString::from("review this repo"),
        ]
    );
}

#[test]
fn native_kiro_cli_does_not_rewrite_non_chat_subcommands() {
    let args = [OsString::from("settings"), OsString::from("list")];
    assert_eq!(
        runtime_super_native_cli_launch_args(SuperCliAgent::Kiro, &args, Some("claude-4-sonnet"),),
        args
    );
}

#[test]
fn native_kiro_cli_runtime_request_uses_only_transport_proxy_features() {
    let strategy = SuperNativeCliLaunchStrategy {
        args: native_cli_super_args(),
        presidio_enabled: true,
        agent: SuperCliAgent::Kiro,
        sub_agent: None,
    };
    let request = strategy.runtime_request();
    assert_eq!(request.external_provider, Some("kiro"));
    assert!(!request.smart_context_enabled);
    assert!(!request.presidio_redaction_enabled);
    assert_eq!(request.base_url, None);
    assert!(!request.allow_auto_rotate);
}

#[test]
fn native_antigravity_cli_runtime_request_skips_proxy_features() {
    let strategy = SuperNativeCliLaunchStrategy {
        args: native_cli_super_args(),
        presidio_enabled: true,
        agent: SuperCliAgent::Agy,
        sub_agent: None,
    };
    let request = strategy.runtime_request();
    assert_eq!(request.external_provider, Some("antigravity"));
    assert!(!request.smart_context_enabled);
    assert!(!request.presidio_redaction_enabled);
    assert!(!request.allow_auto_rotate);
    assert_eq!(request.base_url, None);
}

#[test]
fn native_gemini_cli_runtime_request_skips_prodex_oauth_and_proxy_features() {
    let mut args = native_cli_super_args();
    args.profile = None;
    args.provider = Some(SuperExternalProvider::Gemini);
    let strategy = SuperNativeCliLaunchStrategy {
        args,
        presidio_enabled: false,
        agent: SuperCliAgent::Gemini,
        sub_agent: None,
    };
    let request = strategy.runtime_request();
    assert_eq!(request.external_provider, Some("gemini-native"));
    assert!(!request.smart_context_enabled);
    assert!(!request.presidio_redaction_enabled);
    assert!(!request.allow_auto_rotate);
    assert_eq!(request.model_provider_override, None);
    assert_eq!(request.base_url, None);
}

#[test]
fn native_copilot_cli_runtime_request_enables_provider_proxy() {
    let mut args = native_cli_super_args();
    args.profile = Some("copilot-main".to_string());
    args.provider = Some(SuperExternalProvider::Copilot);
    args.api_key = Some("provider-test-key".to_string());
    let strategy = SuperNativeCliLaunchStrategy {
        args,
        presidio_enabled: true,
        agent: SuperCliAgent::Copilot,
        sub_agent: None,
    };

    let request = strategy.runtime_request();
    assert_eq!(request.external_provider, Some("copilot"));
    assert_eq!(request.external_provider_api_key, Some("provider-test-key"));
    assert_eq!(
        request.model_provider_override,
        Some(SUPER_COPILOT_PROVIDER_ID)
    );
    assert_eq!(
        request.base_url,
        Some(SuperExternalProvider::Copilot.default_base_url())
    );
    assert!(request.smart_context_enabled);
    assert!(request.presidio_redaction_enabled);
}

#[test]
fn native_gemini_cli_build_plan_uses_base_home_without_optional_tool_overlay() {
    let root = std::env::temp_dir()
        .canonicalize()
        .expect("temporary directory should resolve")
        .join(format!(
            "prodex-native-cli-overlay-cleanup-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
    let base_home = root.join("base");
    let shared_home = root.join("shared");
    std::fs::create_dir_all(&base_home).expect("base home should exist");
    std::fs::create_dir_all(&shared_home).expect("shared home should exist");
    let mut args = native_cli_super_args();
    args.profile = None;
    args.cli = Some(SuperCliAgent::Gemini);
    args.provider = Some(SuperExternalProvider::Gemini);
    let strategy = SuperNativeCliLaunchStrategy {
        args,
        presidio_enabled: false,
        agent: SuperCliAgent::Gemini,
        sub_agent: None,
    };
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

    let mut unsupported_args = native_cli_super_args();
    unsupported_args.profile = None;
    unsupported_args.cli = Some(SuperCliAgent::Gemini);
    unsupported_args.provider = Some(SuperExternalProvider::Gemini);
    unsupported_args
        .required_tools
        .push(prodex_optional_tools::OptionalToolId::Rtk);
    let unsupported_strategy = SuperNativeCliLaunchStrategy {
        args: unsupported_args,
        presidio_enabled: false,
        agent: SuperCliAgent::Gemini,
        sub_agent: None,
    };
    let error = unsupported_strategy
        .build_plan(&prepared, None)
        .expect_err("native launch plans must reject Codex-only required tools");
    assert!(error.to_string().contains("--require-tool rtk"));

    let plan = strategy
        .build_plan(&prepared, None)
        .expect("native launch plan should build");

    assert_eq!(plan.child.codex_home, prepared.codex_home);
    assert!(plan.cleanup_paths.is_empty());
    assert!(!prepared.paths.managed_profiles_root.exists());
    std::fs::remove_dir_all(root).expect("test root should be removed");
}
