use super::*;

#[test]
fn app_state_schema_migrates_legacy_and_rejects_future_versions() {
    let legacy: AppState = serde_json::from_str(r#"{"active_profile":null}"#).unwrap();
    assert_eq!(
        serde_json::to_value(&legacy).unwrap()["schema_version"],
        APP_STATE_SCHEMA_VERSION
    );

    let future = format!(
        r#"{{"schema_version":{},"active_profile":null}}"#,
        APP_STATE_SCHEMA_VERSION + 1
    );
    assert!(serde_json::from_str::<AppState>(&future).is_err());
}

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
fn provider_capabilities_define_route_policy_and_quota_shape() {
    let openai = ProfileProvider::Openai.capabilities();
    assert_eq!(openai.runtime_route_policy, RuntimeRoutePolicy::NativeCodex);
    assert_eq!(openai.quota_shape, ProviderQuotaShape::OpenAiWindows);
    assert!(openai.uses_openai_client_format);
    assert!(openai.supports_runtime_rotation);
    assert!(ProfileProvider::Openai.supports_codex_runtime());

    let gemini = ProfileProvider::Gemini {
        email: "gemini@example.com".to_string(),
        project_id: None,
    }
    .capabilities();
    assert_eq!(
        gemini.runtime_route_policy,
        RuntimeRoutePolicy::ResponsesAdapter
    );
    assert_eq!(gemini.quota_shape, ProviderQuotaShape::GeminiBuckets);
    assert!(gemini.uses_openai_client_format);
    assert!(!gemini.supports_runtime_rotation);

    let copilot = ProfileProvider::Copilot {
        host: "github.com".to_string(),
        login: "octo".to_string(),
        api_url: "https://api.githubcopilot.com".to_string(),
        access_type_sku: None,
        copilot_plan: None,
    }
    .capabilities();
    assert_eq!(copilot.quota_shape, ProviderQuotaShape::CopilotMonthly);

    let kiro = ProfileProvider::Kiro {
        auth_key: "kiro:key".to_string(),
        auth_kind: None,
        profile_arn: None,
        profile_name: None,
        start_url: None,
        region: None,
    }
    .capabilities();
    assert_eq!(
        kiro.runtime_route_policy,
        RuntimeRoutePolicy::ResponsesAdapter
    );
    assert_eq!(kiro.quota_shape, ProviderQuotaShape::ExternalStatus);
    assert!(kiro.uses_openai_client_format);
    assert!(!kiro.supports_runtime_rotation);

    let agy = ProfileProvider::Agy { account: None }.capabilities();
    assert_eq!(agy.runtime_route_policy, RuntimeRoutePolicy::ExternalCli);
    assert!(!agy.uses_openai_client_format);
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
fn timestamp_ties_merge_commutatively() {
    let profiles = BTreeMap::from([profile("p1"), profile("p2")]);
    let left_bindings = BTreeMap::from([("same".to_string(), binding("p1", 10))]);
    let right_bindings = BTreeMap::from([("same".to_string(), binding("p2", 10))]);
    assert_eq!(
        merge_profile_bindings(&left_bindings, &right_bindings, &profiles),
        merge_profile_bindings(&right_bindings, &left_bindings, &profiles)
    );

    let left_policies = BTreeMap::from([(
        "p1".to_string(),
        ProfileGovernancePolicy {
            weight: 100,
            updated_at: 10,
            ..ProfileGovernancePolicy::default()
        },
    )]);
    let right_policies = BTreeMap::from([(
        "p1".to_string(),
        ProfileGovernancePolicy {
            weight: 200,
            updated_at: 10,
            ..ProfileGovernancePolicy::default()
        },
    )]);
    assert_eq!(
        merge_profile_governance_policies(&left_policies, &right_policies, &profiles),
        merge_profile_governance_policies(&right_policies, &left_policies, &profiles)
    );
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

    assert_eq!(merged.active_profile.as_deref(), Some("p1"));
    assert!(merged.profiles.contains_key("p1"));
    assert!(!merged.profiles.contains_key("p2"));
    assert_eq!(
        merged.last_run_selected_at,
        BTreeMap::from([("p1".to_string(), 10)])
    );
    assert!(merged.response_profile_bindings.is_empty());
}

#[test]
fn merge_app_state_for_save_preserves_concurrent_profile_additions() {
    let existing = AppState {
        active_profile: Some("first".to_string()),
        profiles: BTreeMap::from([profile("first")]),
        ..AppState::default()
    };
    let desired = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([profile("second")]),
        ..AppState::default()
    };

    let merged = merge_app_state_for_save_with_policy(
        existing,
        &desired,
        1,
        AppStateCompactionPolicy::default(),
    );

    assert_eq!(merged.active_profile.as_deref(), Some("second"));
    assert_eq!(merged.profiles.len(), 2);
    assert!(merged.profiles.contains_key("first"));
    assert!(merged.profiles.contains_key("second"));
}

#[test]
fn merge_app_state_for_save_does_not_restore_stale_active_profile() {
    let profiles = BTreeMap::from([profile("first"), profile("second")]);
    let existing = AppState {
        active_profile: Some("second".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::from([
            ("first".to_string(), 10),
            ("second".to_string(), 20),
        ]),
        ..AppState::default()
    };
    let stale = AppState {
        active_profile: Some("first".to_string()),
        profiles,
        last_run_selected_at: BTreeMap::from([("first".to_string(), 10)]),
        ..AppState::default()
    };

    let merged = merge_app_state_for_save_with_policy(
        existing,
        &stale,
        20,
        AppStateCompactionPolicy::default(),
    );

    assert_eq!(merged.active_profile.as_deref(), Some("second"));
}

#[test]
fn profile_governance_policy_normalizes_tags_weight_and_note() {
    let policy = normalize_profile_governance_policy(ProfileGovernancePolicy {
        tags: vec![
            " Prod ".to_string(),
            "PROD".to_string(),
            "bad tag!".to_string(),
            "region.us-east".to_string(),
        ],
        weight: PROFILE_GOVERNANCE_MAX_WEIGHT + 1,
        paused: false,
        drained: false,
        note: Some(format!(
            "  {}  ",
            "x".repeat(PROFILE_GOVERNANCE_MAX_NOTE_LENGTH + 20)
        )),
        updated_at: 10,
    });

    assert_eq!(
        policy.tags,
        vec![
            "prod".to_string(),
            "badtag".to_string(),
            "region.us-east".to_string()
        ]
    );
    assert_eq!(policy.weight, PROFILE_GOVERNANCE_MAX_WEIGHT);
    assert_eq!(
        policy.note.as_deref().map(str::len),
        Some(PROFILE_GOVERNANCE_MAX_NOTE_LENGTH)
    );
}

#[test]
fn mutate_profile_governance_policy_sets_updated_at_and_defaults() {
    let mut policies = BTreeMap::new();

    mutate_profile_governance_policy(&mut policies, "main", 42, |policy| {
        policy.tags = vec!["Hot Path".to_string()];
        policy.weight = -5;
        policy.paused = true;
    });

    let policy = policies.get("main").expect("policy inserted");
    assert_eq!(policy.updated_at, 42);
    assert_eq!(policy.tags, vec!["hotpath".to_string()]);
    assert_eq!(policy.weight, PROFILE_GOVERNANCE_MIN_WEIGHT);
    assert!(policy.paused);
}

#[test]
fn profile_selection_eligibility_uses_pause_drain_and_missing_profile() {
    let profiles = BTreeMap::from([profile("ready"), profile("paused"), profile("drained")]);
    let policies = BTreeMap::from([
        (
            "paused".to_string(),
            ProfileGovernancePolicy {
                paused: true,
                ..ProfileGovernancePolicy::default()
            },
        ),
        (
            "drained".to_string(),
            ProfileGovernancePolicy {
                drained: true,
                ..ProfileGovernancePolicy::default()
            },
        ),
    ]);

    assert_eq!(
        profile_selection_eligibility_reason(&profiles, &policies, "ready"),
        ProfileSelectionEligibilityReason::Eligible
    );
    assert_eq!(
        profile_selection_eligibility_reason(&profiles, &policies, "paused").label(),
        "paused"
    );
    assert_eq!(
        profile_selection_eligibility_reason(&profiles, &policies, "drained").label(),
        "drained"
    );
    assert_eq!(
        profile_selection_eligibility_reason(&profiles, &policies, "missing").label(),
        "missing_profile"
    );
}

#[test]
fn profile_governance_prune_and_merge_keep_known_newer_policy() {
    let profiles = BTreeMap::from([profile("main"), profile("second")]);
    let existing = BTreeMap::from([
        (
            "main".to_string(),
            ProfileGovernancePolicy {
                weight: 100,
                updated_at: 10,
                ..ProfileGovernancePolicy::default()
            },
        ),
        (
            "orphan".to_string(),
            ProfileGovernancePolicy {
                updated_at: 99,
                ..ProfileGovernancePolicy::default()
            },
        ),
    ]);
    let incoming = BTreeMap::from([
        (
            "main".to_string(),
            ProfileGovernancePolicy {
                weight: 200,
                updated_at: 5,
                ..ProfileGovernancePolicy::default()
            },
        ),
        (
            "second".to_string(),
            ProfileGovernancePolicy {
                weight: 300,
                updated_at: 20,
                ..ProfileGovernancePolicy::default()
            },
        ),
    ]);

    let merged = merge_profile_governance_policies(&existing, &incoming, &profiles);

    assert_eq!(merged["main"].weight, 100);
    assert_eq!(merged["second"].weight, 300);
    assert!(!merged.contains_key("orphan"));
}
