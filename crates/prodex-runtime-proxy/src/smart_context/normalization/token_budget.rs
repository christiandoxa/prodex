use super::*;

pub(in crate::smart_context) fn smart_context_u64_budget_tier(
    available_tokens: u64,
) -> SmartContextTokenBudgetTier {
    match prodex_mojo_core::rich::smart_context_budget_tier(available_tokens)
        .expect("Mojo Smart Context budget tier returned invalid output")
    {
        0 => SmartContextTokenBudgetTier::Exact,
        1 => SmartContextTokenBudgetTier::Large,
        2 => SmartContextTokenBudgetTier::Condensed,
        3 => SmartContextTokenBudgetTier::Minimal,
        _ => unreachable!("Mojo Smart Context budget tier was validated"),
    }
}

pub(in crate::smart_context) fn smart_context_u64_saturating_usize(value: u64) -> usize {
    if value > usize::MAX as u64 {
        usize::MAX
    } else {
        value as usize
    }
}

pub(in crate::smart_context) fn smart_context_memory_capsule_token_budget_impl(
    accounting: &SmartContextObservedTokenAccounting,
    policy: &SmartContextAdaptiveBudgetPolicy,
) -> usize {
    let mode = match policy.mode {
        SmartContextBudgetMode::ExactPassThrough => {
            prodex_mojo_core::runtime::SMART_CONTEXT_BUDGET_MODE_EXACT
        }
        SmartContextBudgetMode::LargeLossless => {
            prodex_mojo_core::runtime::SMART_CONTEXT_BUDGET_MODE_LARGE
        }
        SmartContextBudgetMode::ArtifactCondensed => {
            prodex_mojo_core::runtime::SMART_CONTEXT_BUDGET_MODE_CONDENSED
        }
        SmartContextBudgetMode::MinimalRefsOnly => {
            prodex_mojo_core::runtime::SMART_CONTEXT_BUDGET_MODE_MINIMAL
        }
    };
    let tier = match policy.tier {
        SmartContextTokenBudgetTier::Exact => 0,
        SmartContextTokenBudgetTier::Large => 1,
        SmartContextTokenBudgetTier::Condensed => 2,
        SmartContextTokenBudgetTier::Minimal => 3,
    };
    let reason_bits = policy.reasons.iter().fold(0_u64, |bits, reason| {
        bits | match reason {
            SmartContextBudgetPolicyReason::ExactnessRequired => 1 << 0,
            SmartContextBudgetPolicyReason::StaticContextChanged => 1 << 1,
            SmartContextBudgetPolicyReason::MissingRehydrateRefs => 1 << 2,
            SmartContextBudgetPolicyReason::UnknownTokenWindow => 1 << 3,
            SmartContextBudgetPolicyReason::UnsafeAccounting => 1 << 4,
            SmartContextBudgetPolicyReason::RecentRewriteSavingsSafe => 1 << 5,
            SmartContextBudgetPolicyReason::PlentyOfBudget => 1 << 6,
            SmartContextBudgetPolicyReason::ModerateBudget => 1 << 7,
            SmartContextBudgetPolicyReason::TightBudget => 1 << 8,
            SmartContextBudgetPolicyReason::CriticalBudget => 1 << 9,
        }
    });
    smart_context_u64_saturating_usize(
        prodex_mojo_core::rich::smart_context_memory_capsule_token_budget(
            accounting.available_context_tokens,
            mode,
            tier,
            policy.max_rehydrate_tokens,
            reason_bits,
            accounting.accounting_risks.is_empty(),
        )
        .expect("Mojo Smart Context capsule budget returned invalid output"),
    )
}

pub(in crate::smart_context) fn smart_context_select_memory_capsules_impl(
    capsules: impl IntoIterator<Item = SmartContextMemoryCapsule>,
    token_budget: usize,
) -> SmartContextMemoryCapsuleSelection {
    let mut required = Vec::new();
    let mut optional = Vec::new();
    for capsule in capsules {
        if capsule.required {
            required.push(capsule);
        } else {
            optional.push(capsule);
        }
    }

    required.sort_by(|left, right| left.id.cmp(&right.id));
    optional.sort_by(smart_context_capsule_order);

    let capsules = required.into_iter().chain(optional).collect::<Vec<_>>();

    let mut selected = Vec::with_capacity(capsules.len());
    let mut used_tokens = 0;
    for batch in capsules.chunks(65_536) {
        let inputs = batch
            .iter()
            .map(|capsule| prodex_mojo_core::rich::SmartContextCapsuleInput {
                token_cost: capsule.token_cost,
                required: capsule.required,
            })
            .collect::<Vec<_>>();
        let plan = prodex_mojo_core::rich::plan_smart_context_capsules(
            &inputs,
            token_budget - used_tokens,
        )
        .expect("Mojo Smart Context capsule selector returned invalid output");
        used_tokens += plan.used_tokens;
        selected.extend(plan.selected);
    }
    let mut selected_ids = Vec::new();
    let mut omitted_ids = Vec::new();
    for (capsule, selected) in capsules.into_iter().zip(selected) {
        if selected {
            selected_ids.push(capsule.id);
        } else {
            omitted_ids.push(capsule.id);
        }
    }
    SmartContextMemoryCapsuleSelection {
        selected_ids,
        omitted_ids,
        used_tokens,
    }
}

#[cfg(test)]
#[path = "../../../tests/src/smart_context/token_budget.rs"]
mod tests;
