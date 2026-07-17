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
    args.extract_provider_overrides_from_codex_args().unwrap();
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
    args.extract_provider_overrides_from_codex_args().unwrap();
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
    args.extract_provider_overrides_from_codex_args().unwrap();
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
fn s_session_tail_harness_is_extracted_and_never_forwarded_to_codex() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "019ef8ae-c7cc-75c3-8575-a8d247ad291b",
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
        os_args(&["019ef8ae-c7cc-75c3-8575-a8d247ad291b"])
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
fn s_session_tail_invalid_non_url_values_stay_for_codex_to_reject() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "019ef8ae-c7cc-75c3-8575-a8d247ad291b",
        "--provider",
        "unknown",
        "--local-context-window",
        "many",
        "--cli=unknown",
    ])
    .expect("s session command should parse");
    let Commands::Super(mut args) = command else {
        panic!("expected super command");
    };
    args.extract_provider_overrides_from_codex_args().unwrap();
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
            "--local-context-window",
            "many",
            "--cli=unknown",
        ])
    );
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
            "019ef8ae-c7cc-75c3-8575-a8d247ad291b",
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
fn s_session_tail_equals_forms_cover_value_options_and_aliases() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "019ef8ae-c7cc-75c3-8575-a8d247ad291b",
        "--provider=anthropic",
        "--harness=native",
        "--api-key=test-key",
        "--model=model-a",
        "--local-model=model-b",
        "--profile=tail",
        "--base-url=https://example.test/backend-api",
        "--url=http://127.0.0.1:8131/v1",
        "--context-window=32000",
        "--local-context-window=33000",
        "--auto-compact-token-limit=24000",
        "--local-auto-compact-token-limit=25000",
        "--cli=agy",
    ])
    .unwrap();
    let Commands::Super(mut args) = command else {
        panic!("expected super command");
    };

    args.extract_provider_overrides_from_codex_args().unwrap();

    assert_eq!(args.provider, Some(SuperExternalProvider::Anthropic));
    assert_eq!(
        args.harness,
        Some(prodex_provider_core::HarnessMode::Native)
    );
    assert_eq!(args.api_key.as_deref(), Some("test-key"));
    assert_eq!(args.local_model.as_deref(), Some("model-b"));
    assert_eq!(args.profile.as_deref(), Some("tail"));
    assert_eq!(
        args.base_url.as_deref(),
        Some("https://example.test/backend-api")
    );
    assert_eq!(args.url.as_deref(), Some("http://127.0.0.1:8131/v1"));
    assert_eq!(args.local_context_window, Some(33_000));
    assert_eq!(args.local_auto_compact_token_limit, Some(25_000));
    assert_eq!(args.cli, Some(SuperCliAgent::Agy));
    assert_eq!(
        args.codex_args,
        os_args(&["019ef8ae-c7cc-75c3-8575-a8d247ad291b"])
    );
}

#[test]
fn s_session_tail_extracts_native_copilot_cli() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "019ef8ae-c7cc-75c3-8575-a8d247ad291b",
        "--provider=copilot",
        "--cli=copilot",
    ])
    .unwrap();
    let Commands::Super(mut args) = command else {
        panic!("expected super command");
    };

    args.extract_provider_overrides_from_codex_args().unwrap();

    assert_eq!(args.provider, Some(SuperExternalProvider::Copilot));
    assert_eq!(args.cli, Some(SuperCliAgent::Copilot));
    assert_eq!(
        args.codex_args,
        os_args(&["019ef8ae-c7cc-75c3-8575-a8d247ad291b"])
    );
}

#[test]
fn s_session_tail_missing_and_invalid_values_keep_exact_forward_order() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "019ef8ae-c7cc-75c3-8575-a8d247ad291b",
        "--unknown-before",
        "value",
        "--provider",
        "unknown",
        "--context-window=many",
        "--auto-compact-token-limit",
        "many",
        "--cli=unknown",
        "--api-key",
    ])
    .unwrap();
    let Commands::Super(mut args) = command else {
        panic!("expected super command");
    };

    args.extract_provider_overrides_from_codex_args().unwrap();

    assert_eq!(
        args.codex_args,
        os_args(&[
            "019ef8ae-c7cc-75c3-8575-a8d247ad291b",
            "--unknown-before",
            "value",
            "--provider",
            "unknown",
            "--context-window=many",
            "--auto-compact-token-limit",
            "many",
            "--cli=unknown",
            "--api-key",
        ])
    );
}

#[test]
fn s_session_tail_boolean_pairs_use_last_value() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "019ef8ae-c7cc-75c3-8575-a8d247ad291b",
        "--auto-rotate",
        "--no-auto-rotate",
        "--presidio",
        "--no-presidio",
        "--presidio",
    ])
    .unwrap();
    let Commands::Super(mut args) = command else {
        panic!("expected super command");
    };

    args.extract_provider_overrides_from_codex_args().unwrap();

    assert!(!args.auto_rotate);
    assert!(args.no_auto_rotate);
    assert!(args.presidio);
    assert!(!args.no_presidio);
    assert_eq!(
        args.codex_args,
        os_args(&["019ef8ae-c7cc-75c3-8575-a8d247ad291b"])
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
        OsString::from("019ef8ae-c7cc-75c3-8575-a8d247ad291b"),
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
            OsString::from("019ef8ae-c7cc-75c3-8575-a8d247ad291b"),
            non_utf8,
        ]
    );
}
