use super::*;

#[test]
fn s_expose_rewrites_to_expose_command() {
    assert!(!should_default_cli_invocation_to_run(&os_args(&[
        "prodex", "expose",
    ])));

    let command = parse_cli_command_from([
        "prodex",
        "s",
        "expose",
        "--no-tunnel",
        "--cols",
        "120",
        "--rows",
        "40",
    ])
    .expect("super expose alias should parse");
    let Commands::Expose(args) = command else {
        panic!("expected expose command");
    };
    assert!(args.no_tunnel);
    assert_eq!(args.cols, 120);
    assert_eq!(args.rows, 40);
}
