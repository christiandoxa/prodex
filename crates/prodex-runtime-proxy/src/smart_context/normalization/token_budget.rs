use super::*;

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

pub(in crate::smart_context) fn smart_context_memory_capsule_policy_allows_unbounded_budget(
    accounting: &SmartContextObservedTokenAccounting,
    policy: &SmartContextAdaptiveBudgetPolicy,
) -> bool {
    smart_context_accounting_safe_for_adaptive_policy(accounting)
        && policy.mode == SmartContextBudgetMode::ExactPassThrough
        && policy.tier == SmartContextTokenBudgetTier::Exact
        && policy.reasons == [SmartContextBudgetPolicyReason::PlentyOfBudget]
}

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
