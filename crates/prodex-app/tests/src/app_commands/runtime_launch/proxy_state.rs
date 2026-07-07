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
