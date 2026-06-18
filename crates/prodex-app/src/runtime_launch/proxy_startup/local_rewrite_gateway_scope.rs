#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RuntimeGatewayGovernanceScope {
    pub(super) tenant_id: Option<String>,
    pub(super) team_id: Option<String>,
    pub(super) project_id: Option<String>,
    pub(super) user_id: Option<String>,
    pub(super) budget_id: Option<String>,
}

impl RuntimeGatewayGovernanceScope {
    pub(super) fn new(
        tenant_id: Option<String>,
        team_id: Option<String>,
        project_id: Option<String>,
        user_id: Option<String>,
        budget_id: Option<String>,
    ) -> Self {
        Self {
            tenant_id,
            team_id,
            project_id,
            user_id,
            budget_id,
        }
    }

    pub(super) fn is_unscoped(&self) -> bool {
        self.tenant_id.is_none()
            && self.team_id.is_none()
            && self.project_id.is_none()
            && self.user_id.is_none()
            && self.budget_id.is_none()
    }

    pub(super) fn matches_tenant(&self, tenant_id: Option<&str>) -> bool {
        scoped_value_matches(self.tenant_id.as_deref(), tenant_id)
    }

    pub(super) fn matches_dimensions(
        &self,
        team_id: Option<&str>,
        project_id: Option<&str>,
        user_id: Option<&str>,
        budget_id: Option<&str>,
    ) -> bool {
        scoped_value_matches(self.team_id.as_deref(), team_id)
            && scoped_value_matches(self.project_id.as_deref(), project_id)
            && scoped_value_matches(self.user_id.as_deref(), user_id)
            && scoped_value_matches(self.budget_id.as_deref(), budget_id)
    }

    pub(super) fn matches(
        &self,
        tenant_id: Option<&str>,
        team_id: Option<&str>,
        project_id: Option<&str>,
        user_id: Option<&str>,
        budget_id: Option<&str>,
    ) -> bool {
        self.matches_tenant(tenant_id)
            && self.matches_dimensions(team_id, project_id, user_id, budget_id)
    }
}

fn scoped_value_matches(scope: Option<&str>, value: Option<&str>) -> bool {
    scope.map(|scope| value == Some(scope)).unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unscoped_scope_matches_any_governance_values() {
        let scope = RuntimeGatewayGovernanceScope::default();
        assert!(scope.is_unscoped());
        assert!(scope.matches(
            Some("tenant-a"),
            Some("team-a"),
            Some("project-a"),
            Some("alice"),
            Some("budget-a"),
        ));
    }

    #[test]
    fn scoped_values_must_match_present_resource_values() {
        let scope = RuntimeGatewayGovernanceScope::new(
            Some("tenant-a".to_string()),
            Some("team-a".to_string()),
            None,
            None,
            Some("budget-a".to_string()),
        );
        assert!(scope.matches(
            Some("tenant-a"),
            Some("team-a"),
            Some("project-a"),
            Some("alice"),
            Some("budget-a"),
        ));
        assert!(!scope.matches(
            Some("tenant-b"),
            Some("team-a"),
            Some("project-a"),
            Some("alice"),
            Some("budget-a"),
        ));
        assert!(!scope.matches(
            Some("tenant-a"),
            Some("team-b"),
            Some("project-a"),
            Some("alice"),
            Some("budget-a"),
        ));
        assert!(!scope.matches(
            Some("tenant-a"),
            Some("team-a"),
            Some("project-a"),
            Some("alice"),
            None,
        ));
    }
}
