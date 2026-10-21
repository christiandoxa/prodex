//! Compiled persisted quota snapshot to usage-window reconstruction policy.
use crate::MojoError;

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_runtime_quota_snapshot_rebuild_v1(
        abi_version: i64,
        facts_address: u64,
        fact_count: i64,
        output_address: u64,
        output_capacity: i64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaSnapshotRebuildWindow {
    pub include: bool,
    pub used_percent: i64,
    pub include_reset: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaSnapshotRebuildPlan {
    pub five_hour: QuotaSnapshotRebuildWindow,
    pub weekly: QuotaSnapshotRebuildWindow,
}

/// Status tags are Ready=0, Thin=1, Critical=2, Exhausted=3, Unknown=4.
/// Mojo exclusively owns window inclusion, saturation and reset presence.
pub fn quota_snapshot_rebuild_plan(facts: [i64; 6]) -> Result<QuotaSnapshotRebuildPlan, MojoError> {
    let mut output = [-1_i64; 6];
    let status = unsafe {
        prodex_runtime_quota_snapshot_rebuild_v1(
            ABI_VERSION,
            facts.as_ptr() as usize as u64,
            6,
            output.as_mut_ptr() as usize as u64,
            6,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let decode = |input: [i64; 3]| -> Result<QuotaSnapshotRebuildWindow, MojoError> {
        let [include, used_percent, include_reset] = input;
        let include = crate::launch::boolean(include)?;
        let include_reset = crate::launch::boolean(include_reset)?;
        if !(0..=100).contains(&used_percent) {
            return Err(MojoError::InvalidOutput);
        }
        if !include && (used_percent != 0 || include_reset) {
            return Err(MojoError::InvalidOutput);
        }
        Ok(QuotaSnapshotRebuildWindow {
            include,
            used_percent,
            include_reset,
        })
    };
    let five_hour = decode([output[0], output[1], output[2]])?;
    let weekly = decode([output[3], output[4], output[5]])?;
    Ok(QuotaSnapshotRebuildPlan { five_hour, weekly })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_snapshot_rebuild_handles_extremes_and_unknown_windows() {
        assert_eq!(
            quota_snapshot_rebuild_plan([0, 60, 3600, 4, i64::MIN, i64::MAX]),
            Ok(QuotaSnapshotRebuildPlan {
                five_hour: QuotaSnapshotRebuildWindow {
                    include: true,
                    used_percent: 40,
                    include_reset: true,
                },
                weekly: QuotaSnapshotRebuildWindow {
                    include: false,
                    used_percent: 0,
                    include_reset: false,
                },
            })
        );
        assert_eq!(
            quota_snapshot_rebuild_plan([3, i64::MIN, i64::MAX, 1, i64::MAX, 2]),
            Ok(QuotaSnapshotRebuildPlan {
                five_hour: QuotaSnapshotRebuildWindow {
                    include: true,
                    used_percent: 100,
                    include_reset: false,
                },
                weekly: QuotaSnapshotRebuildWindow {
                    include: true,
                    used_percent: 0,
                    include_reset: true,
                },
            })
        );
        assert_eq!(
            quota_snapshot_rebuild_plan([5, 0, 0, 0, 0, 0]),
            Err(MojoError::InvalidInput)
        );
    }
}
