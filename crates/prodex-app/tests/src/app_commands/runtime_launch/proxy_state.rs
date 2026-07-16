use super::*;

#[test]
fn runtime_proxy_endpoint_state_honors_no_auto_rotate() {
    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/main"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/second"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        response_profile_bindings: BTreeMap::from([
            (
                "resp-main".to_string(),
                prodex_state::ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: 1,
                },
            ),
            (
                "resp-second".to_string(),
                prodex_state::ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: 2,
                },
            ),
        ]),
        ..AppState::default()
    };
    let selection = RuntimeLaunchSelection {
        initial_profile_name: "main".to_string(),
        selected_profile_name: "main".to_string(),
        codex_home: PathBuf::from("/tmp/main"),
        explicit_profile_requested: true,
        non_openai_model_provider: None,
        profileless_local_home: false,
    };

    let proxy_state = runtime_proxy_endpoint_state(&state, &selection, true).unwrap();

    assert_eq!(proxy_state.active_profile.as_deref(), Some("main"));
    assert_eq!(
        proxy_state
            .profiles
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["main"]
    );
    assert_eq!(
        proxy_state
            .response_profile_bindings
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["resp-main"]
    );
}

#[test]
fn smart_context_window_uses_active_model_cache_metadata() {
    let root = temp_dir("smart-context-window-active-model-cache");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("config.toml"), "model = 'gpt-5.5'\n").unwrap();
    fs::write(
        root.join("models_cache.json"),
        r#"{"models":[{"slug":"gpt-5.5","context_window":272000,"max_context_window":272000}]}"#,
    )
    .unwrap();
    let selection = RuntimeLaunchSelection {
        initial_profile_name: "main".to_string(),
        selected_profile_name: "main".to_string(),
        codex_home: root,
        explicit_profile_requested: true,
        non_openai_model_provider: None,
        profileless_local_home: false,
    };
    let request = RuntimeLaunchRequest {
        profile: Some("main"),
        allow_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: false,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: true,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: None,
        profile_v2_name: None,
        external_provider: None,
        external_provider_api_key: None,
    };

    assert_eq!(
        runtime_launch_effective_model_context_window_tokens(&request, &selection.codex_home)
            .unwrap(),
        Some(272_000)
    );
}

#[test]
fn smart_context_window_keeps_explicit_context_override() {
    let root = temp_dir("smart-context-window-explicit-override");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("config.toml"), "model = 'gpt-5.5'\n").unwrap();
    fs::write(
        root.join("models_cache.json"),
        r#"{"models":[{"slug":"gpt-5.5","context_window":272000}]}"#,
    )
    .unwrap();
    let selection = RuntimeLaunchSelection {
        initial_profile_name: "main".to_string(),
        selected_profile_name: "main".to_string(),
        codex_home: root,
        explicit_profile_requested: true,
        non_openai_model_provider: None,
        profileless_local_home: false,
    };
    let request = RuntimeLaunchRequest {
        profile: Some("main"),
        allow_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: false,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: true,
        presidio_redaction_enabled: false,
        model_context_window_tokens: Some(123_456),
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: None,
        profile_v2_name: None,
        external_provider: None,
        external_provider_api_key: None,
    };

    assert_eq!(
        runtime_launch_effective_model_context_window_tokens(&request, &selection.codex_home)
            .unwrap(),
        Some(123_456)
    );
}
