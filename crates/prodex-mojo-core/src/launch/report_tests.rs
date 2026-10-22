use super::{
    RuntimeLaunchReportCompletion, RuntimeLaunchReportInput, RuntimeLaunchReportProxy,
    complete_runtime_launch_report, prodex_runtime_launch_report_v1, render_runtime_launch_report,
};

#[test]
fn launch_report_bridge_preserves_full_schema_presence_order_and_empty_entries() {
    let args = vec!["exec".to_string(), String::new()];
    let env = vec![
        ("VISIBLE".to_string(), "first".to_string()),
        ("VISIBLE".to_string(), String::new()),
    ];
    let removed = vec!["OLD".to_string(), "OLD".to_string()];
    let report = render_runtime_launch_report(RuntimeLaunchReportInput {
        flow: "run",
        binary: "codex",
        provider: Some(""),
        model: None,
        proxy: Some(RuntimeLaunchReportProxy {
            address: "[::1]:0",
            mount: "/v1",
            port: 0,
        }),
        args: &args,
        env: &env,
        removed_env: &removed,
    })
    .unwrap();
    assert_eq!(
        report,
        "Prodex dry run: launch diagnostics\nFlow: run\nBinary: codex\nProvider: \nModel: (codex default)\nCODEX_HOME: <CODEX_HOME>\nRuntime proxy: would be enabled with mount /v1\nArgs:\n  exec\n  \nEnv:\n  CODEX_HOME=<CODEX_HOME>\n  VISIBLE=first\n  VISIBLE=\nRemoved env:\n  OLD\n  OLD\nCodex/TUI not started because --dry-run was set.\n"
    );
    let mut metadata = [77_i64; 2];
    let address = metadata.as_mut_ptr() as u64;
    // SAFETY: version rejection precedes all input/output dereferences.
    let status =
        unsafe { prodex_runtime_launch_report_v1(2, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, address) };
    assert_eq!(status, 4);
    assert_eq!(metadata, [77; 2]);
}

#[test]
fn launch_completion_bridge_preserves_optional_empty_text_and_control_bytes() {
    for (configured_profile, presidio_redaction_enabled, extra_report, expected) in [
        (
            false,
            false,
            None,
            "base\0\nProfile: (active/default)\nPresidio redaction: disabled (not started)\n",
        ),
        (
            true,
            true,
            None,
            "base\0\nProfile: <configured>\nPresidio redaction: enabled (would start at launch)\n",
        ),
        (
            false,
            false,
            Some(""),
            "base\0\nProfile: (active/default)\n\nPresidio redaction: disabled (not started)\n",
        ),
        (
            true,
            false,
            Some(" \t雪\n"),
            "base\0\nProfile: <configured>\n \t雪\n\nPresidio redaction: disabled (not started)\n",
        ),
    ] {
        assert_eq!(
            complete_runtime_launch_report(RuntimeLaunchReportCompletion {
                report: "base\0\n",
                configured_profile,
                extra_report,
                presidio_redaction_enabled,
            })
            .unwrap(),
            expected
        );
    }
}
