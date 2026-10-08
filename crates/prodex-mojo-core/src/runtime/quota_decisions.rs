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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum CompactRetryStage {
    Start = 0,
    AfterAutoRedeem = 1,
    AfterBackoff = 2,
    AfterQuotaFallback = 3,
    AfterAffinityRecovery = 4,
    AfterAffinityRelease = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompactRetryDecisionInput {
    pub stage: CompactRetryStage,
    pub overload: bool,
    pub auto_redeemed: bool,
    pub owner_retry_used: bool,
    pub owner_match: bool,
    pub quota_fallback_available: Option<bool>,
    pub previous_response_owner: bool,
    pub hard_affinity: bool,
    pub committed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactRetryAction {
    TryAutoRedeem,
    RetryOwnerOverload,
    MarkRetryBackoff,
    CheckQuotaFallback,
    ReturnQuotaExhausted,
    RecoverHardAffinity,
    ReleaseQuotaState,
    ReturnAffinityFailure,
    RotateQuota,
    RotateOverload,
    ReturnCommitted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactRetryReason {
    Quota,
    Overload,
}

impl CompactRetryReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::Quota => "quota",
            Self::Overload => "overload",
        }
    }

    pub fn recovery_label(self) -> &'static str {
        match self {
            Self::Quota => "compact_quota",
            Self::Overload => "compact_overload",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompactRetryDecision {
    pub action: CompactRetryAction,
    pub reason: CompactRetryReason,
}

unsafe extern "C" {
    fn prodex_runtime_compact_retry_decision_v1(
        stage: i64,
        overload: i64,
        auto_redeemed: i64,
        owner_retry_used: i64,
        owner_match: i64,
        quota_fallback_available: i64,
        previous_response_owner: i64,
        hard_affinity: i64,
        committed: i64,
        output: *mut i64,
    ) -> i64;
    fn prodex_runtime_precommit_budget_exhausted_v1(
        attempts: u64,
        elapsed_ms: u64,
        attempt_limit: u64,
        budget_ms: u64,
    ) -> i64;
    fn prodex_runtime_precommit_budget_plan_v2(
        continuation: i64,
        pressure_mode: i64,
        standard_attempt_limit: u64,
        standard_budget_ms: u64,
        continuation_attempt_limit: u64,
        continuation_budget_ms: u64,
        pressure_attempt_limit: u64,
        pressure_budget_ms: u64,
        profile_count: u64,
        attempts_per_profile: u64,
        attempt_limit_cap: u64,
        attempt_limit_out: *mut u64,
        budget_ms_out: *mut u64,
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

pub fn compact_retry_decision(
    input: CompactRetryDecisionInput,
) -> Result<CompactRetryDecision, MojoError> {
    let bool_tag = i64::from;
    let quota_fallback_available = input.quota_fallback_available.map(bool_tag).unwrap_or(-1);
    let mut output = [i64::MIN; 2];
    let status = unsafe {
        prodex_runtime_compact_retry_decision_v1(
            input.stage as i64,
            bool_tag(input.overload),
            bool_tag(input.auto_redeemed),
            bool_tag(input.owner_retry_used),
            bool_tag(input.owner_match),
            quota_fallback_available,
            bool_tag(input.previous_response_owner),
            bool_tag(input.hard_affinity),
            bool_tag(input.committed),
            output.as_mut_ptr(),
        )
    };
    if status != 0 {
        return Err(MojoError::InvalidInput);
    }
    let action = match output[0] {
        1 => CompactRetryAction::TryAutoRedeem,
        2 => CompactRetryAction::RetryOwnerOverload,
        3 => CompactRetryAction::MarkRetryBackoff,
        4 => CompactRetryAction::CheckQuotaFallback,
        5 => CompactRetryAction::ReturnQuotaExhausted,
        6 => CompactRetryAction::RecoverHardAffinity,
        7 => CompactRetryAction::ReleaseQuotaState,
        8 => CompactRetryAction::ReturnAffinityFailure,
        9 => CompactRetryAction::RotateQuota,
        10 => CompactRetryAction::RotateOverload,
        11 => CompactRetryAction::ReturnCommitted,
        _ => return Err(MojoError::InvalidOutput),
    };
    let reason = match output[1] {
        0 => CompactRetryReason::Quota,
        1 => CompactRetryReason::Overload,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(CompactRetryDecision { action, reason })
}

/// Canonical retry exhaustion; elapsed time alone cannot reject unsent work.
pub fn precommit_budget_exhausted(
    attempts: usize,
    elapsed_ms: u64,
    attempt_limit: usize,
    budget_ms: u64,
) -> Result<bool, MojoError> {
    let status = unsafe {
        prodex_runtime_precommit_budget_exhausted_v1(
            u64::try_from(attempts).unwrap_or(u64::MAX),
            elapsed_ms,
            u64::try_from(attempt_limit).unwrap_or(u64::MAX),
            budget_ms,
        )
    };
    match status {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
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
    let usize_to_u64 = |value| u64::try_from(value).unwrap_or(u64::MAX);
    let attempt_limit_cap = usize_to_u64(usize::MAX);
    let mut attempt_limit = 0_u64;
    let mut budget_ms = 0_u64;
    let status = unsafe {
        prodex_runtime_precommit_budget_plan_v2(
            i64::from(continuation),
            i64::from(pressure_mode),
            usize_to_u64(standard_attempt_limit),
            standard_budget_ms,
            usize_to_u64(continuation_attempt_limit),
            continuation_budget_ms,
            usize_to_u64(pressure_attempt_limit),
            pressure_budget_ms,
            usize_to_u64(profile_count),
            usize_to_u64(attempts_per_profile),
            attempt_limit_cap,
            &mut attempt_limit,
            &mut budget_ms,
        )
    };
    if status != 0 || attempt_limit == 0 || attempt_limit > attempt_limit_cap {
        return Err(MojoError::InvalidOutput);
    }
    Ok(PrecommitBudgetPlan {
        attempt_limit: usize::try_from(attempt_limit).map_err(|_| MojoError::InvalidOutput)?,
        budget_ms,
    })
}

/// Snapshot values are already classified observations, not percentage-policy
/// inputs. Preserve signed remaining values exactly until an expired or unknown
/// window becomes neutral, matching the existing Rust snapshot contract.
pub fn quota_snapshot_plan(input: QuotaSnapshotPlanInput) -> Result<QuotaSnapshotPlan, MojoError> {
    if !(0..=4).contains(&input.five_hour_status)
        || !(0..=4).contains(&input.weekly_status)
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
        || !(0..=4).contains(&output[3])
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

#[cfg(all(test, feature = "mojo-runtime"))]
mod tests {
    use super::*;

    fn compact_retry_input(stage: CompactRetryStage) -> CompactRetryDecisionInput {
        CompactRetryDecisionInput {
            stage,
            overload: false,
            auto_redeemed: false,
            owner_retry_used: false,
            owner_match: false,
            quota_fallback_available: None,
            previous_response_owner: false,
            hard_affinity: false,
            committed: false,
        }
    }

    #[test]
    fn compact_retry_stages_preserve_reason_affinity_and_exhaustion() {
        let mut input = compact_retry_input(CompactRetryStage::Start);
        assert_eq!(
            compact_retry_decision(input),
            Ok(CompactRetryDecision {
                action: CompactRetryAction::TryAutoRedeem,
                reason: CompactRetryReason::Quota,
            })
        );

        input.stage = CompactRetryStage::AfterAutoRedeem;
        assert_eq!(
            compact_retry_decision(input).unwrap().action,
            CompactRetryAction::MarkRetryBackoff
        );

        input.stage = CompactRetryStage::Start;
        input.overload = true;
        input.owner_match = true;
        assert_eq!(
            compact_retry_decision(input),
            Ok(CompactRetryDecision {
                action: CompactRetryAction::RetryOwnerOverload,
                reason: CompactRetryReason::Overload,
            })
        );

        input.stage = CompactRetryStage::AfterBackoff;
        input.owner_match = false;
        assert_eq!(
            compact_retry_decision(input).unwrap().action,
            CompactRetryAction::RotateOverload
        );
        input.hard_affinity = true;
        assert_eq!(
            compact_retry_decision(input).unwrap().action,
            CompactRetryAction::RecoverHardAffinity
        );

        input = compact_retry_input(CompactRetryStage::AfterBackoff);
        assert_eq!(
            compact_retry_decision(input).unwrap().action,
            CompactRetryAction::CheckQuotaFallback
        );
        input.stage = CompactRetryStage::AfterQuotaFallback;
        input.quota_fallback_available = Some(false);
        assert_eq!(
            compact_retry_decision(input).unwrap().action,
            CompactRetryAction::ReturnQuotaExhausted
        );
        input.quota_fallback_available = Some(true);
        input.hard_affinity = true;
        assert_eq!(
            compact_retry_decision(input).unwrap().action,
            CompactRetryAction::RecoverHardAffinity
        );
        input.stage = CompactRetryStage::AfterAffinityRecovery;
        input.quota_fallback_available = None;
        input.previous_response_owner = true;
        assert_eq!(
            compact_retry_decision(input).unwrap().action,
            CompactRetryAction::ReturnAffinityFailure
        );
        input.previous_response_owner = false;
        assert_eq!(
            compact_retry_decision(input).unwrap().action,
            CompactRetryAction::ReleaseQuotaState
        );
        input.stage = CompactRetryStage::AfterAffinityRelease;
        assert_eq!(
            compact_retry_decision(input).unwrap().action,
            CompactRetryAction::ReturnAffinityFailure
        );
        input.hard_affinity = false;
        assert_eq!(
            compact_retry_decision(input).unwrap().action,
            CompactRetryAction::RotateQuota
        );
        input.committed = true;
        assert_eq!(
            compact_retry_decision(input).unwrap().action,
            CompactRetryAction::ReturnCommitted
        );
    }

    #[test]
    fn compact_retry_abi_rejects_missing_fallback_observation() {
        let mut input = compact_retry_input(CompactRetryStage::AfterQuotaFallback);
        assert_eq!(compact_retry_decision(input), Err(MojoError::InvalidInput));
        input.quota_fallback_available = Some(true);
        input.overload = true;
        assert_eq!(compact_retry_decision(input), Err(MojoError::InvalidInput));
    }

    #[test]
    fn compact_retry_raw_abi_rejects_out_of_range_stage_without_output() {
        let mut output = [i64::MIN; 2];
        let status = unsafe {
            prodex_runtime_compact_retry_decision_v1(
                6,
                0,
                0,
                0,
                0,
                -1,
                0,
                0,
                0,
                output.as_mut_ptr(),
            )
        };
        assert_eq!(status, 1);
        assert_eq!(output, [i64::MIN; 2]);
    }
}
