use super::runtime::{ProfileScoreInput, profile_scores_batch};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTuningDefaults {
    pub worker_count: usize,
    pub long_lived_worker_count: usize,
    pub probe_refresh_worker_count: usize,
    pub async_worker_count: usize,
    pub log_queue_capacity: usize,
    pub websocket_connect_worker_count: usize,
    pub websocket_dns_worker_count: usize,
}

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
    fn prodex_runtime_tuning_defaults(
        parallelism: i64,
        worker_count: *mut i64,
        long_lived_worker_count: *mut i64,
        probe_refresh_worker_count: *mut i64,
        async_worker_count: *mut i64,
        log_queue_capacity: *mut i64,
        websocket_connect_worker_count: *mut i64,
        websocket_dns_worker_count: *mut i64,
    ) -> i64;
}

pub fn profile_scores_self_test() -> bool {
    profile_scores_batch(&[ProfileScoreInput {
        weekly_pressure: 1_000,
        five_hour_pressure: 2_000,
        scale_bps: 10_000,
        weekly_remaining: 90,
        five_hour_remaining: 80,
        reserve_bias: 0,
        weekly_weight: 1,
    }])
    .is_ok_and(|scores| {
        scores.first().is_some_and(|score| {
            score.total_pressure == 3_000
                && score.weekly_pressure == 1_000
                && score.five_hour_pressure == 2_000
                && score.reserve_floor == 80
        })
    })
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

pub fn runtime_tuning_defaults(
    parallelism: usize,
) -> Result<RuntimeTuningDefaults, crate::MojoError> {
    let parallelism = i64::try_from(parallelism).unwrap_or(i64::MAX);
    let mut values = [0_i64; 7];
    let status = unsafe {
        prodex_runtime_tuning_defaults(
            parallelism,
            &mut values[0],
            &mut values[1],
            &mut values[2],
            &mut values[3],
            &mut values[4],
            &mut values[5],
            &mut values[6],
        )
    };
    if status != 0 || values.iter().any(|value| *value < 0) {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(RuntimeTuningDefaults {
        worker_count: usize::try_from(values[0]).map_err(|_| crate::MojoError::InvalidOutput)?,
        long_lived_worker_count: usize::try_from(values[1])
            .map_err(|_| crate::MojoError::InvalidOutput)?,
        probe_refresh_worker_count: usize::try_from(values[2])
            .map_err(|_| crate::MojoError::InvalidOutput)?,
        async_worker_count: usize::try_from(values[3])
            .map_err(|_| crate::MojoError::InvalidOutput)?,
        log_queue_capacity: usize::try_from(values[4])
            .map_err(|_| crate::MojoError::InvalidOutput)?,
        websocket_connect_worker_count: usize::try_from(values[5])
            .map_err(|_| crate::MojoError::InvalidOutput)?,
        websocket_dns_worker_count: usize::try_from(values[6])
            .map_err(|_| crate::MojoError::InvalidOutput)?,
    })
}
