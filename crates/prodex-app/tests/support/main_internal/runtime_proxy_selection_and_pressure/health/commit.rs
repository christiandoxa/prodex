use super::*;

#[derive(Clone, Copy)]
struct CommitProfileFixture {
    name: &'static str,
    account_id: &'static str,
    email: &'static str,
}

const MAIN_PROFILE: CommitProfileFixture = CommitProfileFixture {
    name: "main",
    account_id: "main-account",
    email: "main@example.com",
};

const SECOND_PROFILE: CommitProfileFixture = CommitProfileFixture {
    name: "second",
    account_id: "second-account",
    email: "second@example.com",
};

struct CommitRuntimeFixture {
    _temp_dir: TestDir,
    shared: RuntimeRotationProxyShared,
}

impl CommitRuntimeFixture {
    fn shared(&self) -> &RuntimeRotationProxyShared {
        &self.shared
    }
}

struct CommitRuntimeOptions {
    current_profile: &'static str,
    profile_route_circuit_open_until: BTreeMap<String, i64>,
    profile_health: BTreeMap<String, RuntimeProfileHealth>,
}

impl Default for CommitRuntimeOptions {
    fn default() -> Self {
        Self {
            current_profile: "main",
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        }
    }
}

fn commit_runtime_fixture(profiles: &[CommitProfileFixture]) -> CommitRuntimeFixture {
    commit_runtime_fixture_with_options(profiles, CommitRuntimeOptions::default())
}

fn commit_runtime_fixture_with_options(
    profiles: &[CommitProfileFixture],
    options: CommitRuntimeOptions,
) -> CommitRuntimeFixture {
    let temp_dir = TestDir::isolated();

    let state_profiles = profiles
        .iter()
        .map(|profile| {
            let codex_home = temp_dir.path.join(format!("homes/{}", profile.name));
            write_auth_json(&codex_home.join("auth.json"), profile.account_id);
            (
                profile.name.to_string(),
                ProfileEntry {
                    codex_home,
                    managed: true,
                    email: Some(profile.email.to_string()),
                    provider: ProfileProvider::Openai,
                },
            )
        })
        .collect();

    let runtime = RuntimeRotationState {
        paths: AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        },
        state: AppState {
            active_profile: Some("main".to_string()),
            profiles: state_profiles,
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
            session_profile_bindings: BTreeMap::new(),
        },
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: options.current_profile.to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: options.profile_route_circuit_open_until,
        profile_inflight: BTreeMap::new(),
        profile_health: options.profile_health,
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    CommitRuntimeFixture {
        _temp_dir: temp_dir,
        shared,
    }
}

fn runtime_health(
    entries: impl IntoIterator<Item = (String, u32, i64)>,
) -> BTreeMap<String, RuntimeProfileHealth> {
    entries
        .into_iter()
        .map(|(key, score, updated_at)| (key, RuntimeProfileHealth { score, updated_at }))
        .collect()
}

#[test]
fn commit_runtime_proxy_profile_selection_skips_persist_when_nothing_changed() {
    let fixture = commit_runtime_fixture(&[MAIN_PROFILE]);
    let shared = fixture.shared();

    commit_runtime_proxy_profile_selection(shared, "main", RuntimeRouteKind::Responses)
        .expect("profile commit should succeed");

    assert_eq!(
        shared.state_save_revision.load(Ordering::SeqCst),
        0,
        "unchanged commit should not enqueue a state save"
    );
}

#[test]
fn commit_runtime_proxy_profile_selection_can_skip_current_profile_tracking() {
    let fixture = commit_runtime_fixture(&[MAIN_PROFILE, SECOND_PROFILE]);
    let shared = fixture.shared();

    let switched = commit_runtime_proxy_profile_selection_with_policy(
        shared,
        "second",
        RuntimeRouteKind::Websocket,
        false,
    )
    .expect("profile commit should succeed");

    assert!(
        !switched,
        "tracked current profile should stay on the heuristic profile"
    );
    assert_eq!(shared.state_save_revision.load(Ordering::SeqCst), 0);
    let runtime = shared.runtime.lock().expect("runtime should lock");
    assert_eq!(runtime.current_profile, "main");
    assert_eq!(runtime.state.active_profile.as_deref(), Some("main"));
    assert!(
        runtime.state.last_run_selected_at.is_empty(),
        "continuation commit should not promote the global active profile"
    );
}

#[test]
fn commit_runtime_proxy_profile_selection_clears_matching_route_bad_pairing() {
    let now = Local::now().timestamp();
    let websocket_bad_pairing_key =
        runtime_profile_route_bad_pairing_key("main", RuntimeRouteKind::Websocket);
    let compact_bad_pairing_key =
        runtime_profile_route_bad_pairing_key("main", RuntimeRouteKind::Compact);
    let fixture = commit_runtime_fixture_with_options(
        &[MAIN_PROFILE],
        CommitRuntimeOptions {
            profile_health: runtime_health([
                (
                    websocket_bad_pairing_key.clone(),
                    RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
                    now,
                ),
                (
                    compact_bad_pairing_key.clone(),
                    RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
                    now,
                ),
            ]),
            ..CommitRuntimeOptions::default()
        },
    );
    let shared = fixture.shared();

    commit_runtime_proxy_profile_selection(shared, "main", RuntimeRouteKind::Websocket)
        .expect("profile commit should succeed");

    let runtime = shared.runtime.lock().expect("runtime should lock");
    for (key, should_exist, message) in [
        (
            &websocket_bad_pairing_key,
            false,
            "successful commit should clear bad pairing memory for the successful route",
        ),
        (
            &compact_bad_pairing_key,
            true,
            "successful commit should keep unrelated route bad pairing memory intact",
        ),
    ] {
        assert_eq!(
            runtime.profile_health.contains_key(key),
            should_exist,
            "{message}"
        );
    }
}

#[test]
fn commit_runtime_proxy_profile_selection_clears_profile_health() {
    let now = Local::now().timestamp();
    let profile_key = "main".to_string();
    let route_circuit_key = runtime_profile_route_circuit_key("main", RuntimeRouteKind::Responses);
    let route_health_key = runtime_profile_route_health_key("main", RuntimeRouteKind::Responses);
    let route_circuit_reopen_key =
        runtime_profile_route_circuit_reopen_key("main", RuntimeRouteKind::Responses);
    let fixture = commit_runtime_fixture_with_options(
        &[MAIN_PROFILE],
        CommitRuntimeOptions {
            profile_route_circuit_open_until: BTreeMap::from([(
                route_circuit_key.clone(),
                now + RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS,
            )]),
            profile_health: runtime_health([
                (
                    profile_key.clone(),
                    RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY,
                    now,
                ),
                (route_health_key.clone(), 1, now),
                (route_circuit_reopen_key.clone(), 2, now),
            ]),
            ..CommitRuntimeOptions::default()
        },
    );
    let shared = fixture.shared();

    commit_runtime_proxy_profile_selection(shared, "main", RuntimeRouteKind::Responses)
        .expect("profile commit should succeed");

    let runtime = shared.runtime.lock().expect("runtime should lock");
    assert!(
        !runtime
            .profile_route_circuit_open_until
            .contains_key(&route_circuit_key),
        "successful commit should clear the matching route circuit"
    );
    for (key, message) in [
        (
            &profile_key,
            "successful commit should clear temporary health penalty",
        ),
        (
            &route_health_key,
            "successful commit should clear the matching route health penalty",
        ),
        (
            &route_circuit_reopen_key,
            "successful commit should clear the matching route circuit reopen stage",
        ),
    ] {
        assert!(!runtime.profile_health.contains_key(key), "{message}");
    }
}

#[test]
fn commit_runtime_proxy_profile_selection_switches_runtime_but_not_global_profile_for_compact() {
    let fixture = commit_runtime_fixture(&[MAIN_PROFILE, SECOND_PROFILE]);
    let shared = fixture.shared();

    let switched =
        commit_runtime_proxy_profile_selection(shared, "second", RuntimeRouteKind::Compact)
            .expect("compact profile commit should succeed");

    assert!(
        switched,
        "compact commit should switch the runtime current profile"
    );
    let runtime = shared.runtime.lock().expect("runtime should lock");
    assert_eq!(runtime.current_profile, "second");
    assert_eq!(runtime.state.active_profile.as_deref(), Some("main"));
}

#[test]
fn commit_runtime_proxy_profile_selection_recovers_only_matching_route_profile_health() {
    let now = Local::now().timestamp();
    let websocket_health_key =
        runtime_profile_route_health_key("main", RuntimeRouteKind::Websocket);
    let compact_health_key = runtime_profile_route_health_key("main", RuntimeRouteKind::Compact);
    let fixture = commit_runtime_fixture_with_options(
        &[MAIN_PROFILE],
        CommitRuntimeOptions {
            profile_health: runtime_health([
                (
                    websocket_health_key.clone(),
                    RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY,
                    now,
                ),
                (
                    compact_health_key.clone(),
                    RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
                    now,
                ),
            ]),
            ..CommitRuntimeOptions::default()
        },
    );
    let shared = fixture.shared();

    commit_runtime_proxy_profile_selection(shared, "main", RuntimeRouteKind::Websocket)
        .expect("profile commit should succeed");

    let runtime = shared.runtime.lock().expect("runtime should lock");
    assert_eq!(
        runtime
            .profile_health
            .get(&websocket_health_key)
            .map(|entry| entry.score),
        Some(
            RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY
                .saturating_sub(RUNTIME_PROFILE_HEALTH_SUCCESS_RECOVERY_SCORE)
        ),
        "successful commit should only partially recover heavier route penalties"
    );
    assert!(
        runtime.profile_health.contains_key(&compact_health_key),
        "successful commit should keep unrelated route health penalty intact"
    );
}

#[test]
fn commit_runtime_proxy_profile_selection_accelerates_recovery_after_success_streak() {
    let now = Local::now().timestamp();
    let route_key = runtime_profile_route_health_key("main", RuntimeRouteKind::Responses);
    let fixture = commit_runtime_fixture_with_options(
        &[MAIN_PROFILE],
        CommitRuntimeOptions {
            profile_health: runtime_health([(route_key.clone(), 5, now)]),
            ..CommitRuntimeOptions::default()
        },
    );
    let shared = fixture.shared();

    commit_runtime_proxy_profile_selection(shared, "main", RuntimeRouteKind::Responses)
        .expect("first profile commit should succeed");
    let first_remaining = shared
        .runtime
        .lock()
        .expect("runtime should lock")
        .profile_health
        .get(&route_key)
        .map(|entry| entry.score)
        .expect("first success should keep partial penalty");
    assert_eq!(first_remaining, 3);

    commit_runtime_proxy_profile_selection(shared, "main", RuntimeRouteKind::Responses)
        .expect("second profile commit should succeed");
    assert!(
        !shared
            .runtime
            .lock()
            .expect("runtime should lock")
            .profile_health
            .contains_key(&route_key),
        "consecutive successes should accelerate route recovery"
    );
}
