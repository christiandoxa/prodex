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
    record_run_selection_at(&mut state, "main", 25);

    assert_eq!(
        state.last_run_selected_at,
        BTreeMap::from([("main".to_string(), 31)])
    );
}

#[test]
fn later_run_selection_advances_past_another_profile() {
    let mut state = prodex_state::AppState {
        profiles: BTreeMap::from([
            ("first".to_string(), test_profile("/tmp/first")),
            ("second".to_string(), test_profile("/tmp/second")),
        ]),
        ..prodex_state::AppState::default()
    };
    prodex_state::activate_profile(&mut state, "first");
    let explicit_at = state.last_run_selected_at["first"];

    record_run_selection_at(&mut state, "second", explicit_at.saturating_sub(10));

    assert!(state.last_run_selected_at["second"] > explicit_at);
}

#[test]
fn record_run_selection_preserves_negative_epochs_and_saturation() {
    let mut state = prodex_state::AppState {
        profiles: BTreeMap::from([
            ("first".to_string(), test_profile("/tmp/first")),
            ("second".to_string(), test_profile("/tmp/second")),
        ]),
        last_run_selected_at: BTreeMap::from([("first".to_string(), i64::MIN)]),
        ..prodex_state::AppState::default()
    };
    record_run_selection_at(&mut state, "second", -50);
    assert_eq!(state.last_run_selected_at["second"], 1);
    state
        .last_run_selected_at
        .insert("first".to_string(), i64::MAX);
    record_run_selection_at(&mut state, "second", 42);
    assert_eq!(state.last_run_selected_at["second"], i64::MAX);
    record_run_selection_at(&mut state, "second", i64::MIN);
    assert_eq!(state.last_run_selected_at["second"], i64::MAX);
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
                    binding_identity: None,
                    profile_name: "main".to_string(),
                    bound_at: 1_000,
                },
            ),
            (
                "resp-second".to_string(),
                prodex_state::ResponseProfileBinding {
                    binding_identity: None,
                    profile_name: "second".to_string(),
                    bound_at: 2_000,
                },
            ),
        ]),
        session_profile_bindings: BTreeMap::from([
            (
                "session-main".to_string(),
                prodex_state::ResponseProfileBinding {
                    binding_identity: None,
                    profile_name: "main".to_string(),
                    bound_at: 1_000,
                },
            ),
            (
                "session-second".to_string(),
                prodex_state::ResponseProfileBinding {
                    binding_identity: None,
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

#[test]
fn fixed_proxy_state_uses_compiled_exact_name_filter_across_maps() {
    let selected = "雪";
    let state = prodex_state::AppState {
        profiles: BTreeMap::from([
            (selected.to_string(), test_profile("/tmp/unicode")),
            ("雪-extra".to_string(), test_profile("/tmp/unicode-extra")),
            ("SNOW".to_string(), test_profile("/tmp/snow")),
        ]),
        last_run_selected_at: BTreeMap::from([
            (selected.to_string(), 4),
            ("雪-extra".to_string(), 5),
        ]),
        response_profile_bindings: BTreeMap::from([
            (
                "selected".to_string(),
                prodex_state::ResponseProfileBinding {
                    binding_identity: None,
                    profile_name: selected.to_string(),
                    bound_at: 4,
                },
            ),
            (
                "other".to_string(),
                prodex_state::ResponseProfileBinding {
                    binding_identity: None,
                    profile_name: "雪-extra".to_string(),
                    bound_at: 5,
                },
            ),
        ]),
        session_profile_bindings: BTreeMap::from([
            (
                "selected".to_string(),
                prodex_state::ResponseProfileBinding {
                    binding_identity: None,
                    profile_name: selected.to_string(),
                    bound_at: 4,
                },
            ),
            (
                "other".to_string(),
                prodex_state::ResponseProfileBinding {
                    binding_identity: None,
                    profile_name: "雪-extra".to_string(),
                    bound_at: 5,
                },
            ),
        ]),
        ..prodex_state::AppState::default()
    };
    let selected_state = fixed_runtime_proxy_state(&state, selected).expect("selected state");
    assert_eq!(selected_state.active_profile.as_deref(), Some(selected));
    assert_eq!(
        selected_state.profiles.keys().collect::<Vec<_>>(),
        vec![selected]
    );
    assert_eq!(
        selected_state
            .last_run_selected_at
            .keys()
            .collect::<Vec<_>>(),
        vec![selected]
    );
    assert_eq!(
        selected_state
            .response_profile_bindings
            .keys()
            .collect::<Vec<_>>(),
        vec!["selected"]
    );
    assert_eq!(
        selected_state
            .session_profile_bindings
            .keys()
            .collect::<Vec<_>>(),
        vec!["selected"]
    );
    // The original state is not mutated by the fixed snapshot projection.
    assert_eq!(state.profiles.len(), 3);
    assert!(fixed_runtime_proxy_state(&state, "missing").is_err());
}
