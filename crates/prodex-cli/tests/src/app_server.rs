use super::*;

#[test]
fn codex_app_server_protocol_args_are_exact_run_passthrough() {
    let passthrough = [
        "app-server",
        "experimentalFeature/list",
        "--thread-id",
        "thread_019c9e3d45a07ad0a6eeb194ac2d44f9",
        "--image-detail",
        "high",
        "--payload",
        r#"{"method":"experimentalFeature/list","params":{"thread_id":"thread_019c9e3d45a07ad0a6eeb194ac2d44f9","image":{"detail":"high"}}}"#,
    ];

    assert!(should_default_cli_invocation_to_run(&os_args(&[
        "prodex",
        passthrough[0],
        passthrough[1],
    ])));

    let mut argv = vec!["prodex"];
    argv.extend(passthrough);
    let command = parse_cli_command_from(argv).expect("app-server args should parse as run");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };

    assert_eq!(args.codex_args, os_args(&passthrough));
}

#[test]
fn codex_app_server_account_usage_rpc_is_run_passthrough() {
    let passthrough = [
        "app-server",
        "account/usage/read",
        "--payload",
        r#"{"method":"account/usage/read","params":{}}"#,
    ];

    let mut argv = vec!["prodex"];
    argv.extend(passthrough);
    let command = parse_cli_command_from(argv).expect("app-server usage RPC should parse as run");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };

    assert_eq!(args.codex_args, os_args(&passthrough));
}

#[test]
fn app_server_broker_is_explicit_prodex_command_not_default_passthrough() {
    let command = parse_cli_command_from(["prodex", "app-server-broker", "--json"])
        .expect("app-server-broker should parse");
    let Commands::AppServerBroker(args) = command else {
        panic!("expected app-server-broker command");
    };

    assert!(args.json);
    assert!(!args.experimental_stdio);
    assert!(!args.experimental_stdio_passthrough_preview);
    assert!(!args.experimental_stdio_validate);
    assert!(!args.experimental_stdio_validate_passthrough);
}

#[test]
fn app_server_broker_accepts_passthrough_preview_flag() {
    let command = parse_cli_command_from([
        "prodex",
        "app-server-broker",
        "--experimental-stdio-passthrough-preview",
    ])
    .expect("app-server-broker passthrough preview flag should parse");
    let Commands::AppServerBroker(args) = command else {
        panic!("expected app-server-broker command");
    };

    assert!(!args.json);
    assert!(!args.experimental_stdio);
    assert!(args.experimental_stdio_passthrough_preview);
    assert!(!args.experimental_stdio_validate);
    assert!(!args.experimental_stdio_validate_passthrough);
}

#[test]
fn app_server_broker_accepts_live_flag() {
    let command =
        parse_cli_command_from(["prodex", "app-server-broker", "--experimental-stdio-live"])
            .expect("app-server-broker live flag should parse");
    let Commands::AppServerBroker(args) = command else {
        panic!("expected app-server-broker command");
    };

    assert!(!args.json);
    assert!(!args.experimental_stdio);
    assert!(!args.experimental_stdio_passthrough_preview);
    assert!(!args.experimental_stdio_validate);
    assert!(!args.experimental_stdio_validate_passthrough);
    assert!(args.experimental_stdio_live);
}

#[test]
fn app_server_broker_accepts_validate_flag() {
    let command = parse_cli_command_from([
        "prodex",
        "app-server-broker",
        "--experimental-stdio-validate",
    ])
    .expect("app-server-broker validate flag should parse");
    let Commands::AppServerBroker(args) = command else {
        panic!("expected app-server-broker command");
    };

    assert!(!args.json);
    assert!(!args.experimental_stdio);
    assert!(!args.experimental_stdio_passthrough_preview);
    assert!(args.experimental_stdio_validate);
    assert!(!args.experimental_stdio_validate_passthrough);
}

#[test]
fn app_server_broker_accepts_validate_passthrough_flag() {
    let command = parse_cli_command_from([
        "prodex",
        "app-server-broker",
        "--experimental-stdio-validate-passthrough",
    ])
    .expect("app-server-broker validate passthrough flag should parse");
    let Commands::AppServerBroker(args) = command else {
        panic!("expected app-server-broker command");
    };

    assert!(!args.json);
    assert!(!args.experimental_stdio);
    assert!(!args.experimental_stdio_passthrough_preview);
    assert!(!args.experimental_stdio_validate);
    assert!(args.experimental_stdio_validate_passthrough);
}

#[test]
fn app_server_broker_experimental_stdio_modes_conflict() {
    let err = parse_cli_command_from([
        "prodex",
        "app-server-broker",
        "--experimental-stdio",
        "--experimental-stdio-passthrough-preview",
    ])
    .expect_err("experimental stdio modes should conflict");

    let message = err.to_string();
    assert!(message.contains("--experimental-stdio"));
    assert!(message.contains("--experimental-stdio-passthrough-preview"));
}

#[test]
fn app_server_broker_live_conflicts_with_preview_modes() {
    let err = parse_cli_command_from([
        "prodex",
        "app-server-broker",
        "--experimental-stdio-live",
        "--experimental-stdio",
    ])
    .expect_err("live mode should conflict with legacy preview mode");

    let message = err.to_string();
    assert!(message.contains("--experimental-stdio-live"));
    assert!(message.contains("--experimental-stdio"));
}

#[test]
fn app_server_broker_validate_conflicts_with_preview_modes() {
    let err = parse_cli_command_from([
        "prodex",
        "app-server-broker",
        "--experimental-stdio",
        "--experimental-stdio-validate",
    ])
    .expect_err("validate and preview modes should conflict");

    let message = err.to_string();
    assert!(message.contains("--experimental-stdio"));
    assert!(message.contains("--experimental-stdio-validate"));
}

#[test]
fn app_server_broker_validate_passthrough_conflicts_with_preview_modes() {
    let err = parse_cli_command_from([
        "prodex",
        "app-server-broker",
        "--experimental-stdio-passthrough-preview",
        "--experimental-stdio-validate-passthrough",
    ])
    .expect_err("validate passthrough and preview modes should conflict");

    let message = err.to_string();
    assert!(message.contains("--experimental-stdio-passthrough-preview"));
    assert!(message.contains("--experimental-stdio-validate-passthrough"));
}

#[test]
fn codex_app_server_release_0141_rpcs_remain_exact_run_passthrough() {
    for passthrough in [
        vec![
            "app-server",
            "thread/listChildren",
            "--payload",
            r#"{"method":"thread/listChildren","params":{"parentThreadId":"thread_parent"}}"#,
        ],
        vec![
            "app-server",
            "externalAgent/import",
            "--payload",
            r#"{"method":"externalAgent/import","params":{"externalAgentId":"agent_1"}}"#,
        ],
        vec![
            "app-server",
            "rateLimit/resetCredits/read",
            "--payload",
            r#"{"method":"rateLimit/resetCredits/read","params":{}}"#,
        ],
    ] {
        let mut argv = vec!["prodex"];
        argv.extend(passthrough.iter().copied());
        let command = parse_cli_command_from(argv).expect("app-server RPC should parse as run");
        let Commands::Run(args) = command else {
            panic!("expected run command");
        };

        assert_eq!(args.codex_args, os_args(&passthrough));
    }
}

#[test]
fn codex_exec_hook_trust_flags_are_exact_run_passthrough() {
    let passthrough = ["exec", "--dangerously-bypass-hook-trust", "summarize hooks"];
    let mut argv = vec!["prodex"];
    argv.extend(passthrough);
    let command = parse_cli_command_from(argv).expect("codex exec args should parse as run");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };

    assert_eq!(args.codex_args, os_args(&passthrough));
}

#[test]
fn codex_exec_server_release_0141_remote_args_are_exact_run_passthrough() {
    let passthrough = [
        "exec-server",
        "--listen",
        "ws://127.0.0.1:0",
        "--remote",
        "wss://remote.example/executor",
        "--environment-id",
        "env_123",
        "--name",
        "native-shell",
        "--use-agent-identity-auth",
    ];

    let mut argv = vec!["prodex"];
    argv.extend(passthrough);
    let command = parse_cli_command_from(argv).expect("exec-server args should parse as run");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };

    assert_eq!(args.codex_args, os_args(&passthrough));
}
