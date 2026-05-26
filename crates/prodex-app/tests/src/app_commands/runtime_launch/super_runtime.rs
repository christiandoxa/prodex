use super::{
    AppState, BTreeMap, ProfileEntry, ProfileProvider, RUNTIME_PROXY_OPENAI_MOUNT_PATH,
    RuntimeLaunchRequest, TestEnvVarGuard, fs, prepare_runtime_launch,
    prepare_runtime_launch_dry_run, temp_dir, write_state,
};

#[test]
fn prepare_runtime_launch_enables_runtime_proxy_for_openai_smart_context_single_profile() {
    let root = temp_dir("smart-context-single-profile-runtime-proxy");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let main_home = root.join("main-home");
    fs::create_dir_all(&main_home).unwrap();
    fs::write(
        secret_store::auth_json_path(&main_home),
        r#"{"tokens":{"access_token":"main-token"}}"#,
    )
    .unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
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
        smart_context_enabled: true,
        presidio_redaction_enabled: false,
        model_context_window_tokens: Some(65_536),
        force_runtime_proxy: false,
        model_provider_override: None,
        profile_v2_name: None,
        external_provider: None,
        external_provider_api_key: None,
    })
    .unwrap();

    assert_eq!(
        prepared
            .runtime_proxy
            .as_ref()
            .expect("Smart Context should force runtime proxy for OpenAI")
            .openai_mount_path,
        RUNTIME_PROXY_OPENAI_MOUNT_PATH
    );
}

#[test]
fn prepare_runtime_launch_dry_run_previews_proxy_for_presidio_redaction() {
    let root = temp_dir("presidio-dry-run-runtime-proxy");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let main_home = root.join("main-home");
    fs::create_dir_all(&main_home).unwrap();
    fs::write(
        secret_store::auth_json_path(&main_home),
        r#"{"tokens":{"access_token":"main-token"}}"#,
    )
    .unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        },
    );

    let prepared = prepare_runtime_launch_dry_run(RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: true,
        skip_quota_check: true,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: true,
        model_context_window_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: None,
        profile_v2_name: None,
        external_provider: None,
        external_provider_api_key: None,
    })
    .unwrap();

    assert_eq!(
        prepared
            .runtime_proxy
            .as_ref()
            .expect("Presidio should force runtime proxy preview")
            .openai_mount_path,
        RUNTIME_PROXY_OPENAI_MOUNT_PATH
    );
}
