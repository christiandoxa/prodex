use super::*;

#[test]
fn runtime_launch_selection_resolve_chooses_profile_when_none_is_active() {
    let root = temp_dir("resolve-no-active-profile");
    let copilot_home = root.join("copilot");
    let openai_home = root.join("openai-main");
    fs::create_dir_all(&copilot_home).unwrap();
    fs::create_dir_all(&openai_home).unwrap();

    let state = AppState {
        active_profile: None,
        profiles: BTreeMap::from([
            (
                "copilot".to_string(),
                ProfileEntry {
                    codex_home: copilot_home,
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Copilot {
                        host: "https://github.com".to_string(),
                        login: "copilot-user".to_string(),
                        api_url: "https://api.business.githubcopilot.com".to_string(),
                        access_type_sku: None,
                        copilot_plan: None,
                    },
                },
            ),
            (
                "openai-main".to_string(),
                ProfileEntry {
                    codex_home: openai_home,
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        ..AppState::default()
    };

    let selected =
        resolve_runtime_launch_profile_name(&state, None).expect("resolve runtime launch name");
    assert_eq!(selected, "openai-main");
}

#[test]
fn runtime_launch_selection_resolve_chooses_profile_when_active_was_deleted() {
    let root = temp_dir("resolve-deleted-active-profile");
    let openai_home = root.join("openai-main");
    fs::create_dir_all(&openai_home).unwrap();

    let state = AppState {
        active_profile: Some("deleted".to_string()),
        profiles: BTreeMap::from([(
            "openai-main".to_string(),
            ProfileEntry {
                codex_home: openai_home,
                managed: true,
                email: None,
                provider: ProfileProvider::Openai,
            },
        )]),
        ..AppState::default()
    };

    let selected =
        resolve_runtime_launch_profile_name(&state, None).expect("resolve runtime launch name");
    assert_eq!(selected, "openai-main");
}

#[test]
fn prepare_runtime_launch_persists_implicit_selection_when_none_is_active() {
    let root = temp_dir("persist-no-active-profile-selection");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let main_home = root.join("main-home");
    let second_home = root.join("second-home");
    fs::create_dir_all(&main_home).unwrap();
    fs::create_dir_all(&second_home).unwrap();
    write_state(
        &root,
        AppState {
            active_profile: None,
            profiles: BTreeMap::from([
                (
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home.clone(),
                        managed: false,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
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
        profile: None,
        allow_auto_rotate: true,
        skip_quota_check: true,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
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

    assert_eq!(prepared.codex_home, main_home);
    let paths = AppPaths::discover().unwrap();
    let state = AppState::load(&paths).unwrap();
    assert_eq!(state.active_profile.as_deref(), Some("main"));
    assert!(state.last_run_selected_at.contains_key("main"));
}

#[test]
fn prepare_runtime_launch_replaces_deleted_active_profile_selection() {
    let root = temp_dir("persist-deleted-active-profile-selection");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let main_home = root.join("main-home");
    fs::create_dir_all(&main_home).unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("deleted".to_string()),
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home.clone(),
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        },
    );

    let prepared = prepare_runtime_launch(RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: true,
        skip_quota_check: true,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
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

    assert_eq!(prepared.codex_home, main_home);
    let paths = AppPaths::discover().unwrap();
    let state = AppState::load(&paths).unwrap();
    assert_eq!(state.active_profile.as_deref(), Some("main"));
    assert!(state.last_run_selected_at.contains_key("main"));
}
