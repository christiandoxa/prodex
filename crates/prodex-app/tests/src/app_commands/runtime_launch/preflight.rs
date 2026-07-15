use super::*;

#[test]
fn no_ready_runtime_profiles_returns_error_for_blocked_report() {
    let report = RunProfileProbeReport {
        name: "main".to_string(),
        order_index: 0,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Ok(UsageResponse {
            email: None,
            plan_type: None,
            rate_limit: None,
            code_review_rate_limit: None,
            rate_limit_reset_credits: None,
            additional_rate_limits: Vec::new(),
        }),
    };

    let err = handle_no_ready_runtime_profiles(
        &report,
        "main",
        &RuntimeLaunchRequest {
            profile: None,
            allow_auto_rotate: true,
            auto_redeem: false,
            skip_quota_check: false,
            base_url: None,
            upstream_no_proxy: false,
            include_code_review: false,
            smart_context_enabled: false,
            presidio_redaction_enabled: false,
            model_context_window_tokens: None,
            gemini_thinking_budget_tokens: None,
            force_runtime_proxy: false,
            model_provider_override: None,
            profile_v2_name: None,
            external_provider: None,
            external_provider_api_key: None,
        },
    )
    .expect_err("blocked preflight should return an error instead of exiting");

    let message = format!("{err:#}");
    assert!(message.contains("quota preflight blocked profile 'main'"));
    assert!(message.contains("no ready profile"));
    assert_eq!(
        err.downcast_ref::<crate::command_dispatch::ProdexCommandExit>()
            .expect("blocked preflight should carry an explicit exit code")
            .code(),
        2
    );
}

#[test]
fn no_ready_runtime_profiles_continues_when_probe_failed() {
    let report = RunProfileProbeReport {
        name: "main".to_string(),
        order_index: 0,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Err("network down".to_string()),
    };

    handle_no_ready_runtime_profiles(
        &report,
        "main",
        &RuntimeLaunchRequest {
            profile: None,
            allow_auto_rotate: true,
            auto_redeem: false,
            skip_quota_check: false,
            base_url: None,
            upstream_no_proxy: false,
            include_code_review: false,
            smart_context_enabled: false,
            presidio_redaction_enabled: false,
            model_context_window_tokens: None,
            gemini_thinking_budget_tokens: None,
            force_runtime_proxy: false,
            model_provider_override: None,
            profile_v2_name: None,
            external_provider: None,
            external_provider_api_key: None,
        },
    )
    .expect("probe failure should still continue without quota gate");
}

#[test]
fn runtime_launch_preflight_error_message_redacts_secret_like_chain() {
    let err = anyhow::anyhow!("failed: Authorization: Bearer preflight-token")
        .context("quota preflight failed");

    let message = runtime_launch_preflight_error_message(&err);

    assert!(message.contains("quota preflight failed"));
    assert!(message.contains("Authorization: Bearer <redacted>"));
    assert!(!message.contains("preflight-token"));
}

#[test]
fn persisted_weekly_only_snapshot_does_not_block_launch_preflight() {
    let usage = runtime_launch_usage_from_snapshot(&RuntimeProfileUsageSnapshot {
        checked_at: Local::now().timestamp(),
        five_hour_status: RuntimeQuotaWindowStatus::Unknown,
        five_hour_remaining_percent: 0,
        five_hour_reset_at: i64::MAX,
        weekly_status: RuntimeQuotaWindowStatus::Ready,
        weekly_remaining_percent: 100,
        weekly_reset_at: Local::now().timestamp() + 604_800,
    });

    assert!(usage.rate_limit.as_ref().unwrap().primary_window.is_none());
    assert!(collect_blocked_limits(&usage, false).is_empty());
}

#[test]
fn prepare_runtime_launch_rechecks_persisted_exhausted_quota_snapshot() {
    let root = temp_dir("launch-preflight-persisted-exhausted-snapshot");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let main_home = root.join("main-home");
    fs::create_dir_all(&main_home).unwrap();
    write_runtime_launch_auth(
        main_home.join("auth.json"),
        serde_json::json!({
            "tokens": {
                "access_token": "main-token",
                "account_id": "main-account"
            }
        })
        .to_string(),
    )
    .unwrap();
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home.clone(),
                managed: false,
                email: None,
                provider: ProfileProvider::Openai,
            },
        )]),
        ..AppState::default()
    };
    write_state(&root, state.clone());
    let paths = AppPaths::discover().unwrap();
    let now = Local::now().timestamp();
    save_runtime_usage_snapshots_for_profiles(
        &paths,
        &BTreeMap::from([(
            "main".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: now,
                five_hour_status: RuntimeQuotaWindowStatus::Unknown,
                five_hour_remaining_percent: 0,
                five_hour_reset_at: i64::MAX,
                weekly_status: RuntimeQuotaWindowStatus::Exhausted,
                weekly_remaining_percent: 0,
                weekly_reset_at: now + 432_000,
            },
        )]),
        &state.profiles,
    )
    .unwrap();

    let server = TinyServer::http("127.0.0.1:0").expect("quota test server should bind");
    let addr = server.server_addr().to_ip().unwrap();
    let handle = thread::spawn(move || {
        let request = server
            .recv_timeout(Duration::from_secs(5))
            .expect("quota request should be readable")
            .expect("exhausted snapshot should trigger a live quota request");
        assert_eq!(request.url(), "/backend-api/wham/usage");
        request
            .respond(
                TinyResponse::from_string(
                    serde_json::json!({
                        "email": "member@example.com",
                        "plan_type": "team",
                        "rate_limit": {
                            "primary_window": null,
                            "secondary_window": {
                                "used_percent": 0,
                                "reset_at": now + 604_800,
                                "limit_window_seconds": 604_800
                            }
                        }
                    })
                    .to_string(),
                )
                .with_header(TinyHeader::from_bytes("Content-Type", "application/json").unwrap()),
            )
            .expect("quota response should send");
    });
    let base_url = format!("http://{addr}/backend-api");

    let prepared = prepare_runtime_launch(RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: false,
        base_url: Some(&base_url),
        upstream_no_proxy: true,
        include_code_review: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: None,
        profile_v2_name: None,
        external_provider: None,
        external_provider_api_key: None,
    })
    .expect("live ready quota should override an exhausted persisted snapshot");
    handle.join().expect("quota test server should finish");

    assert_eq!(prepared.codex_home, main_home);
}

#[test]
fn prepare_runtime_launch_auto_selects_ready_snapshot_before_current_network_preflight() {
    let root = temp_dir("launch-preflight-ready-snapshot-before-current-network");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let current_home = root.join("current-home");
    let ready_home = root.join("ready-home");
    fs::create_dir_all(&current_home).unwrap();
    fs::create_dir_all(&ready_home).unwrap();
    write_runtime_launch_auth(
        ready_home.join("auth.json"),
        serde_json::json!({
            "tokens": {
                "access_token": "ready-token",
                "account_id": "ready-account"
            }
        })
        .to_string(),
    )
    .unwrap();
    let state = AppState {
        active_profile: Some("current".to_string()),
        profiles: BTreeMap::from([
            (
                "current".to_string(),
                ProfileEntry {
                    codex_home: current_home,
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "ready".to_string(),
                ProfileEntry {
                    codex_home: ready_home.clone(),
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        ..AppState::default()
    };
    write_state(&root, state.clone());
    let paths = AppPaths::discover().unwrap();
    let now = Local::now().timestamp();
    save_runtime_usage_snapshots_for_profiles(
        &paths,
        &BTreeMap::from([(
            "ready".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: now,
                five_hour_status: RuntimeQuotaWindowStatus::Ready,
                five_hour_remaining_percent: 72,
                five_hour_reset_at: now + 18_000,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 33,
                weekly_reset_at: now + 604_800,
            },
        )]),
        &state.profiles,
    )
    .unwrap();

    let mut state = AppState::load(&paths).unwrap();
    let request = RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: false,
        base_url: Some("http://127.0.0.1:9"),
        upstream_no_proxy: true,
        include_code_review: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: None,
        profile_v2_name: None,
        external_provider: None,
        external_provider_api_key: None,
    };
    let selection = select_runtime_launch_profile(&paths, &mut state, &request).unwrap();

    assert_eq!(selection.codex_home, ready_home);
    let state = AppState::load(&paths).unwrap();
    assert_eq!(state.active_profile.as_deref(), Some("ready"));
}
