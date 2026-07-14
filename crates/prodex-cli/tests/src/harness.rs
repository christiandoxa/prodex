use super::*;

#[test]
fn harness_parses_only_for_local_super_bridges_and_gateway() {
    let Commands::Super(args) = parse_cli_command_from([
        "prodex",
        "s",
        "--provider",
        "anthropic",
        "--harness",
        "minimal",
    ])
    .unwrap() else {
        panic!("expected super command");
    };
    assert_eq!(
        args.harness,
        Some(prodex_provider_core::HarnessMode::Minimal)
    );

    let Commands::Gateway(args) =
        parse_cli_command_from(["prodex", "gateway", "--harness=native"]).unwrap()
    else {
        panic!("expected gateway command");
    };
    assert_eq!(
        args.harness,
        Some(prodex_provider_core::HarnessMode::Native)
    );

    assert!(parse_cli_command_from(["prodex", "s", "--harness", "minimal"]).is_err());
    assert!(parse_cli_command_from(["prodex", "gateway", "--harness", "unsupported"]).is_err());
}

#[test]
fn harness_is_rejected_for_native_external_agent_clis() {
    let Commands::Super(args) = parse_cli_command_from([
        "prodex",
        "s",
        "--provider",
        "gemini",
        "--cli",
        "gemini",
        "--harness",
        "minimal",
    ])
    .unwrap() else {
        panic!("expected super command");
    };
    assert!(
        args.validate_urls()
            .unwrap_err()
            .contains("Codex CLI bridge")
    );
}

#[test]
fn harness_help_lists_the_v1_modes() {
    let error = parse_cli_command_from(["prodex", "s", "--help"]).unwrap_err();
    let help = error.to_string();
    assert!(help.contains("--harness <auto|native|minimal>"), "{help}");

    let error = parse_cli_command_from(["prodex", "gateway", "--help"]).unwrap_err();
    let help = error.to_string();
    assert!(help.contains("--harness <auto|native|minimal>"), "{help}");
}
