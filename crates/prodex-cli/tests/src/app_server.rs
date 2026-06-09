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
