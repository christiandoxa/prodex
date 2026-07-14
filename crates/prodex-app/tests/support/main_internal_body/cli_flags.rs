use super::*;

#[test]
fn update_command_is_prodex_self_update() {
    let command = parse_cli_command_from(["prodex", "update"])
        .expect("update command should parse");
    let Commands::Update(_) = command else {
        panic!("expected update command");
    };
    assert!(parse_cli_command_from(["prodex", "update", "--check"]).is_err());
}

#[test]
fn launch_commands_accept_dry_run_as_prodex_flag() {
    let run = parse_cli_command_from(["prodex", "run", "--dry-run", "exec", "hello"])
        .expect("run dry-run should parse");
    let Commands::Run(run_args) = run else {
        panic!("expected run command");
    };
    assert!(run_args.dry_run);
    assert_eq!(
        run_args.codex_args,
        vec![OsString::from("exec"), OsString::from("hello")]
    );

    let caveman = parse_cli_command_from(["prodex", "caveman", "--dry-run", "exec", "hello"])
        .expect("caveman dry-run should parse");
    let Commands::Caveman(caveman_args) = caveman else {
        panic!("expected caveman command");
    };
    assert!(caveman_args.dry_run);
    assert_eq!(
        caveman_args.codex_args,
        vec![OsString::from("exec"), OsString::from("hello")]
    );

    let super_command = parse_cli_command_from(["prodex", "super", "--dry-run", "exec", "hello"])
        .expect("super dry-run should parse");
    let Commands::Super(super_args) = super_command else {
        panic!("expected super command");
    };
    assert!(super_args.dry_run);
    assert_eq!(
        super_args.codex_args,
        vec![OsString::from("exec"), OsString::from("hello")]
    );
}
