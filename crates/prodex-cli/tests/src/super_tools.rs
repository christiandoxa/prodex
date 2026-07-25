use super::*;

#[test]
fn super_and_s_parse_to_same_default_super_behavior() {
    let super_args = parse_super_as_runtime_tools(&["prodex", "super"]);
    let alias_args = parse_super_as_runtime_tools(&["prodex", "s"]);
    assert_same_runtime_tool_args(super_args, alias_args);
}

#[test]
fn super_and_s_parse_to_same_super_behavior_with_options() {
    let super_args = parse_super_as_runtime_tools(&[
        "prodex",
        "super",
        "--profile",
        "main",
        "--no-auto-rotate",
        "--skip-quota-check",
        "--dry-run",
        "--no-proxy",
        "--url",
        "http://127.0.0.1:8131",
        "--model",
        "local-model",
        "--context-window",
        "32000",
        "--auto-compact-token-limit",
        "24000",
        "exec",
        "review",
        "--dangerously-bypass-approvals-and-sandbox",
    ]);
    let alias_args = parse_super_as_runtime_tools(&[
        "prodex",
        "s",
        "--profile",
        "main",
        "--no-auto-rotate",
        "--skip-quota-check",
        "--dry-run",
        "--no-proxy",
        "--url",
        "http://127.0.0.1:8131",
        "--model",
        "local-model",
        "--context-window",
        "32000",
        "--auto-compact-token-limit",
        "24000",
        "exec",
        "review",
        "--dangerously-bypass-approvals-and-sandbox",
    ]);
    assert_same_runtime_tool_args(super_args, alias_args);
}

#[test]
fn s_profile_shortcut_selects_profile() {
    let command = parse_cli_command_from(["prodex", "s", "--profile", "nama_profile"])
        .expect("s profile command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };
    assert_eq!(args.profile.as_deref(), Some("nama_profile"));
}

#[test]
fn super_defaults_to_yolo_access_with_minimal_super_prefixes() {
    let args = parse_super_as_runtime_tools(&["prodex", "super", "exec", "review"]);
    assert!(args.full_access);
    assert!(args.smart_context);
    let tools = args.selected_tool_set();
    assert!(tools.contains(prodex_optional_tools::OptionalToolId::Caveman));
    assert!(tools.contains(prodex_optional_tools::OptionalToolId::Rtk));
    assert!(tools.contains(prodex_optional_tools::OptionalToolId::Ponytail));
    assert_eq!(args.codex_args, os_args(&["exec", "review"]));
}

#[test]
fn super_full_access_flag_remains_compatible_with_yolo_default() {
    for args in [
        parse_super_as_runtime_tools(&["prodex", "super", "exec", "review"]),
        parse_super_as_runtime_tools(&["prodex", "super", "--full-access", "exec", "review"]),
    ] {
        assert!(args.full_access);
    }
}

#[test]
fn super_and_s_enable_smart_context_autopilot() {
    assert!(parse_super_as_runtime_tools(&["prodex", "super"]).smart_context);
    assert!(parse_super_as_runtime_tools(&["prodex", "s"]).smart_context);
}

#[test]
fn super_and_s_enable_typed_optional_tool_set() {
    for args in [
        parse_super_as_runtime_tools(&["prodex", "super"]),
        parse_super_as_runtime_tools(&["prodex", "s"]),
    ] {
        assert!(
            args.selected_tool_set()
                .contains(prodex_optional_tools::OptionalToolId::CodebaseMemoryMcp)
        );
    }
}

#[test]
fn super_omits_presidio_unless_explicitly_enabled() {
    let command = parse_cli_command_from(["prodex", "super", "exec", "hello"])
        .expect("super command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };
    assert_eq!(
        args.into_runtime_tool_args().codex_args,
        os_args(&["exec", "hello"])
    );
}

#[test]
fn super_includes_presidio_prefix_when_opted_in() {
    let command = parse_cli_command_from(["prodex", "super", "exec", "hello"])
        .expect("super command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };
    assert_eq!(
        args.into_runtime_tool_args_with_presidio(true).codex_args,
        os_args(&["exec", "hello"])
    );
}

#[test]
fn super_presidio_flag_enables_presidio_without_prompt() {
    let args = parse_super_as_runtime_tools_with_presidio_preference(&[
        "prodex",
        "super",
        "--presidio",
        "exec",
        "hello",
    ]);
    assert!(args.presidio);
    assert_eq!(args.codex_args, os_args(&["exec", "hello"]));
}

#[test]
fn s_presidio_flag_matches_super_presidio_flag() {
    let super_args = parse_super_as_runtime_tools_with_presidio_preference(&[
        "prodex",
        "super",
        "--presidio",
        "exec",
        "hello",
    ]);
    let alias_args = parse_super_as_runtime_tools_with_presidio_preference(&[
        "prodex",
        "s",
        "--presidio",
        "exec",
        "hello",
    ]);
    assert_same_runtime_tool_args(super_args, alias_args);
}

#[test]
fn super_no_presidio_flag_disables_presidio_without_prompt() {
    let args = parse_super_as_runtime_tools_with_presidio_preference(&[
        "prodex",
        "super",
        "--no-presidio",
        "exec",
        "hello",
    ]);
    assert_eq!(args.codex_args, os_args(&["exec", "hello"]));
}

#[test]
fn super_leading_optional_prefixes_are_consumed_before_passthrough() {
    let args =
        parse_super_as_runtime_tools(&["prodex", "s", "ponytail", "presidio", "exec", "hello"]);
    assert!(args.presidio);
    assert_eq!(args.codex_args, os_args(&["exec", "hello"]));
}

#[test]
fn super_presidio_flags_conflict() {
    assert!(
        parse_cli_command_from(["prodex", "super", "--presidio", "--no-presidio", "exec"]).is_err()
    );
}
