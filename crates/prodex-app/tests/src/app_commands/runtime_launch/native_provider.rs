use super::*;

#[test]
fn prepare_runtime_launch_skips_proxy_for_non_openai_model_provider() {
    for provider in ["amazon-bedrock", "gcp"] {
    let root = temp_dir("skip-proxy-non-openai");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let bedrock_home = root.join("bedrock-home");
    let openai_home = root.join("openai-home");
    fs::create_dir_all(&bedrock_home).unwrap();
    fs::create_dir_all(&openai_home).unwrap();
    fs::write(
        bedrock_home.join("config.toml"),
        format!("model_provider = '{provider}'\n"),
    )
    .unwrap();
    write_runtime_launch_auth(
        secret_store::auth_json_path(&openai_home),
        r#"{"tokens":{"access_token":"chatgpt-token"}}"#,
    )
    .unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("bedrock".to_string()),
            profiles: BTreeMap::from([
                (
                    "bedrock".to_string(),
                    ProfileEntry {
                        codex_home: bedrock_home.clone(),
                        managed: false,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                ),
                (
                    "openai".to_string(),
                    ProfileEntry {
                        codex_home: openai_home,
                        managed: false,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                ),
            ]),
            ..AppState::default()
        },
    );
    let prepared = prepare_runtime_launch(RuntimeLaunchRequest {
        profile: Some("bedrock"),
        allow_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: false,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        requested_model: None,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: None,
        profile_v2_name: None,
        external_provider: None,
        external_provider_api_key: None,
    })
    .unwrap();
    assert_eq!(prepared.codex_home, bedrock_home);
    assert!(prepared.runtime_proxy.is_none());    }
}

#[test]
fn prepare_runtime_launch_rejects_claude_for_non_openai_model_provider() {
    let root = temp_dir("reject-claude-non-openai");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let bedrock_home = root.join("bedrock-home");
    fs::create_dir_all(&bedrock_home).unwrap();
    fs::write(
        bedrock_home.join("config.toml"),
        "model_provider = 'amazon-bedrock'\n",
    )
    .unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("bedrock".to_string()),
            profiles: BTreeMap::from([(
                "bedrock".to_string(),
                ProfileEntry {
                    codex_home: bedrock_home,
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        },
    );
    let err = match prepare_runtime_launch(RuntimeLaunchRequest {
        profile: Some("bedrock"),
        allow_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: false,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        requested_model: None,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: true,
        model_provider_override: None,
        profile_v2_name: None,
        external_provider: None,
        external_provider_api_key: None,
    }) {
        Ok(_) => panic!("expected Claude launch to reject non-OpenAI model providers"),
        Err(err) => err,
    };
    let message = format!("{err:#}");
    assert!(message.contains("amazon-bedrock"));
    assert!(message.contains("prodex claude"));
}
