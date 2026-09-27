use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MODE_TIMESTAMP_PERSIST: i64 = 0;
const MODE_FRESHNESS: i64 = 1;
const MODE_SNAPSHOT_USABLE: i64 = 2;
const MODE_PROBE_APPLY: i64 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeCacheFreshness {
    Fresh,
    StaleUsable,
    Expired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotUsabilityPlan {
    pub usable: bool,
    pub hold_active: bool,
    pub hold_expired: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeUsageSnapshotApplyPlan {
    pub snapshot_should_persist: bool,
    pub blocking_reset_at: Option<i64>,
    pub retry_backoff_until: Option<i64>,
    pub retry_backoff_changed: bool,
}

unsafe extern "C" {
    fn prodex_runtime_state_quota_policy_v1(
        abi_version: i64,
        mode: i64,
        input0: i64,
        input1: i64,
        input2: i64,
        input3: i64,
        input4: i64,
        input5: i64,
        input6: i64,
        input7: i64,
        input8: i64,
        input9: i64,
        input10: i64,
        output_address: u64,
    ) -> i64;
}

fn call(mode: i64, input: [i64; 11]) -> Result<[i64; 8], MojoError> {
    let mut output = [0_i64; 8];
    let status = unsafe {
        prodex_runtime_state_quota_policy_v1(
            ABI_VERSION,
            mode,
            input[0],
            input[1],
            input[2],
            input[3],
            input[4],
            input[5],
            input[6],
            input[7],
            input[8],
            input[9],
            input[10],
            output.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => Ok(output),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn bool_output(value: i64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn timestamp_touch_should_persist(
    timestamp: i64,
    now: i64,
    persist_interval_seconds: i64,
) -> Result<bool, MojoError> {
    bool_output(
        call(
            MODE_TIMESTAMP_PERSIST,
            [
                timestamp,
                now,
                persist_interval_seconds,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ],
        )?[0],
    )
}

pub fn probe_cache_freshness(
    checked_at: i64,
    now: i64,
    fresh_seconds: i64,
    stale_grace_seconds: i64,
) -> Result<ProbeCacheFreshness, MojoError> {
    match call(
        MODE_FRESHNESS,
        [
            checked_at,
            now,
            fresh_seconds,
            stale_grace_seconds,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ],
    )?[0]
    {
        0 => Ok(ProbeCacheFreshness::Fresh),
        1 => Ok(ProbeCacheFreshness::StaleUsable),
        2 => Ok(ProbeCacheFreshness::Expired),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn snapshot_usability(
    five_hour_exhausted: bool,
    five_hour_reset_at: i64,
    weekly_exhausted: bool,
    weekly_reset_at: i64,
    checked_at: i64,
    now: i64,
    stale_grace_seconds: i64,
) -> Result<SnapshotUsabilityPlan, MojoError> {
    let output = call(
        MODE_SNAPSHOT_USABLE,
        [
            i64::from(five_hour_exhausted),
            five_hour_reset_at,
            i64::from(weekly_exhausted),
            weekly_reset_at,
            checked_at,
            now,
            stale_grace_seconds,
            0,
            0,
            0,
            0,
        ],
    )?;
    Ok(SnapshotUsabilityPlan {
        usable: bool_output(output[0])?,
        hold_active: bool_output(output[1])?,
        hold_expired: bool_output(output[2])?,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn probe_usage_snapshot_apply_plan(
    previous_snapshot_present: bool,
    snapshots_materially_match: bool,
    previous_checked_at: i64,
    previous_retry_backoff_until: Option<i64>,
    blocking_reset_at: Option<i64>,
    quota_blocked: bool,
    now: i64,
    quota_quarantine_fallback_seconds: i64,
    touch_persist_interval_seconds: i64,
) -> Result<ProbeUsageSnapshotApplyPlan, MojoError> {
    let output = call(
        MODE_PROBE_APPLY,
        [
            i64::from(previous_snapshot_present),
            i64::from(snapshots_materially_match),
            previous_checked_at,
            i64::from(previous_retry_backoff_until.is_some()),
            previous_retry_backoff_until.unwrap_or_default(),
            i64::from(blocking_reset_at.is_some()),
            blocking_reset_at.unwrap_or_default(),
            i64::from(quota_blocked),
            now,
            quota_quarantine_fallback_seconds,
            touch_persist_interval_seconds,
        ],
    )?;
    let blocking_reset_at = match bool_output(output[1])? {
        true => Some(output[2]),
        false => None,
    };
    let retry_backoff_until = match bool_output(output[3])? {
        true => Some(output[4]),
        false => None,
    };
    Ok(ProbeUsageSnapshotApplyPlan {
        snapshot_should_persist: bool_output(output[0])?,
        blocking_reset_at,
        retry_backoff_until,
        retry_backoff_changed: bool_output(output[5])?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_state_quota_kernel_smoke() {
        assert!(timestamp_touch_should_persist(0, 11, 10).unwrap());
        assert_eq!(
            probe_cache_freshness(90, 100, 5, 20).unwrap(),
            ProbeCacheFreshness::StaleUsable
        );
        assert_eq!(
            snapshot_usability(true, 120, false, i64::MAX, 0, 100, 10).unwrap(),
            SnapshotUsabilityPlan {
                usable: true,
                hold_active: true,
                hold_expired: false,
            }
        );
        let plan = probe_usage_snapshot_apply_plan(
            true,
            true,
            95,
            Some(110),
            Some(120),
            true,
            100,
            30,
            10,
        )
        .unwrap();
        assert!(!plan.snapshot_should_persist);
        assert_eq!(plan.blocking_reset_at, Some(120));
        assert_eq!(plan.retry_backoff_until, Some(120));
        assert!(plan.retry_backoff_changed);
    }
}
