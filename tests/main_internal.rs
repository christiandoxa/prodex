mod prodex_impl {
    #![allow(dead_code, unused_imports)]

    include!("../src/main.rs");

    mod tests {
        #![allow(dead_code, unused_imports)]

        use tungstenite::connect as ws_connect;

        fn runtime_proxy_lane_admission_for_global_limit(
            global_limit: usize,
        ) -> RuntimeProxyLaneAdmission {
            RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
                responses: global_limit.max(1),
                compact: global_limit.max(1),
                websocket: global_limit.max(1),
                standard: global_limit.max(1),
            })
        }

        fn runtime_proxy_in_local_overload_backoff(shared: &RuntimeRotationProxyShared) -> bool {
            let now = Local::now().timestamp().max(0) as u64;
            shared.local_overload_backoff_until.load(Ordering::SeqCst) > now
        }

        fn save_runtime_continuations(
            paths: &AppPaths,
            continuations: &RuntimeContinuationStore,
        ) -> Result<()> {
            let profiles = AppState::load(paths)
                .map(|state| state.profiles)
                .unwrap_or_default();
            save_runtime_continuations_for_profiles(paths, continuations, &profiles)
        }

        fn select_runtime_response_candidate(
            shared: &RuntimeRotationProxyShared,
            excluded_profiles: &BTreeSet<String>,
            pinned_profile: Option<&str>,
            turn_state_profile: Option<&str>,
            session_profile: Option<&str>,
            discover_previous_response_owner: bool,
        ) -> Result<Option<String>> {
            select_runtime_response_candidate_for_route(
                shared,
                excluded_profiles,
                None,
                pinned_profile,
                turn_state_profile,
                session_profile,
                discover_previous_response_owner,
                None,
                RuntimeRouteKind::Responses,
            )
        }

        fn runtime_proxy_optimistic_current_candidate(
            shared: &RuntimeRotationProxyShared,
            excluded_profiles: &BTreeSet<String>,
        ) -> Result<Option<String>> {
            runtime_proxy_optimistic_current_candidate_for_route(
                shared,
                excluded_profiles,
                RuntimeRouteKind::Responses,
            )
        }

        fn next_runtime_response_candidate(
            shared: &RuntimeRotationProxyShared,
            excluded_profiles: &BTreeSet<String>,
        ) -> Result<Option<String>> {
            next_runtime_response_candidate_for_route(
                shared,
                excluded_profiles,
                RuntimeRouteKind::Responses,
            )
        }

        include!("support/main_internal_body.rs");

        #[test]
        fn runtime_profile_usage_auth_cache_entry_matches_detects_auth_json_changes() {
            let temp_dir = TestDir::new();
            let profile_home = temp_dir.path.join("homes/main");
            let auth_path = profile_home.join("auth.json");
            write_auth_json(&auth_path, "second-account");

            let cached = load_runtime_profile_usage_auth_cache_entry(&profile_home)
                .expect("auth cache entry should load");
            assert!(
                runtime_profile_usage_auth_cache_entry_matches(&cached)
                    .expect("auth cache entry should match fresh file")
            );

            std::thread::sleep(std::time::Duration::from_millis(5));
            write_auth_json(&auth_path, "third-account");

            assert!(
                !runtime_profile_usage_auth_cache_entry_matches(&cached)
                    .expect("auth cache entry should detect auth.json change")
            );
        }
    }
}
