use super::*;

#[test]
fn status_defaults_to_live_one_second_sampling() {
    let command = parse_cli_command_from(["prodex", "status"]).expect("status should parse");
    let Commands::Status(args) = command else {
        panic!("expected status command");
    };

    assert!(!args.once);
    assert_eq!(args.interval, 1);
}

#[test]
fn status_accepts_once_and_bounded_interval() {
    let command = parse_cli_command_from(["prodex", "status", "--once", "--interval", "5"])
        .expect("status options should parse");
    let Commands::Status(args) = command else {
        panic!("expected status command");
    };

    assert!(args.once);
    assert_eq!(args.interval, 5);
    assert!(parse_cli_command_from(["prodex", "status", "--interval", "0"]).is_err());
}
