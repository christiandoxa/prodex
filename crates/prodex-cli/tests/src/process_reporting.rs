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
