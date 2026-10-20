//! One compiled plan for balancing live runtime and session log followers.
//! Rust acquires original filename/time metadata; Mojo owns sorting,
//! descriptor-slot reservations and source/path uniqueness.

use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_ROWS: usize = 65_536;

unsafe extern "C" {
    fn prodex_mojo_log_follow_selection_v1(
        abi: i64,
        rows_address: u64,
        count: i64,
        max_files: i64,
        scratch_address: u64,
        scratch_capacity: i64,
        flags_address: u64,
        flags_capacity: i64,
        selected_count_address: u64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy)]
pub struct LogFollowSelectionInput<'a> {
    pub path: &'a [u8],
    pub source: u8,
    pub modified_before_unix_epoch: bool,
    pub modified_abs_seconds: u64,
    pub modified_abs_nanos: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogFollowSelectionPlan {
    /// Original input rows to return. Identical paths from one source may
    /// therefore appear more than once, matching the legacy filter behavior.
    pub selected: Vec<bool>,
    pub unique_files: usize,
}

pub fn log_follow_selection_plan(
    records: &[LogFollowSelectionInput<'_>],
    max_files: usize,
) -> Result<LogFollowSelectionPlan, MojoError> {
    if records.len() > MAX_ROWS || max_files > 512 {
        return Err(MojoError::InvalidInput);
    }
    let mut rows = Vec::<u64>::new();
    rows.try_reserve_exact(records.len().checked_mul(6).ok_or(MojoError::Capacity)?)
        .map_err(|_| MojoError::Capacity)?;
    for record in records {
        if record.source > 1 || record.modified_abs_nanos >= 1_000_000_000 {
            return Err(MojoError::InvalidInput);
        }
        rows.extend([
            record.path.as_ptr() as usize as u64,
            u64::try_from(record.path.len()).map_err(|_| MojoError::InvalidInput)?,
            u64::from(record.source),
            u64::from(record.modified_before_unix_epoch),
            record.modified_abs_seconds,
            u64::from(record.modified_abs_nanos),
        ]);
    }
    let mut scratch = vec![-1_i64; records.len()];
    let mut selected = vec![-1_i64; records.len()];
    let mut unique_files = -1_i64;
    match unsafe {
        prodex_mojo_log_follow_selection_v1(
            ABI_VERSION,
            rows.as_ptr() as usize as u64,
            i64::try_from(records.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_files).map_err(|_| MojoError::InvalidInput)?,
            scratch.as_mut_ptr() as usize as u64,
            i64::try_from(scratch.len()).map_err(|_| MojoError::InvalidInput)?,
            selected.as_mut_ptr() as usize as u64,
            i64::try_from(selected.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut unique_files as *mut i64) as usize as u64,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let unique_files = usize::try_from(unique_files).map_err(|_| MojoError::InvalidOutput)?;
    if unique_files > max_files || unique_files > records.len() {
        return Err(MojoError::InvalidOutput);
    }
    let mut flags = Vec::with_capacity(selected.len());
    for value in selected {
        match value {
            0 => flags.push(false),
            1 => flags.push(true),
            _ => return Err(MojoError::InvalidOutput),
        }
    }
    Ok(LogFollowSelectionPlan {
        selected: flags,
        unique_files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row<'a>(path: &'a str, source: u8, time: u64) -> LogFollowSelectionInput<'a> {
        LogFollowSelectionInput {
            path: path.as_bytes(),
            source,
            modified_before_unix_epoch: false,
            modified_abs_seconds: time,
            modified_abs_nanos: 0,
        }
    }

    #[test]
    fn compiled_log_follow_selection_reserves_each_source_and_fills_spare_slots() {
        let mut records = Vec::new();
        for index in 0..64_u64 {
            records.push(row(
                Box::leak(format!("runtime-{index:03}").into_boxed_str()),
                0,
                index,
            ));
        }
        for index in 0..10_u64 {
            records.push(row(
                Box::leak(format!("session-{index:03}").into_boxed_str()),
                1,
                index,
            ));
        }
        let plan = log_follow_selection_plan(&records, 32).unwrap();
        assert_eq!(plan.unique_files, 32);
        assert_eq!(
            plan.selected.iter().take(64).filter(|yes| **yes).count(),
            22
        );
        assert_eq!(
            plan.selected.iter().skip(64).filter(|yes| **yes).count(),
            10
        );
        assert!(plan.selected[63]);
        assert!(!plan.selected[0]);
    }

    #[test]
    fn compiled_log_follow_selection_preserves_duplicates_and_exact_small_input() {
        let records = [
            row("same", 0, 1),
            row("same", 0, 2),
            row("same", 1, 3),
            row("other", 1, 4),
        ];
        assert_eq!(
            log_follow_selection_plan(&records, 4),
            Ok(LogFollowSelectionPlan {
                selected: vec![true, true, true, true],
                unique_files: 3,
            }),
        );
        let plan = log_follow_selection_plan(&records, 2).unwrap();
        assert_eq!(plan.unique_files, 2);
        assert_eq!(plan.selected, vec![true, true, false, true]);
        assert_eq!(
            log_follow_selection_plan(&records, 0),
            Ok(LogFollowSelectionPlan {
                selected: vec![false; 4],
                unique_files: 0,
            }),
        );
        assert_eq!(
            log_follow_selection_plan(&[], 32),
            Ok(LogFollowSelectionPlan {
                selected: vec![],
                unique_files: 0,
            }),
        );
    }

    #[test]
    fn compiled_log_follow_selection_abi_rejects_malformed_late_row_without_output() {
        let mut rows = [0_u64; 12];
        let mut scratch = [71_i64; 2];
        let mut selected = [73_i64; 2];
        let mut written = 91_i64;
        let source = rows.as_ptr() as usize as u64;
        let work = scratch.as_mut_ptr() as usize as u64;
        let flags = selected.as_mut_ptr() as usize as u64;
        let count = (&mut written as *mut i64) as usize as u64;
        for (abi, input, length, work_cap, flag_cap, result, expected) in [
            (2, source, 2, 2, 2, count, 4),
            (1, 0, 2, 2, 2, count, 1),
            (1, source, -1, 2, 2, count, 1),
            (1, source, 65_537, 2, 2, count, 1),
            (1, source, 2, 1, 2, count, 1),
            (1, source, 2, 2, 1, count, 1),
            (1, source, 2, 2, 2, 0, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_mojo_log_follow_selection_v1(
                        abi, input, length, 32, work, work_cap, flags, flag_cap, result,
                    )
                },
                expected,
            );
            assert_eq!(scratch, [71; 2]);
            assert_eq!(selected, [73; 2]);
            assert_eq!(written, 91);
        }
        rows[6 + 2] = 2;
        assert_eq!(
            unsafe {
                prodex_mojo_log_follow_selection_v1(
                    1,
                    rows.as_ptr() as usize as u64,
                    2,
                    32,
                    work,
                    2,
                    flags,
                    2,
                    count,
                )
            },
            1,
        );
        assert_eq!(scratch, [71; 2]);
        assert_eq!(selected, [73; 2]);
        assert_eq!(written, 91);
    }
}
