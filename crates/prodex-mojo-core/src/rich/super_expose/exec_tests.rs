use super::*;

#[test]
fn exec_validation_preserves_text_and_boundaries() {
    const { assert!(crate::MOJO_REQUIRED) };
    assert_eq!(
        super_expose_exec_validate_text("sh", SuperExposeExecTextKind::Program).unwrap(),
        SuperExposeExecValidation::Valid
    );
    assert_eq!(
        super_expose_exec_validate_text("東京", SuperExposeExecTextKind::Cwd).unwrap(),
        SuperExposeExecValidation::Valid
    );
    assert_eq!(
        super_expose_exec_validate_text("", SuperExposeExecTextKind::Program).unwrap(),
        SuperExposeExecValidation::Invalid
    );
    assert_eq!(
        super_expose_exec_validate_text("bad\u{0085}path", SuperExposeExecTextKind::Cwd).unwrap(),
        SuperExposeExecValidation::Invalid
    );
    assert_eq!(
        super_expose_exec_validate_text("bad\0path", SuperExposeExecTextKind::Program).unwrap(),
        SuperExposeExecValidation::Invalid
    );
}

#[test]
fn exec_validation_preserves_argument_and_environment_error_kinds() {
    assert_eq!(
        super_expose_exec_validate_count(256, SuperExposeExecCountKind::Arguments).unwrap(),
        SuperExposeExecValidation::Valid
    );
    assert_eq!(
        super_expose_exec_validate_count(257, SuperExposeExecCountKind::Arguments).unwrap(),
        SuperExposeExecValidation::CountLimit
    );
    assert_eq!(
        super_expose_exec_validate_argument("ok", 262_142).unwrap(),
        (SuperExposeExecValidation::Valid, 262_144)
    );
    assert_eq!(
        super_expose_exec_validate_argument("x", 262_144).unwrap().0,
        SuperExposeExecValidation::TotalArgumentLimit
    );
    assert_eq!(
        super_expose_exec_validate_argument("bad\0arg", 0)
            .unwrap()
            .0,
        SuperExposeExecValidation::ArgumentLimit
    );
    assert_eq!(
        super_expose_exec_validate_env_entry("GOOD", "ok").unwrap(),
        SuperExposeExecValidation::Valid
    );
    assert_eq!(
        super_expose_exec_validate_env_entry("BAD=KEY", "ok").unwrap(),
        SuperExposeExecValidation::EnvironmentKey
    );
    assert_eq!(
        super_expose_exec_validate_env_entry("GOOD", "bad\0value").unwrap(),
        SuperExposeExecValidation::EnvironmentValue
    );
}

#[test]
fn exec_validation_preserves_stdin_and_timeout_limits() {
    assert_eq!(
        super_expose_exec_validate_stdin(262_144).unwrap(),
        SuperExposeExecValidation::Valid
    );
    assert_eq!(
        super_expose_exec_validate_stdin(262_145).unwrap(),
        SuperExposeExecValidation::StdinLimit
    );
    assert_eq!(
        super_expose_exec_validate_timeout(1).unwrap(),
        SuperExposeExecValidation::Valid
    );
    assert_eq!(
        super_expose_exec_validate_timeout(120_000).unwrap(),
        SuperExposeExecValidation::Valid
    );
    assert_eq!(
        super_expose_exec_validate_timeout(0).unwrap(),
        SuperExposeExecValidation::TimeoutLimit
    );
    assert_eq!(
        super_expose_exec_validate_timeout(120_001).unwrap(),
        SuperExposeExecValidation::TimeoutLimit
    );
}

#[test]
fn exec_validation_abi_rejects_bad_version_shape_without_output() {
    let input = b"program";
    let mut output = [77_i64; 2];
    let call = |version, operation, input_address, input_length, output_address| unsafe {
        prodex_mojo_super_expose_exec_validation_v1(
            version,
            operation,
            input_address,
            input_length,
            0,
            0,
            0,
            output_address,
        )
    };
    let input_address = input.as_ptr() as usize as u64;
    let output_address = output.as_mut_ptr() as usize as u64;
    assert_eq!(call(2, 0, input_address, 7, output_address), 4);
    assert_eq!(call(1, 0, 0, 7, output_address), 1);
    assert_eq!(call(1, 99, input_address, 7, output_address), 1);
    assert_eq!(output, [77; 2]);
}
#[test]
fn compiled_super_expose_text_cut_preserves_scalar_widths_and_byte_caps() {
    let expected = [0, 1, 1, 3, 3, 3, 6, 6, 6, 6, 10, 11, 11];
    for (maximum, end) in expected.into_iter().enumerate() {
        assert_eq!(super_expose_text_cut("aé雪🙂z", maximum), Ok(end));
    }
    assert_eq!(super_expose_text_cut("aé雪🙂z", usize::MAX), Ok(11));
    assert_eq!(super_expose_text_cut("", 0), Ok(0));
    assert_eq!(super_expose_text_cut("\0é", 2), Ok(1));
}

#[test]
fn compiled_super_expose_text_cut_rejects_invalid_views_before_writes() {
    let invalid = [255_u8];
    let input = "雪";
    let mut output = [71_i64; 2];
    for (version, op, address, length, expected) in [
        (2, OP_TEXT_CUT, input.as_ptr() as u64, 3, 4),
        (1, 99, input.as_ptr() as u64, 3, 1),
        (1, OP_TEXT_CUT, invalid.as_ptr() as u64, 1, 1),
        (1, OP_TEXT_CUT, 0, 3, 1),
        (1, OP_TEXT_CUT, input.as_ptr() as u64, -1, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_mojo_super_expose_exec_validation_v1(
                    version,
                    op,
                    address,
                    length,
                    0,
                    0,
                    3,
                    output.as_mut_ptr() as u64,
                )
            },
            expected
        );
        assert_eq!(output, [71; 2]);
    }
    assert_eq!(
        unsafe {
            prodex_mojo_super_expose_exec_validation_v1(
                1,
                OP_TEXT_CUT,
                input.as_ptr() as u64,
                3,
                0,
                0,
                3,
                output.as_mut_ptr() as u64 + 1,
            )
        },
        1
    );
    assert_eq!(output, [71; 2]);
}
#[test]
fn compiled_bounded_text_preserves_byte_caps_and_unicode_control_rules() {
    for (text, max, expected) in [
        ("", 256, false),
        (" ", 256, true),
        (" é雪🙂 ", 256, true),
        ("é", 1, false),
        ("é", 2, true),
        ("é", 0, false),
        ("\0", 256, false),
        ("line\nnext", 256, false),
        ("\u{0085}", 256, false),
        ("\u{009F}", 256, false),
        ("\u{00A0}", 256, true),
        ("\u{200B}", 256, true),
        ("model", usize::MAX, true),
    ] {
        assert_eq!(
            super_expose_bounded_text_valid(text, max),
            Ok(expected),
            "{text:?}"
        );
    }
    assert_eq!(
        super_expose_bounded_text_valid(&"é".repeat(64), 128),
        Ok(true)
    );
    assert_eq!(
        super_expose_bounded_text_valid(&"é".repeat(65), 128),
        Ok(false)
    );
}

#[test]
fn compiled_bounded_text_rejects_malformed_utf8_atomically() {
    let input = [255_u8];
    let mut output = [71_i64; 2];
    assert_eq!(
        unsafe {
            prodex_mojo_super_expose_exec_validation_v1(
                1,
                OP_BOUNDED_TEXT,
                input.as_ptr() as u64,
                1,
                0,
                0,
                256,
                output.as_mut_ptr() as u64,
            )
        },
        1
    );
    assert_eq!(output, [71; 2]);
}
