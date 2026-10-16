use super::*;

#[test]
fn redaction_cli_display_policy_selects_host_escaping_via_mojo() {
    for (value, expected) in [
        ("", RedactionDisplayTextMode::Empty),
        ("alpha_012-./:=<>", RedactionDisplayTextMode::Unquoted),
        ("A-Z", RedactionDisplayTextMode::Unquoted),
        ("a b", RedactionDisplayTextMode::DebugQuoted),
        ("a\tb", RedactionDisplayTextMode::DebugQuoted),
        ("'secret'", RedactionDisplayTextMode::DebugQuoted),
        ("\"x\"", RedactionDisplayTextMode::DebugQuoted),
        ("a@example.test", RedactionDisplayTextMode::DebugQuoted),
        ("東京", RedactionDisplayTextMode::DebugQuoted),
        ("🧭", RedactionDisplayTextMode::DebugQuoted),
    ] {
        assert_eq!(
            redaction_display_text_mode(value).unwrap(),
            expected,
            "{value:?}"
        );
    }
}

#[test]
fn redaction_cli_display_policy_strict_abi_rejects_bad_unicode_and_version() {
    let input = b"safe";
    let pointer = input.as_ptr() as usize as u64;
    unsafe {
        assert_eq!(prodex_redaction_display_text_mode_v1(2, pointer, 4), -4);
        assert_eq!(prodex_redaction_display_text_mode_v1(1, 0, 4), -1);
        assert_eq!(prodex_redaction_display_text_mode_v1(1, pointer, -1), -1);
        assert_eq!(prodex_redaction_display_text_mode_v1(1, pointer, 4), 1);
        assert_eq!(prodex_redaction_display_text_mode_v1(1, pointer, 0), 0);
    }
    let invalid = [0xff_u8];
    assert_eq!(
        unsafe { prodex_redaction_display_text_mode_v1(1, invalid.as_ptr() as usize as u64, 1) },
        -1
    );
}

#[test]
fn redaction_cli_flag_next_argument_policy_is_compiled_mojo() {
    for (value, expected) in [
        ("", false),
        ("-", false),
        ("--", false),
        ("--model", false),
        ("--api-key", true),
        ("---authorization", true),
        ("--access_token", true),
        ("--secret-東京", true),
        ("--api-key=value", false),
        ("--api-key=", false),
        ("api-key", false),
        ("-🙂", false),
        ("---API-KEY", true),
    ] {
        assert_eq!(
            redaction_cli_redact_next(value).unwrap(),
            expected,
            "{value:?}"
        );
    }
}

#[test]
fn redaction_cli_flag_mojo_abi_rejects_bad_version_and_utf8() {
    let value = b"--api-key";
    let address = value.as_ptr() as usize as u64;
    unsafe {
        assert_eq!(prodex_redaction_cli_redact_next_v1(2, address, 9), -4);
        assert_eq!(prodex_redaction_cli_redact_next_v1(1, 0, 9), -1);
        assert_eq!(prodex_redaction_cli_redact_next_v1(1, address, -1), -1);
        assert_eq!(prodex_redaction_cli_redact_next_v1(1, address, 9), 1);
    }
    let invalid = [b'-', b'-', 0xff];
    assert_eq!(
        unsafe { prodex_redaction_cli_redact_next_v1(1, invalid.as_ptr() as usize as u64, 3) },
        -1,
    );
}

#[test]
fn redaction_expose_flag_policy_preserves_inline_and_short_alias_rules() {
    for (value, expected) in [
        ("--api-key", true),
        ("--token=inline-secret", true),
        ("--client-secret", true),
        ("-H", true),
        ("--model", false),
        ("--api-key=", true),
        ("api-key", true),
    ] {
        assert_eq!(
            redaction_cli_flag_sensitive(value).unwrap(),
            expected,
            "{value:?}"
        );
    }
}

#[test]
fn redaction_expose_flag_policy_rejects_bad_abi_inputs() {
    let value = b"--api-key";
    let address = value.as_ptr() as usize as u64;
    unsafe {
        assert_eq!(prodex_redaction_cli_flag_sensitive_v1(2, address, 9), -4);
        assert_eq!(prodex_redaction_cli_flag_sensitive_v1(1, 0, 9), -1);
        assert_eq!(prodex_redaction_cli_flag_sensitive_v1(1, address, -1), -1);
        assert_eq!(prodex_redaction_cli_flag_sensitive_v1(1, address, 9), 1);
    }
}

#[test]
fn redaction_text_snippet_mojo_normalizes_unicode_and_clips_scalars() {
    for (input, max_chars, expected) in [
        ("", 0, "-"),
        (" \t\u{a0}\u{3000}", 0, "-"),
        (" \u{2003}東京 \t🙂  done\u{3000}", 100, "東京 🙂 done"),
        ("a   b\nc", 5, "a b c"),
        ("a   b\nc", 4, "a b ..."),
        ("a   b\nc", 0, "..."),
        ("🧭日本", 1, "🧭..."),
        ("e\u{301}🙂", 1, "e..."),
        ("e\u{301}🙂", 2, "e\u{301}..."),
        ("   abc   ", 3, "abc"),
        ("   abc   ", 2, "ab..."),
        ("-", 0, "..."),
    ] {
        assert_eq!(
            redaction_text_snippet(input, max_chars).unwrap(),
            expected,
            "{input:?}/{max_chars}"
        );
    }
}

#[test]
fn redaction_text_snippet_mojo_abi_rejects_bad_inputs_without_writing() {
    let input = b"ok";
    let mut output = [73_u8; 10];
    let mut written = 91_i64;
    let args = |version, input_ptr, input_len, capacity, result_ptr, written_ptr| unsafe {
        prodex_redaction_text_snippet_v1(
            version,
            input_ptr,
            input_len,
            1,
            result_ptr,
            capacity,
            written_ptr,
        )
    };
    let source = input.as_ptr() as usize as u64;
    let target = output.as_mut_ptr() as usize as u64;
    let count = (&mut written as *mut i64) as usize as u64;
    assert_eq!(args(2, source, 2, 10, target, count), 4);
    assert_eq!(args(1, source, 2, 5, target, count), 1);
    assert_eq!(args(1, 0, 2, 10, target, count), 1);
    assert_eq!(args(1, source, -1, 10, target, count), 1);
    assert_eq!(args(1, source, 2, 10, target, 0), 1);
    assert_eq!(output, [73; 10]);
    assert_eq!(written, 91);
}
