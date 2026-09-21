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

#[test]
fn manual_redeem_parses_the_preserved_04294_contract() {
    let command = parse_cli_command_from(["prodex", "redeem", "main"])
        .expect("manual redeem command should parse");
    let Commands::Redeem(args) = command else {
        panic!("expected redeem command");
    };
    assert_eq!(args.profile, "main");
    assert!(!args.yes);
    assert_eq!(args.base_url, None);
    assert!(!args.no_proxy);
}

#[test]
fn manual_redeem_parses_confirmation_and_transport_overrides() {
    let command = parse_cli_command_from([
        "prodex",
        "redeem",
        "main",
        "--yes",
        "--base-url",
        "https://chatgpt.com/backend-api",
        "--no-proxy",
    ])
    .expect("manual redeem command should parse");
    let Commands::Redeem(args) = command else {
        panic!("expected redeem command");
    };
    assert_eq!(args.profile, "main");
    assert!(args.yes);
    assert_eq!(
        args.base_url.as_deref(),
        Some("https://chatgpt.com/backend-api")
    );
    assert!(args.no_proxy);
    assert_eq!(Commands::Redeem(args).process_label(), "redeem");
}

#[test]
fn manual_redeem_is_not_rewritten_to_default_run() {
    let args = os_args(&["prodex", "redeem", "main"]);
    assert!(!should_default_cli_invocation_to_run(&args));
}

#[test]
fn manual_redeem_debug_does_not_emit_base_url_contents() {
    let command = parse_cli_command_from([
        "prodex",
        "redeem",
        "main",
        "--base-url",
        "https://example.invalid/<redacted>",
    ])
    .expect("manual redeem command should parse");
    let Commands::Redeem(args) = command else {
        panic!("expected redeem command");
    };
    let rendered = format!("{args:?}");
    assert!(rendered.contains("profile_configured"));
    assert!(rendered.contains("base_url_configured"));
    assert!(!rendered.contains("example.invalid"));
}
