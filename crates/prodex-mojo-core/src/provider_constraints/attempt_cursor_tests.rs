use super::*;

#[test]
fn provider_retry_cursor_mojo_exhaustion_and_credential_reset() {
    assert_eq!(
        provider_attempt_cursor_plan(PROVIDER_ATTEMPT_CURSOR_INITIAL, 0, 2, None).unwrap(),
        None,
    );
    assert_eq!(
        provider_attempt_cursor_plan(PROVIDER_ATTEMPT_CURSOR_INITIAL, 2, 0, None).unwrap(),
        None,
    );
    let mut cursor =
        provider_attempt_cursor_plan(PROVIDER_ATTEMPT_CURSOR_INITIAL, 3, 2, None).unwrap();
    let mut sequence = Vec::new();
    while let Some(indices) = cursor {
        sequence.push(indices);
        cursor = if indices.0 + 1 < 3 {
            provider_attempt_cursor_plan(PROVIDER_ATTEMPT_CURSOR_NEXT_MODEL, 3, 2, cursor).unwrap()
        } else {
            provider_attempt_cursor_plan(PROVIDER_ATTEMPT_CURSOR_NEXT_CREDENTIAL, 3, 2, cursor)
                .unwrap()
        };
    }
    assert_eq!(
        sequence,
        vec![(0, 0), (1, 0), (2, 0), (0, 1), (1, 1), (2, 1)],
    );
    assert_eq!(
        provider_attempt_cursor_plan(PROVIDER_ATTEMPT_CURSOR_NEXT_MODEL, 3, 2, None).unwrap(),
        None,
    );
    assert_eq!(
        provider_attempt_cursor_plan(PROVIDER_ATTEMPT_CURSOR_NEXT_CREDENTIAL, 3, 2, None).unwrap(),
        None,
    );
    assert_eq!(
        provider_attempt_cursor_plan(PROVIDER_ATTEMPT_CURSOR_NEXT_MODEL, 3, 2, Some((2, 0)))
            .unwrap(),
        None,
    );
    assert_eq!(
        provider_attempt_cursor_plan(PROVIDER_ATTEMPT_CURSOR_NEXT_CREDENTIAL, 3, 2, Some((2, 0)))
            .unwrap(),
        Some((0, 1)),
    );
}

#[test]
fn provider_retry_cursor_abi_rejects_bad_version_and_indices_before_writing() {
    let mut output = [123_i64; 2];
    let result = output.as_mut_ptr() as usize as u64;
    unsafe {
        assert_eq!(
            prodex_provider_attempt_cursor_v1(2, 0, 2, 2, -1, -1, result),
            4
        );
        assert_eq!(
            prodex_provider_attempt_cursor_v1(1, 3, 2, 2, -1, -1, result),
            1
        );
        assert_eq!(
            prodex_provider_attempt_cursor_v1(1, 0, 2, 2, 0, 0, result),
            1
        );
        assert_eq!(
            prodex_provider_attempt_cursor_v1(1, 1, 2, 2, -1, 0, result),
            1
        );
        assert_eq!(
            prodex_provider_attempt_cursor_v1(1, 1, 2, 2, 2, 0, result),
            1
        );
        assert_eq!(
            prodex_provider_attempt_cursor_v1(1, 1, 2, 2, 0, 2, result),
            1
        );
        assert_eq!(
            prodex_provider_attempt_cursor_v1(1, 0, -1, 2, -1, -1, result),
            1
        );
        assert_eq!(prodex_provider_attempt_cursor_v1(1, 0, 2, 2, -1, -1, 0), 1);
    }
    assert_eq!(output, [123, 123]);
    assert!(matches!(
        provider_attempt_cursor_plan(PROVIDER_ATTEMPT_CURSOR_INITIAL, 1, 1, Some((0, 0))),
        Err(crate::MojoError::InvalidInput)
    ));
    assert!(matches!(
        provider_attempt_cursor_plan(PROVIDER_ATTEMPT_CURSOR_NEXT_CREDENTIAL, 1, 1, Some((0, 1))),
        Err(crate::MojoError::InvalidInput)
    ));
}
