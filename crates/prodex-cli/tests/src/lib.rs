use super::*;
use std::ffi::OsString;

fn parse_super_as_caveman(args: &[&str]) -> CavemanArgs {
    let command = parse_cli_command_from(args.iter().copied()).expect("super command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };
    args.into_caveman_args()
}

fn os_args(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

fn assert_same_caveman_args(left: CavemanArgs, right: CavemanArgs) {
    assert_eq!(left.profile, right.profile);
    assert_eq!(left.auto_rotate, right.auto_rotate);
    assert_eq!(left.no_auto_rotate, right.no_auto_rotate);
    assert_eq!(left.skip_quota_check, right.skip_quota_check);
    assert_eq!(left.full_access, right.full_access);
    assert_eq!(left.dry_run, right.dry_run);
    assert_eq!(left.base_url, right.base_url);
    assert_eq!(left.no_proxy, right.no_proxy);
    assert_eq!(left.smart_context, right.smart_context);
    assert_eq!(left.codex_args, right.codex_args);
}

#[test]
fn super_and_s_parse_to_same_default_super_behavior() {
    let super_args = parse_super_as_caveman(&["prodex", "super"]);
    let alias_args = parse_super_as_caveman(&["prodex", "s"]);

    assert_same_caveman_args(super_args, alias_args);
}

#[test]
fn super_and_s_parse_to_same_super_behavior_with_options() {
    let super_args = parse_super_as_caveman(&[
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
        "--mem-full",
        "exec",
        "review",
        "--dangerously-bypass-approvals-and-sandbox",
    ]);
    let alias_args = parse_super_as_caveman(&[
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
        "--mem-full",
        "exec",
        "review",
        "--dangerously-bypass-approvals-and-sandbox",
    ]);

    assert_same_caveman_args(super_args, alias_args);
}

#[test]
fn super_mem_super_slim_expands_to_super_slim_mem_prefix() {
    let args = parse_super_as_caveman(&["prodex", "super", "--mem-super-slim", "exec", "review"]);

    assert_eq!(
        args.codex_args,
        vec![
            OsString::from("mem-super-slim"),
            OsString::from("exec"),
            OsString::from("review")
        ]
    );
}

#[test]
fn super_default_keeps_slim_mem_prefix() {
    let args = parse_super_as_caveman(&["prodex", "super", "exec", "review"]);

    assert_eq!(
        args.codex_args,
        vec![
            OsString::from("mem"),
            OsString::from("exec"),
            OsString::from("review")
        ]
    );
}

#[test]
fn super_rejects_conflicting_mem_schema_flags() {
    assert!(
        parse_cli_command_from([
            "prodex",
            "super",
            "--mem-full",
            "--mem-super-slim",
            "exec",
            "review",
        ])
        .is_err()
    );
}

#[test]
fn super_and_s_enable_smart_context_autopilot() {
    assert!(parse_super_as_caveman(&["prodex", "super"]).smart_context);
    assert!(parse_super_as_caveman(&["prodex", "s"]).smart_context);
}

#[test]
fn super_url_sets_runtime_base_url_for_local_rewrite_proxy() {
    let args = parse_super_as_caveman(&["prodex", "super", "--url", "http://127.0.0.1:8131"]);

    assert_eq!(args.base_url.as_deref(), Some("http://127.0.0.1:8131/v1"));
}

#[test]
fn caveman_command_keeps_smart_context_autopilot_disabled() {
    let command = parse_cli_command_from(["prodex", "caveman", "exec", "hello"])
        .expect("caveman command should parse");
    let Commands::Caveman(args) = command else {
        panic!("expected caveman command");
    };

    assert!(!args.smart_context);
}

#[test]
fn s_is_recognized_as_super_not_default_run_argument() {
    assert!(!should_default_cli_invocation_to_run(&os_args(&[
        "prodex", "super"
    ])));
    assert!(!should_default_cli_invocation_to_run(&os_args(&[
        "prodex", "s"
    ])));

    let command = parse_cli_command_from(["prodex", "s", "exec", "hello"])
        .expect("super alias command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };
    assert_eq!(args.codex_args, os_args(&["exec", "hello"]));
}
