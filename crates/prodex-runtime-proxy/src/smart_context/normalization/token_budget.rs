use super::*;

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_effective_input_source(
    current_input_tokens: u64,
    estimated_current_request_tokens: u64,
    current_request_accounted_tokens: u64,
    last_accounted_input_tokens: u64,
    effective_input_tokens: u64,
) -> SmartContextTokenAccountingSource {
    if effective_input_tokens == 0 {
        SmartContextTokenAccountingSource::Unknown
    } else if last_accounted_input_tokens > current_request_accounted_tokens {
        SmartContextTokenAccountingSource::ObservedHistory
    } else if current_input_tokens >= estimated_current_request_tokens && current_input_tokens > 0 {
        SmartContextTokenAccountingSource::CurrentRequestTokens
    } else if estimated_current_request_tokens > 0 {
        SmartContextTokenAccountingSource::CurrentRequestBodyEstimate
    } else if last_accounted_input_tokens > 0 {
        SmartContextTokenAccountingSource::ObservedHistory
    } else {
        SmartContextTokenAccountingSource::Unknown
    }
}

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_token_accounting_risks(
    model_context_window_tokens: Option<u64>,
    reserved_output_tokens: u64,
    effective_input_source: SmartContextTokenAccountingSource,
) -> Vec<SmartContextTokenAccountingRisk> {
    let mut risks = Vec::new();

    match model_context_window_tokens {
        Some(0) => risks.push(SmartContextTokenAccountingRisk::ZeroContextWindow),
        Some(window) if reserved_output_tokens >= window => {
            risks.push(SmartContextTokenAccountingRisk::ReservedOutputConsumesWindow);
        }
        Some(_) => {}
        None => risks.push(SmartContextTokenAccountingRisk::UnknownTokenWindow),
    }
    if effective_input_source == SmartContextTokenAccountingSource::Unknown {
        risks.push(SmartContextTokenAccountingRisk::UnknownCurrentRequestAccounting);
    }

    risks
}

pub(in crate::smart_context) fn smart_context_u64_budget_tier(
    available_tokens: u64,
) -> SmartContextTokenBudgetTier {
    #[cfg(feature = "mojo")]
    {
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

    #[cfg(not(feature = "mojo"))]
    if available_tokens > usize::MAX as u64 {
        SmartContextTokenBudgetTier::Exact
    } else {
        smart_context_token_budget_tier(available_tokens as usize)
    }
}

pub(in crate::smart_context) fn smart_context_u64_saturating_usize(value: u64) -> usize {
    if value > usize::MAX as u64 {
        usize::MAX
    } else {
        value as usize
    }
}

#[cfg(not(feature = "mojo"))]
pub(in crate::smart_context) fn smart_context_memory_capsule_policy_allows_unbounded_budget(
    accounting: &SmartContextObservedTokenAccounting,
    policy: &SmartContextAdaptiveBudgetPolicy,
) -> bool {
    smart_context_accounting_safe_for_adaptive_policy(accounting)
        && policy.mode == SmartContextBudgetMode::ExactPassThrough
        && policy.tier == SmartContextTokenBudgetTier::Exact
        && policy.reasons == [SmartContextBudgetPolicyReason::PlentyOfBudget]
}

#[cfg(not(feature = "mojo"))]
pub(in crate::smart_context) fn smart_context_memory_capsule_policy_allows_bounded_budget(
    accounting: &SmartContextObservedTokenAccounting,
    policy: &SmartContextAdaptiveBudgetPolicy,
) -> bool {
    smart_context_accounting_safe_for_adaptive_policy(accounting)
        && !policy.reasons.iter().any(|reason| {
            matches!(
                reason,
                SmartContextBudgetPolicyReason::ExactnessRequired
                    | SmartContextBudgetPolicyReason::StaticContextChanged
                    | SmartContextBudgetPolicyReason::MissingRehydrateRefs
                    | SmartContextBudgetPolicyReason::UnknownTokenWindow
                    | SmartContextBudgetPolicyReason::UnsafeAccounting
            )
        })
}

pub(in crate::smart_context) fn smart_context_memory_capsule_token_budget_impl(
    accounting: &SmartContextObservedTokenAccounting,
    policy: &SmartContextAdaptiveBudgetPolicy,
) -> usize {
    #[cfg(feature = "mojo")]
    {
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

    #[cfg(not(feature = "mojo"))]
    {
        if smart_context_memory_capsule_policy_allows_unbounded_budget(accounting, policy) {
            return usize::MAX;
        }
        if !smart_context_memory_capsule_policy_allows_bounded_budget(accounting, policy) {
            return 0;
        }

        let Some(available_context_tokens) = accounting.available_context_tokens else {
            return 0;
        };

        let mode_budget = match policy.mode {
            SmartContextBudgetMode::MinimalRefsOnly => {
                SMART_CONTEXT_MEMORY_CAPSULE_MINIMAL_TOKEN_BUDGET
            }
            SmartContextBudgetMode::ArtifactCondensed => {
                SMART_CONTEXT_MEMORY_CAPSULE_CONDENSED_TOKEN_BUDGET
            }
            SmartContextBudgetMode::LargeLossless => {
                SMART_CONTEXT_MEMORY_CAPSULE_LARGE_TOKEN_BUDGET
            }
            SmartContextBudgetMode::ExactPassThrough => match policy.tier {
                SmartContextTokenBudgetTier::Exact | SmartContextTokenBudgetTier::Large => {
                    SMART_CONTEXT_MEMORY_CAPSULE_LARGE_TOKEN_BUDGET
                }
                SmartContextTokenBudgetTier::Condensed => {
                    SMART_CONTEXT_MEMORY_CAPSULE_CONDENSED_TOKEN_BUDGET
                }
                SmartContextTokenBudgetTier::Minimal => {
                    SMART_CONTEXT_MEMORY_CAPSULE_MINIMAL_TOKEN_BUDGET
                }
            },
        };

        mode_budget
            .min(smart_context_u64_saturating_usize(
                policy.max_rehydrate_tokens,
            ))
            .min(smart_context_u64_saturating_usize(available_context_tokens))
    }
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

    #[cfg(feature = "mojo")]
    {
        let inputs = capsules
            .iter()
            .map(|capsule| prodex_mojo_core::rich::SmartContextCapsuleInput {
                token_cost: capsule.token_cost,
                required: capsule.required,
            })
            .collect::<Vec<_>>();
        let plan = prodex_mojo_core::rich::plan_smart_context_capsules(&inputs, token_budget)
            .expect("Mojo Smart Context capsule selector returned invalid output");
        let mut selected_ids = Vec::new();
        let mut omitted_ids = Vec::new();
        for (capsule, selected) in capsules.into_iter().zip(plan.selected) {
            if selected {
                selected_ids.push(capsule.id);
            } else {
                omitted_ids.push(capsule.id);
            }
        }
        SmartContextMemoryCapsuleSelection {
            selected_ids,
            omitted_ids,
            used_tokens: plan.used_tokens,
        }
    }

    #[cfg(not(feature = "mojo"))]
    {
        let mut selected_ids = Vec::new();
        let mut omitted_ids = Vec::new();
        let mut used_tokens = 0usize;
        for capsule in capsules {
            if used_tokens.saturating_add(capsule.token_cost) <= token_budget {
                used_tokens += capsule.token_cost;
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
}
