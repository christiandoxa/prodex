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
