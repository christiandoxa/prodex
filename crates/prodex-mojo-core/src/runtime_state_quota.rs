use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MODE_TIMESTAMP_PERSIST: i64 = 0;
const MODE_FRESHNESS: i64 = 1;
const MODE_SNAPSHOT_USABLE: i64 = 2;
const MODE_PROBE_APPLY: i64 = 3;
const MODE_CACHED_SOURCE: i64 = 4;
const MODE_MODEL_CACHED_SOURCE: i64 = 5;
const MODE_MODEL_FINALIZE: i64 = 6;
const MODE_UNKNOWN_WINDOW: i64 = 7;
const MODE_USAGE_SNAPSHOT_RETAIN: i64 = 8;

pub const CACHED_MODEL_STANDARD: i64 = 0;
pub const CACHED_MODEL_LUNA: i64 = 1;
pub const CACHED_MODEL_RETIRED: i64 = 2;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CachedQuotaSummaryKind {
    Unknown,
    Live,
    Snapshot,
    Retired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CachedQuotaSourceKind {
    None,
    Live,
    Snapshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CachedQuotaSummaryPlan {
    pub summary: CachedQuotaSummaryKind,
    pub source: CachedQuotaSourceKind,
}

/// Inputs for Mojo's stable runtime-quota log-field formatter.
pub struct QuotaSummaryLogFields<'a> {
    /// Mojo-owned quota pressure-band label.
    pub pressure_band: &'a str,
    /// Mojo-owned five-hour window status label.
    pub five_hour_status: &'a str,
    /// Five-hour remaining percentage.
    pub five_hour_remaining: i64,
    /// Five-hour reset timestamp.
    pub five_hour_reset_at: i64,
    /// Mojo-owned weekly window status label.
    pub weekly_status: &'a str,
    /// Weekly remaining percentage.
    pub weekly_remaining: i64,
    /// Weekly reset timestamp.
    pub weekly_reset_at: i64,
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
    fn prodex_runtime_state_quota_summary_log_fields_v1(
        abi_version: i64,
        band_address: u64,
        band_length: i64,
        five_hour_status_address: u64,
        five_hour_status_length: i64,
        five_hour_remaining: i64,
        five_hour_reset_at: i64,
        weekly_status_address: u64,
        weekly_status_length: i64,
        weekly_remaining: i64,
        weekly_reset_at: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
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

pub fn usage_snapshot_should_retain(
    profile_present: bool,
    checked_at: i64,
    now: i64,
    retention_seconds: i64,
) -> Result<bool, MojoError> {
    bool_output(
        call(
            MODE_USAGE_SNAPSHOT_RETAIN,
            [
                i64::from(profile_present),
                checked_at,
                now,
                retention_seconds,
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

fn cached_summary_plan(output: [i64; 8]) -> Result<CachedQuotaSummaryPlan, MojoError> {
    let summary = match output[0] {
        0 => CachedQuotaSummaryKind::Unknown,
        1 => CachedQuotaSummaryKind::Live,
        2 => CachedQuotaSummaryKind::Snapshot,
        3 => CachedQuotaSummaryKind::Retired,
        _ => return Err(MojoError::InvalidOutput),
    };
    let source = match output[1] {
        0 => CachedQuotaSourceKind::None,
        1 => CachedQuotaSourceKind::Live,
        2 => CachedQuotaSourceKind::Snapshot,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(CachedQuotaSummaryPlan { summary, source })
}

pub fn cached_summary_source_plan(
    live_present: bool,
    snapshot_present: bool,
    snapshot_usable: bool,
) -> Result<CachedQuotaSummaryPlan, MojoError> {
    cached_summary_plan(call(
        MODE_CACHED_SOURCE,
        [
            i64::from(live_present),
            i64::from(snapshot_present),
            i64::from(snapshot_usable),
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ],
    )?)
}

pub fn cached_model_summary_source_plan(
    model_kind: i64,
    live_present: bool,
    snapshot_present: bool,
    snapshot_usable: bool,
    requested_model_present: bool,
    snapshot_model_pair_present: bool,
) -> Result<CachedQuotaSummaryPlan, MojoError> {
    cached_summary_plan(call(
        MODE_MODEL_CACHED_SOURCE,
        [
            model_kind,
            i64::from(live_present),
            i64::from(snapshot_present),
            i64::from(snapshot_usable),
            i64::from(requested_model_present),
            i64::from(snapshot_model_pair_present),
            0,
            0,
            0,
            0,
            0,
        ],
    )?)
}

pub fn cached_model_summary_force_unknown(
    model_kind: i64,
    selected_band_exhausted: bool,
) -> Result<bool, MojoError> {
    bool_output(
        call(
            MODE_MODEL_FINALIZE,
            [
                model_kind,
                i64::from(selected_band_exhausted),
                0,
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

pub fn unknown_window_override(
    used_percent_present: bool,
    reset_at: Option<i64>,
) -> Result<Option<i64>, MojoError> {
    let output = call(
        MODE_UNKNOWN_WINDOW,
        [
            i64::from(used_percent_present),
            i64::from(reset_at.is_some()),
            reset_at.unwrap_or_default(),
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ],
    )?;
    Ok(match bool_output(output[0])? {
        true => Some(output[1]),
        false => None,
    })
}

/// Format runtime quota log fields through the versioned Mojo quota ABI.
pub fn format_quota_summary_log_fields(
    fields: QuotaSummaryLogFields<'_>,
) -> Result<String, MojoError> {
    const OUTPUT_CAPACITY: usize = 512;
    let mut output = [0_u8; OUTPUT_CAPACITY];
    let mut written = -1_i64;
    let band_length =
        i64::try_from(fields.pressure_band.len()).map_err(|_| MojoError::InvalidInput)?;
    let five_hour_status_length =
        i64::try_from(fields.five_hour_status.len()).map_err(|_| MojoError::InvalidInput)?;
    let weekly_status_length =
        i64::try_from(fields.weekly_status.len()).map_err(|_| MojoError::InvalidInput)?;
    let status = unsafe {
        prodex_runtime_state_quota_summary_log_fields_v1(
            ABI_VERSION,
            fields.pressure_band.as_ptr() as usize as u64,
            band_length,
            fields.five_hour_status.as_ptr() as usize as u64,
            five_hour_status_length,
            fields.five_hour_remaining,
            fields.five_hour_reset_at,
            fields.weekly_status.as_ptr() as usize as u64,
            weekly_status_length,
            fields.weekly_remaining,
            fields.weekly_reset_at,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        2 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    let bytes = output.get(..written).ok_or(MojoError::InvalidOutput)?;
    String::from_utf8(bytes.to_vec()).map_err(|_| MojoError::InvalidOutput)
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
    fn quota_summary_log_formatter_preserves_extremes_and_rejects_invalid_labels() {
        assert_eq!(
            format_quota_summary_log_fields(QuotaSummaryLogFields {
                pressure_band: "quota_unknown",
                five_hour_status: "critical",
                five_hour_remaining: i64::MIN,
                five_hour_reset_at: i64::MAX,
                weekly_status: "exhausted",
                weekly_remaining: 0,
                weekly_reset_at: -1,
            })
            .unwrap(),
            "quota_band=quota_unknown five_hour_status=critical five_hour_remaining=-9223372036854775808 five_hour_reset_at=9223372036854775807 weekly_status=exhausted weekly_remaining=0 weekly_reset_at=-1",
        );

        let too_long = "z".repeat(65);
        assert_eq!(
            format_quota_summary_log_fields(QuotaSummaryLogFields {
                pressure_band: &too_long,
                five_hour_status: "critical",
                five_hour_remaining: 10,
                five_hour_reset_at: 20,
                weekly_status: "ready",
                weekly_remaining: 30,
                weekly_reset_at: 40,
            }),
            Err(MojoError::InvalidInput)
        );
        assert_eq!(
            format_quota_summary_log_fields(QuotaSummaryLogFields {
                pressure_band: "quota_ready",
                five_hour_status: "",
                five_hour_remaining: 10,
                five_hour_reset_at: 20,
                weekly_status: "ready",
                weekly_remaining: 30,
                weekly_reset_at: 40,
            }),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn runtime_state_quota_kernel_smoke() {
        assert!(timestamp_touch_should_persist(0, 11, 10).unwrap());
        assert!(usage_snapshot_should_retain(true, 130, 250, 120).unwrap());
        assert!(!usage_snapshot_should_retain(true, 129, 250, 120).unwrap());
        assert!(!usage_snapshot_should_retain(false, 200, 250, 120).unwrap());
        assert!(usage_snapshot_should_retain(true, i64::MIN, i64::MIN, i64::MAX).unwrap());
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

        assert_eq!(
            cached_summary_source_plan(false, true, true).unwrap(),
            CachedQuotaSummaryPlan {
                summary: CachedQuotaSummaryKind::Snapshot,
                source: CachedQuotaSourceKind::Snapshot,
            }
        );
        assert_eq!(
            cached_model_summary_source_plan(CACHED_MODEL_RETIRED, false, true, true, true, false,)
                .unwrap(),
            CachedQuotaSummaryPlan {
                summary: CachedQuotaSummaryKind::Retired,
                source: CachedQuotaSourceKind::Snapshot,
            }
        );
        assert!(cached_model_summary_force_unknown(CACHED_MODEL_LUNA, true).unwrap());
        assert_eq!(
            unknown_window_override(false, Some(123)).unwrap(),
            Some(123)
        );
        assert_eq!(unknown_window_override(true, Some(123)).unwrap(), None);
    }
}
