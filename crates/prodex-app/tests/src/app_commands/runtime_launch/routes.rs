use super::*;

#[test]
fn resolved_gateway_request_constraints_map_defaults_and_strict_policy() {
    let default_config = resolve_gateway_guardrail_config(
        &gateway_args(),
        &prodex_runtime_policy::RuntimePolicyGatewaySettings::default(),
    )
    .unwrap();
    assert!(!default_config.request_constraints.enabled);
    assert_eq!(
        default_config.request_constraints.unknown_context,
        prodex_provider_core::ProviderUnknownContextPolicy::Allow
    );
    assert_eq!(
        default_config.request_constraints.safe_window_tokens,
        prodex_provider_core::PROVIDER_REQUEST_SAFE_WINDOW_TOKENS_DEFAULT
    );
    assert_eq!(
        default_config.request_constraints.oversized_output,
        prodex_provider_core::ProviderOversizedOutputPolicy::Passthrough
    );

    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy.request_constraints.enabled = Some(true);
    policy.request_constraints.unknown_context = Some("reject".to_string());
    policy.request_constraints.safe_window_tokens = Some(65_536);
    policy.request_constraints.oversized_output = Some("reject".to_string());
    let strict_config = resolve_gateway_guardrail_config(&gateway_args(), &policy).unwrap();
    assert!(strict_config.request_constraints.enabled);
    assert_eq!(
        strict_config.request_constraints.unknown_context,
        prodex_provider_core::ProviderUnknownContextPolicy::Reject
    );
    assert_eq!(strict_config.request_constraints.safe_window_tokens, 65_536);
    assert_eq!(
        strict_config.request_constraints.oversized_output,
        prodex_provider_core::ProviderOversizedOutputPolicy::Reject
    );
}

#[test]
fn run_launch_route_sends_command_server_subcommands_through_managed_stdio() {
    let mcp_args = test_run_args(vec![
        OsString::from("mcp-server"),
        OsString::from("--stdio"),
    ]);
    let app_args = test_run_args(vec![
        OsString::from("app-server"),
        OsString::from("--stdio"),
    ]);
    let exec_args = test_run_args(vec![
        OsString::from("exec-server"),
        OsString::from("--stdio"),
    ]);

    assert_eq!(
        run_launch_route(&mcp_args),
        RunLaunchRoute::CodexCommandServerManagedStdio
    );
    assert_eq!(
        run_launch_route(&app_args),
        RunLaunchRoute::CodexCommandServerManagedStdio
    );
    assert_eq!(
        run_launch_route(&exec_args),
        RunLaunchRoute::CodexCommandServerManagedStdio
    );
}

#[test]
fn run_launch_route_preserves_app_server_protocol_args_for_managed_stdio() {
    let passthrough = vec![
        OsString::from("app-server"),
        OsString::from("experimentalFeature/list"),
        OsString::from("--thread-id"),
        OsString::from("thread_019c9e3d45a07ad0a6eeb194ac2d44f9"),
        OsString::from("--image-detail"),
        OsString::from("high"),
        OsString::from("--payload"),
        OsString::from(
            r#"{"method":"experimentalFeature/list","params":{"thread_id":"thread_019c9e3d45a07ad0a6eeb194ac2d44f9","image":{"detail":"high"}}}"#,
        ),
    ];
    let args = test_run_args(passthrough.clone());

    assert_eq!(
        run_launch_route(&args),
        RunLaunchRoute::CodexCommandServerManagedStdio
    );
    assert_eq!(args.codex_args, passthrough);
}

#[test]
fn run_launch_route_preserves_release_0141_command_server_args_for_managed_stdio() {
    for passthrough in [
        vec![
            OsString::from("app-server"),
            OsString::from("thread/listChildren"),
            OsString::from("--payload"),
            OsString::from(
                r#"{"method":"thread/listChildren","params":{"parentThreadId":"thread_parent"}}"#,
            ),
        ],
        vec![
            OsString::from("app-server"),
            OsString::from("externalAgent/import"),
            OsString::from("--payload"),
            OsString::from(
                r#"{"method":"externalAgent/import","params":{"externalAgentId":"agent_1"}}"#,
            ),
        ],
        vec![
            OsString::from("app-server"),
            OsString::from("rateLimit/resetCredits/read"),
            OsString::from("--payload"),
            OsString::from(r#"{"method":"rateLimit/resetCredits/read","params":{}}"#),
        ],
        vec![
            OsString::from("exec-server"),
            OsString::from("--listen"),
            OsString::from("ws://127.0.0.1:0"),
            OsString::from("--remote"),
            OsString::from("wss://remote.example/executor"),
            OsString::from("--environment-id"),
            OsString::from("env_123"),
            OsString::from("--name"),
            OsString::from("native-shell"),
            OsString::from("--use-agent-identity-auth"),
        ],
    ] {
        let args = test_run_args(passthrough.clone());

        assert_eq!(
            run_launch_route(&args),
            RunLaunchRoute::CodexCommandServerManagedStdio
        );
        assert_eq!(args.codex_args, passthrough);
    }
}

#[test]
fn run_launch_route_keeps_codex_exec_hook_trust_flag_managed_and_unmodified() {
    let passthrough = vec![
        OsString::from("exec"),
        OsString::from("--dangerously-bypass-hook-trust"),
        OsString::from("summarize hooks"),
    ];
    let args = test_run_args(passthrough.clone());

    assert_eq!(run_launch_route(&args), RunLaunchRoute::ManagedRuntime);
    assert_eq!(args.codex_args, passthrough);
}

#[test]
fn run_launch_route_preserves_mcp_meta_args_for_managed_stdio() {
    let passthrough = vec![
        OsString::from("mcp-server"),
        OsString::from("--stdio"),
        OsString::from("--method"),
        OsString::from("tools/call"),
        OsString::from("--params"),
        OsString::from(
            r#"{"_meta":{"trace_id":"trace-smoke"},"meta":{"compat":"codex-rust-v0.132.0"},"arguments":{"image":{"detail":"low"}}}"#,
        ),
    ];
    let args = test_run_args(passthrough.clone());

    assert_eq!(
        run_launch_route(&args),
        RunLaunchRoute::CodexCommandServerManagedStdio
    );
    assert_eq!(args.codex_args, passthrough);
}

#[test]
fn command_server_managed_stdio_uses_selected_profile_home() {
    let root = temp_dir("command-server-profile-home");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let main_home = root.join("main-home");
    let second_home = root.join("second-home");
    fs::create_dir_all(&main_home).unwrap();
    fs::create_dir_all(&second_home).unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([
                (
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: false,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home.clone(),
                        managed: false,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                ),
            ]),
            ..AppState::default()
        },
    );

    let mut args = test_run_args(vec![
        OsString::from("exec-server"),
        OsString::from("--remote"),
        OsString::from("https://example.invalid/register"),
    ]);
    args.profile = Some("second".to_string());

    let strategy = RunCommandStrategy::new(args).unwrap();
    let prepared = prepare_codex_command_server_runtime_launch(strategy.runtime_request()).unwrap();
    let plan = strategy
        .build_plan(&prepared, prepared.runtime_proxy.as_ref())
        .unwrap();

    assert_eq!(plan.child.codex_home, second_home);
    assert_eq!(
        plan.child.args,
        vec![
            OsString::from("exec-server"),
            OsString::from("--remote"),
            OsString::from("https://example.invalid/register"),
        ]
    );
}

#[test]
fn command_server_managed_stdio_rewrites_legacy_profile_v2_for_codex_0134() {
    let root = temp_dir("command-server-profile-v2-rewrite");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let main_home = root.join("main-home");
    fs::create_dir_all(&main_home).unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        },
    );

    let args = test_run_args(vec![
        OsString::from("exec-server"),
        OsString::from("--profile-v2"),
        OsString::from("work"),
        OsString::from("--stdio"),
    ]);

    let strategy = RunCommandStrategy::new(args).unwrap();
    let prepared = prepare_codex_command_server_runtime_launch(strategy.runtime_request()).unwrap();
    let plan = strategy
        .build_plan(&prepared, prepared.runtime_proxy.as_ref())
        .unwrap();

    assert_eq!(
        plan.child.args,
        vec![
            OsString::from("exec-server"),
            OsString::from("--profile"),
            OsString::from("work"),
            OsString::from("--stdio"),
        ]
    );
}

#[test]
fn command_server_managed_stdio_routes_model_traffic_through_runtime_proxy() {
    let root = temp_dir("command-server-runtime-proxy");
    let codex_home = root.join("codex-home");
    fs::create_dir_all(&codex_home).unwrap();
    let strategy = RunCommandStrategy::new(test_run_args(vec![
        OsString::from("app-server"),
        OsString::from("--stdio"),
    ]))
    .unwrap();
    let endpoint = RuntimeProxyEndpoint {
        listen_addr: "127.0.0.1:4455".parse().unwrap(),
        openai_mount_path: RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string(),
        local_model_provider_id: None,
        force_http_responses: true,
        realtime_ws_base_url: None,
        realtime_ws_model: None,
        lease_dir: root.join("leases"),
        broker_session_affinity_control: None,
        _lease: None,
        _direct_proxy: None,
        _kiro_connect_proxy: None,
    };
    let prepared = PreparedRuntimeLaunch {
        paths: AppPaths {
            root: root.clone(),
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("shared-codex"),
            legacy_shared_codex_root: root.join("legacy-shared-codex"),
        },
        codex_home,
        managed: false,
        runtime_proxy: None,
    };

    let plan = strategy.build_plan(&prepared, Some(&endpoint)).unwrap();
    let args = plan
        .child
        .args
        .iter()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>();

    assert!(
        args.iter().any(|arg| {
            arg.as_ref() == "chatgpt_base_url=\"http://127.0.0.1:4455/backend-api\""
        })
    );
    assert!(args.iter().any(|arg| {
        arg.as_ref()
            == format!(
                "openai_base_url=\"http://127.0.0.1:4455{}\"",
                RUNTIME_PROXY_OPENAI_MOUNT_PATH
            )
    }));
    assert!(args.iter().any(|arg| {
        arg.as_ref() == "model_providers.prodex-openai-governed-http.supports_websockets=false"
    }));
    assert_eq!(args[args.len() - 2..], ["app-server", "--stdio"]);
    assert!(!args.iter().any(|arg| arg.contains("disable_paste_burst")));
}

#[test]
fn response_governance_forces_runtime_proxy_for_single_profile() {
    let root = temp_dir("response-governance-single-profile-proxy");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let codex_home = root.join("codex-home");
    fs::create_dir_all(&codex_home).unwrap();
    fs::write(
        root.join("policy.toml"),
        "version = 1\n[governance]\nmode = \"enterprise_observe\"\ninspection = \"observe\"\n",
    )
    .unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home,
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        },
    );
    let strategy =
        RunCommandStrategy::new(test_run_args(vec![OsString::from("app-server")])).unwrap();

    let prepared = prepare_runtime_launch_dry_run(strategy.runtime_request()).unwrap();
    let endpoint = prepared
        .runtime_proxy
        .expect("response governance should force a runtime proxy");

    assert!(endpoint.force_http_responses);
}

#[test]
fn run_launch_route_keeps_remote_control_managed() {
    let args = test_run_args(vec![OsString::from("remote-control")]);

    assert_eq!(run_launch_route(&args), RunLaunchRoute::ManagedRuntime);
}

#[test]
fn run_launch_route_keeps_explicit_dry_run_managed_for_command_server() {
    let mut args = test_run_args(vec![OsString::from("mcp-server")]);
    args.dry_run = true;

    assert_eq!(run_launch_route(&args), RunLaunchRoute::ManagedRuntime);
}

#[test]
fn run_launch_route_keeps_passthrough_dry_run_managed_for_command_server() {
    let args = test_run_args(vec![
        OsString::from("--dry-run"),
        OsString::from("mcp-server"),
    ]);

    assert_eq!(run_launch_route(&args), RunLaunchRoute::ManagedRuntime);
}
