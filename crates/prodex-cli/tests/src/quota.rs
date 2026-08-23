use super::*;

#[test]
fn quota_rejects_conflicting_output_and_scope_flags() {
    for args in [
        ["prodex", "quota", "--profile", "main", "--all"].as_slice(),
        ["prodex", "quota", "--raw", "--all"].as_slice(),
        ["prodex", "quota", "--raw", "--detail"].as_slice(),
        ["prodex", "quota", "--raw", "--once"].as_slice(),
        ["prodex", "quota", "--raw", "--provider", "openai"].as_slice(),
        ["prodex", "quota", "--raw", "--auth", "quota-compatible"].as_slice(),
    ] {
        assert!(
            parse_cli_command_from(args.iter().copied()).is_err(),
            "{args:?}"
        );
    }
}

#[test]
fn quota_defaults_to_detailed_pool_and_filters_need_no_all_flag() {
    for (argv, expected_provider, expected_auth) in [
        (["prodex", "quota"].as_slice(), None, None),
        (
            ["prodex", "quota", "--provider", "openai"].as_slice(),
            Some("openai"),
            None,
        ),
        (
            ["prodex", "quota", "--auth", "quota-compatible"].as_slice(),
            None,
            Some("quota-compatible"),
        ),
    ] {
        let Commands::Quota(args) =
            parse_cli_command_from(argv.iter().copied()).expect("quota command should parse")
        else {
            panic!("expected quota command");
        };
        assert!(args.all, "{argv:?}");
        assert!(args.detail, "{argv:?}");
        assert_eq!(args.provider.as_deref(), expected_provider, "{argv:?}");
        assert_eq!(args.auth.as_deref(), expected_auth, "{argv:?}");
    }

    let Commands::Quota(args) =
        parse_cli_command_from(["prodex", "quota", "--all"]).expect("quota command should parse")
    else {
        panic!("expected quota command");
    };
    assert!(args.all);
    assert!(
        !args.detail,
        "explicit --all should preserve the compact view"
    );
}
