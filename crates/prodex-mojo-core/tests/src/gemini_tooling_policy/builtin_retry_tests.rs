use super::*;

#[test]
fn compiled_gemini_builtin_retry_chooses_priority_and_exhausts() {
    for (rejected, attempted, expected) in [
        (0, 0, None),
        (0b1111, 0, Some(0)),
        (0b1111, 0b0001, Some(1)),
        (0b1111, 0b0011, Some(2)),
        (0b1111, 0b0111, Some(3)),
        (0b1111, 0b1111, None),
        (0b1010, 0, Some(1)),
        (0b1010, 0b0010, Some(3)),
        (0b1010, 0b1010, None),
        (0b1000, 0b0111, Some(3)),
    ] {
        assert_eq!(
            gemini_unsupported_tool_retry_choice(rejected, attempted).unwrap(),
            expected,
            "{rejected:04b}/{attempted:04b}"
        );
    }
}

#[test]
fn compiled_gemini_builtin_retry_abi_rejects_invalid_masks_without_mutation() {
    let mut output = -7_i64;
    let target = (&mut output as *mut i64) as usize as u64;
    unsafe {
        assert_eq!(
            prodex_gemini_unsupported_tool_retry_choice_v1(2, 1, 0, target),
            4
        );
        assert_eq!(
            prodex_gemini_unsupported_tool_retry_choice_v1(1, -1, 0, target),
            1
        );
        assert_eq!(
            prodex_gemini_unsupported_tool_retry_choice_v1(1, 16, 0, target),
            1
        );
        assert_eq!(
            prodex_gemini_unsupported_tool_retry_choice_v1(1, 1, 16, target),
            1
        );
        assert_eq!(
            prodex_gemini_unsupported_tool_retry_choice_v1(1, 1, 0, 0),
            1
        );
    }
    assert_eq!(output, -7);
    assert!(matches!(
        gemini_unsupported_tool_retry_choice(0b10000, 0),
        Err(MojoError::InvalidInput),
    ));
    assert!(matches!(
        gemini_unsupported_tool_retry_choice(0, 0b10000),
        Err(MojoError::InvalidInput),
    ));
}
