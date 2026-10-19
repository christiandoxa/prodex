//! Compiled stable top-K retention of Smart Context rewrite-safety history.
//! The host owns persisted records; Mojo chooses fresh/sorted tail indexes.

use crate::MojoError;

unsafe extern "C" {
    fn prodex_smart_context_unique_tail_select_v1(
        abi_version: i64,
        views_address: u64,
        existing_count: i64,
        total_count: i64,
        limit: i64,
        scratch_address: u64,
        scratch_capacity: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
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

/// Stable, exact-byte canonical JSON deduplication: retain all existing
/// entries, add only incoming entries not already present, then select the
/// final K positions. The compiled owner decides all equality and trimming;
/// Rust only serializes persisted record shapes and materializes the result.
pub fn smart_context_unique_tail_positions(
    existing: &[Vec<u8>],
    incoming: &[Vec<u8>],
    limit: usize,
) -> Result<Vec<usize>, MojoError> {
    let total_count = existing
        .len()
        .checked_add(incoming.len())
        .ok_or(MojoError::InvalidInput)?;
    let mut views = Vec::with_capacity(total_count.checked_mul(2).ok_or(MojoError::InvalidInput)?);
    for value in existing.iter().chain(incoming) {
        views.push(value.as_ptr() as usize as u64);
        views.push(u64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?);
    }
    let mut scratch = vec![u64::MAX; total_count];
    let mut output = vec![u64::MAX; total_count.min(limit)];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_smart_context_unique_tail_select_v1(
            1,
            views.as_ptr() as usize as u64,
            i64::try_from(existing.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(total_count).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(limit).map_err(|_| MojoError::InvalidInput)?,
            scratch.as_mut_ptr() as usize as u64,
            i64::try_from(scratch.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
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
    if count > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    let mut prior = None;
    let mut result = Vec::with_capacity(count);
    for raw in &output[..count] {
        let position = usize::try_from(*raw).map_err(|_| MojoError::InvalidOutput)?;
        if position >= total_count || prior.is_some_and(|last| position <= last) {
            return Err(MojoError::InvalidOutput);
        }
        prior = Some(position);
        result.push(position);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_smart_context_unique_tail_preserves_existing_duplicates_and_tail_order() {
        let existing = ["a", "a", "b"].map(|value| value.as_bytes().to_vec());
        let incoming = ["a", "c", "c", "d", "b", "e"].map(|value| value.as_bytes().to_vec());
        assert_eq!(
            smart_context_unique_tail_positions(&existing, &incoming, 4),
            Ok(vec![2, 4, 6, 8]),
        );
        assert_eq!(
            smart_context_unique_tail_positions(&existing, &incoming, 10),
            Ok(vec![0, 1, 2, 4, 6, 8]),
        );
        assert_eq!(
            smart_context_unique_tail_positions(&existing, &incoming, 0),
            Ok(vec![]),
        );
        assert_eq!(smart_context_unique_tail_positions(&[], &[], 4), Ok(vec![]));
        assert_eq!(
            smart_context_unique_tail_positions(&[], &[b"\x00".to_vec(), b"\x00".to_vec()], 4),
            Ok(vec![0]),
        );
    }

    #[test]
    fn compiled_smart_context_unique_tail_matches_test_only_rust_equality_oracle() {
        for original in [
            vec![],
            vec!["a", "b"],
            vec!["a", "a", "b"],
            vec!["雪", "🧭", "東京"],
        ] {
            for incoming in [
                vec![],
                vec!["a", "b", "a"],
                vec!["雪", "東京", "🧭", "雪"],
                vec!["c", "d", "c", "e", "b", "f", "b"],
            ] {
                let a = original
                    .iter()
                    .map(|value| value.as_bytes().to_vec())
                    .collect::<Vec<_>>();
                let b = incoming
                    .iter()
                    .map(|value| value.as_bytes().to_vec())
                    .collect::<Vec<_>>();
                for limit in [0, 1, 2, 4, 16] {
                    let mut expected = a
                        .iter()
                        .enumerate()
                        .map(|(index, value)| (index, value.clone()))
                        .collect::<Vec<_>>();
                    for (index, value) in b.iter().enumerate() {
                        if !expected.iter().any(|(_, prior)| prior == value) {
                            expected.push((a.len() + index, value.clone()));
                        }
                    }
                    let overflow = expected.len().saturating_sub(limit);
                    expected.drain(0..overflow);
                    assert_eq!(
                        smart_context_unique_tail_positions(&a, &b, limit),
                        Ok(expected.into_iter().map(|(index, _)| index).collect()),
                        "{original:?}/{incoming:?}/{limit}",
                    );
                }
            }
        }
    }

    #[test]
    fn compiled_smart_context_unique_tail_abi_rejects_invalid_source_before_output() {
        let item = b"hello";
        let source = [item.as_ptr() as usize as u64, 5_u64];
        let invalid = [0_u64, 5_u64];
        let mut scratch = [59_u64; 1];
        let mut output = [67_u64; 1];
        let mut written = 73_i64;
        let s = scratch.as_mut_ptr() as usize as u64;
        let o = output.as_mut_ptr() as usize as u64;
        let w = (&mut written as *mut i64) as usize as u64;
        let call = |version, view, existing, total, limit, scratch, cap, out, out_cap, written| unsafe {
            prodex_smart_context_unique_tail_select_v1(
                version, view, existing, total, limit, scratch, cap, out, out_cap, written,
            )
        };
        let view = source.as_ptr() as usize as u64;
        assert_eq!(call(2, view, 0, 1, 1, s, 1, o, 1, w), 4);
        assert_eq!(call(1, 0, 0, 1, 1, s, 1, o, 1, w), 1);
        assert_eq!(call(1, view, 2, 1, 1, s, 1, o, 1, w), 1);
        assert_eq!(call(1, view, 0, 1, -1, s, 1, o, 1, w), 1);
        assert_eq!(call(1, view, 0, 1, 1, 0, 1, o, 1, w), 1);
        assert_eq!(call(1, view, 0, 1, 1, s, 0, o, 1, w), 1);
        assert_eq!(call(1, view, 0, 1, 1, s, 1, o, 0, w), 1);
        assert_eq!(call(1, view, 0, 1, 1, s, 1, o, 1, 0), 1);
        assert_eq!(
            call(1, invalid.as_ptr() as usize as u64, 0, 1, 1, s, 1, o, 1, w),
            1
        );
        assert_eq!(scratch, [59]);
        assert_eq!(output, [67]);
        assert_eq!(written, 73);
    }

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
