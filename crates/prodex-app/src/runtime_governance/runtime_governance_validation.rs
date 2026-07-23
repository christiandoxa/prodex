use anyhow::Result;
use prodex_runtime_policy::{
    RuntimeGovernancePolicyObligation as Obligation, RuntimeGovernancePolicyRule,
};

pub(super) fn validate_runtime_governance_supported_evidence(
    rule: &RuntimeGovernancePolicyRule,
) -> Result<()> {
    if rule
        .condition
        .minimum_authentication_strength
        .is_some_and(|strength| !(1..=3).contains(&strength))
    {
        anyhow::bail!("gateway governance policy authentication strength must be between 1 and 3");
    }
    if rule.obligations.iter().any(|obligation| {
        matches!(
            obligation,
            Obligation::MinimumAuthenticationStrength { value } if !(1..=3).contains(value)
        )
    }) {
        anyhow::bail!(
            "gateway governance policy obligation authentication strength must be between 1 and 3"
        );
    }
    Ok(())
}
