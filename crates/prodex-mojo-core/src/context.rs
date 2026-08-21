unsafe extern "C" {
    fn prodex_context_signal_diff(
        before: *const i64,
        after: *const i64,
        lost: *mut i64,
        gained: *mut i64,
    ) -> i64;
}

pub fn signal_diff(
    before: &[usize; 7],
    after: &[usize; 7],
) -> Result<([usize; 7], [usize; 7]), crate::MojoError> {
    let before = before
        .iter()
        .map(|value| i64::try_from(*value).map_err(|_| crate::MojoError::InvalidInput))
        .collect::<Result<Vec<_>, _>>()?;
    let after = after
        .iter()
        .map(|value| i64::try_from(*value).map_err(|_| crate::MojoError::InvalidInput))
        .collect::<Result<Vec<_>, _>>()?;
    let mut lost = [0_i64; 7];
    let mut gained = [0_i64; 7];
    let status = unsafe {
        prodex_context_signal_diff(
            before.as_ptr(),
            after.as_ptr(),
            lost.as_mut_ptr(),
            gained.as_mut_ptr(),
        )
    };
    if status != 0 || lost.iter().any(|value| *value < 0) || gained.iter().any(|value| *value < 0) {
        return Err(crate::MojoError::InvalidOutput);
    }
    let lost = lost
        .map(|value| usize::try_from(value).map_err(|_| crate::MojoError::InvalidOutput))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| crate::MojoError::InvalidOutput)?;
    let gained = gained
        .map(|value| usize::try_from(value).map_err(|_| crate::MojoError::InvalidOutput))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| crate::MojoError::InvalidOutput)?;
    Ok((lost, gained))
}

pub fn self_test() -> bool {
    signal_diff(&[3, 0, 4, 1, 0, 2, 8], &[1, 2, 4, 0, 3, 0, 9]).is_ok_and(|(lost, gained)| {
        lost == [2, 0, 0, 1, 0, 2, 0] && gained == [0, 2, 0, 0, 3, 0, 1]
    })
}

#[cfg(all(test, feature = "mojo-runtime"))]
#[test]
fn signal_diff_self_test_passes() {
    assert!(self_test());
}

#[cfg(all(test, feature = "mojo-runtime"))]
#[test]
fn signal_diff_matches_rust_oracle_for_generated_counters() {
    let mut state = 0x637269746963616c_u64;
    for case in 0..2_000 {
        let before = std::array::from_fn(|_| (next_random(&mut state) % 10_000) as usize);
        let after = std::array::from_fn(|_| (next_random(&mut state) % 10_000) as usize);
        let expected_lost = std::array::from_fn(|index| before[index].saturating_sub(after[index]));
        let expected_gained =
            std::array::from_fn(|index| after[index].saturating_sub(before[index]));
        let actual = signal_diff(&before, &after).expect("generated signal counters are valid");
        assert_eq!(
            actual,
            (expected_lost, expected_gained),
            "signal case {case}"
        );
    }
}

#[cfg(all(test, feature = "mojo-runtime"))]
fn next_random(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state
}
