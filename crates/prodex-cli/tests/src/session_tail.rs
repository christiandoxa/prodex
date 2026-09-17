use super::*;

const SESSION_ID: &str = "00000000-0000-7000-8000-000000000042";

#[test]
fn s_session_tail_profile_and_no_auto_rotate_are_prodex_flags() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "00000000-0000-7000-8000-000000000042",
        "--profile",
        "main",
        "--no-auto-rotate",
    ])
    .expect("s session command should parse");
    let Commands::Super(mut args) = command else {
        panic!("expected super command");
    };
    args.extract_provider_overrides_from_codex_args().unwrap();
    assert_eq!(args.profile.as_deref(), Some("main"));
    assert!(args.no_auto_rotate);
    assert_eq!(
        args.codex_args,
        os_args(&["00000000-0000-7000-8000-000000000042"])
    );
}

#[test]
fn s_session_tail_profile_does_not_override_existing_profile() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "--profile",
        "main",
        "00000000-0000-7000-8000-000000000042",
        "--profile=tail",
    ])
    .expect("s session command should parse");
    let Commands::Super(mut args) = command else {
        panic!("expected super command");
    };
    args.extract_provider_overrides_from_codex_args().unwrap();
    assert_eq!(args.profile.as_deref(), Some("main"));
    assert_eq!(
        args.codex_args,
        os_args(&["00000000-0000-7000-8000-000000000042"])
    );
}

#[test]
fn s_session_tail_super_launch_flags_are_prodex_flags() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "00000000-0000-7000-8000-000000000042",
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
    args.extract_provider_overrides_from_codex_args().unwrap();
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
        os_args(&["00000000-0000-7000-8000-000000000042"])
    );
}

#[test]
fn s_session_tail_codex_feature_flags_are_extracted_after_target() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "00000000-0000-7000-8000-000000000042",
        "--web-search=live",
        "--rollout-budget-tokens",
        "100000",
        "--rollout-budget-reminders=75000,50000",
        "--rollout-budget-sampling-weight",
        "1.5",
        "--rollout-budget-prefill-weight=0.25",
        "--current-time-reminder",
        "--current-time-reminder-interval=2",
        "--current-time-clock-source",
        "external",
        "--respect-system-proxy",
    ])
    .expect("s session feature tail should parse");
    let Commands::Super(mut args) = command else {
        panic!("expected super command");
    };

    args.extract_provider_overrides_from_codex_args().unwrap();

    assert_eq!(
        args.codex_features.web_search,
        Some(CodexWebSearchMode::Live)
    );
    assert_eq!(args.codex_features.rollout_budget_tokens, Some(100_000));
    assert_eq!(
        args.codex_features.rollout_budget_reminders,
        vec![75_000, 50_000]
    );
    assert_eq!(
        args.codex_features.rollout_budget_sampling_weight,
        Some(1.5)
    );
    assert_eq!(
        args.codex_features.rollout_budget_prefill_weight,
        Some(0.25)
    );
    assert!(args.codex_features.current_time_reminder);
    assert_eq!(args.codex_features.current_time_reminder_interval, Some(2));
    assert_eq!(
        args.codex_features.current_time_clock_source,
        Some(CodexCurrentTimeClockSource::External)
    );
    assert!(args.codex_features.respect_system_proxy);
    assert!(!args.codex_features.no_respect_system_proxy);
    assert_eq!(
        args.codex_args,
        os_args(&["00000000-0000-7000-8000-000000000042"])
    );
}

#[test]
fn s_session_tail_harness_is_extracted_and_never_forwarded_to_codex() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "00000000-0000-7000-8000-000000000042",
        "--provider",
        "deepseek",
        "--harness=minimal",
    ])
    .unwrap();
    let Commands::Super(mut args) = command else {
        panic!("expected super command");
    };

    args.extract_provider_overrides_from_codex_args().unwrap();
    args.validate_urls().unwrap();

    assert_eq!(args.provider, Some(SuperExternalProvider::DeepSeek));
    assert_eq!(
        args.harness,
        Some(prodex_provider_core::HarnessMode::Minimal)
    );
    assert_eq!(
        args.codex_args,
        os_args(&["00000000-0000-7000-8000-000000000042"])
    );
}

#[test]
fn s_provider_alias_accepts_harness_after_alias_position() {
    let command =
        parse_cli_command_from(["prodex", "s", "deepseek", "--harness", "minimal"]).unwrap();
    let Commands::Super(mut args) = command else {
        panic!("expected super command");
    };

    args.extract_provider_overrides_from_codex_args().unwrap();
    args.validate_urls().unwrap();

    assert_eq!(args.provider, Some(SuperExternalProvider::DeepSeek));
    assert_eq!(
        args.harness,
        Some(prodex_provider_core::HarnessMode::Minimal)
    );
    assert!(args.codex_args.is_empty());
}

#[test]
fn s_session_tail_rejects_credential_bearing_urls_without_echoing_them() {
    for argument in [
        "--url=https://user:tail-url-secret-sentinel@example.test/v1",
        "--base-url=https://example.test/backend-api?token=tail-base-secret-sentinel",
    ] {
        let command = parse_cli_command_from([
            "prodex",
            "s",
            "00000000-0000-7000-8000-000000000042",
            argument,
        ])
        .expect("s session command should parse before tail extraction");
        let Commands::Super(mut args) = command else {
            panic!("expected super command");
        };

        let error = args
            .extract_provider_overrides_from_codex_args()
            .unwrap_err();

        assert!(
            error.contains("no credentials, query, or fragment"),
            "{error}"
        );
        assert!(!error.contains("secret-sentinel"), "{error}");
    }
}

#[test]
fn s_session_tail_presidio_conflicts_are_rejected_after_target() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "00000000-0000-7000-8000-000000000042",
        "--presidio",
        "--no-presidio",
    ])
    .unwrap();
    let Commands::Super(mut args) = command else {
        panic!("expected super command");
    };

    args.extract_provider_overrides_from_codex_args().unwrap();

    assert!(args.presidio);
    assert!(args.no_presidio);
    let error = args.validate_urls().unwrap_err();
    assert!(
        error.contains("--presidio conflicts with --no-presidio"),
        "{error}"
    );
    assert_eq!(
        args.codex_args,
        os_args(&["00000000-0000-7000-8000-000000000042"])
    );
}

#[test]
fn s_explicit_resume_presidio_conflicts_are_rejected_after_target() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "--presidio",
        "resume",
        "00000000-0000-7000-8000-000000000042",
        "--no-presidio",
    ])
    .unwrap();
    let Commands::Super(mut args) = command else {
        panic!("expected super command");
    };

    args.extract_provider_overrides_from_codex_args().unwrap();

    assert!(args.presidio);
    assert!(args.no_presidio);
    let error = args.validate_urls().unwrap_err();
    assert!(
        error.contains("--presidio conflicts with --no-presidio"),
        "{error}"
    );
    assert_eq!(
        args.codex_args,
        os_args(&["resume", "00000000-0000-7000-8000-000000000042"])
    );
}

#[test]
fn s_session_tail_boolean_pairs_keep_last_value_for_non_conflicting_flags() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "00000000-0000-7000-8000-000000000042",
        "--auto-rotate",
        "--no-auto-rotate",
        "--auto-rotate",
    ])
    .unwrap();
    let Commands::Super(mut args) = command else {
        panic!("expected super command");
    };

    args.extract_provider_overrides_from_codex_args().unwrap();

    assert!(args.auto_rotate);
    assert!(!args.no_auto_rotate);
}

#[test]
fn s_session_tail_extracts_flags_before_and_after_bare_and_explicit_resume() {
    for (argv, expected_codex_args) in [
        (
            vec!["prodex", "s", "--no-auto-rotate", SESSION_ID, "--dry-run"],
            vec![SESSION_ID],
        ),
        (
            vec![
                "prodex",
                "s",
                "--no-auto-rotate",
                "resume",
                SESSION_ID,
                "--dry-run",
            ],
            vec!["resume", SESSION_ID],
        ),
    ] {
        let Commands::Super(mut args) =
            parse_cli_command_from(argv).expect("resume tail should parse")
        else {
            panic!("expected super command");
        };
        args.extract_provider_overrides_from_codex_args()
            .expect("Super flags should be extracted");
        assert!(args.no_auto_rotate);
        assert!(args.dry_run);
        assert_eq!(args.codex_args, os_args(&expected_codex_args));
    }
}

#[test]
fn s_session_tail_keeps_literal_boundary_and_following_flags_for_codex() {
    let Commands::Super(mut args) = parse_cli_command_from([
        "prodex",
        "s",
        SESSION_ID,
        "--",
        "--dry-run",
        "--provider",
        "gemini",
    ])
    .expect("literal boundary should parse") else {
        panic!("expected super command");
    };

    args.extract_provider_overrides_from_codex_args()
        .expect("literal boundary should stop Super extraction");

    assert!(!args.dry_run);
    assert_eq!(args.provider, None);
    assert_eq!(
        args.codex_args,
        os_args(&[SESSION_ID, "--", "--dry-run", "--provider", "gemini"])
    );
}

#[cfg(unix)]
#[test]
fn s_session_tail_preserves_non_utf8_arguments() {
    use std::os::unix::ffi::OsStringExt;

    let non_utf8 = OsString::from_vec(vec![0xff, 0xfe]);
    let command = parse_cli_command_from(vec![
        OsString::from("prodex"),
        OsString::from("s"),
        OsString::from("00000000-0000-7000-8000-000000000042"),
        non_utf8.clone(),
        OsString::from("--provider=gemini"),
    ])
    .unwrap();
    let Commands::Super(mut args) = command else {
        panic!("expected super command");
    };

    args.extract_provider_overrides_from_codex_args().unwrap();

    assert_eq!(args.provider, Some(SuperExternalProvider::Gemini));
    assert_eq!(
        args.codex_args,
        vec![
            OsString::from("00000000-0000-7000-8000-000000000042"),
            non_utf8,
        ]
    );
}
