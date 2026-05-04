use super::*;

fn profile(name: &str) -> (String, ProfileEntry) {
    (
        name.to_string(),
        ProfileEntry {
            codex_home: PathBuf::from(format!("/tmp/{name}")),
            managed: true,
            email: None,
            provider: ProfileProvider::Openai,
        },
    )
}

fn binding(profile_name: &str, bound_at: i64) -> ResponseProfileBinding {
    ResponseProfileBinding {
        profile_name: profile_name.to_string(),
        bound_at,
    }
}

fn external_profile(name: &str) -> (String, ProfileEntry) {
    let (name, mut profile) = profile(name);
    profile.managed = false;
    (name, profile)
}

#[test]
fn merge_profile_bindings_prefers_newer_known_profile_binding() {
    let profiles = BTreeMap::from([profile("p1"), profile("p2")]);
    let existing = BTreeMap::from([
        ("same".to_string(), binding("p1", 10)),
        ("old_only".to_string(), binding("p1", 20)),
    ]);
    let incoming = BTreeMap::from([
        ("same".to_string(), binding("p2", 30)),
        ("stale".to_string(), binding("missing", 40)),
    ]);

    let merged = merge_profile_bindings(&existing, &incoming, &profiles);

    assert_eq!(merged.get("same"), Some(&binding("p2", 30)));
    assert_eq!(merged.get("old_only"), Some(&binding("p1", 20)));
    assert!(!merged.contains_key("stale"));
}

#[test]
fn duplicate_identity_key_combines_account_and_email_when_both_exist() {
    assert_eq!(
        duplicate_profile_identity_key(Some(" acct "), Some("User@Example.COM")),
        Some("account:acct|email:user@example.com".to_string())
    );
    assert_eq!(
        duplicate_profile_identity_key(Some(" acct "), None),
        Some("account:acct".to_string())
    );
    assert_eq!(
        duplicate_profile_identity_key(None, Some(" User@Example.COM ")),
        Some("email:user@example.com".to_string())
    );
    assert_ne!(
        duplicate_profile_identity_key(Some("acct"), Some("first@example.com")),
        duplicate_profile_identity_key(Some("acct"), Some("second@example.com"))
    );
    assert_eq!(duplicate_profile_identity_key(Some(" "), Some(" ")), None);
}

#[test]
fn canonical_duplicate_profile_prefers_active_recent_external_then_name() {
    let state = AppState {
        active_profile: Some("active".to_string()),
        profiles: BTreeMap::from([
            profile("active"),
            profile("recent"),
            external_profile("external"),
            profile("alpha"),
            profile("beta"),
        ]),
        last_run_selected_at: BTreeMap::from([
            ("active".to_string(), 1),
            ("recent".to_string(), 20),
            ("external".to_string(), 20),
            ("alpha".to_string(), 5),
            ("beta".to_string(), 5),
        ]),
        ..AppState::default()
    };

    assert_eq!(
        select_canonical_duplicate_profile(
            &state,
            &[
                "recent".to_string(),
                "external".to_string(),
                "active".to_string()
            ],
        ),
        Some("active".to_string())
    );
    assert_eq!(
        select_canonical_duplicate_profile(&state, &["recent".to_string(), "external".to_string()],),
        Some("external".to_string())
    );
    assert_eq!(
        select_canonical_duplicate_profile(&state, &["beta".to_string(), "alpha".to_string()]),
        Some("alpha".to_string())
    );
}

#[test]
fn remove_duplicate_profile_from_state_merges_selection_and_remaps_bindings() {
    let mut state = AppState {
        active_profile: Some("duplicate".to_string()),
        profiles: BTreeMap::from([profile("canonical"), profile("duplicate")]),
        last_run_selected_at: BTreeMap::from([
            ("canonical".to_string(), 10),
            ("duplicate".to_string(), 20),
        ]),
        response_profile_bindings: BTreeMap::from([("r".to_string(), binding("duplicate", 1))]),
        session_profile_bindings: BTreeMap::from([("s".to_string(), binding("duplicate", 2))]),
    };

    let removed = remove_duplicate_profile_from_state(&mut state, "duplicate", "canonical");

    assert!(removed.is_some());
    assert_eq!(state.active_profile.as_deref(), Some("canonical"));
    assert!(!state.profiles.contains_key("duplicate"));
    assert_eq!(state.last_run_selected_at.get("canonical"), Some(&20));
    assert_eq!(
        state.response_profile_bindings["r"].profile_name,
        "canonical"
    );
    assert_eq!(
        state.session_profile_bindings["s"].profile_name,
        "canonical"
    );
}

#[test]
fn compact_app_state_prunes_missing_profiles_and_expired_sessions() {
    let now = 120;
    let mut state = AppState {
        active_profile: Some("missing".to_string()),
        profiles: BTreeMap::from([profile("p1")]),
        last_run_selected_at: BTreeMap::from([
            ("p1".to_string(), now),
            ("missing".to_string(), now),
            (
                "old".to_string(),
                now - APP_STATE_LAST_RUN_RETENTION_SECONDS - 1,
            ),
        ]),
        response_profile_bindings: BTreeMap::from([
            ("live".to_string(), binding("p1", 1)),
            ("orphan".to_string(), binding("missing", 1)),
        ]),
        session_profile_bindings: BTreeMap::from([
            ("fresh".to_string(), binding("p1", now)),
            (
                "expired".to_string(),
                binding("p1", now - APP_STATE_SESSION_BINDING_RETENTION_SECONDS - 1),
            ),
        ]),
    };

    state = compact_app_state(state, now);

    assert_eq!(state.active_profile, None);
    assert_eq!(
        state.last_run_selected_at,
        BTreeMap::from([("p1".to_string(), now)])
    );
    assert_eq!(
        state.response_profile_bindings,
        BTreeMap::from([("live".to_string(), binding("p1", 1))])
    );
    assert_eq!(
        state.session_profile_bindings,
        BTreeMap::from([("fresh".to_string(), binding("p1", now))])
    );
}

#[test]
fn merge_runtime_state_snapshot_keeps_existing_profile_set_when_present() {
    let existing = AppState {
        active_profile: Some("p1".to_string()),
        profiles: BTreeMap::from([profile("p1")]),
        last_run_selected_at: BTreeMap::from([("p1".to_string(), 10)]),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let snapshot = AppState {
        active_profile: Some("p2".to_string()),
        profiles: BTreeMap::from([profile("p2")]),
        last_run_selected_at: BTreeMap::from([("p2".to_string(), 20)]),
        response_profile_bindings: BTreeMap::from([("r".to_string(), binding("p2", 20))]),
        session_profile_bindings: BTreeMap::new(),
    };

    let merged = merge_runtime_state_snapshot_with_policy(
        existing,
        &snapshot,
        20,
        AppStateCompactionPolicy::default(),
    );

    assert_eq!(merged.active_profile, None);
    assert!(merged.profiles.contains_key("p1"));
    assert!(!merged.profiles.contains_key("p2"));
    assert_eq!(
        merged.last_run_selected_at,
        BTreeMap::from([("p1".to_string(), 10)])
    );
    assert!(merged.response_profile_bindings.is_empty());
}
