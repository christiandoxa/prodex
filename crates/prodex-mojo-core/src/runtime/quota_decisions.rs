use crate::MojoError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrecommitBudgetPlan {
    pub attempt_limit: usize,
    pub budget_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaSnapshotPlanInput {
    pub five_hour_status: i64,
    pub five_hour_remaining: i64,
    pub five_hour_reset_at: i64,
    pub weekly_status: i64,
    pub weekly_remaining: i64,
    pub weekly_reset_at: i64,
    pub route_kind: i64,
    pub checked_at: i64,
    pub now: i64,
    pub stale_grace_seconds: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaSnapshotPlan {
    pub five_hour_status: i64,
    pub five_hour_remaining: i64,
    pub five_hour_reset_at: i64,
    pub weekly_status: i64,
    pub weekly_remaining: i64,
    pub weekly_reset_at: i64,
    pub route_band: i64,
    pub hold_active: bool,
    pub hold_expired: bool,
    pub usable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaGatePlanInput {
    pub five_hour_status: i64,
    pub five_hour_reset_at: i64,
    pub weekly_status: i64,
    pub weekly_reset_at: i64,
    pub route_kind: i64,
    pub source: i64,
    pub has_continuation_context: bool,
    pub has_alternative_quota_profile: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaGatePlan {
    pub requires_precommit_live_probe: bool,
    pub requires_live_source_after_probe: bool,
    pub block_reason: i64,
    pub blocking_reset_at: Option<i64>,
    pub initial_decision: i64,
    pub initial_reason: i64,
    pub final_blocked: bool,
    pub final_reason: i64,
}

unsafe extern "C" {
    fn prodex_runtime_precommit_budget_plan_v1(
        continuation: i64,
        pressure_mode: i64,
        standard_attempt_limit: i64,
        standard_budget_ms: i64,
        continuation_attempt_limit: i64,
        continuation_budget_ms: i64,
        pressure_attempt_limit: i64,
        pressure_budget_ms: i64,
        profile_count: i64,
        attempts_per_profile: i64,
        attempt_limit_out: *mut i64,
        budget_ms_out: *mut i64,
    ) -> i64;
    fn prodex_runtime_quota_snapshot_plan_v1(
        five_hour_status: i64,
        five_hour_remaining: i64,
        five_hour_reset_at: i64,
        weekly_status: i64,
        weekly_remaining: i64,
        weekly_reset_at: i64,
        route_kind: i64,
        checked_at: i64,
        now: i64,
        stale_grace_seconds: i64,
        output: *mut i64,
    ) -> i64;
    fn prodex_runtime_quota_gate_plan_v1(
        five_hour_status: i64,
        five_hour_reset_at: i64,
        weekly_status: i64,
        weekly_reset_at: i64,
        route_kind: i64,
        source: i64,
        has_continuation_context: i64,
        has_alternative_quota_profile: i64,
        output: *mut i64,
    ) -> i64;
}

#[allow(clippy::too_many_arguments)]
pub fn precommit_budget_plan(
    continuation: bool,
    pressure_mode: bool,
    standard_attempt_limit: usize,
    standard_budget_ms: u64,
    continuation_attempt_limit: usize,
    continuation_budget_ms: u64,
    pressure_attempt_limit: usize,
    pressure_budget_ms: u64,
    profile_count: usize,
    attempts_per_profile: usize,
) -> Result<PrecommitBudgetPlan, MojoError> {
    let usize_to_i64 = |value| i64::try_from(value).map_err(|_| MojoError::InvalidInput);
    let u64_to_i64 = |value| i64::try_from(value).map_err(|_| MojoError::InvalidInput);
    let mut attempt_limit = 0;
    let mut budget_ms = 0;
    let status = unsafe {
        prodex_runtime_precommit_budget_plan_v1(
            i64::from(continuation),
            i64::from(pressure_mode),
            usize_to_i64(standard_attempt_limit)?,
            u64_to_i64(standard_budget_ms)?,
            usize_to_i64(continuation_attempt_limit)?,
            u64_to_i64(continuation_budget_ms)?,
            usize_to_i64(pressure_attempt_limit)?,
            u64_to_i64(pressure_budget_ms)?,
            usize_to_i64(profile_count)?,
            usize_to_i64(attempts_per_profile)?,
            &mut attempt_limit,
            &mut budget_ms,
        )
    };
    if status != 0 || attempt_limit <= 0 || budget_ms < 0 {
        return Err(MojoError::InvalidOutput);
    }
    Ok(PrecommitBudgetPlan {
        attempt_limit: usize::try_from(attempt_limit).map_err(|_| MojoError::InvalidOutput)?,
        budget_ms: u64::try_from(budget_ms).map_err(|_| MojoError::InvalidOutput)?,
    })
}

pub fn quota_snapshot_plan(input: QuotaSnapshotPlanInput) -> Result<QuotaSnapshotPlan, MojoError> {
    if !(0..=4).contains(&input.five_hour_status)
        || !(0..=4).contains(&input.weekly_status)
        || !(0..=100).contains(&input.five_hour_remaining)
        || !(0..=100).contains(&input.weekly_remaining)
        || !(0..=3).contains(&input.route_kind)
        || input.stale_grace_seconds < 0
    {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [0_i64; 10];
    let status = unsafe {
        prodex_runtime_quota_snapshot_plan_v1(
            input.five_hour_status,
            input.five_hour_remaining,
            input.five_hour_reset_at,
            input.weekly_status,
            input.weekly_remaining,
            input.weekly_reset_at,
            input.route_kind,
            input.checked_at,
            input.now,
            input.stale_grace_seconds,
            output.as_mut_ptr(),
        )
    };
    if status != 0
        || !(0..=4).contains(&output[0])
        || !(0..=100).contains(&output[1])
        || !(0..=4).contains(&output[3])
        || !(0..=100).contains(&output[4])
        || !(0..=4).contains(&output[6])
        || output[7..=9].iter().any(|value| !matches!(value, 0 | 1))
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(QuotaSnapshotPlan {
        five_hour_status: output[0],
        five_hour_remaining: output[1],
        five_hour_reset_at: output[2],
        weekly_status: output[3],
        weekly_remaining: output[4],
        weekly_reset_at: output[5],
        route_band: output[6],
        hold_active: output[7] == 1,
        hold_expired: output[8] == 1,
        usable: output[9] == 1,
    })
}

pub fn quota_gate_plan(input: QuotaGatePlanInput) -> Result<QuotaGatePlan, MojoError> {
    if !(0..=4).contains(&input.five_hour_status)
        || !(0..=4).contains(&input.weekly_status)
        || !(0..=3).contains(&input.route_kind)
        || !(-1..=1).contains(&input.source)
    {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [0_i64; 8];
    let status = unsafe {
        prodex_runtime_quota_gate_plan_v1(
            input.five_hour_status,
            input.five_hour_reset_at,
            input.weekly_status,
            input.weekly_reset_at,
            input.route_kind,
            input.source,
            i64::from(input.has_continuation_context),
            i64::from(input.has_alternative_quota_profile),
            output.as_mut_ptr(),
        )
    };
    if status != 0
        || !matches!(output[0], 0 | 1)
        || !matches!(output[1], 0 | 1)
        || !(0..=3).contains(&output[2])
        || !(0..=2).contains(&output[4])
        || !(0..=3).contains(&output[5])
        || !matches!(output[6], 0 | 1)
        || !(0..=3).contains(&output[7])
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(QuotaGatePlan {
        requires_precommit_live_probe: output[0] == 1,
        requires_live_source_after_probe: output[1] == 1,
        block_reason: output[2],
        blocking_reset_at: (output[3] != i64::MIN).then_some(output[3]),
        initial_decision: output[4],
        initial_reason: output[5],
        final_blocked: output[6] == 1,
        final_reason: output[7],
    })
}
