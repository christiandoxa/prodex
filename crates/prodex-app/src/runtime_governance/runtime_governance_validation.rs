use anyhow::Result;
use prodex_runtime_policy::{
    RuntimeGovernancePolicyAction as Action, RuntimeGovernancePolicyNetworkZone as Zone,
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
    if matches!(
        rule.condition.action,
        Some(Action::UseTool | Action::MutateControlPlane)
    ) {
        anyhow::bail!(
            "gateway governance policy action selector is not produced by the data plane"
        );
    }
    if rule.condition.network_zone == Some(Zone::Partner) {
        anyhow::bail!(
            "gateway governance partner network selector requires unavailable trusted zone evidence"
        );
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
