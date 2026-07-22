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
        .is_some_and(|strength| strength > 1)
        || rule.condition.environment_mfa_satisfied == Some(true)
        || rule.condition.session_mfa_satisfied == Some(true)
    {
        anyhow::bail!(
            "gateway governance policy requires authentication evidence that the data plane cannot verify"
        );
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
            Obligation::RequireReauthentication | Obligation::RequireMfa
        )
    }) {
        anyhow::bail!(
            "gateway governance policy obligation requires unavailable reauthentication or MFA evidence"
        );
    }
    Ok(())
}
