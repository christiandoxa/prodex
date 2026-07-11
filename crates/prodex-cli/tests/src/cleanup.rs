use super::*;

#[test]
fn cleanup_parses_explicit_orphan_retention() {
    let command = parse_cli_command_from(["prodex", "cleanup", "--older-than", "7d"])
        .expect("cleanup command should parse");
    let Commands::Cleanup(args) = command else {
        panic!("expected cleanup command");
    };

    assert!(!args.aggressive);
    assert_eq!(
        args.older_than.map(CleanupOlderThan::seconds),
        Some(7 * 24 * 60 * 60)
    );
}

#[test]
fn cleanup_aggressive_conflicts_with_older_than() {
    assert!(
        parse_cli_command_from(["prodex", "cleanup", "--aggressive", "--older-than", "0d"])
            .is_err()
    );
}
