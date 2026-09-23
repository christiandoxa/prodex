use super::*;

#[test]
fn s_is_recognized_as_super_not_default_run_argument() {
    assert!(!should_default_cli_invocation_to_run(&os_args(&[
        "prodex", "super",
    ])));
    assert!(!should_default_cli_invocation_to_run(&os_args(&[
        "prodex", "s",
    ])));

    let command = parse_cli_command_from(["prodex", "s", "exec", "hello"])
        .expect("super alias command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };
    assert_eq!(args.codex_args, os_args(&["exec", "hello"]));
}

#[test]
fn retired_browser_ui_commands_stay_absent_while_super_gui_targets_codex_desktop() {
    for command in ["gui", "dashboard"] {
        assert!(
            parse_cli_command_from(["prodex", command]).is_err(),
            "retired top-level {command} command must stay unavailable"
        );
    }

    let command = parse_cli_command_from(["prodex", "s", "gui"])
        .expect("Super GUI should remain a Codex Desktop frontend passthrough");
    let Commands::Super(args) = command else {
        panic!("expected Super command");
    };
    assert_eq!(args.codex_args, [OsString::from("gui")]);
}
