use super::*;
#[test]
fn rotates_profiles_after_current_profile() {
    let state = AppState {
        active_profile: Some("beta".to_string()),
        profiles: BTreeMap::from([
            (
                "alpha".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/alpha"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "beta".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/beta"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "gamma".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/gamma"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::from([(
            "sess-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
    };
    assert_eq!(
        profile_rotation_order(&state, "beta"),
        vec!["gamma".to_string(), "alpha".to_string()]
    );
}
#[test]
fn custom_base_url_maps_to_codex_usage() {
    assert_eq!(
        usage_url("http://127.0.0.1:8080"),
        "http://127.0.0.1:8080/api/codex/usage"
    );
}
#[test]
fn runtime_responses_refreshes_access_token_after_401_before_rotating() {
    let temp_dir = TestDir::isolated();
    let prodex_home = temp_dir.path.join("prodex-home");
    let prodex_home_string = prodex_home.to_string_lossy().to_string();
    let _prodex_home_guard = TestEnvVarGuard::set("PRODEX_HOME", &prodex_home_string);
    let backend = TokenAwareServer::start_responses("fresh-token", "main-account");
    let refresh_server = AuthRefreshServer::start("fresh-token", "fresh-refresh-token");
    let _refresh_guard =
        TestEnvVarGuard::set(CODEX_REFRESH_TOKEN_URL_OVERRIDE_ENV, &refresh_server.url());
    let codex_home = temp_dir.path.join("homes/main");
    write_auth_json_with_tokens(
        &codex_home.join("auth.json"),
        "stale-token",
        "main-account",
        Some("stale-refresh-token"),
        None,
    );
    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
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
                        codex_home: codex_home.clone(),
                        managed: true,
                        email: Some("main@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: backend.base_url(),
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
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: br#"{"input":"hello"}"#.to_vec(),
    };
    let response = proxy_runtime_responses_request(1, &request, &shared)
        .expect("responses request should refresh auth and succeed");
    let RuntimeResponsesReply::Buffered(parts) = response else {
        panic!("expected buffered responses reply");
    };
    assert_eq!(parts.status, 200);
    let body: serde_json::Value =
        serde_json::from_slice(&parts.body).expect("response body should parse");
    assert_eq!(body["id"], "resp-fresh");
    let auth_headers = backend.auth_headers();
    assert_eq!(
        auth_headers.first().map(String::as_str),
        Some("Bearer stale-token")
    );
    assert_eq!(
        auth_headers.last().map(String::as_str),
        Some("Bearer fresh-token")
    );
    assert_eq!(refresh_server.request_bodies().len(), 1);
    let auth_json = fs::read_to_string(codex_home.join("auth.json"))
        .expect("updated auth.json should be readable");
    let auth_json: serde_json::Value =
        serde_json::from_str(&auth_json).expect("updated auth.json should parse");
    assert_eq!(auth_json["tokens"]["access_token"], "fresh-token");
    assert_eq!(auth_json["tokens"]["refresh_token"], "fresh-refresh-token");
}
#[test]
fn runtime_responses_auto_redeems_reset_credit_before_rotating() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_usage_limit_auto_redeem();
    let codex_home = temp_dir.path.join("homes/main");
    write_auth_json(&codex_home.join("auth.json"), "main-account");
    let shared = runtime_rotation_proxy_shared_with_auto_redeem(
        &temp_dir,
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
                        codex_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: backend.base_url(),
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
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
        true,
    );
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: br#"{"input":"hello"}"#.to_vec(),
    };
    let response = proxy_runtime_responses_request(42, &request, &shared)
        .expect("responses request should auto-redeem and retry");
    match response {
        RuntimeResponsesReply::Buffered(parts) => {
            assert_eq!(parts.status, 200);
            assert!(
                String::from_utf8_lossy(&parts.body).contains("resp-main"),
                "buffered response should be same-profile success"
            );
        }
        RuntimeResponsesReply::Streaming(response) => {
            assert_eq!(response.status, 200);
        }
    }
    assert_eq!(backend.responses_accounts(), vec!["main-account".to_string()]);
    let consume_bodies = backend.reset_credit_consume_bodies();
    assert_eq!(consume_bodies.len(), 1);
    let consume_body: serde_json::Value =
        serde_json::from_str(&consume_bodies[0]).expect("consume body should parse");
    assert!(
        consume_body
            .get("redeem_request_id")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| value.starts_with("prodex-auto-redeem-"))
    );
    assert!(
        String::from_utf8_lossy(&fs::read(&shared.log_path).expect("log should be readable"))
            .contains("responses_precommit_reprobe_auto_redeem_result")
    );
    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    let cached_usage = runtime
        .profile_probe_cache
        .get("main")
        .expect("post-redeem usage should be cached")
        .result
        .as_ref()
        .expect("post-redeem usage should be successful");
    let weekly_reset_at = cached_usage
        .rate_limit
        .as_ref()
        .and_then(|limits| limits.secondary_window.as_ref())
        .and_then(|window| window.reset_at)
        .expect("weekly reset_at should be cached");
    assert!(
        weekly_reset_at > future_epoch(1_000_000),
        "post-redeem weekly reset_at must come from refreshed usage, got {weekly_reset_at}"
    );
}
#[test]
fn runtime_auto_redeem_disabled_by_default_does_not_consume_credit() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_usage_limit_auto_redeem();
    let codex_home = temp_dir.path.join("homes/main");
    write_auth_json(&codex_home.join("auth.json"), "main-account");
    let mut exhausted_usage: UsageResponse =
        serde_json::from_str(&runtime_proxy_usage_body_with_remaining("main@example.com", 0, 0))
            .expect("exhausted usage should parse");
    exhausted_usage.plan_type = Some("plus".to_string());
    exhausted_usage.rate_limit_reset_credits = Some(prodex_quota::RateLimitResetCreditsSummary {
        available_count: 1,
    });
    let now = Local::now().timestamp();
    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
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
                        codex_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: backend.base_url(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::from([(
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(exhausted_usage),
                },
            )]),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );
    let outcome = runtime_auto_redeem_usage_limit_reset_credit(
        &shared,
        "main",
        RuntimeRouteKind::Responses,
        "responses_precommit_reprobe",
        true,
    )
    .expect("auto-redeem disabled path should succeed");
    assert_eq!(outcome, RuntimeAutoRedeemResetCreditOutcome::NothingToRedeem);
    assert!(
        backend.usage_accounts().is_empty(),
        "disabled auto-redeem must not probe usage"
    );
    assert!(
        backend.reset_credit_consume_accounts().is_empty(),
        "disabled auto-redeem must not consume a credit"
    );
}
#[test]
fn runtime_responses_auto_redeem_pool_prefers_plus_before_prolite() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_usage_limit_auto_redeem();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    let mut prolite_usage: UsageResponse =
        serde_json::from_str(&runtime_proxy_usage_body_with_remaining("second@example.com", 0, 0))
            .expect("prolite usage should parse");
    prolite_usage.plan_type = Some("prolite".to_string());
    prolite_usage.rate_limit_reset_credits = Some(prodex_quota::RateLimitResetCreditsSummary {
        available_count: 1,
    });
    let now = Local::now().timestamp();
    let shared = runtime_rotation_proxy_shared_with_auto_redeem(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
                active_profile: Some("second".to_string()),
                profiles: BTreeMap::from([
                    (
                        "main".to_string(),
                        ProfileEntry {
                            codex_home: main_home,
                            managed: true,
                            email: Some("main@example.com".to_string()),
                            provider: ProfileProvider::Openai,
                        },
                    ),
                    (
                        "second".to_string(),
                        ProfileEntry {
                            codex_home: second_home,
                            managed: true,
                            email: Some("second@example.com".to_string()),
                            provider: ProfileProvider::Openai,
                        },
                    ),
                ]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: backend.base_url(),
            include_code_review: false,
            current_profile: "second".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::from([(
                "second".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(prolite_usage),
                },
            )]),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
        true,
    );
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: br#"{"input":"hello"}"#.to_vec(),
    };
    let response = proxy_runtime_responses_request(43, &request, &shared)
        .expect("responses request should redeem the Plus profile");
    match response {
        RuntimeResponsesReply::Buffered(parts) => assert_eq!(parts.status, 200),
        RuntimeResponsesReply::Streaming(response) => assert_eq!(response.status, 200),
    }
    let log = String::from_utf8_lossy(&fs::read(&shared.log_path).expect("log should be readable"))
        .to_string();
    assert_eq!(
        backend.reset_credit_consume_accounts(),
        vec!["main-account".to_string()],
        "{log}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string()]
    );
    assert!(
        backend
            .usage_accounts()
            .iter()
            .any(|account| account == "main-account"),
        "Plus account should be probed before redeem when its quota cache is missing"
    );
    assert!(log.contains("mode=auto_redeem"));
}
#[test]
fn runtime_auto_redeem_waits_while_another_profile_has_weekly_remaining() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_usage_limit_auto_redeem();
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");
    let mut exhausted_usage: UsageResponse =
        serde_json::from_str(&runtime_proxy_usage_body_with_remaining("second@example.com", 0, 0))
            .expect("exhausted usage should parse");
    exhausted_usage.plan_type = Some("plus".to_string());
    exhausted_usage.rate_limit_reset_credits = Some(prodex_quota::RateLimitResetCreditsSummary {
        available_count: 1,
    });
    let now = Local::now().timestamp();
    let shared = runtime_rotation_proxy_shared_with_auto_redeem(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
                active_profile: Some("second".to_string()),
                profiles: BTreeMap::from([
                    (
                        "second".to_string(),
                        ProfileEntry {
                            codex_home: second_home,
                            managed: true,
                            email: Some("second@example.com".to_string()),
                            provider: ProfileProvider::Openai,
                        },
                    ),
                    (
                        "third".to_string(),
                        ProfileEntry {
                            codex_home: third_home,
                            managed: true,
                            email: Some("third@example.com".to_string()),
                            provider: ProfileProvider::Openai,
                        },
                    ),
                ]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: backend.base_url(),
            include_code_review: false,
            current_profile: "second".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::from([(
                "second".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(exhausted_usage),
                },
            )]),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
        true,
    );
    let outcome = runtime_auto_redeem_usage_limit_reset_credit(
        &shared,
        "second",
        RuntimeRouteKind::Responses,
        "responses_precommit_reprobe",
        true,
    )
    .expect("auto-redeem decision should succeed");
    assert_eq!(outcome, RuntimeAutoRedeemResetCreditOutcome::NothingToRedeem);
    assert!(
        backend
            .usage_accounts()
            .iter()
            .any(|account| account == "third-account"),
        "profile with weekly remaining should be probed before deciding whether redeem is allowed"
    );
    assert!(
        backend.reset_credit_consume_accounts().is_empty(),
        "redeem must not consume a credit while another profile still has weekly quota"
    );
    let log_bytes = fs::read(&shared.log_path).expect("log should be readable");
    let log = String::from_utf8_lossy(&log_bytes);
    assert!(log.contains("reason=weekly_pool_profile"));
}
#[test]
fn read_auth_summary_classifies_api_key_auth() {
    let temp_dir = TestDir::isolated();
    let codex_home = temp_dir.path.join("homes/main");
    write_api_key_auth_json(&codex_home.join("auth.json"));
    let summary = read_auth_summary(&codex_home);
    assert_eq!(summary.label, "api-key");
    assert!(!summary.quota_compatible);
}
#[test]
fn read_usage_auth_prefers_account_id_from_access_token_claims() {
    let temp_dir = TestDir::isolated();
    let codex_home = temp_dir.path.join("homes/main");
    write_auth_json_with_tokens(
        &codex_home.join("auth.json"),
        &fake_jwt_with_exp_and_account_id(Local::now().timestamp() + 600, "jwt-account"),
        "stale-account",
        Some("stale-refresh-token"),
        None,
    );
    let auth = read_usage_auth(&codex_home).expect("usage auth should load");
    assert_eq!(auth.account_id.as_deref(), Some("jwt-account"));
}
#[test]
fn unique_profile_name_adds_numeric_suffix() {
    let state = AppState {
        active_profile: None,
        profiles: BTreeMap::from([(
            "main_example.com".to_string(),
            ProfileEntry {
                codex_home: PathBuf::from("/tmp/existing"),
                managed: true,
                email: Some("other@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let paths = AppPaths {
        root: PathBuf::from("/tmp/prodex-test"),
        state_file: PathBuf::from("/tmp/prodex-test/state.json"),
        managed_profiles_root: PathBuf::from("/tmp/prodex-test/profiles"),
        shared_codex_root: PathBuf::from("/tmp/prodex-test/default-codex"),
        legacy_shared_codex_root: PathBuf::from("/tmp/prodex-test/shared"),
    };
    assert_eq!(
        unique_profile_name_for_email(&paths, &state, "main@example.com"),
        "main_example.com-2"
    );
}
#[test]
fn backend_api_base_url_maps_to_wham_usage() {
    assert_eq!(
        usage_url("https://chatgpt.com/backend-api"),
        "https://chatgpt.com/backend-api/wham/usage"
    );
}
#[test]
fn fetch_usage_json_refreshes_access_token_after_401() {
    let temp_dir = TestDir::isolated();
    let prodex_home = temp_dir.path.join("prodex-home");
    let prodex_home_string = prodex_home.to_string_lossy().to_string();
    let _prodex_home_guard = TestEnvVarGuard::set("PRODEX_HOME", &prodex_home_string);
    let usage_server =
        TokenAwareServer::start_usage("fresh-token", "main-account", "main@example.com");
    let refresh_server = AuthRefreshServer::start("fresh-token", "fresh-refresh-token");
    let _refresh_guard =
        TestEnvVarGuard::set(CODEX_REFRESH_TOKEN_URL_OVERRIDE_ENV, &refresh_server.url());
    let codex_home = temp_dir.path.join("homes/main");
    write_auth_json_with_tokens(
        &codex_home.join("auth.json"),
        "stale-token",
        "main-account",
        Some("stale-refresh-token"),
        None,
    );
    let usage = fetch_usage_json(&codex_home, Some(&usage_server.base_url()))
        .expect("quota fetch should refresh and succeed");
    assert_eq!(usage["email"], "main@example.com");
    assert_eq!(
        usage_server.auth_headers(),
        vec![
            "Bearer stale-token".to_string(),
            "Bearer fresh-token".to_string()
        ]
    );
    assert_eq!(refresh_server.request_bodies().len(), 1);
    let auth_json = fs::read_to_string(codex_home.join("auth.json"))
        .expect("updated auth.json should be readable");
    let auth_json: serde_json::Value =
        serde_json::from_str(&auth_json).expect("updated auth.json should parse");
    assert_eq!(auth_json["tokens"]["access_token"], "fresh-token");
    assert_eq!(auth_json["tokens"]["refresh_token"], "fresh-refresh-token");
    assert!(auth_json.get("last_refresh").is_some());
}
#[test]
fn concurrent_quota_fetches_share_refresh_result_for_same_refresh_token() {
    let temp_dir = TestDir::isolated();
    let prodex_home = temp_dir.path.join("prodex-home");
    let prodex_home_string = prodex_home.to_string_lossy().to_string();
    let _prodex_home_guard = TestEnvVarGuard::set("PRODEX_HOME", &prodex_home_string);
    let usage_server =
        TokenAwareServer::start_usage("fresh-token", "main-account", "main@example.com");
    let refresh_server = AuthRefreshServer::start_single_use("fresh-token", "fresh-refresh-token");
    let _refresh_guard =
        TestEnvVarGuard::set(CODEX_REFRESH_TOKEN_URL_OVERRIDE_ENV, &refresh_server.url());
    let first_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    for codex_home in [&first_home, &second_home] {
        write_auth_json_with_tokens(
            &codex_home.join("auth.json"),
            "stale-token",
            "main-account",
            Some("stale-refresh-token"),
            None,
        );
    }
    let usage_base_url = usage_server.base_url();
    let (first_usage, second_usage) = std::thread::scope(|scope| {
        let first = scope.spawn(|| fetch_usage_json(&first_home, Some(&usage_base_url)));
        let second = scope.spawn(|| fetch_usage_json(&second_home, Some(&usage_base_url)));
        (
            first.join().expect("first quota thread should not panic"),
            second.join().expect("second quota thread should not panic"),
        )
    });
    if first_usage.is_err() || second_usage.is_err() {
        panic!(
            "quota fetches should share refresh result; first={:?}; second={:?}; refresh_requests={}; usage_auth_headers={:?}",
            first_usage.as_ref().err(),
            second_usage.as_ref().err(),
            refresh_server.request_bodies().len(),
            usage_server.auth_headers(),
        );
    }
    let first_usage = first_usage.expect("first quota fetch should refresh and succeed");
    let second_usage = second_usage.expect("second quota fetch should reuse refresh result");
    assert_eq!(first_usage["email"], "main@example.com");
    assert_eq!(second_usage["email"], "main@example.com");
    assert_eq!(refresh_server.request_bodies().len(), 1);
    for codex_home in [first_home, second_home] {
        let auth_json = fs::read_to_string(codex_home.join("auth.json"))
            .expect("updated auth.json should be readable");
        let auth_json: serde_json::Value =
            serde_json::from_str(&auth_json).expect("updated auth.json should parse");
        assert_eq!(auth_json["tokens"]["access_token"], "fresh-token");
        assert_eq!(auth_json["tokens"]["refresh_token"], "fresh-refresh-token");
        assert!(auth_json.get("last_refresh").is_some());
    }
}
#[test]
fn read_auth_summary_classifies_invalid_auth_json() {
    let temp_dir = TestDir::isolated();
    let codex_home = temp_dir.path.join("homes/main");
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(
            &secret_store::SecretLocation::file(codex_home.join("auth.json")),
            "{".to_string(),
        )
        .expect("failed to write invalid auth.json");
    let summary = read_auth_summary(&codex_home);
    assert_eq!(summary.label, "invalid-auth");
    assert!(!summary.quota_compatible);
}
#[test]
fn profile_name_is_derived_from_email() {
    assert_eq!(
        profile_name_from_email("Main+Ops@Example.com"),
        "main-ops_example.com"
    );
}
#[test]
fn usage_response_accepts_null_additional_rate_limits() {
    let usage: UsageResponse = serde_json::from_value(serde_json::json!({
        "email": "user@example.com",
        "plan_type": "plus",
        "rate_limit": null,
        "code_review_rate_limit": null,
        "additional_rate_limits": null
    }))
    .expect("usage response should parse");
    assert!(usage.additional_rate_limits.is_empty());
}
