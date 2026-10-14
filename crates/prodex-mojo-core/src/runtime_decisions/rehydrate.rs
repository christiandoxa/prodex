use super::*;

unsafe extern "C" {
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
