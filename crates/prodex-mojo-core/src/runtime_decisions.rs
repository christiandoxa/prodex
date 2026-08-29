mod calibration;
mod tuning;
pub use calibration::{
    SmartContextCalibrationBucket, SmartContextCalibrationSample, SmartContextCalibrationUsage,
    smart_context_calibration_models_match, smart_context_calibration_observed_input,
};
pub use tuning::{
    RuntimeTuningCapacityDefaults, RuntimeTuningDefaults, runtime_tuning_capacity_defaults,
    runtime_tuning_defaults,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextTokenUsageInput {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextTokenUsageSummary {
    pub observed_input_tokens: u64,
    pub observed_cached_input_tokens: u64,
    pub observed_output_tokens: u64,
    pub observed_reasoning_tokens: u64,
    pub last_input_tokens: u64,
    pub last_accounted_input_tokens: u64,
    pub last_observed_context_tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextTokenAccountingInput {
    pub model_context_window_tokens: Option<u64>,
    pub reserved_output_tokens: u64,
    pub current_input_tokens: u64,
    pub estimated_current_request_tokens: u64,
    pub observed_input_tokens: u64,
    pub observed_cached_input_tokens: u64,
    pub observed_output_tokens: u64,
    pub observed_reasoning_tokens: u64,
    pub last_accounted_input_tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextTokenAccountingSummary {
    pub observed_uncached_input_tokens: u64,
    pub observed_total_tokens: u64,
    pub observed_context_tokens: u64,
    pub current_request_accounted_tokens: u64,
    pub effective_input_tokens: u64,
    pub effective_input_source: i64,
    pub available_context_tokens: Option<u64>,
    pub risk_bits: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextCalibratedEstimateInput {
    pub body_bytes: u64,
    pub baseline_estimate: u64,
    pub observed_accounted_input: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextAdaptiveBudgetPlan {
    pub tier: i64,
    pub mode: i64,
    pub max_inline_bytes: u64,
    pub max_rehydrate_tokens: u64,
    pub reason_bits: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextAdaptiveBudgetPlanInput {
    pub available_context_tokens: Option<u64>,
    pub exactness_required: bool,
    pub static_context_changed: bool,
    pub missing_rehydrate_refs: bool,
    pub unknown_token_window: bool,
    pub unsafe_accounting: bool,
    pub safe_rewrites: usize,
    pub fallback_rewrites: usize,
    pub saved_tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextBudgetAdjustment {
    pub max_inline_bytes: u64,
    pub max_rehydrate_tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextRewriteTelemetryInput {
    pub body_bytes_before: u64,
    pub body_bytes_after: u64,
    pub tokens_before: u64,
    pub tokens_after: u64,
    pub token_count_source: i64,
    pub safe: bool,
    pub fallback: bool,
    pub quality_risk: bool,
}

pub const SMART_CONTEXT_BUDGET_MODE_EXACT: i64 = 0;
pub const SMART_CONTEXT_BUDGET_MODE_LARGE: i64 = 1;
pub const SMART_CONTEXT_BUDGET_MODE_CONDENSED: i64 = 2;
pub const SMART_CONTEXT_BUDGET_MODE_MINIMAL: i64 = 3;
pub const SMART_CONTEXT_BUDGET_DECISION_NO_CHANGE: i64 = 0;
pub const SMART_CONTEXT_BUDGET_DECISION_RELAX: i64 = 1;
pub const SMART_CONTEXT_BUDGET_DECISION_TIGHTEN: i64 = 2;
pub const SMART_CONTEXT_POLICY_REASON_EXACTNESS_REQUIRED: u64 = 1 << 0;
pub const SMART_CONTEXT_POLICY_REASON_STATIC_CONTEXT_CHANGED: u64 = 1 << 1;
pub const SMART_CONTEXT_POLICY_REASON_MISSING_REHYDRATE_REFS: u64 = 1 << 2;
pub const SMART_CONTEXT_POLICY_REASON_UNKNOWN_TOKEN_WINDOW: u64 = 1 << 3;
pub const SMART_CONTEXT_POLICY_REASON_UNSAFE_ACCOUNTING: u64 = 1 << 4;
pub const SMART_CONTEXT_POLICY_REASON_RECENT_REWRITE_SAVINGS_SAFE: u64 = 1 << 5;
pub const SMART_CONTEXT_POLICY_REASON_PLENTY_OF_BUDGET: u64 = 1 << 6;
pub const SMART_CONTEXT_POLICY_REASON_MODERATE_BUDGET: u64 = 1 << 7;
pub const SMART_CONTEXT_POLICY_REASON_TIGHT_BUDGET: u64 = 1 << 8;
pub const SMART_CONTEXT_POLICY_REASON_CRITICAL_BUDGET: u64 = 1 << 9;
pub const SMART_CONTEXT_POLICY_REASON_ALL: u64 = (1 << 10) - 1;

pub const SMART_CONTEXT_TOKEN_ACCOUNTING_MAX_COUNT: usize = 256;
pub const SMART_CONTEXT_ACCOUNTING_SOURCE_CURRENT_TOKENS: i64 = 0;
pub const SMART_CONTEXT_ACCOUNTING_SOURCE_BODY_ESTIMATE: i64 = 1;
pub const SMART_CONTEXT_ACCOUNTING_SOURCE_OBSERVED_HISTORY: i64 = 2;
pub const SMART_CONTEXT_ACCOUNTING_SOURCE_UNKNOWN: i64 = 3;
pub const SMART_CONTEXT_ACCOUNTING_RISK_UNKNOWN_WINDOW: u64 = 1 << 0;
pub const SMART_CONTEXT_ACCOUNTING_RISK_ZERO_WINDOW: u64 = 1 << 1;
pub const SMART_CONTEXT_ACCOUNTING_RISK_RESERVED_OUTPUT: u64 = 1 << 2;
pub const SMART_CONTEXT_ACCOUNTING_RISK_UNKNOWN_INPUT: u64 = 1 << 3;
pub const SMART_CONTEXT_ACCOUNTING_RISK_ALL: u64 = (1 << 4) - 1;

pub const OPTIMISTIC_CANDIDATE_KEEP: i64 = 0;
pub const OPTIMISTIC_CANDIDATE_AUTH_FAILURE: i64 = 1;
pub const OPTIMISTIC_CANDIDATE_SELECTION_BACKOFF: i64 = 2;
pub const OPTIMISTIC_CANDIDATE_ROUTE_CIRCUIT: i64 = 3;
pub const OPTIMISTIC_CANDIDATE_HEALTH: i64 = 4;
pub const OPTIMISTIC_CANDIDATE_PERFORMANCE: i64 = 5;
pub const OPTIMISTIC_CANDIDATE_QUOTA_PROBE: i64 = 6;
pub const OPTIMISTIC_CANDIDATE_STALE_PERSISTED_QUOTA: i64 = 7;
pub const OPTIMISTIC_CANDIDATE_QUOTA_THIN: i64 = 8;
pub const OPTIMISTIC_CANDIDATE_QUOTA_CRITICAL: i64 = 9;
pub const OPTIMISTIC_CANDIDATE_QUOTA_EXHAUSTED: i64 = 10;
pub const OPTIMISTIC_CANDIDATE_QUOTA_UNKNOWN: i64 = 11;
pub const OPTIMISTIC_CANDIDATE_INFLIGHT: i64 = 12;
pub const OPTIMISTIC_CANDIDATE_INCOMPATIBLE: i64 = 13;
pub const OPTIMISTIC_CANDIDATE_PROMPT_CACHE: i64 = 14;

pub const SMART_CONTEXT_REHYDRATE_MAX_COUNT: usize = 256;
pub const SMART_CONTEXT_REHYDRATE_MINIMAL_TIER: i64 = 0;
pub const SMART_CONTEXT_REHYDRATE_CONDENSED_TIER: i64 = 1;
pub const SMART_CONTEXT_REHYDRATE_LARGE_TIER: i64 = 2;
pub const SMART_CONTEXT_REHYDRATE_EXACT_TIER: i64 = 3;
pub const SMART_CONTEXT_REHYDRATE_ACTION_REHYDRATE: i64 = 0;
pub const SMART_CONTEXT_REHYDRATE_ACTION_MISSING: i64 = 1;
pub const SMART_CONTEXT_REHYDRATE_ACTION_BUDGET: i64 = 2;
pub const SMART_CONTEXT_REHYDRATE_ACTION_MINIMAL: i64 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimisticCandidateInput {
    pub route_kind: i64,
    pub auth_failure_active: bool,
    pub in_selection_backoff: bool,
    pub circuit_open: bool,
    pub health_score: u32,
    pub performance_score: u32,
    pub current_profile_quota_compatible: bool,
    pub has_alternative_quota_compatible_profile: bool,
    pub quota_band: i64,
    pub quota_source: Option<i64>,
    pub inflight_count: usize,
    pub inflight_soft_limit: usize,
    pub prompt_cache_present: bool,
    pub prompt_cache_owner_matches: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextRehydrateInput {
    pub token_cost: u64,
    pub required: bool,
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextRehydratePlan {
    pub action_tags: Vec<i64>,
    pub used_tokens: u64,
}

unsafe extern "C" {
    fn prodex_runtime_optimistic_current_candidate_decision(
        route_kind: i64,
        auth_failure_active: i64,
        in_selection_backoff: i64,
        circuit_open: i64,
        health_score: i64,
        performance_score: i64,
        current_profile_quota_compatible: i64,
        has_alternative_quota_compatible_profile: i64,
        quota_band: i64,
        quota_source_present: i64,
        quota_source: i64,
        inflight_count: i64,
        inflight_soft_limit: i64,
        prompt_cache_present: i64,
        prompt_cache_owner_matches: i64,
    ) -> i64;
    fn prodex_smart_context_rehydrate_plan_batch(
        token_costs: *const u64,
        required: *const i64,
        available: *const i64,
        action_tags: *mut i64,
        used_tokens: *mut u64,
        count: i64,
        token_budget: u64,
        tier: i64,
    ) -> i64;
    fn prodex_smart_context_token_usage_summary_batch(
        input_tokens_address: u64,
        cached_input_tokens_address: u64,
        output_tokens_address: u64,
        reasoning_tokens_address: u64,
        observed_input_tokens_address: u64,
        observed_cached_input_tokens_address: u64,
        observed_output_tokens_address: u64,
        observed_reasoning_tokens_address: u64,
        last_input_tokens_address: u64,
        last_accounted_input_tokens_address: u64,
        last_observed_context_tokens_address: u64,
        count: i64,
    ) -> i64;
    fn prodex_smart_context_token_accounting_v1(
        model_context_window_tokens: u64,
        model_context_window_has_value: i64,
        reserved_output_tokens: u64,
        current_input_tokens: u64,
        estimated_current_request_tokens: u64,
        observed_input_tokens: u64,
        observed_cached_input_tokens: u64,
        observed_output_tokens: u64,
        observed_reasoning_tokens: u64,
        last_accounted_input_tokens: u64,
        observed_uncached_input_tokens: *mut u64,
        observed_total_tokens: *mut u64,
        observed_context_tokens: *mut u64,
        current_request_accounted_tokens: *mut u64,
        effective_input_tokens: *mut u64,
        effective_input_source: *mut i64,
        available_context_tokens: *mut u64,
        available_context_has_value: *mut i64,
        risk_bits: *mut u64,
    ) -> i64;
    fn prodex_smart_context_exactness_plan_v1(
        exact_mode: i64,
        previous_response_present: i64,
        turn_state_present: i64,
        session_present: i64,
        tool_output_without_artifact: i64,
        decision: *mut i64,
        reason_bits: *mut u64,
    ) -> i64;
    fn prodex_smart_context_calibrated_estimate_batch(
        body_bytes_address: u64,
        baseline_estimate_address: u64,
        observed_accounted_input_address: u64,
        observed_present_address: u64,
        calibrated_estimate_address: u64,
        count: i64,
    ) -> i64;
    fn prodex_smart_context_adaptive_budget_plan_v1(
        available_context_tokens: u64,
        available_has_value: i64,
        exactness_required: i64,
        static_context_changed: i64,
        missing_rehydrate_refs: i64,
        unknown_token_window: i64,
        unsafe_accounting: i64,
        safe_rewrites: u64,
        fallback_rewrites: u64,
        saved_tokens: u64,
        tier: *mut i64,
        mode: *mut i64,
        max_inline_bytes: *mut u64,
        max_rehydrate_tokens: *mut u64,
        reason_bits: *mut u64,
    ) -> i64;
    fn prodex_smart_context_budget_adjustment_v1(
        tier: i64,
        mode: i64,
        max_inline_bytes: u64,
        max_rehydrate_tokens: u64,
        decision: i64,
        available_context_tokens: u64,
        available_has_value: i64,
        adjusted_inline_bytes: *mut u64,
        adjusted_rehydrate_tokens: *mut u64,
    ) -> i64;
    fn prodex_smart_context_rewrite_telemetry_decision_v1(
        body_bytes_before_address: u64,
        body_bytes_after_address: u64,
        tokens_before_address: u64,
        tokens_after_address: u64,
        token_count_source_address: u64,
        safe_address: u64,
        fallback_address: u64,
        quality_risk_address: u64,
        recent_safe_rewrites: u64,
        recent_fallback_rewrites: u64,
        recent_saved_tokens: u64,
        decision: *mut i64,
        count: i64,
    ) -> i64;
}

pub fn rehydrate_plan_self_test() -> bool {
    smart_context_rehydrate_plan_batch(
        &[SmartContextRehydrateInput {
            token_cost: 4,
            required: true,
            available: true,
        }],
        4,
        SMART_CONTEXT_REHYDRATE_EXACT_TIER,
    )
    .is_ok_and(|plan| {
        plan.action_tags == [SMART_CONTEXT_REHYDRATE_ACTION_REHYDRATE] && plan.used_tokens == 4
    })
}

pub fn tuning_defaults_self_test() -> bool {
    runtime_tuning_defaults(8).is_ok_and(|defaults| {
        defaults.worker_count == 8
            && defaults.long_lived_worker_count == 16
            && defaults.log_queue_capacity == 2_048
            && defaults.websocket_connect_worker_count == 8
            && defaults.websocket_dns_worker_count == 8
    })
}

pub fn smart_context_token_usage_summary_batch(
    inputs: &[SmartContextTokenUsageInput],
) -> Result<SmartContextTokenUsageSummary, crate::MojoError> {
    if inputs.len() > SMART_CONTEXT_TOKEN_ACCOUNTING_MAX_COUNT {
        return Err(crate::MojoError::InvalidInput);
    }
    let input_tokens = inputs
        .iter()
        .map(|input| input.input_tokens)
        .collect::<Vec<_>>();
    let cached_input_tokens = inputs
        .iter()
        .map(|input| input.cached_input_tokens)
        .collect::<Vec<_>>();
    let output_tokens = inputs
        .iter()
        .map(|input| input.output_tokens)
        .collect::<Vec<_>>();
    let reasoning_tokens = inputs
        .iter()
        .map(|input| input.reasoning_tokens)
        .collect::<Vec<_>>();
    let mut observed_input_tokens = 0;
    let mut observed_cached_input_tokens = 0;
    let mut observed_output_tokens = 0;
    let mut observed_reasoning_tokens = 0;
    let mut last_input_tokens = 0;
    let mut last_accounted_input_tokens = 0;
    let mut last_observed_context_tokens = 0;
    let status = unsafe {
        prodex_smart_context_token_usage_summary_batch(
            input_tokens.as_ptr() as u64,
            cached_input_tokens.as_ptr() as u64,
            output_tokens.as_ptr() as u64,
            reasoning_tokens.as_ptr() as u64,
            &mut observed_input_tokens as *mut u64 as u64,
            &mut observed_cached_input_tokens as *mut u64 as u64,
            &mut observed_output_tokens as *mut u64 as u64,
            &mut observed_reasoning_tokens as *mut u64 as u64,
            &mut last_input_tokens as *mut u64 as u64,
            &mut last_accounted_input_tokens as *mut u64 as u64,
            &mut last_observed_context_tokens as *mut u64 as u64,
            i64::try_from(inputs.len()).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    if status != 0 {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(SmartContextTokenUsageSummary {
        observed_input_tokens,
        observed_cached_input_tokens,
        observed_output_tokens,
        observed_reasoning_tokens,
        last_input_tokens,
        last_accounted_input_tokens,
        last_observed_context_tokens,
    })
}

pub fn smart_context_token_usage_summary_self_test() -> bool {
    smart_context_token_usage_summary_batch(&[
        SmartContextTokenUsageInput {
            input_tokens: 20,
            cached_input_tokens: 4,
            output_tokens: 3,
            reasoning_tokens: 1,
        },
        SmartContextTokenUsageInput {
            input_tokens: 0,
            cached_input_tokens: 8,
            output_tokens: 0,
            reasoning_tokens: 0,
        },
    ])
    .is_ok_and(|summary| {
        summary.observed_input_tokens == 20
            && summary.observed_cached_input_tokens == 12
            && summary.observed_output_tokens == 3
            && summary.observed_reasoning_tokens == 1
            && summary.last_input_tokens == 0
            && summary.last_accounted_input_tokens == 8
            && summary.last_observed_context_tokens == 8
    })
}

pub fn smart_context_token_accounting(
    input: SmartContextTokenAccountingInput,
) -> Result<SmartContextTokenAccountingSummary, crate::MojoError> {
    let mut observed_uncached_input_tokens = 0_u64;
    let mut observed_total_tokens = 0_u64;
    let mut observed_context_tokens = 0_u64;
    let mut current_request_accounted_tokens = 0_u64;
    let mut effective_input_tokens = 0_u64;
    let mut effective_input_source = -1_i64;
    let mut available_context_tokens = 0_u64;
    let mut available_context_has_value = 0_i64;
    let mut risk_bits = 0_u64;
    let status = unsafe {
        prodex_smart_context_token_accounting_v1(
            input.model_context_window_tokens.unwrap_or_default(),
            i64::from(input.model_context_window_tokens.is_some()),
            input.reserved_output_tokens,
            input.current_input_tokens,
            input.estimated_current_request_tokens,
            input.observed_input_tokens,
            input.observed_cached_input_tokens,
            input.observed_output_tokens,
            input.observed_reasoning_tokens,
            input.last_accounted_input_tokens,
            &mut observed_uncached_input_tokens,
            &mut observed_total_tokens,
            &mut observed_context_tokens,
            &mut current_request_accounted_tokens,
            &mut effective_input_tokens,
            &mut effective_input_source,
            &mut available_context_tokens,
            &mut available_context_has_value,
            &mut risk_bits,
        )
    };
    if status != 0
        || !matches!(effective_input_source, 0..=3)
        || !matches!(available_context_has_value, 0 | 1)
        || risk_bits & !SMART_CONTEXT_ACCOUNTING_RISK_ALL != 0
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(SmartContextTokenAccountingSummary {
        observed_uncached_input_tokens,
        observed_total_tokens,
        observed_context_tokens,
        current_request_accounted_tokens,
        effective_input_tokens,
        effective_input_source,
        available_context_tokens: (available_context_has_value == 1)
            .then_some(available_context_tokens),
        risk_bits,
    })
}

pub fn smart_context_exactness_plan(
    exact_mode: bool,
    previous_response_present: bool,
    turn_state_present: bool,
    session_present: bool,
    tool_output_without_artifact: bool,
) -> Result<(i64, u64), crate::MojoError> {
    let mut decision = -1_i64;
    let mut reason_bits = 0_u64;
    let status = unsafe {
        prodex_smart_context_exactness_plan_v1(
            i64::from(exact_mode),
            i64::from(previous_response_present),
            i64::from(turn_state_present),
            i64::from(session_present),
            i64::from(tool_output_without_artifact),
            &mut decision,
            &mut reason_bits,
        )
    };
    if status != 0 {
        return Err(crate::MojoError::InvalidOutput);
    }
    if !matches!(decision, 0 | 1) || reason_bits & !31 != 0 {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok((decision, reason_bits))
}

pub const SMART_CONTEXT_CALIBRATION_MAX_COUNT: usize = 64;

pub fn smart_context_calibrated_estimate_batch(
    inputs: &[SmartContextCalibratedEstimateInput],
) -> Result<Vec<u64>, crate::MojoError> {
    if inputs.len() > SMART_CONTEXT_CALIBRATION_MAX_COUNT {
        return Err(crate::MojoError::InvalidInput);
    }
    if inputs.is_empty() {
        return Ok(Vec::new());
    }
    let body_bytes = inputs
        .iter()
        .map(|input| input.body_bytes)
        .collect::<Vec<_>>();
    let baseline_estimates = inputs
        .iter()
        .map(|input| input.baseline_estimate)
        .collect::<Vec<_>>();
    let observed_accounted_input = inputs
        .iter()
        .map(|input| input.observed_accounted_input.unwrap_or_default())
        .collect::<Vec<_>>();
    let observed_present = inputs
        .iter()
        .map(|input| i64::from(input.observed_accounted_input.is_some()))
        .collect::<Vec<_>>();
    let mut calibrated_estimates = vec![0_u64; inputs.len()];
    let status = unsafe {
        prodex_smart_context_calibrated_estimate_batch(
            body_bytes.as_ptr() as u64,
            baseline_estimates.as_ptr() as u64,
            observed_accounted_input.as_ptr() as u64,
            observed_present.as_ptr() as u64,
            calibrated_estimates.as_mut_ptr() as u64,
            i64::try_from(inputs.len()).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    if status != 0 {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(calibrated_estimates)
}

pub fn smart_context_adaptive_budget_plan(
    input: SmartContextAdaptiveBudgetPlanInput,
) -> Result<SmartContextAdaptiveBudgetPlan, crate::MojoError> {
    let mut tier = -1_i64;
    let mut mode = -1_i64;
    let mut max_inline_bytes = 0_u64;
    let mut max_rehydrate_tokens = 0_u64;
    let mut reason_bits = 0_u64;
    let status = unsafe {
        prodex_smart_context_adaptive_budget_plan_v1(
            input.available_context_tokens.unwrap_or_default(),
            i64::from(input.available_context_tokens.is_some()),
            i64::from(input.exactness_required),
            i64::from(input.static_context_changed),
            i64::from(input.missing_rehydrate_refs),
            i64::from(input.unknown_token_window),
            i64::from(input.unsafe_accounting),
            u64::try_from(input.safe_rewrites).map_err(|_| crate::MojoError::InvalidInput)?,
            u64::try_from(input.fallback_rewrites).map_err(|_| crate::MojoError::InvalidInput)?,
            input.saved_tokens,
            &mut tier,
            &mut mode,
            &mut max_inline_bytes,
            &mut max_rehydrate_tokens,
            &mut reason_bits,
        )
    };
    if status != 0
        || !(0..=3).contains(&tier)
        || !(SMART_CONTEXT_BUDGET_MODE_EXACT..=SMART_CONTEXT_BUDGET_MODE_MINIMAL).contains(&mode)
        || reason_bits & !SMART_CONTEXT_POLICY_REASON_ALL != 0
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(SmartContextAdaptiveBudgetPlan {
        tier,
        mode,
        max_inline_bytes,
        max_rehydrate_tokens,
        reason_bits,
    })
}

pub fn smart_context_budget_adjustment(
    tier: i64,
    mode: i64,
    max_inline_bytes: u64,
    max_rehydrate_tokens: u64,
    decision: i64,
    available_context_tokens: Option<u64>,
) -> Result<SmartContextBudgetAdjustment, crate::MojoError> {
    if !(0..=3).contains(&tier)
        || !(SMART_CONTEXT_BUDGET_MODE_EXACT..=SMART_CONTEXT_BUDGET_MODE_MINIMAL).contains(&mode)
        || !(SMART_CONTEXT_BUDGET_DECISION_NO_CHANGE..=SMART_CONTEXT_BUDGET_DECISION_TIGHTEN)
            .contains(&decision)
    {
        return Err(crate::MojoError::InvalidInput);
    }
    let mut adjusted_inline_bytes = 0;
    let mut adjusted_rehydrate_tokens = 0;
    let status = unsafe {
        prodex_smart_context_budget_adjustment_v1(
            tier,
            mode,
            max_inline_bytes,
            max_rehydrate_tokens,
            decision,
            available_context_tokens.unwrap_or_default(),
            i64::from(available_context_tokens.is_some()),
            &mut adjusted_inline_bytes,
            &mut adjusted_rehydrate_tokens,
        )
    };
    if status != 0 {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(SmartContextBudgetAdjustment {
        max_inline_bytes: adjusted_inline_bytes,
        max_rehydrate_tokens: adjusted_rehydrate_tokens,
    })
}

pub const SMART_CONTEXT_REWRITE_TELEMETRY_MAX_COUNT: usize = 64;

pub fn smart_context_rewrite_telemetry_budget_decision(
    inputs: &[SmartContextRewriteTelemetryInput],
    recent_safe_rewrites: usize,
    recent_fallback_rewrites: usize,
    recent_saved_tokens: u64,
) -> Result<i64, crate::MojoError> {
    if inputs.len() > SMART_CONTEXT_REWRITE_TELEMETRY_MAX_COUNT {
        return Err(crate::MojoError::InvalidInput);
    }
    let body_bytes_before = inputs
        .iter()
        .map(|input| input.body_bytes_before)
        .collect::<Vec<_>>();
    let body_bytes_after = inputs
        .iter()
        .map(|input| input.body_bytes_after)
        .collect::<Vec<_>>();
    let tokens_before = inputs
        .iter()
        .map(|input| input.tokens_before)
        .collect::<Vec<_>>();
    let tokens_after = inputs
        .iter()
        .map(|input| input.tokens_after)
        .collect::<Vec<_>>();
    let token_count_source = inputs
        .iter()
        .map(|input| input.token_count_source)
        .collect::<Vec<_>>();
    let safe = inputs
        .iter()
        .map(|input| i64::from(input.safe))
        .collect::<Vec<_>>();
    let fallback = inputs
        .iter()
        .map(|input| i64::from(input.fallback))
        .collect::<Vec<_>>();
    let quality_risk = inputs
        .iter()
        .map(|input| i64::from(input.quality_risk))
        .collect::<Vec<_>>();
    let mut decision = -1_i64;
    let status = unsafe {
        prodex_smart_context_rewrite_telemetry_decision_v1(
            body_bytes_before.as_ptr() as u64,
            body_bytes_after.as_ptr() as u64,
            tokens_before.as_ptr() as u64,
            tokens_after.as_ptr() as u64,
            token_count_source.as_ptr() as u64,
            safe.as_ptr() as u64,
            fallback.as_ptr() as u64,
            quality_risk.as_ptr() as u64,
            u64::try_from(recent_safe_rewrites).map_err(|_| crate::MojoError::InvalidInput)?,
            u64::try_from(recent_fallback_rewrites).map_err(|_| crate::MojoError::InvalidInput)?,
            recent_saved_tokens,
            &mut decision,
            i64::try_from(inputs.len()).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    if status != 0
        || !(SMART_CONTEXT_BUDGET_DECISION_NO_CHANGE..=SMART_CONTEXT_BUDGET_DECISION_TIGHTEN)
            .contains(&decision)
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(decision)
}

pub fn optimistic_current_candidate_decision(
    input: OptimisticCandidateInput,
) -> Result<i64, crate::MojoError> {
    let inflight_count =
        i64::try_from(input.inflight_count).map_err(|_| crate::MojoError::InvalidInput)?;
    let inflight_soft_limit =
        i64::try_from(input.inflight_soft_limit).map_err(|_| crate::MojoError::InvalidInput)?;
    let result = unsafe {
        prodex_runtime_optimistic_current_candidate_decision(
            input.route_kind,
            i64::from(input.auth_failure_active),
            i64::from(input.in_selection_backoff),
            i64::from(input.circuit_open),
            i64::from(input.health_score),
            i64::from(input.performance_score),
            i64::from(input.current_profile_quota_compatible),
            i64::from(input.has_alternative_quota_compatible_profile),
            input.quota_band,
            i64::from(input.quota_source.is_some()),
            input.quota_source.unwrap_or(0),
            inflight_count,
            inflight_soft_limit,
            i64::from(input.prompt_cache_present),
            i64::from(input.prompt_cache_owner_matches),
        )
    };
    if (OPTIMISTIC_CANDIDATE_KEEP..=OPTIMISTIC_CANDIDATE_PROMPT_CACHE).contains(&result) {
        Ok(result)
    } else {
        Err(crate::MojoError::InvalidOutput)
    }
}

pub fn smart_context_rehydrate_plan_batch(
    inputs: &[SmartContextRehydrateInput],
    token_budget: usize,
    tier: i64,
) -> Result<SmartContextRehydratePlan, crate::MojoError> {
    if inputs.len() > SMART_CONTEXT_REHYDRATE_MAX_COUNT {
        return Err(crate::MojoError::InvalidInput);
    }
    if !(SMART_CONTEXT_REHYDRATE_MINIMAL_TIER..=SMART_CONTEXT_REHYDRATE_EXACT_TIER).contains(&tier)
    {
        return Err(crate::MojoError::InvalidInput);
    }
    let token_budget = u64::try_from(token_budget).map_err(|_| crate::MojoError::InvalidInput)?;
    let token_costs = inputs
        .iter()
        .map(|input| input.token_cost)
        .collect::<Vec<_>>();
    let required = inputs
        .iter()
        .map(|input| i64::from(input.required))
        .collect::<Vec<_>>();
    let available = inputs
        .iter()
        .map(|input| i64::from(input.available))
        .collect::<Vec<_>>();
    let mut action_tags = vec![0_i64; inputs.len()];
    let mut used_tokens = 0_u64;
    let status = unsafe {
        prodex_smart_context_rehydrate_plan_batch(
            token_costs.as_ptr(),
            required.as_ptr(),
            available.as_ptr(),
            action_tags.as_mut_ptr(),
            &mut used_tokens,
            i64::try_from(inputs.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            token_budget,
            tier,
        )
    };
    if status != 0
        || used_tokens > token_budget
        || action_tags.iter().any(|tag| {
            !(SMART_CONTEXT_REHYDRATE_ACTION_REHYDRATE..=SMART_CONTEXT_REHYDRATE_ACTION_MINIMAL)
                .contains(tag)
        })
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(SmartContextRehydratePlan {
        action_tags,
        used_tokens,
    })
}
