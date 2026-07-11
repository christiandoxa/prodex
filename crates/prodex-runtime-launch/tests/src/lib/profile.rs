use super::*;

#[test]
fn profileless_local_home_requires_no_requested_profile_and_matching_provider() {
    assert!(allow_profileless_local_home(
        None,
        Some("PRODEX-LOCAL"),
        "prodex-local"
    ));
    assert!(!allow_profileless_local_home(
        Some("main"),
        Some("prodex-local"),
        "prodex-local"
    ));
    assert!(!allow_profileless_local_home(
        None,
        Some("openai"),
        "prodex-local"
    ));
    assert!(!allow_profileless_local_home(None, None, "prodex-local"));
}

#[test]
fn resolve_profile_name_prefers_requested_then_active_then_single_profile() {
    let state = prodex_state::AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            ("backup".to_string(), test_profile("/tmp/backup")),
            ("main".to_string(), test_profile("/tmp/main")),
        ]),
        ..prodex_state::AppState::default()
    };

    assert_eq!(
        resolve_profile_name(&state, Some("backup")).expect("requested profile"),
        "backup"
    );
    assert_eq!(
        resolve_profile_name(&state, None).expect("active profile"),
        "main"
    );

    let single = prodex_state::AppState {
        profiles: BTreeMap::from([("only".to_string(), test_profile("/tmp/only"))]),
        ..prodex_state::AppState::default()
    };
    assert_eq!(
        resolve_profile_name(&single, None).expect("single profile"),
        "only"
    );
}

#[test]
fn ensure_profile_path_is_unique_rejects_existing_profile_home() {
    let state = prodex_state::AppState {
        profiles: BTreeMap::from([("main".to_string(), test_profile("/tmp/main"))]),
        ..prodex_state::AppState::default()
    };

    let err = ensure_profile_path_is_unique(&state, Path::new("/tmp/main"))
        .expect_err("duplicate profile path should fail");

    assert!(err.to_string().contains("already used by profile 'main'"));
}
