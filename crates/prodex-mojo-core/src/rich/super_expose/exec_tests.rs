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
