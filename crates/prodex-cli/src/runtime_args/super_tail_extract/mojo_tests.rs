use super::*;

use prodex_mojo_core::launch::scan_super_overrides;

#[test]
fn mojo_override_scan_matches_rust_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--provider",
            "gemini",
            "--cli=agy",
            "--api-key=fake-api-key",
            "--sub-agent-provider=GOOGLE",
            "--sub-agent-model",
            "模型/β",
            "--sub-agent-model-reasoning-effort=xhigh",
            "--sub-agent-url=http://example.com",
            "--sub-agent-max-concurrency=16",
            "--model",
            "vendor/model",
            "--profile=synthetic",
            "--base-url=http://example.com/v1",
            "--url=http://127.0.0.1:3000/v1",
            "--local-context-window=8192",
            "--auto-compact-token-limit",
            "1024",
            "--tool=rtk",
            "--require-tool=ponytail",
            "--web-search=live",
            "--rollout-budget-tokens=64",
            "--rollout-budget-reminders=1,2,3",
            "--rollout-budget-sampling-weight=0.5",
            "--rollout-budget-prefill-weight=0.25",
            "--current-time-reminder-interval=120",
            "--current-time-clock-source=external",
            "--no-auto-rotate",
            "--auto-rotate",
            "--auto-redeem",
            "--skip-quota-check",
            "--dry-run",
            "--no-proxy",
            "--presidio",
            "--no-presidio",
            "--sub-agent",
            "--no-sub-agent",
            "--full-access",
            "--current-time-reminder",
            "--respect-system-proxy",
            "--no-respect-system-proxy",
        ],
        vec![
            "--local-model=fast",
            "--local-context-window",
            "4096",
            "--local-auto-compact-token-limit=512",
        ],
        vec!["--model", "--dry-run"],
        vec!["--model", "--dry-run=false"],
        vec!["--dry-run=false"],
        vec!["--model", "--unrecognized-option"],
        vec!["--model=", "--context-window=invalid"],
        vec!["--", "--dry-run", "--provider=gemini"],
    ];

    for case in cases {
        let args = case.iter().copied().map(OsString::from).collect::<Vec<_>>();
        assert_scan_parity(&args);
    }
    assert_scan_parity(&[OsString::from("--model"), opaque_argument()]);
    assert_scan_parity(&[opaque_argument(), OsString::from("--dry-run")]);
}

fn assert_scan_parity(args: &[OsString]) {
    let views = args
        .iter()
        .map(|argument| argument.to_str())
        .collect::<Vec<_>>();
    let plan = scan_super_overrides(&views).expect("Mojo override scan");
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--" {
            break;
        }
        let rust = scan_override_rust(args, index);
        let mojo = scan_mojo_override(plan[index]);
        assert_eq!(mojo, rust, "argument index {index}");
        match rust {
            Ok(ScanOutcome::Apply { consumed_count, .. }) => index += consumed_count,
            Ok(ScanOutcome::Unknown) => index += 1,
            Err(_) => break,
        }
    }
}

#[cfg(unix)]
fn opaque_argument() -> OsString {
    use std::os::unix::ffi::OsStringExt;
    OsString::from_vec(vec![0xff])
}

#[cfg(windows)]
fn opaque_argument() -> OsString {
    use std::os::windows::ffi::OsStringExt;
    OsString::from_wide(&[0xd800])
}
