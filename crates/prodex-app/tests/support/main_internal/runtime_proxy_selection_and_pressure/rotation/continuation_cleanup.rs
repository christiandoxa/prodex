use super::*;

#[path = "continuation_cleanup/dead_bindings.rs"]
mod dead_bindings;
#[path = "continuation_cleanup/compact_lineage.rs"]
mod compact_lineage;
#[path = "continuation_cleanup/startup_selection.rs"]
mod startup_selection;
#[path = "continuation_cleanup/websocket_policy.rs"]
mod websocket_policy;
#[path = "continuation_cleanup/optimistic_current.rs"]
mod optimistic_current;
#[path = "continuation_cleanup/session_affinity.rs"]
mod session_affinity;

fn runtime_shared_for_dead_response_binding_cleanup(
    temp_dir: &TestDir,
) -> RuntimeRotationProxyShared {
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    runtime_rotation_proxy_shared(
        temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
                active_profile: Some("main".to_string()),
                profiles: BTreeMap::from([(
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "http://127.0.0.1:1/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_backoff_updated_at: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    )
}
