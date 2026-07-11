use super::*;

fn artifact() -> SmartContextArtifactRef {
    SmartContextArtifactRef {
        id: "sc:0123456789abcdef".to_string(),
        byte_len: 8192,
        content_hash: "sc:fedcba9876543210".to_string(),
    }
}

#[test]
fn repeat_tool_output_reference_summary_uses_short_ref_without_critical_signals() {
    let summary = smart_context_repeat_tool_output_reference_summary(
        &artifact(),
        &"finished successfully\n".repeat(400),
        Some("call_1"),
    );

    assert_eq!(summary, "psc:0123456789abcdef");
}

#[test]
fn repeat_tool_output_reference_summary_preserves_critical_signal_sample() {
    let text = format!(
        "error: failed to compile crate\n{}",
        "status: still failed after retry\n".repeat(180)
    );
    let summary =
        smart_context_repeat_tool_output_reference_summary(&artifact(), &text, Some("call bad"));

    assert!(summary.contains("psc co same id=call_bad ref=psc:0123456789abcdef"));
    assert!(summary.contains("psc co crit n=181"));
    assert!(summary.contains("sig: error: failed to compile crate"));
    assert!(summary.ends_with("\nref psc:0123456789abcdef"));
}
