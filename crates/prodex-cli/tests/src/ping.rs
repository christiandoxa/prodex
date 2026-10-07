use super::*;

#[test]
fn ping_openai_command_parses() {
    let command = parse_cli_command_from([
        "prodex",
        "ping",
        "openai",
        "--model",
        "gpt-5.6-luna",
        "--effort",
        "max",
        "--base-url",
        "http://127.0.0.1:9/backend-api",
        "--no-proxy",
        "--json",
    ])
    .expect("ping openai should parse");

    let Commands::Ping(PingCommands::Openai(args)) = command else {
        panic!("expected ping openai command");
    };
    assert_eq!(args.model.as_deref(), Some("gpt-5.6-luna"));
    assert_eq!(args.effort.as_deref(), Some("max"));
    assert_eq!(
        args.base_url.as_deref(),
        Some("http://127.0.0.1:9/backend-api")
    );
    assert!(args.no_proxy);
    assert!(args.json);
}
