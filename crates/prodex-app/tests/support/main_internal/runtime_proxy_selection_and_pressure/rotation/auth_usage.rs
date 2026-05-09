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
fn read_auth_summary_classifies_invalid_auth_json() {
    let temp_dir = TestDir::isolated();
    let codex_home = temp_dir.path.join("homes/main");
    fs::create_dir_all(&codex_home).expect("failed to create codex home");
    fs::write(codex_home.join("auth.json"), "{").expect("failed to write invalid auth.json");

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
