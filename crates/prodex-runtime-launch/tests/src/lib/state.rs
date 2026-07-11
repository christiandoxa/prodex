use super::*;

#[test]
fn record_run_selection_prunes_missing_profiles_before_insert() {
    let mut state = prodex_state::AppState {
        profiles: BTreeMap::from([("main".to_string(), test_profile("/tmp/main"))]),
        last_run_selected_at: BTreeMap::from([
            ("deleted".to_string(), 10),
            ("main".to_string(), 20),
        ]),
        ..prodex_state::AppState::default()
    };

    record_run_selection_at(&mut state, "main", 30);

    assert_eq!(
        state.last_run_selected_at,
        BTreeMap::from([("main".to_string(), 30)])
    );
}

#[test]
fn fixed_runtime_proxy_state_keeps_only_selected_profile() {
    let state = prodex_state::AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([
            ("main".to_string(), test_profile("/tmp/main-home")),
            ("second".to_string(), test_profile("/tmp/second-home")),
        ]),
        last_run_selected_at: BTreeMap::from([
            ("main".to_string(), 1_000),
            ("second".to_string(), 2_000),
        ]),
        response_profile_bindings: BTreeMap::from([
            (
                "resp-main".to_string(),
                prodex_state::ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: 1_000,
                },
            ),
            (
                "resp-second".to_string(),
                prodex_state::ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: 2_000,
                },
            ),
        ]),
        session_profile_bindings: BTreeMap::from([
            (
                "session-main".to_string(),
                prodex_state::ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: 1_000,
                },
            ),
            (
                "session-second".to_string(),
                prodex_state::ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: 2_000,
                },
            ),
        ]),
    };

    let fixed = fixed_runtime_proxy_state(&state, "main").unwrap();

    assert_eq!(fixed.active_profile.as_deref(), Some("main"));
    assert_eq!(
        fixed
            .profiles
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["main"]
    );
    assert_eq!(
        fixed
            .last_run_selected_at
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["main"]
    );
    assert_eq!(
        fixed
            .response_profile_bindings
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["resp-main"]
    );
    assert_eq!(
        fixed
            .session_profile_bindings
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["session-main"]
    );
}
