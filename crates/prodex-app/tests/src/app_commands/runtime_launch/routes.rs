use super::*;

#[test]
fn run_launch_route_sends_command_server_subcommands_directly() {
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
        RunLaunchRoute::CodexCommandServerDirectPassthrough
    );
    assert_eq!(
        run_launch_route(&app_args),
        RunLaunchRoute::CodexCommandServerDirectPassthrough
    );
    assert_eq!(
        run_launch_route(&exec_args),
        RunLaunchRoute::CodexCommandServerDirectPassthrough
    );
}

#[test]
fn run_launch_route_preserves_app_server_protocol_args_for_direct_passthrough() {
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
        RunLaunchRoute::CodexCommandServerDirectPassthrough
    );
    assert_eq!(args.codex_args, passthrough);
}

#[test]
fn run_launch_route_preserves_release_0141_command_server_args_for_direct_passthrough() {
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
            RunLaunchRoute::CodexCommandServerDirectPassthrough
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
fn run_launch_route_preserves_mcp_meta_args_for_direct_passthrough() {
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
        RunLaunchRoute::CodexCommandServerDirectPassthrough
    );
    assert_eq!(args.codex_args, passthrough);
}

#[test]
fn command_server_direct_passthrough_uses_selected_profile_home() {
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

    let plan = codex_command_server_direct_passthrough_plan(args).unwrap();

    assert_eq!(plan.codex_home, second_home);
    assert_eq!(
        plan.args,
        vec![
            OsString::from("exec-server"),
            OsString::from("--remote"),
            OsString::from("https://example.invalid/register"),
        ]
    );
}

#[test]
fn command_server_direct_passthrough_rewrites_legacy_profile_v2_for_codex_0134() {
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

    let plan = codex_command_server_direct_passthrough_plan(args).unwrap();

    assert_eq!(
        plan.args,
        vec![
            OsString::from("exec-server"),
            OsString::from("--profile"),
            OsString::from("work"),
            OsString::from("--stdio"),
        ]
    );
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
