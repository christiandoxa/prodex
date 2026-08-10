use super::*;

#[test]
fn command_runtime_and_process_labels_follow_canonical_parsing() {
    for args in [
        &["prodex"][..],
        &["prodex", "fix this bug"][..],
        &["prodex", "run"][..],
        &["prodex", "s"][..],
        &["prodex", "super", "expose"][..],
        &["prodex", "__runtime-broker"][..],
    ] {
        let command = parse_cli_command_from(args.iter().copied())
            .unwrap_or_else(|err| panic!("{args:?} should parse: {err}"));
        assert!(command.launches_runtime(), "{args:?}");
    }

    let command =
        parse_cli_command_from(["prodex", "super", "doctor"]).expect("super doctor should parse");
    assert!(!command.launches_runtime());
    assert_eq!(command.process_label(), "capability");

    let command = parse_cli_command_from(["prodex", "info"]).expect("info should parse");
    assert!(!command.launches_runtime());
    assert_eq!(command.process_label(), "info");
}

#[test]
fn super_positioned_aliases_survive_no_presidio() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "--no-presidio",
        "doctor",
        "--json",
        "--strict",
    ])
    .expect("positioned Super doctor should parse");
    assert!(matches!(
        command,
        Commands::Capability(CapabilityCommands::SuperDoctor(SuperDoctorArgs {
            json: true,
            strict: true,
            presidio: false,
        }))
    ));

    let command = parse_cli_command_from(["prodex", "s", "--no-presidio", "expose", "--tunnel"])
        .expect("positioned Super expose should parse");
    assert!(matches!(
        command,
        Commands::Expose(ExposeArgs { tunnel: true, .. })
    ));

    let Commands::Super(mut args) = parse_cli_command_from([
        "prodex",
        "s",
        "--no-presidio",
        "--dry-run",
        "gemini",
        "--api-key",
        "test-key",
        "exec",
        "review",
    ])
    .expect("positioned Super provider alias should parse") else {
        panic!("expected Super command");
    };
    args.extract_super_overrides_from_codex_args()
        .expect("Super tail should extract");
    assert_eq!(args.provider, Some(SuperExternalProvider::Gemini));
    assert_eq!(args.api_key.as_deref(), Some("test-key"));
    assert!(args.dry_run);
    assert_eq!(args.codex_args, os_args(&["exec", "review"]));

    let Commands::Super(args) = parse_cli_command_from(["prodex", "s", "--", "doctor"])
        .expect("literal Super argument should parse")
    else {
        panic!("expected Super command");
    };
    assert_eq!(args.codex_args, os_args(&["--", "doctor"]));
}
