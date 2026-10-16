use super::*;

#[test]
fn active_backoff_selection_is_owned_by_mojo() {
    assert_eq!(profile_active_backoff_max(&[], 10), Ok(None));
    assert_eq!(profile_active_backoff_max(&[9, 10, 11], 10), Ok(Some(11)));
    assert_eq!(
        profile_active_backoff_max(&[i64::MIN, -1, 0, i64::MAX, i64::MAX - 1], 0),
        Ok(Some(i64::MAX)),
    );
    assert_eq!(profile_active_backoff_max(&[i64::MAX], i64::MAX), Ok(None));
    assert_eq!(profile_active_backoff_max(&[-100, -20], -30), Ok(Some(-20)));
    assert_eq!(profile_active_backoff_max(&[5, 6, 7], 7), Ok(None));
    assert_eq!(
        profile_active_backoff_max(&[11, 11, 10, 9], 10),
        Ok(Some(11))
    );
}

#[test]
fn active_backoff_abi_rejects_invalid_count_and_version_without_output() {
    let mut output = [777_i64; 2];
    let input = [12_i64];
    unsafe {
        assert_eq!(
            prodex_runtime_profile_active_backoff_max_v1(
                2,
                input.as_ptr() as u64,
                1,
                10,
                output.as_mut_ptr() as u64,
            ),
            4
        );
        assert_eq!(
            prodex_runtime_profile_active_backoff_max_v1(
                1,
                input.as_ptr() as u64,
                -1,
                10,
                output.as_mut_ptr() as u64,
            ),
            1
        );
        assert_eq!(
            prodex_runtime_profile_active_backoff_max_v1(1, 0, 1, 10, output.as_mut_ptr() as u64,),
            1
        );
    }
    assert_eq!(output, [777; 2]);
}
