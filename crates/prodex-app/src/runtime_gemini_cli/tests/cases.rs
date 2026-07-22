#![cfg(test)]

use super::{
    PRODEX_COPILOT_PROXY_API_KEY, RuntimeProxyEndpoint, SUPER_COPILOT_PROVIDER_ID, SuperArgs,
    SuperCliAgent, SuperExternalProvider, SuperNativeCliLaunchStrategy,
    runtime_super_copilot_cli_env, runtime_super_gemini_cli_oauth_env,
    runtime_super_gemini_cli_system_settings_from, runtime_super_native_cli_launch_args,
};
use crate::{GeminiOAuthSecret, RuntimeLaunchStrategy, write_gemini_oauth_secret};
use prodex_cli::CodexRuntimeFeatureArgs;
use std::ffi::OsString;
use std::time::{SystemTime, UNIX_EPOCH};

fn native_cli_super_args() -> SuperArgs {
    SuperArgs {
        profile: Some("kiro-main".to_string()),
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: false,
        dry_run: false,
        base_url: None,
        no_proxy: false,
        presidio: false,
        no_presidio: false,
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
fn native_gemini_cli_uses_profile_oauth_token_env() {
    let home = std::env::temp_dir().join(format!(
        "prodex-native-gemini-cli-oauth-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let secret = GeminiOAuthSecret {
        auth_mode: "gemini_oauth".to_string(),
        access_token: "profile-access-token".to_string(),
        refresh_token: Some("profile-refresh-token".to_string()),
        token_type: Some("Bearer".to_string()),
        scope: None,
        expiry_date: None,
        email: "gemini-user@example.com".to_string(),
        project_id: Some("profile-project".to_string()),
    };
    write_gemini_oauth_secret(&home, &secret).expect("secret should write");

    let env = runtime_super_gemini_cli_oauth_env(&home).expect("env should build");
    assert_eq!(
        env.iter()
            .find(|(key, _)| key == "GOOGLE_CLOUD_ACCESS_TOKEN")
            .map(|(_, value)| value.as_os_str()),
        Some(std::ffi::OsStr::new("profile-access-token"))
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| key == "GOOGLE_CLOUD_PROJECT")
            .map(|(_, value)| value.as_os_str()),
        Some(std::ffi::OsStr::new("profile-project"))
    );

    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn native_gemini_cli_system_override_preserves_policy_and_forces_profile_oauth() {
    let home = std::env::temp_dir().join(format!(
        "prodex-native-gemini-cli-settings-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&home).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let source = home.join("source.json");
    std::fs::write(
        &source,
        r#"{
                // The native CLI accepts comments.
                "security": {"auth": {"selectedType": "gemini-api-key"}},
                "tools": {"exclude": ["run_shell_command"]}
            }"#,
    )
    .unwrap();

    let path = runtime_super_gemini_cli_system_settings_from(&home, Some(&source)).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(
        value.pointer("/security/auth/selectedType"),
        Some(&serde_json::json!("oauth-personal"))
    );
    assert_eq!(
        value.pointer("/tools/exclude/0"),
        Some(&serde_json::json!("run_shell_command"))
    );

    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn native_gemini_cli_rejects_incompatible_enforced_auth_policy() {
    let home = std::env::temp_dir().join(format!(
        "prodex-native-gemini-cli-enforced-auth-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&home).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let source = home.join("source.json");
    std::fs::write(
        &source,
        r#"{"security":{"auth":{"enforcedType":"gemini-api-key"}}}"#,
    )
    .unwrap();

    let error = runtime_super_gemini_cli_system_settings_from(&home, Some(&source))
        .unwrap_err()
        .to_string();
    assert!(error.contains("enforces auth type `gemini-api-key`"));

    let _ = std::fs::remove_dir_all(home);
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
    };
    let request = strategy.runtime_request();
    assert_eq!(request.external_provider, Some("antigravity"));
    assert!(!request.smart_context_enabled);
    assert!(!request.presidio_redaction_enabled);
    assert!(!request.allow_auto_rotate);
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
