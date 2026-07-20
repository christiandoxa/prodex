use anyhow::{Context, Result};
use prodex_domain::PolicySelector;
use prodex_runtime_policy::RuntimeGovernancePolicyRuleCondition;

pub(super) struct RuntimeGovernancePrincipalSelectors {
    pub(super) team_id: Option<PolicySelector>,
    pub(super) project_id: Option<PolicySelector>,
    pub(super) user_id: Option<PolicySelector>,
    pub(super) group_id: Option<PolicySelector>,
    pub(super) department_id: Option<PolicySelector>,
}

pub(super) fn runtime_governance_principal_selectors(
    condition: &RuntimeGovernancePolicyRuleCondition,
) -> Result<RuntimeGovernancePrincipalSelectors> {
    Ok(RuntimeGovernancePrincipalSelectors {
        team_id: policy_selector(condition.team_id.as_deref(), "team")?,
        project_id: policy_selector(condition.project_id.as_deref(), "project")?,
        user_id: policy_selector(condition.user_id.as_deref(), "user")?,
        group_id: policy_selector(condition.group_id.as_deref(), "group")?,
        department_id: policy_selector(condition.department_id.as_deref(), "department")?,
    })
}

fn policy_selector(value: Option<&str>, label: &str) -> Result<Option<PolicySelector>> {
    value
        .map(PolicySelector::new)
        .transpose()
        .with_context(|| format!("invalid governance policy {label} selector"))
}

#[cfg(test)]
mod tests {
    use super::{RuntimeGovernancePolicyRuleCondition, runtime_governance_principal_selectors};
    use prodex_domain::PolicySelector;

    #[test]
    fn serialized_organization_selectors_map_to_domain_policy() {
        let condition = RuntimeGovernancePolicyRuleCondition {
            group_id: Some("engineering".to_string()),
            department_id: Some("research".to_string()),
            ..Default::default()
        };

        let mapped = runtime_governance_principal_selectors(&condition).unwrap();

        assert_eq!(
            mapped.group_id.as_ref().map(PolicySelector::as_str),
            Some("engineering")
        );
        assert_eq!(
            mapped.department_id.as_ref().map(PolicySelector::as_str),
            Some("research")
        );
    }
}
