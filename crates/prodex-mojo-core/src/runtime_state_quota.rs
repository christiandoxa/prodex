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
const MODE_STARTUP_PROBE_REFRESH: i64 = 9;

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
pub struct StartupProbeRefreshCandidatePlan {
    pub selected: bool,
    pub remaining: usize,
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
    fn prodex_runtime_state_quota_snapshot_compare_v1(
        abi_version: i64,
        operation: i64,
        facts_address: u64,
        fact_count: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_runtime_state_startup_probe_batch_v1(
        abi_version: i64,
        rows_address: u64,
        candidate_count: i64,
        warm_remaining: i64,
        output_address: u64,
        output_capacity: i64,
        remaining_address: u64,
    ) -> i64;
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

/// Borrowed generic-status comparisons and concrete numeric quota windows.
#[derive(Debug, Clone, Copy)]
pub struct RuntimeQuotaSnapshotCompareFacts {
    pub previous_present: bool,
    pub previous_five_hour_remaining: i64,
    pub previous_five_hour_reset_at: i64,
    pub previous_weekly_remaining: i64,
    pub previous_weekly_reset_at: i64,
    pub next_five_hour_remaining: i64,
    pub next_five_hour_reset_at: i64,
    pub next_weekly_remaining: i64,
    pub next_weekly_reset_at: i64,
    pub five_hour_status_equal: bool,
    pub weekly_status_equal: bool,
    pub plan_type_equal: bool,
    pub previous_checked_at: i64,
    pub now: i64,
    pub touch_interval_seconds: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeQuotaSnapshotComparePlan {
    pub materially_matches: bool,
    pub should_persist: bool,
}

/// Compare normalized numerical quota fields and decide persistence solely
/// in Mojo; host generic status/plan string Eq cannot cross a scalar ABI.
pub fn snapshot_compare_plan(
    facts: RuntimeQuotaSnapshotCompareFacts,
    decide_persistence: bool,
) -> Result<RuntimeQuotaSnapshotComparePlan, MojoError> {
    let fields = [
        facts.previous_five_hour_remaining,
        facts.previous_five_hour_reset_at,
        facts.previous_weekly_remaining,
        facts.previous_weekly_reset_at,
        facts.next_five_hour_remaining,
        facts.next_five_hour_reset_at,
        facts.next_weekly_remaining,
        facts.next_weekly_reset_at,
        i64::from(facts.previous_present),
        i64::from(facts.five_hour_status_equal),
        i64::from(facts.weekly_status_equal),
        i64::from(facts.plan_type_equal),
        facts.previous_checked_at,
        facts.now,
        facts.touch_interval_seconds,
    ];
    let mut output = [-1_i64; 2];
    match unsafe {
        prodex_runtime_state_quota_snapshot_compare_v1(
            ABI_VERSION,
            i64::from(decide_persistence),
            fields.as_ptr() as usize as u64,
            15,
            output.as_mut_ptr() as usize as u64,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    Ok(RuntimeQuotaSnapshotComparePlan {
        materially_matches: bool_output(output[0])?,
        should_persist: bool_output(output[1])?,
    })
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

/// One candidate's externally classified freshness/usable observations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartupProbeRefreshObservation {
    pub probe_fresh: bool,
    pub snapshot_usable: bool,
}

/// An entire bounded batch's ordered selection and remaining warm budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupProbeRefreshBatchPlan {
    pub selected: Vec<bool>,
    pub remaining: usize,
}

pub fn startup_probe_refresh_batch(
    candidates: &[StartupProbeRefreshObservation],
    warm_remaining: usize,
) -> Result<StartupProbeRefreshBatchPlan, MojoError> {
    if candidates.len() > 65_536 {
        return Err(MojoError::InvalidInput);
    }
    let remaining = i64::try_from(warm_remaining).map_err(|_| MojoError::InvalidInput)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(candidates.len().checked_mul(2).ok_or(MojoError::Capacity)?)
        .map_err(|_| MojoError::Capacity)?;
    for candidate in candidates {
        rows.extend([
            i64::from(candidate.probe_fresh),
            i64::from(candidate.snapshot_usable),
        ]);
    }
    let mut selected = vec![-1_i64; candidates.len()];
    let mut next_remaining = -1_i64;
    match unsafe {
        prodex_runtime_state_startup_probe_batch_v1(
            ABI_VERSION,
            rows.as_ptr() as usize as u64,
            i64::try_from(candidates.len()).map_err(|_| MojoError::InvalidInput)?,
            remaining,
            selected.as_mut_ptr() as usize as u64,
            i64::try_from(selected.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut next_remaining as *mut i64) as usize as u64,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let next_remaining = usize::try_from(next_remaining).map_err(|_| MojoError::InvalidOutput)?;
    if next_remaining > warm_remaining {
        return Err(MojoError::InvalidOutput);
    }
    let mut flags = Vec::with_capacity(selected.len());
    for flag in selected {
        match flag {
            0 => flags.push(false),
            1 => flags.push(true),
            _ => return Err(MojoError::InvalidOutput),
        }
    }
    if flags.iter().filter(|selected| **selected).count() != warm_remaining - next_remaining {
        return Err(MojoError::InvalidOutput);
    }
    Ok(StartupProbeRefreshBatchPlan {
        selected: flags,
        remaining: next_remaining,
    })
}

pub fn startup_probe_refresh_candidate_plan(
    probe_fresh: bool,
    snapshot_usable: bool,
    remaining: usize,
) -> Result<StartupProbeRefreshCandidatePlan, MojoError> {
    let remaining = i64::try_from(remaining).map_err(|_| MojoError::InvalidInput)?;
    let output = call(
        MODE_STARTUP_PROBE_REFRESH,
        [
            i64::from(probe_fresh),
            i64::from(snapshot_usable),
            remaining,
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
    let selected = bool_output(output[0])?;
    let next_remaining = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
    if next_remaining > remaining as usize
        || (selected && next_remaining.saturating_add(1) != remaining as usize)
        || (!selected && next_remaining != remaining as usize)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(StartupProbeRefreshCandidatePlan {
        selected,
        remaining: next_remaining,
    })
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
    fn usage_snapshot_retention_matches_saturating_boundary_contract() {
        let boundary = [
            i64::MIN,
            i64::MIN + 1,
            -1000,
            -1,
            0,
            1,
            120,
            i64::MAX - 1,
            i64::MAX,
        ];
        for profile_present in [false, true] {
            for &now in &boundary {
                for &checked_at in &boundary {
                    for &retention_seconds in &boundary {
                        let expected =
                            profile_present && checked_at >= now.saturating_sub(retention_seconds);
                        let actual = usage_snapshot_should_retain(
                            profile_present,
                            checked_at,
                            now,
                            retention_seconds,
                        )
                        .expect("Mojo retention policy must produce a boolean");
                        assert_eq!(
                            actual, expected,
                            "profile_present={profile_present}, now={now}, checked_at={checked_at}, retention_seconds={retention_seconds}"
                        );
                    }
                }
            }
        }
    }

    fn snapshot_fact() -> RuntimeQuotaSnapshotCompareFacts {
        RuntimeQuotaSnapshotCompareFacts {
            previous_present: true,
            previous_five_hour_remaining: 100,
            previous_five_hour_reset_at: i64::MIN,
            previous_weekly_remaining: 0,
            previous_weekly_reset_at: i64::MAX,
            next_five_hour_remaining: 100,
            next_five_hour_reset_at: i64::MIN,
            next_weekly_remaining: 0,
            next_weekly_reset_at: i64::MAX,
            five_hour_status_equal: true,
            weekly_status_equal: true,
            plan_type_equal: true,
            previous_checked_at: 100,
            now: 110,
            touch_interval_seconds: 60,
        }
    }

    #[test]
    fn compiled_quota_snapshot_compare_detects_individual_differences_and_stale_touches() {
        let initial = snapshot_fact();
        assert_eq!(
            snapshot_compare_plan(initial, true),
            Ok(RuntimeQuotaSnapshotComparePlan {
                materially_matches: true,
                should_persist: false,
            }),
        );
        let mut stale = initial;
        stale.now = 161;
        assert_eq!(
            snapshot_compare_plan(stale, true),
            Ok(RuntimeQuotaSnapshotComparePlan {
                materially_matches: true,
                should_persist: true,
            }),
        );
        let mut missing = initial;
        missing.previous_present = false;
        assert_eq!(
            snapshot_compare_plan(missing, true),
            Ok(RuntimeQuotaSnapshotComparePlan {
                materially_matches: false,
                should_persist: true,
            }),
        );
        for mutate in [
            |f: &mut RuntimeQuotaSnapshotCompareFacts| f.next_five_hour_remaining = 99,
            |f: &mut RuntimeQuotaSnapshotCompareFacts| f.next_five_hour_reset_at += 1,
            |f: &mut RuntimeQuotaSnapshotCompareFacts| f.next_weekly_remaining = 1,
            |f: &mut RuntimeQuotaSnapshotCompareFacts| f.next_weekly_reset_at -= 1,
            |f: &mut RuntimeQuotaSnapshotCompareFacts| f.five_hour_status_equal = false,
            |f: &mut RuntimeQuotaSnapshotCompareFacts| f.weekly_status_equal = false,
            |f: &mut RuntimeQuotaSnapshotCompareFacts| f.plan_type_equal = false,
        ] {
            let mut changed = initial;
            mutate(&mut changed);
            assert_eq!(
                snapshot_compare_plan(changed, true),
                Ok(RuntimeQuotaSnapshotComparePlan {
                    materially_matches: false,
                    should_persist: true,
                }),
            );
        }
        assert_eq!(
            snapshot_compare_plan(initial, false),
            Ok(RuntimeQuotaSnapshotComparePlan {
                materially_matches: true,
                should_persist: false,
            }),
        );
        let mut saturated = initial;
        saturated.previous_checked_at = i64::MIN;
        saturated.now = i64::MAX;
        saturated.touch_interval_seconds = i64::MAX;
        assert_eq!(
            snapshot_compare_plan(saturated, true)
                .unwrap()
                .should_persist,
            false
        );
    }

    #[test]
    fn compiled_quota_snapshot_compare_abi_rejects_shape_and_boolean_flags_before_writing() {
        let mut inputs = [0_i64; 15];
        let mut output = [73_i64; 2];
        let source = inputs.as_ptr() as usize as u64;
        let result = output.as_mut_ptr() as usize as u64;
        for (version, operation, address, count, expected) in [
            (2, 0, source, 15, 4),
            (1, -1, source, 15, 1),
            (1, 2, source, 15, 1),
            (1, 0, 0, 15, 1),
            (1, 0, source, 14, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_runtime_state_quota_snapshot_compare_v1(
                        version, operation, address, count, result,
                    )
                },
                expected,
            );
            assert_eq!(output, [73; 2]);
        }
        for index in 8..12 {
            inputs[index] = 2;
            assert_eq!(
                unsafe {
                    prodex_runtime_state_quota_snapshot_compare_v1(
                        1,
                        0,
                        inputs.as_ptr() as usize as u64,
                        15,
                        result,
                    )
                },
                1,
            );
            assert_eq!(output, [73; 2]);
            inputs[index] = 0;
        }
        assert_eq!(
            unsafe { prodex_runtime_state_quota_snapshot_compare_v1(1, 0, source, 15, 0) },
            1,
        );
        assert_eq!(output, [73; 2]);
    }

    #[test]
    fn compiled_startup_probe_batch_retains_order_and_warm_budget() {
        let input = [
            StartupProbeRefreshObservation {
                probe_fresh: true,
                snapshot_usable: false,
            },
            StartupProbeRefreshObservation {
                probe_fresh: false,
                snapshot_usable: true,
            },
            StartupProbeRefreshObservation {
                probe_fresh: false,
                snapshot_usable: false,
            },
            StartupProbeRefreshObservation {
                probe_fresh: false,
                snapshot_usable: false,
            },
            StartupProbeRefreshObservation {
                probe_fresh: false,
                snapshot_usable: false,
            },
        ];
        assert_eq!(
            startup_probe_refresh_batch(&input, 2),
            Ok(StartupProbeRefreshBatchPlan {
                selected: vec![false, false, true, true, false],
                remaining: 0,
            }),
        );
        assert_eq!(
            startup_probe_refresh_batch(&input, 0),
            Ok(StartupProbeRefreshBatchPlan {
                selected: vec![false; 5],
                remaining: 0,
            }),
        );
        assert_eq!(
            startup_probe_refresh_batch(&[], 10),
            Ok(StartupProbeRefreshBatchPlan {
                selected: Vec::new(),
                remaining: 10,
            }),
        );
        let mut many = vec![
            StartupProbeRefreshObservation {
                probe_fresh: true,
                snapshot_usable: true
            };
            65_536
        ];
        many[65_534] = StartupProbeRefreshObservation {
            probe_fresh: false,
            snapshot_usable: false,
        };
        many[65_535] = many[65_534];
        let plan = startup_probe_refresh_batch(&many, 1).unwrap();
        assert_eq!(plan.selected.len(), 65_536);
        assert_eq!(plan.selected.iter().filter(|flag| **flag).count(), 1);
        assert!(plan.selected[65_534]);
        assert!(!plan.selected[65_535]);
        assert_eq!(plan.remaining, 0);
        many.push(many[0]);
        assert_eq!(
            startup_probe_refresh_batch(&many, 1),
            Err(MojoError::InvalidInput)
        );
        assert_eq!(
            startup_probe_refresh_batch(&input, usize::MAX),
            Err(MojoError::InvalidInput),
        );
    }

    #[test]
    fn compiled_startup_probe_batch_abi_rejects_late_invalid_row_without_writes() {
        let mut rows = [0_i64, 0, 1, 0, 0, 0];
        let mut output = [83_i64; 3];
        let mut remaining = 79_i64;
        let source = rows.as_ptr() as usize as u64;
        let destination = output.as_mut_ptr() as usize as u64;
        let rest = (&mut remaining as *mut i64) as usize as u64;
        for (version, fields, count, warm, capacity, remaining_ptr, expected) in [
            (2, source, 3, 2, 3, rest, 4),
            (1, 0, 3, 2, 3, rest, 1),
            (1, source, -1, 2, 3, rest, 1),
            (1, source, 65_537, 2, 3, rest, 1),
            (1, source, 3, -1, 3, rest, 1),
            (1, source, 3, 2, 2, rest, 1),
            (1, source, 3, 2, 3, 0, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_runtime_state_startup_probe_batch_v1(
                        version,
                        fields,
                        count,
                        warm,
                        destination,
                        capacity,
                        remaining_ptr,
                    )
                },
                expected,
            );
            assert_eq!(output, [83; 3]);
            assert_eq!(remaining, 79);
        }
        rows[5] = 2;
        assert_eq!(
            unsafe {
                prodex_runtime_state_startup_probe_batch_v1(
                    1,
                    rows.as_ptr() as usize as u64,
                    3,
                    2,
                    destination,
                    3,
                    rest,
                )
            },
            1,
        );
        assert_eq!(output, [83; 3]);
        assert_eq!(remaining, 79);
    }

    #[test]
    fn startup_probe_refresh_candidate_plan_matches_independent_oracle() {
        for probe_fresh in [false, true] {
            for snapshot_usable in [false, true] {
                for remaining in [0, 1, 2, 17, i64::MAX as usize] {
                    let selected = remaining > 0 && !probe_fresh && !snapshot_usable;
                    let expected = StartupProbeRefreshCandidatePlan {
                        selected,
                        remaining: remaining.saturating_sub(usize::from(selected)),
                    };
                    assert_eq!(
                        startup_probe_refresh_candidate_plan(
                            probe_fresh,
                            snapshot_usable,
                            remaining,
                        )
                        .expect("Mojo startup probe plan must preserve the boundary contract"),
                        expected,
                        "probe_fresh={probe_fresh} snapshot_usable={snapshot_usable} remaining={remaining}",
                    );
                }
            }
        }
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
