use crate::MojoError;

const ABI_VERSION: i64 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedHistoryDedupPlan {
    pub keep: Vec<bool>,
    pub unique_count: usize,
    pub total_bytes: u64,
    pub exceeds_limit: bool,
}

unsafe extern "C" {
    fn prodex_shared_history_timestamp_order_v1(
        abi_version: i64,
        rows_address: u64,
        count: i64,
        output_address: u64,
        output_capacity: i64,
    ) -> i64;
    fn prodex_shared_history_dedup_plan_v1(
        abi_version: i64,
        addresses_address: u64,
        lengths_address: u64,
        count: i64,
        slots_address: u64,
        slot_count: i64,
        keep_address: u64,
        max_bytes: u64,
        output_count_address: u64,
        output_total_bytes_address: u64,
        output_exceeds_address: u64,
    ) -> i64;
}

fn status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Stable chronological merge selection over original JSONL observations.
/// A missing timestamp remains in its original position, separating
/// consecutive runs of timestamped observations. Original String ownership
/// and JSON parsing remain in the Rust filesystem adapter.
pub fn timestamp_merge_order(timestamps: &[Option<i64>]) -> Result<Vec<usize>, MojoError> {
    if timestamps.len() > 4_000_000 {
        return Err(MojoError::InvalidInput);
    }
    let mut rows = Vec::<i64>::new();
    rows.try_reserve_exact(timestamps.len().checked_mul(2).ok_or(MojoError::Capacity)?)
        .map_err(|_| MojoError::Capacity)?;
    for timestamp in timestamps {
        match timestamp {
            Some(timestamp) => rows.extend([1, *timestamp]),
            None => rows.extend([0, 0]),
        }
    }
    let mut output = Vec::<i64>::new();
    output
        .try_reserve_exact(timestamps.len())
        .map_err(|_| MojoError::Capacity)?;
    output.resize(timestamps.len(), -1);
    status(unsafe {
        prodex_shared_history_timestamp_order_v1(
            ABI_VERSION,
            rows.as_ptr() as usize as u64,
            i64::try_from(timestamps.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    })?;
    let mut seen = vec![false; timestamps.len()];
    let mut selected = Vec::with_capacity(timestamps.len());
    for index in output {
        let index = usize::try_from(index).map_err(|_| MojoError::InvalidOutput)?;
        let used = seen.get_mut(index).ok_or(MojoError::InvalidOutput)?;
        if *used {
            return Err(MojoError::InvalidOutput);
        }
        *used = true;
        selected.push(index);
    }
    Ok(selected)
}

pub fn dedup_plan(lines: &[&str], max_bytes: u64) -> Result<SharedHistoryDedupPlan, MojoError> {
    if lines.is_empty() {
        return Ok(SharedHistoryDedupPlan {
            keep: Vec::new(),
            unique_count: 0,
            total_bytes: 0,
            exceeds_limit: false,
        });
    }

    let addresses = lines
        .iter()
        .map(|line| line.as_ptr() as usize as u64)
        .collect::<Vec<_>>();
    let lengths = lines
        .iter()
        .map(|line| i64::try_from(line.len()).map_err(|_| MojoError::InvalidInput))
        .collect::<Result<Vec<_>, _>>()?;
    let slot_count = lines
        .len()
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or(MojoError::InvalidInput)?;
    let mut slots = vec![0_i64; slot_count];
    let mut keep = vec![0_i64; lines.len()];
    let mut unique_count = -1_i64;
    let mut total_bytes = 0_u64;
    let mut exceeds = -1_i64;

    status(unsafe {
        prodex_shared_history_dedup_plan_v1(
            ABI_VERSION,
            addresses.as_ptr() as usize as u64,
            lengths.as_ptr() as usize as u64,
            i64::try_from(lines.len()).map_err(|_| MojoError::InvalidInput)?,
            slots.as_mut_ptr() as usize as u64,
            i64::try_from(slots.len()).map_err(|_| MojoError::InvalidInput)?,
            keep.as_mut_ptr() as usize as u64,
            max_bytes,
            (&mut unique_count as *mut i64) as usize as u64,
            (&mut total_bytes as *mut u64) as usize as u64,
            (&mut exceeds as *mut i64) as usize as u64,
        )
    })?;

    let unique_count = usize::try_from(unique_count).map_err(|_| MojoError::InvalidOutput)?;
    let keep = keep
        .into_iter()
        .map(|value| match value {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(MojoError::InvalidOutput),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if keep.iter().filter(|value| **value).count() != unique_count {
        return Err(MojoError::InvalidOutput);
    }
    let exceeds_limit = match exceeds {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };

    Ok(SharedHistoryDedupPlan {
        keep,
        unique_count,
        total_bytes,
        exceeds_limit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_history_dedup_preserves_first_occurrence_and_size_budget() {
        let plan = dedup_plan(
            &[
                r#"{"ts":2,"text":"b"}"#,
                r#"{"ts":1,"text":"a"}"#,
                r#"{"ts":2,"text":"b"}"#,
                "plain",
            ],
            1024,
        )
        .unwrap();
        assert_eq!(plan.keep, vec![true, true, false, true]);
        assert_eq!(plan.unique_count, 3);
        assert_eq!(
            plan.total_bytes,
            (r#"{"ts":2,"text":"b"}"#.len() + r#"{"ts":1,"text":"a"}"#.len() + "plain".len() + 2)
                as u64
        );
        assert!(!plan.exceeds_limit);

        let limited = dedup_plan(&["alpha", "beta"], 5).unwrap();
        assert!(limited.exceeds_limit);
    }
}

#[cfg(test)]
mod compiled_timestamp_merge_tests {
    use super::*;

    #[test]
    fn compiled_history_timestamp_merge_orders_only_consecutive_valid_runs() {
        let timestamps = [
            Some(50),
            Some(-2),
            Some(-2),
            None,
            Some(i64::MAX),
            Some(i64::MIN),
            Some(0),
            None,
            None,
            Some(7),
            Some(1),
            Some(7),
        ];
        assert_eq!(
            timestamp_merge_order(&timestamps),
            Ok(vec![1, 2, 0, 3, 5, 6, 4, 7, 8, 10, 9, 11]),
        );
        assert_eq!(timestamp_merge_order(&[]), Ok(Vec::new()));
        assert_eq!(timestamp_merge_order(&[None, None]), Ok(vec![0, 1]));
        assert_eq!(timestamp_merge_order(&[Some(1)]), Ok(vec![0]));
        assert_eq!(
            timestamp_merge_order(&[Some(1), None, Some(-1)]),
            Ok(vec![0, 1, 2]),
            "invalid timestamp observations preserve a chronological barrier",
        );
        let descending = (0..1025_i64).rev().map(Some).collect::<Vec<_>>();
        let ordered = timestamp_merge_order(&descending).unwrap();
        assert_eq!(ordered.len(), 1025);
        assert_eq!(ordered[0], 1024);
        assert_eq!(ordered[1024], 0);
    }

    #[test]
    fn compiled_history_timestamp_merge_rejects_invalid_abi_before_output_writes() {
        let mut observations = [1_i64, 5, 0, 0, 1, 2];
        let mut result = [91_i64; 3];
        let input = observations.as_ptr() as usize as u64;
        let output = result.as_mut_ptr() as usize as u64;
        for (version, source, length, capacity, expected) in [
            (2, input, 3, 3, 4),
            (1, 0, 3, 3, 1),
            (1, input, -1, 3, 1),
            (1, input, 4_000_001, 3, 1),
            (1, input, 3, 2, 1),
            (1, input, 3, -1, 1),
            (1, input, 3, 3, 0),
        ] {
            assert_eq!(
                unsafe {
                    prodex_shared_history_timestamp_order_v1(
                        version, source, length, output, capacity,
                    )
                },
                expected,
            );
            if expected != 0 {
                assert_eq!(result, [91; 3]);
            }
        }
        result = [91; 3];
        observations[4] = 2;
        assert_eq!(
            unsafe {
                prodex_shared_history_timestamp_order_v1(
                    1,
                    observations.as_ptr() as usize as u64,
                    3,
                    output,
                    3,
                )
            },
            1,
        );
        assert_eq!(result, [91; 3]);
        assert_eq!(
            unsafe { prodex_shared_history_timestamp_order_v1(1, 0, 0, output, 3) },
            0,
        );
        assert_eq!(result, [91; 3]);
    }
}
