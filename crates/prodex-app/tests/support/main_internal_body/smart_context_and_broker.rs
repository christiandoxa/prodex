use super::*;

fn smart_context_enabled_from_default_super_shortcut() -> bool {
    let command = parse_cli_command_from(["prodex", "s", "exec", "hi"])
        .expect("default Super shortcut should parse");
    let Commands::Super(args) = command else {
        panic!("expected Super command from prodex s");
    };
    args.into_caveman_args().smart_context
}

#[test]
fn default_super_shortcut_starts_smart_context_proxy_that_rewrites_large_tool_output() {
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let temp_dir = TestDir::new();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let proxy = start_runtime_rotation_proxy_with_options(RuntimeRotationProxyStartOptions {
        paths: &paths,
        state: &state,
        current_profile: "second",
        upstream_base_url: backend.base_url(),
        include_code_review: false,
        upstream_no_proxy: false,
        auto_redeem: false,
        smart_context_enabled: smart_context_enabled_from_default_super_shortcut(),
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: None,
    })
    .expect("default Super runtime proxy should start with smart context enabled");
    let tool_output = (0..1900)
        .map(|index| format!("line {index}: repeated command output"))
        .collect::<Vec<_>>()
        .join("\n");
    let body = serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_big",
            "output": tool_output,
        }]
    })
    .to_string();
    let estimated_tokens =
        runtime_proxy_crate::smart_context_estimate_tokens_from_body(body.as_bytes()) as usize;
    let available_tokens = 32_000usize
        .saturating_sub(estimated_tokens)
        .saturating_sub(4_096);
    assert_eq!(
        runtime_proxy_crate::smart_context_token_budget_tier(available_tokens),
        runtime_proxy_crate::SmartContextTokenBudgetTier::Condensed
    );

    let response = Client::builder()
        .timeout(ci_timing_upper_bound_ms(5_000, 10_000))
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body)
        .send()
        .expect("responses request should succeed");

    assert!(
        response.status().is_success(),
        "smart-context request should pass through upstream status: {}",
        response.status()
    );
    let responses_bodies = backend.responses_bodies();
    assert_eq!(responses_bodies.len(), 1);
    assert!(responses_bodies[0].contains("psc art psc:"));
    assert!(!responses_bodies[0].contains("prodex-artifact:sc:"));
    assert!(
        !responses_bodies[0].contains("line 1200: repeated command output"),
        "middle tool-output noise should be artifact-backed, not forwarded inline"
    );

    let log_tail = wait_for_runtime_log_tail_until(
        || fs::read(&proxy.log_path).ok(),
        |text| text.contains("smart_context_autopilot") && text.contains("decision=rewritten"),
        2_000,
        8_000,
        20,
    );
    let log_tail = String::from_utf8_lossy(&log_tail);
    assert!(log_tail.contains("tier=condensed"));
    assert!(log_tail.contains("budget_mode=artifact_condensed"));
    assert!(log_tail.contains("policy_reasons=tight_budget"));
    assert!(log_tail.contains("artifacts_stored=1"));
    assert!(log_tail.contains("tool_outputs_condensed=1"));
}

#[test]
fn runtime_smart_context_proxy_disabled_passes_large_tool_output_unchanged() {
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let temp_dir = TestDir::new();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start with smart context disabled");
    let tool_output = (0..2500)
        .map(|index| format!("line {index}: repeated command output"))
        .collect::<Vec<_>>()
        .join("\n");
    let body = serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_big",
            "output": tool_output,
        }]
    })
    .to_string();

    let response = Client::builder()
        .timeout(ci_timing_upper_bound_ms(2_000, 8_000))
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body)
        .send()
        .expect("responses request should succeed");

    assert!(
        response.status().is_success(),
        "disabled smart-context request should pass through upstream status: {}",
        response.status()
    );
    let responses_bodies = backend.responses_bodies();
    assert_eq!(responses_bodies.len(), 1);
    assert!(!responses_bodies[0].contains("prodex-sc artifact"));
    assert!(responses_bodies[0].contains("line 1200: repeated command output"));
    let log = fs::read_to_string(&proxy.log_path).expect("runtime proxy log should be readable");
    assert!(!log.contains("smart_context_autopilot"));
}

#[test]
fn preferred_runtime_broker_listen_addr_only_reuses_dead_registry_ports() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let broker_key = "reuse-port-test";
    save_runtime_broker_registry(
        &paths,
        broker_key,
        &RuntimeBrokerRegistry {
            pid: 999_999_999,
            listen_addr: "127.0.0.1:33475".to_string(),
            started_at: Local::now().timestamp(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            upstream_no_proxy: false,
            smart_context_enabled: false,
            current_profile: "main".to_string(),
            instance_id: "dead-instance".to_string(),
            prodex_version: None,
            executable_path: None,
            executable_sha256: None,
            openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
            realtime_ws_addr: None,
        },
    )
    .expect("dead broker registry should save");

    assert_eq!(
        preferred_runtime_broker_listen_addr(&paths, broker_key)
            .expect("dead broker port lookup should succeed"),
        Some("127.0.0.1:33475".to_string())
    );

    save_runtime_broker_registry(
        &paths,
        broker_key,
        &RuntimeBrokerRegistry {
            pid: std::process::id(),
            listen_addr: "127.0.0.1:33475".to_string(),
            started_at: Local::now().timestamp(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            upstream_no_proxy: false,
            smart_context_enabled: false,
            current_profile: "main".to_string(),
            instance_id: "live-instance".to_string(),
            prodex_version: None,
            executable_path: None,
            executable_sha256: None,
            openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
            realtime_ws_addr: None,
        },
    )
    .expect("live broker registry should save");

    assert_eq!(
        preferred_runtime_broker_listen_addr(&paths, broker_key)
            .expect("live broker port lookup should succeed"),
        None
    );
}

#[test]
fn runtime_rotation_proxy_can_bind_a_requested_listen_addr() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let state = AppState {
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
    };
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let probe = TcpListener::bind("127.0.0.1:0").expect("probe socket should bind");
    let requested_addr = probe
        .local_addr()
        .expect("probe socket should expose requested addr");
    drop(probe);

    let proxy = start_runtime_rotation_proxy_with_listen_addr(
        &paths,
        &state,
        "main",
        backend.base_url(),
        false,
        false,
        Some(&requested_addr.to_string()),
    )
    .expect("runtime proxy should bind requested listen addr");

    assert_eq!(proxy.listen_addr, requested_addr);
}

#[test]
fn runtime_broker_lease_drop_removes_file() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let lease = create_runtime_broker_lease(&paths, "drop-test")
        .expect("lease should be created for drop test");
    let lease_path = lease.path.clone();
    assert!(lease_path.exists(), "lease file should exist before drop");

    drop(lease);

    assert!(
        !lease_path.exists(),
        "lease file should be removed when the endpoint drops it"
    );
}

#[test]
fn runtime_broker_startup_grace_covers_ready_timeout() {
    let _timeout_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS", "15000");
    assert!(
        runtime_broker_startup_grace_seconds(
            RuntimeConfig::compatibility_current().broker_ready_timeout_ms,
        ) >= 16
    );
}

#[test]
fn runtime_broker_and_update_commands_skip_prodex_update_notice() {
    let runtime_broker = Commands::RuntimeBroker(RuntimeBrokerArgs {});
    let update = Commands::Update(ProdexUpdateArgs {});
    let run = Commands::Run(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: false,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![OsString::from("hello")],
    });

    assert!(!crate::command_dispatch::command_should_show_update_notice(
        &runtime_broker
    ));
    assert!(!crate::command_dispatch::command_should_show_update_notice(
        &update
    ));
    assert!(crate::command_dispatch::command_should_show_update_notice(
        &run
    ));
}

#[test]
fn runtime_proxy_presidio_redaction_failure_uses_generic_response_and_logs_no_endpoint() {
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let temp_dir = TestDir::new();
    let home = temp_dir.path.join("homes/primary");
    write_auth_json(&home.join("auth.json"), "primary-account");
    let paths = AppPaths {
        state_file: temp_dir.path.join("state.json"),
        managed_profiles_root: temp_dir.path.join("homes"),
        shared_codex_root: temp_dir.path.join("shared-codex"),
        legacy_shared_codex_root: temp_dir.path.join("shared"),
        root: temp_dir.path.clone(),
    };
    let state = AppState::default();
    let proxy = start_runtime_rotation_proxy_with_options(RuntimeRotationProxyStartOptions {
        paths: &paths,
        state: &state,
        current_profile: "primary",
        upstream_base_url: backend.base_url(),
        include_code_review: false,
        upstream_no_proxy: false,
        auto_redeem: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: None,
    })
    .expect("runtime proxy should start");
    register_runtime_presidio_redaction_proxy_state(
        &proxy.log_path,
        Some(RuntimePresidioRedactionConfig {
            analyzer_url: "http://127.0.0.1:9/analyze?secret=standard-presidio-secret".to_string(),
            anonymizer_url: "http://127.0.0.1:9/anonymize?secret=standard-presidio-secret"
                .to_string(),
            languages: vec!["en".to_string()],
            language_mode: PresidioLanguageMode::Fixed,
            fail_closed: true,
            trusted_hosts: Vec::new(),
            timeout_ms: 10_000,
            max_response_bytes: 4 * 1024 * 1024,
            max_concurrency: 8,
        }),
    )
    .expect("Presidio state should register");

    let gateway_token = "standard-presidio-gateway-token";
    let sensitive_input = "standard-user@example.com";
    let response = reqwest::blocking::Client::new()
        .post(format!(
            "http://{}/v1/responses?token=standard-query-secret",
            proxy.listen_addr
        ))
        .bearer_auth(gateway_token)
        .json(&serde_json::json!({"model":"gpt-5","input":sensitive_input}))
        .send()
        .expect("proxy request should be sent");
    assert_eq!(response.status().as_u16(), 502);
    assert_eq!(
        response.text().expect("response body should be text"),
        "gateway PII redaction failed"
    );

    let runtime_log = fs::read_to_string(&proxy.log_path).expect("runtime log should be readable");
    assert!(runtime_log.contains("presidio_redaction_error"));
    assert!(runtime_log.contains("reason=presidio_redaction_failed"));
    assert!(runtime_log.contains("presidio_redaction_failed"));
    assert!(!runtime_log.contains(gateway_token));
    assert!(!runtime_log.contains(sensitive_input));
    assert!(!runtime_log.contains("standard-presidio-secret"));
    assert!(!runtime_log.contains("standard-query-secret"));
    assert!(!runtime_log.contains("127.0.0.1:9"));
}

#[test]
fn runtime_proxy_oversized_request_body_uses_generic_response_and_redacted_log() {
    let _limit = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_MAX_REQUEST_BODY_BYTES", "64");
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let temp_dir = TestDir::new();
    let home = temp_dir.path.join("homes/primary");
    write_auth_json(&home.join("auth.json"), "primary-account");
    let paths = AppPaths {
        state_file: temp_dir.path.join("state.json"),
        managed_profiles_root: temp_dir.path.join("homes"),
        shared_codex_root: temp_dir.path.join("shared-codex"),
        legacy_shared_codex_root: temp_dir.path.join("shared"),
        root: temp_dir.path.clone(),
    };
    let state = AppState::default();
    let proxy = start_runtime_rotation_proxy_with_options(RuntimeRotationProxyStartOptions {
        paths: &paths,
        state: &state,
        current_profile: "primary",
        upstream_base_url: backend.base_url(),
        include_code_review: false,
        upstream_no_proxy: false,
        auto_redeem: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        preferred_listen_addr: None,
    })
    .expect("runtime proxy should start");

    let bearer_token = "standard-body-limit-token";
    let oversized_input = "standard-sensitive-body-marker".repeat(16);
    let response = reqwest::blocking::Client::new()
        .post(format!("http://{}/v1/responses", proxy.listen_addr))
        .bearer_auth(bearer_token)
        .json(&serde_json::json!({"model":"gpt-5","input":oversized_input}))
        .send()
        .expect("oversized proxy request should be sent");
    assert_eq!(response.status().as_u16(), 413);
    assert_eq!(
        response.text().expect("response body should be text"),
        "proxied request body is too large"
    );

    let runtime_log = fs::read_to_string(&proxy.log_path).expect("runtime log should be readable");
    assert!(runtime_log.contains("request_body_too_large"));
    assert!(!runtime_log.contains(bearer_token));
    assert!(!runtime_log.contains(&oversized_input));
    assert!(!runtime_log.contains("bytes exceeds"));
}
