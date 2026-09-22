use super::*;
use crate::args_oracle as oracle;

fn compare(args: &[OsString]) {
    assert_eq!(
        normalize_run_codex_args(args),
        oracle::normalize_run_codex_args(args),
        "normalize: {args:?}"
    );
    assert_eq!(
        normalize_codex_profile_args(args),
        oracle::normalize_codex_profile_args(args),
        "profile: {args:?}"
    );
    assert_eq!(
        scope_codex_exec_config_args(args),
        oracle::scope_codex_exec_config_args(args),
        "scope: {args:?}"
    );
    assert_eq!(
        extract_prodex_dry_run_flag(args),
        oracle::extract_prodex_dry_run_flag(args),
        "dry-run: {args:?}"
    );
    assert_eq!(
        prodex_dry_run_requested(args),
        oracle::prodex_dry_run_requested(args),
        "dry-run requested: {args:?}"
    );
    assert_eq!(
        is_review_invocation(args),
        oracle::is_review_invocation(args),
        "review: {args:?}"
    );
    assert_eq!(
        is_codex_exec_invocation(args),
        oracle::is_codex_exec_invocation(args),
        "exec: {args:?}"
    );
    assert_eq!(
        codex_resume_requested(args),
        oracle::codex_resume_requested(args),
        "resume: {args:?}"
    );
    assert_eq!(
        codex_resume_session_id(args),
        oracle::codex_resume_session_id(args),
        "session: {args:?}"
    );
    assert_eq!(
        runtime_launch_cli_model(args),
        oracle::runtime_launch_cli_model(args),
        "model: {args:?}"
    );
    let session = "00000000-0000-0000-0000-000000000001";
    assert_eq!(
        retarget_codex_tui_resume_args(args, session),
        oracle::retarget_codex_tui_resume_args(args, session),
        "retarget tui: {args:?}"
    );
    assert_eq!(
        retarget_codex_exec_resume_args(args, session),
        crate::args_resume::retarget_codex_exec_resume_args_rust(args, session),
        "retarget exec: {args:?}"
    );
    for full_access in [false, true] {
        assert_eq!(
            prepare_codex_launch_args(args, full_access),
            oracle::prepare_codex_launch_args(args, full_access),
            "prepare {full_access}: {args:?}"
        );
    }
    let address = "127.0.0.1:12345".parse().unwrap();
    assert_eq!(
        runtime_proxy_local_model_provider_codex_args(address, "/test/", "test", args),
        oracle::runtime_proxy_local_model_provider_codex_args(address, "/test/", "test", args),
        "override: {args:?}"
    );
    let endpoint = RuntimeProxyCodexEndpoint {
        listen_addr: address,
        openai_mount_path: "/v1",
        force_http_responses: true,
        local_model_provider_id: None,
        realtime_ws_base_url: Some("wss://example.com/realtime"),
        realtime_ws_model: Some("test-model"),
    };
    assert_eq!(
        runtime_proxy_codex_passthrough_args(Some(endpoint), args),
        oracle::runtime_proxy_codex_passthrough_args(Some(endpoint), args),
        "governed: {args:?}"
    );
}

#[test]
fn launch_plans_use_compiled_mojo() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) };
    compare(&[]);
    compare(&[
        "--profile-v2".into(),
        "test".into(),
        "00000000-0000-0000-0000-000000000001".into(),
    ]);
    compare(&[
        "--thread-source".into(),
        "review".into(),
        "00000000-0000-0000-0000-000000000001".into(),
    ]);
}

#[test]
fn launch_plans_match_seeded_rust_oracle() {
    let tokens = [
        "",
        "exec",
        "resume",
        "review",
        "fork",
        "--",
        "-",
        "--last",
        "--all",
        "-c",
        "--config",
        "--config=key=value",
        "--config=invalid",
        "-ckey=value",
        "--profile-v2",
        "--profile-v2=test",
        "--profile",
        "--profile=test",
        "--thread-source",
        "--thread-source=review",
        "--dry-run",
        "--full-access",
        "--model",
        "-m",
        "--model=test",
        "-m=test",
        "-m==test",
        "--model=",
        "-m=",
        "-P",
        "--unknown",
        "--image",
        "--local-provider",
        "--output-schema",
        "hello",
        "00000000-0000-0000-0000-000000000001",
        "ABCDEF12-ABCD-ABCD-ABCD-ABCDEF123456",
        "00000000-0000-0000-0000-00000000000x",
        "🦀",
        "こんにちは",
        "\u{001c}",
        "\u{0085}",
        "\u{2007}",
        " \t",
        "--model=\u{001c}",
        "--model=\u{2003}",
        "model_providers.test.base_url=\"https://example.com\"",
        "--enable",
        "--color",
    ];
    let mut seed = 0x89af_326b_1739_c5d1_u64;
    for _ in 0..5_000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let length = (seed >> 32) as usize % 33;
        let args = (0..length)
            .map(|_| {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                OsString::from(tokens[(seed >> 32) as usize % tokens.len()])
            })
            .collect::<Vec<_>>();
        compare(&args);
    }
}

#[test]
fn launch_plans_preserve_long_unicode_and_separator_inputs() {
    let long = "💡e\u{0301}".repeat(8192);
    compare(&[
        "-m".into(),
        long.into(),
        "--".into(),
        "--full-access".into(),
        "--profile-v2=x".into(),
    ]);
    let many = vec![OsString::from("--json"); 4096];
    compare(&many);
}

#[cfg(unix)]
#[test]
fn launch_plans_preserve_non_utf8_os_arguments() {
    use std::os::unix::ffi::OsStringExt;
    let opaque = OsString::from_vec(vec![b'-', b'm', 0xff, 0x80]);
    for prefix in [
        vec![],
        vec!["-m".into()],
        vec!["exec".into(), "resume".into()],
        vec!["--".into()],
    ] {
        let mut args = prefix;
        args.extend([opaque.clone(), "--profile-v2=x".into(), "review".into()]);
        compare(&args);
    }
}

#[cfg(windows)]
#[test]
fn launch_plans_preserve_unpaired_utf16_os_arguments() {
    use std::os::windows::ffi::OsStringExt;
    compare(&[
        "-m".into(),
        OsString::from_wide(&[0xd800, 0x61]),
        "resume".into(),
    ]);
}

#[test]
fn config_overrides_preserve_native_values_and_provider_precedence() {
    let cases = [
        vec![
            "exec",
            "--config",
            " model_providers.test.base_url =\"https://example.com\"",
            "resume",
            "--last",
        ],
        vec![
            "-cmodel_providers.test.base_url=old",
            "--config=model_providers.test.base_url=second",
            "hello",
        ],
        vec![
            "--config=\u{2003}experimental_realtime_ws_model\u{00a0}=old",
            "exec",
            "--",
            "--config=experimental_realtime_ws_model=literal",
        ],
        vec![
            "--config=\u{001c}experimental_realtime_ws_model=keep",
            "exec",
        ],
        vec!["-c", "--", "--config=experimental_realtime_ws_base_url=old"],
        vec![
            "--config",
            "--config=experimental_realtime_ws_model=not-an-assignment",
        ],
    ];
    for case in cases {
        compare(&case.into_iter().map(OsString::from).collect::<Vec<_>>());
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        compare(&[
            "-c".into(),
            OsString::from_vec(vec![0xff, b'=']),
            "--config=experimental_realtime_ws_model=old".into(),
        ]);
    }
}
