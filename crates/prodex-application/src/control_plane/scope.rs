//! Control-plane credential scope matching.

use std::fmt;

#[derive(Clone, Default, PartialEq, Eq)]
pub struct ApplicationControlPlaneGovernanceScope {
    tenant_id: Option<String>,
    team_id: Option<String>,
    project_id: Option<String>,
    user_id: Option<String>,
    budget_id: Option<String>,
    allowed_resource_prefixes: Vec<String>,
}

impl fmt::Debug for ApplicationControlPlaneGovernanceScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationControlPlaneGovernanceScope")
            .field("tenant_id", &redacted_option(&self.tenant_id))
            .field("team_id", &redacted_option(&self.team_id))
            .field("project_id", &redacted_option(&self.project_id))
            .field("user_id", &redacted_option(&self.user_id))
            .field("budget_id", &redacted_option(&self.budget_id))
            .field(
                "allowed_resource_prefixes",
                &(!self.allowed_resource_prefixes.is_empty()).then_some("<redacted>"),
            )
            .finish()
    }
}

impl ApplicationControlPlaneGovernanceScope {
    pub fn new(
        tenant_id: Option<String>,
        team_id: Option<String>,
        project_id: Option<String>,
        user_id: Option<String>,
        budget_id: Option<String>,
        allowed_resource_prefixes: Vec<String>,
    ) -> Self {
        Self {
            tenant_id,
            team_id,
            project_id,
            user_id,
            budget_id,
            allowed_resource_prefixes,
        }
    }

    pub fn dimensions_are_unrestricted(&self) -> bool {
        self.tenant_id.is_none()
            && self.team_id.is_none()
            && self.project_id.is_none()
            && self.user_id.is_none()
            && self.budget_id.is_none()
    }

    pub fn matches_tenant(&self, tenant_id: Option<&str>) -> bool {
        scoped_value_matches(self.tenant_id.as_deref(), tenant_id)
    }

    pub fn matches_dimensions(
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

    pub fn matches(
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

    pub fn matches_resource_name(&self, name: &str) -> bool {
        self.allowed_resource_prefixes.is_empty()
            || self
                .allowed_resource_prefixes
                .iter()
                .any(|prefix| name.starts_with(prefix))
    }
}

fn redacted_option<T>(value: &Option<T>) -> Option<&'static str> {
    value.as_ref().map(|_| "<redacted>")
}

fn scoped_value_matches(scope: Option<&str>, value: Option<&str>) -> bool {
    scope.map(|scope| value == Some(scope)).unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unscoped_scope_matches_any_governance_values() {
        let scope = ApplicationControlPlaneGovernanceScope::default();
        assert!(scope.dimensions_are_unrestricted());
        assert!(scope.matches(
            Some("tenant-a"),
            Some("team-a"),
            Some("project-a"),
            Some("alice"),
            Some("budget-a"),
        ));
        assert!(scope.matches_resource_name("key-a"));
    }

    #[test]
    fn scoped_values_and_resource_prefixes_must_match() {
        let scope = ApplicationControlPlaneGovernanceScope::new(
            Some("tenant-a".to_string()),
            Some("team-a".to_string()),
            None,
            None,
            Some("budget-a".to_string()),
            vec!["team-a-".to_string()],
        );
        assert!(scope.matches(
            Some("tenant-a"),
            Some("team-a"),
            Some("project-a"),
            Some("alice"),
            Some("budget-a"),
        ));
        assert!(scope.matches_resource_name("team-a-key"));
        assert!(!scope.matches_resource_name("team-b-key"));
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

    #[test]
    fn governance_scope_debug_output_redacts_sensitive_fields() {
        let scope = ApplicationControlPlaneGovernanceScope::new(
            Some("tenant-scope-secret".to_string()),
            Some("team-scope-secret".to_string()),
            Some("project-scope-secret".to_string()),
            Some("user-scope-secret".to_string()),
            Some("budget-scope-secret".to_string()),
            vec!["key-prefix-secret".to_string()],
        );
        let rendered = format!("{scope:?}");

        assert!(rendered.contains("ApplicationControlPlaneGovernanceScope"));
        assert!(rendered.contains("<redacted>"));
        for raw in [
            "tenant-scope-secret",
            "team-scope-secret",
            "project-scope-secret",
            "user-scope-secret",
            "budget-scope-secret",
            "key-prefix-secret",
        ] {
            assert!(!rendered.contains(raw), "{rendered}");
        }
    }
}
