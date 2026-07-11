use super::*;

#[test]
fn ping_openai_command_parses() {
    let command = parse_cli_command_from([
        "prodex",
        "ping",
        "openai",
        "--base-url",
        "http://127.0.0.1:9/backend-api",
        "--no-proxy",
    ])
    .expect("ping openai should parse");

    let Commands::Ping(PingCommands::Openai(args)) = command else {
        panic!("expected ping openai command");
    };
    assert_eq!(
        args.base_url.as_deref(),
        Some("http://127.0.0.1:9/backend-api")
    );
    assert!(args.no_proxy);
}
