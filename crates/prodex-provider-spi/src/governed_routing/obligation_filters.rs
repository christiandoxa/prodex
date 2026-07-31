use super::*;

pub(super) fn append_provider_obligation_rejection_reasons(
    provider: &GovernedProviderDescriptor,
    obligations: &[GovernanceObligation],
    reasons: &mut Vec<GovernedHardFilterReason>,
) {
    append_provider_selector_rejection_reasons(provider, obligations, reasons);
    append_provider_trust_rejection_reasons(provider, obligations, reasons);
    append_provider_data_handling_rejection_reasons(provider, obligations, reasons);
}

fn append_provider_selector_rejection_reasons(
    provider: &GovernedProviderDescriptor,
    obligations: &[GovernanceObligation],
    reasons: &mut Vec<GovernedHardFilterReason>,
) {
    let has_allow_list = obligations
        .iter()
        .any(|item| matches!(item, GovernanceObligation::AllowProvider(_)));
    let allow_list_match = obligations.iter().any(|item| {
        matches!(item, GovernanceObligation::AllowProvider(selector) if selector_matches_provider(selector, provider.provider))
    });
    let denied = obligations.iter().any(|item| {
        matches!(item, GovernanceObligation::DenyProvider(selector) if selector_matches_provider(selector, provider.provider))
    });

    if has_allow_list && !allow_list_match {
        reasons.push(GovernedHardFilterReason::ProviderNotAllowed);
    }
    if denied {
        reasons.push(GovernedHardFilterReason::ProviderDenied);
    }
}

fn append_provider_trust_rejection_reasons(
    provider: &GovernedProviderDescriptor,
    obligations: &[GovernanceObligation],
    reasons: &mut Vec<GovernedHardFilterReason>,
) {
    let minimum_trust = obligations.iter().filter_map(|item| match item {
        GovernanceObligation::MinimumProviderTrust(tier) => Some(*tier),
        _ => None,
    });
    let regions_match = obligations.iter().all(|item| match item {
        GovernanceObligation::RequireRegion(required) => provider
            .regions
            .iter()
            .any(|offered| selectors_overlap(required, offered)),
        _ => true,
    });

    if minimum_trust
        .max()
        .is_some_and(|required| provider.trust_tier < required)
    {
        reasons.push(GovernedHardFilterReason::TrustTierInsufficient);
    }
    if !regions_match {
        reasons.push(GovernedHardFilterReason::RegionUnavailable);
    }
}

fn append_provider_data_handling_rejection_reasons(
    provider: &GovernedProviderDescriptor,
    obligations: &[GovernanceObligation],
    reasons: &mut Vec<GovernedHardFilterReason>,
) {
    let maximum_retention = obligations.iter().filter_map(|item| match item {
        GovernanceObligation::RetentionSeconds(seconds) => Some(*seconds),
        _ => None,
    });

    if obligations.contains(&GovernanceObligation::RequireLocalExecution)
        && !provider.local_execution
    {
        reasons.push(GovernedHardFilterReason::LocalExecutionRequired);
    }
    if obligations.contains(&GovernanceObligation::ProhibitRetention)
        && provider.retention_seconds != 0
    {
        reasons.push(GovernedHardFilterReason::RetentionProhibited);
    }
    if maximum_retention
        .min()
        .is_some_and(|limit| provider.retention_seconds > limit)
    {
        reasons.push(GovernedHardFilterReason::RetentionLimitExceeded);
    }
    if obligations.contains(&GovernanceObligation::ProhibitTrainingUse) && provider.training_use {
        reasons.push(GovernedHardFilterReason::TrainingUseProhibited);
    }
}

fn selector_matches_provider(selector: &PolicySelector, provider: ProviderId) -> bool {
    selector.as_str() == "*" || selector.as_str() == provider.label()
}

fn selectors_overlap(left: &PolicySelector, right: &PolicySelector) -> bool {
    left.as_str() == "*" || right.as_str() == "*" || left.as_str() == right.as_str()
}
