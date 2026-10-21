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
#[path = "../tests/src/runtime_state_quota/tests.rs"]
mod tests;
