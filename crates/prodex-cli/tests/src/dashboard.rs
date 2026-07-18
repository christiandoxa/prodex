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
    assert!(!args.open);
    assert!(!args.fallback_port);
    assert!(!should_default_cli_invocation_to_run(&os_args(&[
        "prodex",
        "dashboard",
    ])));
}

#[test]
fn dashboard_parse_defaults() {
    let command = parse_cli_command_from(["prodex", "dashboard"]).expect("dashboard should parse");
    let Commands::Dashboard(args) = command else {
        panic!("expected dashboard command");
    };

    assert_eq!(args.host, "127.0.0.1");
    assert_eq!(args.port, 8765);
    assert!(args.base_url.is_none());
    assert!(!args.open);
    assert!(!args.fallback_port);
}

#[test]
fn gui_parses_as_desktop_command() {
    let command =
        parse_cli_command_from(["prodex", "gui", "--profile", "main", "--skip-quota-check"])
            .expect("GUI command should parse");
    let Commands::Gui(args) = command else {
        panic!("expected GUI command");
    };

    assert_eq!(args.profile.as_deref(), Some("main"));
    assert!(args.skip_quota_check);
    assert!(!args.no_auto_rotate);
    assert!(!should_default_cli_invocation_to_run(&os_args(&[
        "prodex", "gui",
    ])));
}

#[test]
fn super_gui_stays_on_the_super_runtime_path() {
    for argv in [vec!["prodex", "s", "gui"], vec!["prodex", "super", "gui"]] {
        let command = parse_cli_command_from(argv).expect("Super GUI should parse");
        let Commands::Super(args) = command else {
            panic!("expected Super command");
        };
        assert_eq!(args.codex_args, [OsString::from("gui")]);
    }
}
