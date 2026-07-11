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
