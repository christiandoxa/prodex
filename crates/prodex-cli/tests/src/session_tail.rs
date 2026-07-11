use super::*;

#[test]
fn s_session_tail_profile_and_no_auto_rotate_are_prodex_flags() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "019ef8ae-c7cc-75c3-8575-a8d247ad291b",
        "--profile",
        "main",
        "--no-auto-rotate",
    ])
    .expect("s session command should parse");
    let Commands::Super(mut args) = command else {
        panic!("expected super command");
    };
    args.extract_provider_overrides_from_codex_args();
    assert_eq!(args.profile.as_deref(), Some("main"));
    assert!(args.no_auto_rotate);
    assert_eq!(
        args.codex_args,
        os_args(&["019ef8ae-c7cc-75c3-8575-a8d247ad291b"])
    );
}

#[test]
fn s_session_tail_profile_does_not_override_existing_profile() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "--profile",
        "main",
        "019ef8ae-c7cc-75c3-8575-a8d247ad291b",
        "--profile=tail",
    ])
    .expect("s session command should parse");
    let Commands::Super(mut args) = command else {
        panic!("expected super command");
    };
    args.extract_provider_overrides_from_codex_args();
    assert_eq!(args.profile.as_deref(), Some("main"));
    assert_eq!(
        args.codex_args,
        os_args(&["019ef8ae-c7cc-75c3-8575-a8d247ad291b"])
    );
}

#[test]
fn s_session_tail_super_launch_flags_are_prodex_flags() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "019ef8ae-c7cc-75c3-8575-a8d247ad291b",
        "--skip-quota-check",
        "--dry-run",
        "--auto-redeem",
        "--no-proxy",
        "--base-url",
        "https://chatgpt.test/backend-api",
        "--no-presidio",
        "--no-auto-rotate",
        "--auto-rotate",
    ])
    .expect("s session command should parse");
    let Commands::Super(mut args) = command else {
        panic!("expected super command");
    };
    args.extract_provider_overrides_from_codex_args();
    assert!(args.skip_quota_check);
    assert!(args.dry_run);
    assert!(args.auto_redeem);
    assert!(args.no_proxy);
    assert_eq!(
        args.base_url.as_deref(),
        Some("https://chatgpt.test/backend-api")
    );
    assert!(args.no_presidio);
    assert!(args.auto_rotate);
    assert!(!args.no_auto_rotate);
    assert_eq!(
        args.codex_args,
        os_args(&["019ef8ae-c7cc-75c3-8575-a8d247ad291b"])
    );
}

#[test]
fn s_session_tail_super_local_provider_flags_are_prodex_flags() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "019ef8ae-c7cc-75c3-8575-a8d247ad291b",
        "--url",
        "http://127.0.0.1:8131/v1",
        "--local-model",
        "local-model",
        "--local-context-window",
        "32000",
        "--local-auto-compact-token-limit",
        "24000",
        "--cli",
        "gemini",
    ])
    .expect("s session command should parse");
    let Commands::Super(mut args) = command else {
        panic!("expected super command");
    };
    args.extract_provider_overrides_from_codex_args();
    assert_eq!(args.url.as_deref(), Some("http://127.0.0.1:8131/v1"));
    assert_eq!(args.local_model.as_deref(), Some("local-model"));
    assert_eq!(args.local_context_window, Some(32000));
    assert_eq!(args.local_auto_compact_token_limit, Some(24000));
    assert_eq!(args.cli, Some(SuperCliAgent::Gemini));
    assert_eq!(
        args.codex_args,
        os_args(&["019ef8ae-c7cc-75c3-8575-a8d247ad291b"])
    );
}

#[test]
fn s_session_tail_invalid_super_values_stay_for_codex_to_reject() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "019ef8ae-c7cc-75c3-8575-a8d247ad291b",
        "--provider",
        "unknown",
        "--url=not-a-url",
        "--local-context-window",
        "many",
        "--cli=unknown",
    ])
    .expect("s session command should parse");
    let Commands::Super(mut args) = command else {
        panic!("expected super command");
    };
    args.extract_provider_overrides_from_codex_args();
    assert_eq!(args.provider, None);
    assert_eq!(args.url, None);
    assert_eq!(args.local_context_window, None);
    assert_eq!(args.cli, None);
    assert_eq!(
        args.codex_args,
        os_args(&[
            "019ef8ae-c7cc-75c3-8575-a8d247ad291b",
            "--provider",
            "unknown",
            "--url=not-a-url",
            "--local-context-window",
            "many",
            "--cli=unknown",
        ])
    );
}
