use super::*;

#[test]
fn prepare_runtime_launch_enables_deepseek_rewrite_proxy_for_super_provider() {
    let root = temp_dir("profileless-deepseek-smart-context-proxy");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_root = root.join("shared-codex");
    let _shared = TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", shared_root.to_str().unwrap());
    let paths = AppPaths::discover().unwrap();

    let prepared = prepare_runtime_launch(RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: true,
        skip_quota_check: true,
        base_url: Some("https://api.deepseek.com"),
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: true,
        presidio_redaction_enabled: false,
        model_context_window_tokens: Some(1_048_576),
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: Some(SUPER_DEEPSEEK_PROVIDER_ID),
        profile_v2_name: None,
        external_provider: Some("deepseek"),
        external_provider_api_key: Some("test-deepseek-key"),
    })
    .unwrap();

    assert_eq!(prepared.codex_home, paths.shared_codex_root);
    assert!(prepared.codex_home.is_dir());
    assert!(!prepared.managed);
    let runtime_proxy = prepared
        .runtime_proxy
        .as_ref()
        .expect("DeepSeek provider should use local rewrite proxy");
    assert_eq!(
        runtime_proxy.local_model_provider_id.as_deref(),
        Some(SUPER_DEEPSEEK_PROVIDER_ID)
    );
    assert_eq!(
        runtime_proxy.openai_mount_path,
        RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH
    );
    assert!(
        !paths.state_file.exists(),
        "profileless DeepSeek proxy launch should not persist synthetic profile selection"
    );
}

#[test]
fn prepare_runtime_launch_enables_gemini_rewrite_proxy_for_api_key_provider() {
    let root = temp_dir("profileless-gemini-smart-context-proxy");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_root = root.join("shared-codex");
    let _shared = TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", shared_root.to_str().unwrap());
    let paths = AppPaths::discover().unwrap();

    let prepared = prepare_runtime_launch(RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: true,
        skip_quota_check: true,
        base_url: Some("https://generativelanguage.googleapis.com/v1beta"),
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: true,
        presidio_redaction_enabled: false,
        model_context_window_tokens: Some(1_048_576),
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: Some(SUPER_GEMINI_PROVIDER_ID),
        profile_v2_name: None,
        external_provider: Some("gemini"),
        external_provider_api_key: Some("test-gemini-key"),
    })
    .unwrap();

    assert_eq!(prepared.codex_home, paths.shared_codex_root);
    assert!(prepared.codex_home.is_dir());
    assert!(!prepared.managed);
    let runtime_proxy = prepared
        .runtime_proxy
        .as_ref()
        .expect("Gemini provider should use local rewrite proxy");
    assert_eq!(
        runtime_proxy.local_model_provider_id.as_deref(),
        Some(SUPER_GEMINI_PROVIDER_ID)
    );
    assert_eq!(
        runtime_proxy.openai_mount_path,
        RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH
    );
    assert!(
        !paths.state_file.exists(),
        "profileless Gemini proxy launch should not persist synthetic profile selection"
    );
}

#[test]
fn prepare_runtime_launch_uses_gemini_oauth_profile_without_api_key() {
    let root = temp_dir("gemini-oauth-smart-context-proxy");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let gemini_home = root.join("gemini-home");
    fs::create_dir_all(&gemini_home).unwrap();
    fs::write(
        gemini_home.join("gemini_oauth.json"),
        r#"{
            "auth_mode": "gemini_oauth",
            "access_token": "google-access-token",
            "refresh_token": "google-refresh-token",
            "expiry_date": 4102444800000,
            "email": "dev@example.com",
            "project_id": "dev-project"
        }"#,
    )
    .unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("gemini-dev".to_string()),
            profiles: BTreeMap::from([(
                "gemini-dev".to_string(),
                ProfileEntry {
                    codex_home: gemini_home.clone(),
                    managed: false,
                    email: Some("dev@example.com".to_string()),
                    provider: ProfileProvider::Gemini {
                        email: "dev@example.com".to_string(),
                        project_id: Some("dev-project".to_string()),
                    },
                },
            )]),
            ..AppState::default()
        },
    );

    let prepared = prepare_runtime_launch(RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: true,
        skip_quota_check: true,
        base_url: Some("https://generativelanguage.googleapis.com/v1beta"),
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: true,
        presidio_redaction_enabled: false,
        model_context_window_tokens: Some(1_048_576),
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: Some(SUPER_GEMINI_PROVIDER_ID),
        profile_v2_name: None,
        external_provider: Some("gemini"),
        external_provider_api_key: None,
    })
    .unwrap();

    assert_eq!(prepared.codex_home, gemini_home);
    let runtime_proxy = prepared
        .runtime_proxy
        .as_ref()
        .expect("Gemini OAuth profile should use local rewrite proxy");
    assert_eq!(
        runtime_proxy.local_model_provider_id.as_deref(),
        Some(SUPER_GEMINI_PROVIDER_ID)
    );
}
