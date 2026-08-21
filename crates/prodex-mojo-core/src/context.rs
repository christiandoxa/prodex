unsafe extern "C" {
    fn prodex_context_estimate_tokens(chars: u64, words: u64) -> u64;
    fn prodex_context_signal_diff(
        before: *const i64,
        after: *const i64,
        lost: *mut i64,
        gained: *mut i64,
    ) -> i64;
    fn prodex_context_lost_line_ranges_batch(
        before_rows: *const i64,
        after_available: *mut i64,
        initial_loss: *const i64,
        output_ranges: *mut i64,
        output_count: *mut i64,
        line_count: i64,
        key_count: i64,
        context_lines: i64,
        max_ranges: i64,
        max_range_lines: i64,
    ) -> i64;
}

const CRITICAL_SIGNAL_COUNTER_COUNT: usize = 7;
const CRITICAL_SIGNAL_ROW_WIDTH: usize = 8;
const CRITICAL_SIGNAL_MAX_LINES: usize = 65_536;
const CRITICAL_SIGNAL_MAX_KEYS: usize = 65_536;
const CRITICAL_SIGNAL_MAX_RANGES: usize = 256;

fn counters_to_i64(
    values: &[usize; CRITICAL_SIGNAL_COUNTER_COUNT],
) -> Result<[i64; CRITICAL_SIGNAL_COUNTER_COUNT], crate::MojoError> {
    let mut converted = [0_i64; CRITICAL_SIGNAL_COUNTER_COUNT];
    for (slot, value) in converted.iter_mut().zip(values) {
        *slot = i64::try_from(*value).map_err(|_| crate::MojoError::InvalidInput)?;
    }
    Ok(converted)
}

fn counters_from_i64(
    values: [i64; CRITICAL_SIGNAL_COUNTER_COUNT],
) -> Result<[usize; CRITICAL_SIGNAL_COUNTER_COUNT], crate::MojoError> {
    let mut converted = [0_usize; CRITICAL_SIGNAL_COUNTER_COUNT];
    for (slot, value) in converted.iter_mut().zip(values) {
        *slot = usize::try_from(value).map_err(|_| crate::MojoError::InvalidOutput)?;
    }
    Ok(converted)
}

pub fn estimate_tokens(chars: usize, words: usize) -> Result<usize, crate::MojoError> {
    let chars = u64::try_from(chars).map_err(|_| crate::MojoError::InvalidInput)?;
    let words = u64::try_from(words).map_err(|_| crate::MojoError::InvalidInput)?;
    let tokens = unsafe { prodex_context_estimate_tokens(chars, words) };
    usize::try_from(tokens).map_err(|_| crate::MojoError::InvalidOutput)
}

pub fn signal_diff(
    before: &[usize; 7],
    after: &[usize; 7],
) -> Result<([usize; 7], [usize; 7]), crate::MojoError> {
    let before = counters_to_i64(before)?;
    let after = counters_to_i64(after)?;
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
    let lost = counters_from_i64(lost)?;
    let gained = counters_from_i64(gained)?;
    Ok((lost, gained))
}

pub fn lost_line_ranges_batch(
    before_rows: &[i64],
    after_available: &mut [i64],
    initial_loss: &[usize; CRITICAL_SIGNAL_COUNTER_COUNT],
    line_count: usize,
    context_lines: usize,
    max_ranges: usize,
    max_range_lines: usize,
) -> Result<Vec<(usize, usize)>, crate::MojoError> {
    if line_count > CRITICAL_SIGNAL_MAX_LINES
        || after_available.len() > CRITICAL_SIGNAL_MAX_KEYS
        || max_ranges > CRITICAL_SIGNAL_MAX_RANGES
        || context_lines > CRITICAL_SIGNAL_MAX_LINES
        || max_range_lines > CRITICAL_SIGNAL_MAX_LINES
        || before_rows.len()
            != line_count
                .checked_mul(CRITICAL_SIGNAL_ROW_WIDTH)
                .ok_or(crate::MojoError::InvalidInput)?
        || before_rows
            .chunks_exact(CRITICAL_SIGNAL_ROW_WIDTH)
            .any(|row| row[0] < -1 || row[1..].iter().any(|value| *value < 0))
        || after_available.iter().any(|value| *value < 0)
    {
        return Err(crate::MojoError::InvalidInput);
    }
    if max_ranges == 0 {
        return Ok(Vec::new());
    }

    let initial_loss = counters_to_i64(initial_loss)?;
    let mut output_ranges = vec![
        0_i64;
        max_ranges
            .checked_mul(2)
            .ok_or(crate::MojoError::InvalidInput)?
    ];
    let mut output_count = 0_i64;
    let status = unsafe {
        prodex_context_lost_line_ranges_batch(
            before_rows.as_ptr(),
            after_available.as_mut_ptr(),
            initial_loss.as_ptr(),
            output_ranges.as_mut_ptr(),
            &mut output_count,
            i64::try_from(line_count).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::try_from(after_available.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::try_from(context_lines).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::try_from(max_ranges).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::try_from(max_range_lines).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    if status != 0 || output_count < 0 || output_count as usize > max_ranges {
        return Err(crate::MojoError::InvalidOutput);
    }
    let output_len = usize::try_from(output_count)
        .ok()
        .and_then(|count| count.checked_mul(2))
        .ok_or(crate::MojoError::InvalidOutput)?;
    let mut ranges = Vec::with_capacity(output_len / 2);
    for pair in output_ranges[..output_len].chunks_exact(2) {
        let start = usize::try_from(pair[0]).map_err(|_| crate::MojoError::InvalidOutput)?;
        let end = usize::try_from(pair[1]).map_err(|_| crate::MojoError::InvalidOutput)?;
        if start == 0 || start > line_count || end < start || end > line_count {
            return Err(crate::MojoError::InvalidOutput);
        }
        ranges.push((start, end));
    }
    Ok(ranges)
}

pub fn self_test() -> bool {
    let mut after_available = [0_i64];
    signal_diff(&[3, 0, 4, 1, 0, 2, 8], &[1, 2, 4, 0, 3, 0, 9]).is_ok_and(|(lost, gained)| {
        lost == [2, 0, 0, 1, 0, 2, 0]
            && gained == [0, 2, 0, 0, 3, 0, 1]
            && estimate_tokens(5, 2) == Ok(3)
            && lost_line_ranges_batch(
                &[0, 1, 0, 0, 0, 0, 0, 0],
                &mut after_available,
                &[1, 0, 0, 0, 0, 0, 0],
                1,
                0,
                1,
                1,
            ) == Ok(vec![(1, 1)])
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
