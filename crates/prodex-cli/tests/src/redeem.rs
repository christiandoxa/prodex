use super::*;

#[test]
fn run_command_parses_auto_redeem_as_opt_in() {
    let command = parse_cli_command_from(["prodex", "run", "--auto-redeem", "exec", "hello"])
        .expect("run command should parse");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };
    assert!(args.auto_redeem);

    let command = parse_cli_command_from(["prodex", "run", "exec", "hello"]).expect("run");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };
    assert!(!args.auto_redeem);
}
