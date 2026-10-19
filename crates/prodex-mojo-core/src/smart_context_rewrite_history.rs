//! Compiled stable top-K retention of Smart Context rewrite-safety history.
//! The host owns persisted records; Mojo chooses fresh/sorted tail indexes.

use crate::MojoError;

unsafe extern "C" {
    fn prodex_smart_context_rewrite_history_select_v1(
        abi_version: i64,
        timestamps_address: u64,
        record_count: i64,
        now_secs: u64,
        ttl_secs: u64,
        history_limit: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

/// Return the stable chronological positions of the freshest K records.
/// No Rust expiration predicate, timestamp sort, or tail-selection fallback.
pub fn smart_context_rewrite_history_positions(
    timestamps: &[u64],
    now_secs: u64,
    ttl_secs: u64,
    limit: usize,
) -> Result<Vec<usize>, MojoError> {
    let capacity = timestamps.len().min(limit);
    let mut selected = vec![u64::MAX; capacity];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_smart_context_rewrite_history_select_v1(
            1,
            timestamps.as_ptr() as usize as u64,
            i64::try_from(timestamps.len()).map_err(|_| MojoError::InvalidInput)?,
            now_secs,
            ttl_secs,
            i64::try_from(limit).map_err(|_| MojoError::InvalidInput)?,
            selected.as_mut_ptr() as usize as u64,
            i64::try_from(selected.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let count = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if count > capacity {
        return Err(MojoError::InvalidOutput);
    }
    let mut positions = Vec::with_capacity(count);
    let mut previous: Option<(u64, usize)> = None;
    for &raw in &selected[..count] {
        let index = usize::try_from(raw).map_err(|_| MojoError::InvalidOutput)?;
        let timestamp = *timestamps.get(index).ok_or(MojoError::InvalidOutput)?;
        let next = (timestamp, index);
        if previous.is_some_and(|prev| next <= prev) {
            return Err(MojoError::InvalidOutput);
        }
        previous = Some(next);
        positions.push(index);
    }
    Ok(positions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_rewrite_history_selection_keeps_newest_k_in_stable_timestamp_order() {
        assert_eq!(
            smart_context_rewrite_history_positions(
                &[0, 99_989, 99_990, 100_001, 99_997, 99_999, 99_997],
                100_000,
                10,
                4,
            ),
            Ok(vec![4, 6, 5, 3]),
        );
        assert_eq!(
            smart_context_rewrite_history_positions(&[10, 10, 10, 10, 10, 10], 10, 100, 3),
            Ok(vec![3, 4, 5]),
        );
        assert_eq!(
            smart_context_rewrite_history_positions(&[0, 100, 101, 99], 100, 0, 2),
            Ok(vec![1, 2]),
        );
        assert_eq!(
            smart_context_rewrite_history_positions(&[1, 2, 3], 100, 0, 4),
            Ok(vec![]),
        );
        assert_eq!(
            smart_context_rewrite_history_positions(&[0, 100, 101, 99], 100, 0, 0),
            Ok(vec![]),
        );
        assert_eq!(
            smart_context_rewrite_history_positions(&[], 100, 5, 4),
            Ok(vec![])
        );
    }

    #[test]
    fn compiled_rewrite_history_plan_matches_test_only_stable_sort_oracle() {
        for &now in &[0_u64, 1, 99, 100, u64::MAX] {
            for &ttl in &[0_u64, 1, 5, 100, u64::MAX] {
                let history = [0, 1, 4, 4, 5, 89, 99, 100, 100, 101, 120, u64::MAX];
                for limit in [0, 1, 2, 3, 4, 16] {
                    let mut expected = history
                        .iter()
                        .enumerate()
                        .filter(|&(_, &time)| time == 0 || now.saturating_sub(time) <= ttl)
                        .map(|(index, _)| index)
                        .collect::<Vec<_>>();
                    expected.sort_by_key(|&index| history[index]);
                    let drain = expected.len().saturating_sub(limit);
                    expected.drain(0..drain);
                    assert_eq!(
                        smart_context_rewrite_history_positions(&history, now, ttl, limit),
                        Ok(expected),
                        "now={now}/ttl={ttl}/limit={limit}",
                    );
                }
            }
        }
    }

    #[test]
    fn compiled_rewrite_history_abi_rejects_invalid_shapes_before_output() {
        let times = [100_u64, 105, 106];
        let source = times.as_ptr() as usize as u64;
        let mut output = [73_u64; 3];
        let mut written = 79_i64;
        let output_ptr = output.as_mut_ptr() as usize as u64;
        let count = (&mut written as *mut i64) as usize as u64;
        let call = |version, src, len, limit, dest, capacity, count| unsafe {
            prodex_smart_context_rewrite_history_select_v1(
                version, src, len, 105, 5, limit, dest, capacity, count,
            )
        };
        assert_eq!(call(2, source, 3, 3, output_ptr, 3, count), 4);
        assert_eq!(call(1, 0, 3, 3, output_ptr, 3, count), 1);
        assert_eq!(call(1, source, -1, 3, output_ptr, 3, count), 1);
        assert_eq!(call(1, source, 3, -1, output_ptr, 3, count), 1);
        assert_eq!(call(1, source, 3, 3, 0, 3, count), 1);
        assert_eq!(call(1, source, 3, 3, output_ptr, 2, count), 1);
        assert_eq!(call(1, source, 3, 3, output_ptr, 3, 0), 1);
        assert_eq!(output, [73; 3]);
        assert_eq!(written, 79);
    }
}
