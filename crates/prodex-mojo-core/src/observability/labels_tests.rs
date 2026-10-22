use super::*;

#[test]
fn compiled_log_labels_keep_catalog_aliases_case_and_unicode_fallbacks() {
    for (source, expected) in [
        ("request", "REQUEST"),
        ("mcp", "MCP"),
        ("agent", "AGENT"),
        ("load", "LOAD"),
        ("event", "EVENT"),
        ("ws", "WEBSOCKET"),
        ("turn-context", "MODEL"),
        ("session-context", "SESSION"),
        ("prompt-engineering", "PROMPT"),
        ("tool-output", "TOOL RESULT"),
        ("tool-call:", "TOOL CALL"),
        ("tool-call:é", "TOOL CALL"),
        ("tokens", "TOKENS"),
        ("hook", "HOOK"),
        ("user", "USER"),
        ("assistant", "ASSISTANT"),
        ("reasoning", "REASONING"),
        ("", "stream "),
        ("REQUEST", "stream REQUEST"),
        ("tool-call", "stream tool-call"),
        ("未知", "stream 未知"),
        ("a\u{85}b", "stream a\u{85}b"),
    ] {
        assert_eq!(render_log_event_label(source), Ok(expected.into()));
    }
    let long = "é".repeat(20_000);
    assert_eq!(render_log_event_label(&long), Ok(format!("stream {long}")));
}

#[test]
fn compiled_log_label_abi_rejects_bad_views_flags_and_short_capacity_atomically() {
    let mut output = [73_u8; 64];
    let mut written = 83_i64;
    let mut call = |version, address, length, measuring, capacity| unsafe {
        prodex_mojo_log_event_label_v1(
            version,
            address,
            length,
            measuring,
            output.as_mut_ptr() as u64,
            capacity,
            &mut written as *mut i64 as u64,
        )
    };
    assert_eq!(call(2, 0, 0, 0, 64), 4);
    assert_eq!(call(1, 0, 1, 0, 64), 1);
    assert_eq!(call(1, 0, -1, 0, 64), 1);
    assert_eq!(call(1, "é".as_ptr() as u64, 1, 0, 64), 1);
    assert_eq!(call(1, 0, 0, 2, 64), 1);
    assert_eq!(call(1, 0, 0, 1, 64), 1);
    assert_eq!(call(1, "request".as_ptr() as u64, 7, 0, 1), 2);
    assert_eq!(output, [73; 64]);
    assert_eq!(written, 83);
}

#[test]
fn shared_source_label_copy_keeps_existing_capacity_and_none_contracts() {
    let mut output = [73_u8; 64];
    let mut written = 83_i64;
    let status = unsafe {
        super::super::prodex_mojo_operational_event_source_label_v1(
            1,
            1,
            output.as_mut_ptr() as u64,
            6,
            &mut written as *mut i64 as u64,
        )
    };
    assert_eq!(status, 2);
    assert_eq!(written, 7);
    assert_eq!(output, [73; 64]);
    let status = unsafe {
        super::super::prodex_mojo_operational_event_source_label_v1(
            1,
            0,
            output.as_mut_ptr() as u64,
            0,
            &mut written as *mut i64 as u64,
        )
    };
    assert_eq!(status, 0);
    assert_eq!(written, -1);
    assert_eq!(output, [73; 64]);
}
