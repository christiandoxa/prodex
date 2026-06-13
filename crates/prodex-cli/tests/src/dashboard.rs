use super::*;

#[test]
fn dashboard_parse_as_top_level_command() {
    let command = parse_cli_command_from([
        "prodex",
        "dashboard",
        "--host",
        "127.0.0.1",
        "--port",
        "0",
        "--base-url",
        "http://127.0.0.1:2455/backend-api/codex",
    ])
    .expect("dashboard should parse");
    let Commands::Dashboard(args) = command else {
        panic!("expected dashboard command");
    };

    assert_eq!(args.host, "127.0.0.1");
    assert_eq!(args.port, 0);
    assert_eq!(
        args.base_url.as_deref(),
        Some("http://127.0.0.1:2455/backend-api/codex")
    );
    assert!(!should_default_cli_invocation_to_run(&os_args(&[
        "prodex",
        "dashboard",
    ])));
}
