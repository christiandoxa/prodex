use super::*;
use std::ffi::OsString;

fn os_args(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

#[test]
fn redeem_command_parses_profile_and_base_url() {
    assert!(!should_default_cli_invocation_to_run(&os_args(&[
        "prodex", "redeem", "main",
    ])));
    let command = parse_cli_command_from([
        "prodex",
        "redeem",
        "iandoxa_yahoo.com",
        "--base-url",
        "http://127.0.0.1:8080",
        "--no-proxy",
        "--yes",
    ])
    .expect("redeem command should parse");
    let Commands::Redeem(args) = command else {
        panic!("expected redeem command");
    };
    assert_eq!(args.profile, "iandoxa_yahoo.com");
    assert_eq!(args.base_url.as_deref(), Some("http://127.0.0.1:8080"));
    assert!(args.no_proxy);
    assert!(args.yes);
}

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
