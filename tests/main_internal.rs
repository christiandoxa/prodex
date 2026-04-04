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

        #[test]
        fn prepare_managed_codex_home_does_not_reseed_populated_legacy_sessions_every_run() {
            let temp_dir = TestDir::new();
            let home_dir = temp_dir.path.join("home");
            let prodex_home = temp_dir.path.join("prodex");
            create_codex_home_if_missing(&home_dir).expect("home dir should be created");
            create_codex_home_if_missing(&prodex_home).expect("prodex dir should be created");
            let _home_guard = TestEnvVarGuard::set("HOME", &home_dir.display().to_string());
            let _prodex_guard =
                TestEnvVarGuard::set("PRODEX_HOME", &prodex_home.display().to_string());
            let _shared_override_guard = TestEnvVarGuard::unset("PRODEX_SHARED_CODEX_HOME");

            let legacy_session_dir = home_dir.join(".codex/sessions/2026/04/02");
            create_codex_home_if_missing(&legacy_session_dir)
                .expect("legacy session dir should be created");
            let first_legacy_session = legacy_session_dir.join("legacy-first.jsonl");
            fs::write(&first_legacy_session, "legacy-first")
                .expect("first legacy session should be written");

            let paths = AppPaths::discover().expect("app paths should resolve");
            let profile_home = paths.root.join("profiles/main");

            prepare_managed_codex_home(&paths, &profile_home)
                .expect("first prepare should seed legacy sessions");

            let shared_session_dir = paths.shared_codex_root.join("sessions/2026/04/02");
            let first_shared_session = shared_session_dir.join("legacy-first.jsonl");
            assert_eq!(
                fs::read_to_string(&first_shared_session)
                    .expect("shared session should exist after initial seed"),
                "legacy-first"
            );

            let second_legacy_session = legacy_session_dir.join("legacy-second.jsonl");
            fs::write(&second_legacy_session, "legacy-second")
                .expect("second legacy session should be written");

            prepare_managed_codex_home(&paths, &profile_home)
                .expect("second prepare should keep startup cheap");

            assert!(
                !shared_session_dir.join("legacy-second.jsonl").exists(),
                "populated shared session trees should not be re-seeded on every run"
            );
        }

        #[test]
        fn prepare_managed_codex_home_does_not_reseed_populated_legacy_history_every_run() {
            let temp_dir = TestDir::new();
            let home_dir = temp_dir.path.join("home");
            let prodex_home = temp_dir.path.join("prodex");
            create_codex_home_if_missing(&home_dir).expect("home dir should be created");
            create_codex_home_if_missing(&prodex_home).expect("prodex dir should be created");
            let _home_guard = TestEnvVarGuard::set("HOME", &home_dir.display().to_string());
            let _prodex_guard =
                TestEnvVarGuard::set("PRODEX_HOME", &prodex_home.display().to_string());
            let _shared_override_guard = TestEnvVarGuard::unset("PRODEX_SHARED_CODEX_HOME");

            let legacy_history = home_dir.join(".codex/history.jsonl");
            create_codex_home_if_missing(
                legacy_history
                    .parent()
                    .expect("legacy history parent should exist"),
            )
            .expect("legacy history parent should be created");
            fs::write(
                &legacy_history,
                "{\"session_id\":\"legacy\",\"ts\":100,\"text\":\"legacy-first\"}\n",
            )
            .expect("legacy history should be written");

            let paths = AppPaths::discover().expect("app paths should resolve");
            let profile_home = paths.root.join("profiles/main");

            prepare_managed_codex_home(&paths, &profile_home)
                .expect("first prepare should seed legacy history");

            let shared_history = paths.shared_codex_root.join("history.jsonl");
            assert_eq!(
                fs::read_to_string(&shared_history).expect("shared history should exist"),
                "{\"session_id\":\"legacy\",\"ts\":100,\"text\":\"legacy-first\"}\n"
            );

            fs::write(
                &legacy_history,
                concat!(
                    "{\"session_id\":\"legacy\",\"ts\":100,\"text\":\"legacy-first\"}\n",
                    "{\"session_id\":\"legacy\",\"ts\":200,\"text\":\"legacy-second\"}\n"
                ),
            )
            .expect("updated legacy history should be written");

            prepare_managed_codex_home(&paths, &profile_home)
                .expect("second prepare should keep startup cheap");

            assert_eq!(
                fs::read_to_string(&shared_history)
                    .expect("shared history should remain unchanged after reseed"),
                "{\"session_id\":\"legacy\",\"ts\":100,\"text\":\"legacy-first\"}\n"
            );
        }

        #[cfg(unix)]
        #[test]
        fn prepare_managed_codex_home_merges_split_legacy_symlink_targets_before_relinking() {
            use std::os::unix::fs::symlink;

            let temp_dir = TestDir::new();
            let paths = AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("prodex/.codex"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            };
            let profile_home = paths.managed_profiles_root.join("main");
            let second_profile_home = paths.managed_profiles_root.join("second");
            let legacy_shared_root = temp_dir.path.join("legacy-codex");

            create_codex_home_if_missing(&profile_home).expect("profile home should exist");
            create_codex_home_if_missing(&second_profile_home)
                .expect("second profile home should exist");
            create_codex_home_if_missing(&paths.shared_codex_root)
                .expect("current shared codex root should exist");
            create_codex_home_if_missing(&legacy_shared_root)
                .expect("legacy shared codex root should exist");

            fs::write(
                paths.shared_codex_root.join("history.jsonl"),
                "{\"session_id\":\"current\",\"ts\":200,\"text\":\"current-root\"}\n",
            )
            .expect("current shared history should be written");
            fs::write(
                legacy_shared_root.join("history.jsonl"),
                "{\"session_id\":\"legacy\",\"ts\":100,\"text\":\"legacy-root\"}\n",
            )
            .expect("legacy shared history should be written");

            let current_session_dir = paths.shared_codex_root.join("sessions/2026/04/04");
            let legacy_session_dir = legacy_shared_root.join("sessions/2026/04/03");
            create_codex_home_if_missing(&current_session_dir)
                .expect("current session dir should exist");
            create_codex_home_if_missing(&legacy_session_dir)
                .expect("legacy session dir should exist");
            fs::write(current_session_dir.join("current.jsonl"), "current")
                .expect("current session should be written");
            fs::write(legacy_session_dir.join("legacy.jsonl"), "legacy")
                .expect("legacy session should be written");

            symlink(
                legacy_shared_root.join("history.jsonl"),
                profile_home.join("history.jsonl"),
            )
            .expect("legacy history symlink should be created");
            symlink(
                legacy_shared_root.join("history.jsonl"),
                second_profile_home.join("history.jsonl"),
            )
            .expect("second legacy history symlink should be created");
            symlink(
                legacy_shared_root.join("sessions"),
                profile_home.join("sessions"),
            )
            .expect("legacy sessions symlink should be created");
            symlink(
                legacy_shared_root.join("sessions"),
                second_profile_home.join("sessions"),
            )
            .expect("second legacy sessions symlink should be created");

            prepare_managed_codex_home(&paths, &profile_home)
                .expect("prepare should merge legacy symlink target and relink");
            prepare_managed_codex_home(&paths, &second_profile_home)
                .expect("second prepare should not duplicate merged history");

            assert_eq!(
                fs::read_link(profile_home.join("history.jsonl"))
                    .expect("history symlink should remain present"),
                paths.shared_codex_root.join("history.jsonl")
            );
            assert_eq!(
                fs::read_link(second_profile_home.join("history.jsonl"))
                    .expect("second history symlink should remain present"),
                paths.shared_codex_root.join("history.jsonl")
            );
            assert_eq!(
                fs::read_link(profile_home.join("sessions"))
                    .expect("sessions symlink should remain present"),
                paths.shared_codex_root.join("sessions")
            );
            assert_eq!(
                fs::read_link(second_profile_home.join("sessions"))
                    .expect("second sessions symlink should remain present"),
                paths.shared_codex_root.join("sessions")
            );

            let shared_history = fs::read_to_string(paths.shared_codex_root.join("history.jsonl"))
                .expect("merged shared history should be readable");
            assert_eq!(
                shared_history,
                concat!(
                    "{\"session_id\":\"legacy\",\"ts\":100,\"text\":\"legacy-root\"}\n",
                    "{\"session_id\":\"current\",\"ts\":200,\"text\":\"current-root\"}"
                )
            );
            assert!(
                paths
                    .shared_codex_root
                    .join("sessions/2026/04/04/current.jsonl")
                    .is_file()
            );
            assert!(
                paths
                    .shared_codex_root
                    .join("sessions/2026/04/03/legacy.jsonl")
                    .is_file()
            );
        }
    }
}
