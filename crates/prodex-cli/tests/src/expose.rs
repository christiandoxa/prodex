use super::*;

#[test]
fn s_expose_rewrites_to_expose_command() {
    assert!(!should_default_cli_invocation_to_run(&os_args(&[
        "prodex", "expose",
    ])));

    let command = parse_cli_command_from([
        "prodex", "s", "expose", "--tunnel", "--cols", "120", "--rows", "40",
    ])
    .expect("super expose alias should parse");
    let Commands::Expose(args) = command else {
        panic!("expected expose command");
    };
    assert!(args.tunnel);
    assert!(!args.no_tunnel);
    assert_eq!(args.cols, 120);
    assert_eq!(args.rows, 40);
}

#[test]
fn expose_tunnel_is_opt_in_and_legacy_no_tunnel_is_unambiguous() {
    let Commands::Expose(defaults) =
        parse_cli_command_from(["prodex", "expose"]).expect("expose defaults should parse")
    else {
        panic!("expected expose command");
    };
    assert!(!defaults.tunnel);
    assert!(!defaults.no_tunnel);

    let Commands::Expose(legacy) = parse_cli_command_from(["prodex", "expose", "--no-tunnel"])
        .expect("legacy no-tunnel alias should parse")
    else {
        panic!("expected expose command");
    };
    assert!(!legacy.tunnel);
    assert!(legacy.no_tunnel);

    assert!(parse_cli_command_from(["prodex", "expose", "--tunnel", "--no-tunnel"]).is_err());
    assert!(parse_cli_command_from(["prodex", "expose", "--max-clients", "0"]).is_err());
    assert!(parse_cli_command_from(["prodex", "expose", "--max-clients", "33"]).is_err());
}
