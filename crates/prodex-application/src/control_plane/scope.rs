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
        #[cfg(feature = "mojo")]
        return self.mojo_matches(
            prodex_mojo_core::rich::ApplicationScopeOperation::DimensionsUnrestricted,
            [None; 5],
            &[],
        );

        #[cfg(not(feature = "mojo"))]
        self.dimensions_are_unrestricted_rust()
    }

    pub fn matches_tenant(&self, tenant_id: Option<&str>) -> bool {
        #[cfg(feature = "mojo")]
        return self.mojo_matches(
            prodex_mojo_core::rich::ApplicationScopeOperation::TenantMatches,
            [tenant_id, None, None, None, None],
            &[],
        );

        #[cfg(not(feature = "mojo"))]
        self.matches_tenant_rust(tenant_id)
    }

    pub fn matches_dimensions(
        &self,
        team_id: Option<&str>,
        project_id: Option<&str>,
        user_id: Option<&str>,
        budget_id: Option<&str>,
    ) -> bool {
        #[cfg(feature = "mojo")]
        return self.mojo_matches(
            prodex_mojo_core::rich::ApplicationScopeOperation::DimensionsMatch,
            [None, team_id, project_id, user_id, budget_id],
            &[],
        );

        #[cfg(not(feature = "mojo"))]
        self.matches_dimensions_rust(team_id, project_id, user_id, budget_id)
    }

    pub fn matches(
        &self,
        tenant_id: Option<&str>,
        team_id: Option<&str>,
        project_id: Option<&str>,
        user_id: Option<&str>,
        budget_id: Option<&str>,
    ) -> bool {
        #[cfg(feature = "mojo")]
        return self.mojo_matches(
            prodex_mojo_core::rich::ApplicationScopeOperation::AllMatch,
            [tenant_id, team_id, project_id, user_id, budget_id],
            &[],
        );

        #[cfg(not(feature = "mojo"))]
        self.matches_rust(tenant_id, team_id, project_id, user_id, budget_id)
    }

    pub fn matches_resource_name(&self, name: &str) -> bool {
        #[cfg(feature = "mojo")]
        return self.mojo_matches(
            prodex_mojo_core::rich::ApplicationScopeOperation::ResourceNameMatches,
            [None; 5],
            &[name],
        );

        #[cfg(not(feature = "mojo"))]
        self.matches_resource_name_rust(name)
    }

    pub fn allows_resource_prefixes(&self, prefixes: &[String]) -> bool {
        #[cfg(feature = "mojo")]
        {
            let candidates = prefixes.iter().map(String::as_str).collect::<Vec<_>>();
            self.mojo_matches(
                prodex_mojo_core::rich::ApplicationScopeOperation::ResourcePrefixesAllowed,
                [None; 5],
                &candidates,
            )
        }

        #[cfg(not(feature = "mojo"))]
        self.allows_resource_prefixes_rust(prefixes)
    }

    pub(super) fn tenant_id(&self) -> Option<&str> {
        self.tenant_id.as_deref()
    }

    pub(super) fn team_id(&self) -> Option<&str> {
        self.team_id.as_deref()
    }

    pub(super) fn project_id(&self) -> Option<&str> {
        self.project_id.as_deref()
    }

    pub(super) fn user_id(&self) -> Option<&str> {
        self.user_id.as_deref()
    }

    pub(super) fn budget_id(&self) -> Option<&str> {
        self.budget_id.as_deref()
    }

    pub(super) fn allowed_resource_prefixes(&self) -> &[String] {
        &self.allowed_resource_prefixes
    }

    #[cfg(feature = "mojo")]
    fn mojo_matches(
        &self,
        operation: prodex_mojo_core::rich::ApplicationScopeOperation,
        values: [Option<&str>; 5],
        candidates: &[&str],
    ) -> bool {
        let scope_prefixes = self
            .allowed_resource_prefixes
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        prodex_mojo_core::rich::application_governance_scope(
            prodex_mojo_core::rich::ApplicationScopeInput {
                operation,
                scope_values: [
                    self.tenant_id.as_deref(),
                    self.team_id.as_deref(),
                    self.project_id.as_deref(),
                    self.user_id.as_deref(),
                    self.budget_id.as_deref(),
                ],
                values,
                scope_prefixes: &scope_prefixes,
                candidates,
            },
        )
        .expect("Mojo application governance-scope planner returned invalid output")
    }

    #[cfg(any(not(feature = "mojo"), test))]
    fn dimensions_are_unrestricted_rust(&self) -> bool {
        self.tenant_id.is_none()
            && self.team_id.is_none()
            && self.project_id.is_none()
            && self.user_id.is_none()
            && self.budget_id.is_none()
    }

    #[cfg(any(not(feature = "mojo"), test))]
    fn matches_tenant_rust(&self, tenant_id: Option<&str>) -> bool {
        scoped_value_matches(self.tenant_id.as_deref(), tenant_id)
    }

    #[cfg(any(not(feature = "mojo"), test))]
    fn matches_dimensions_rust(
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

    #[cfg(any(not(feature = "mojo"), test))]
    fn matches_rust(
        &self,
        tenant_id: Option<&str>,
        team_id: Option<&str>,
        project_id: Option<&str>,
        user_id: Option<&str>,
        budget_id: Option<&str>,
    ) -> bool {
        self.matches_tenant_rust(tenant_id)
            && self.matches_dimensions_rust(team_id, project_id, user_id, budget_id)
    }

    #[cfg(any(not(feature = "mojo"), test))]
    fn matches_resource_name_rust(&self, name: &str) -> bool {
        self.allowed_resource_prefixes.is_empty()
            || self
                .allowed_resource_prefixes
                .iter()
                .any(|prefix| name.starts_with(prefix))
    }

    #[cfg(any(not(feature = "mojo"), test))]
    fn allows_resource_prefixes_rust(&self, prefixes: &[String]) -> bool {
        self.allowed_resource_prefixes.is_empty()
            || (!prefixes.is_empty()
                && prefixes
                    .iter()
                    .all(|prefix| self.matches_resource_name_rust(prefix)))
    }
}

fn redacted_option<T>(value: &Option<T>) -> Option<&'static str> {
    value.as_ref().map(|_| "<redacted>")
}

#[cfg(any(not(feature = "mojo"), test))]
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
        assert!(scope.allows_resource_prefixes(&["team-a-children".to_string()]));
        assert!(!scope.allows_resource_prefixes(&[]));
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

    #[cfg(feature = "mojo")]
    #[test]
    fn mojo_scope_plans_match_rust_oracle() {
        let scope = ApplicationControlPlaneGovernanceScope::new(
            Some("tenant-a".to_string()),
            Some("team-a".to_string()),
            None,
            Some("user-a".to_string()),
            Some("budget-a".to_string()),
            vec!["team-a-".to_string(), "shared-".to_string()],
        );
        assert_eq!(
            scope.dimensions_are_unrestricted(),
            scope.dimensions_are_unrestricted_rust()
        );
        for tenant in [Some("tenant-a"), Some("tenant-b"), None] {
            assert_eq!(
                scope.matches_tenant(tenant),
                scope.matches_tenant_rust(tenant)
            );
        }
        let dimensions = [
            (Some("team-a"), None, Some("user-a"), Some("budget-a")),
            (Some("team-b"), None, Some("user-a"), Some("budget-a")),
            (Some("team-a"), None, None, Some("budget-a")),
        ];
        for (team, project, user, budget) in dimensions {
            assert_eq!(
                scope.matches_dimensions(team, project, user, budget),
                scope.matches_dimensions_rust(team, project, user, budget),
            );
            assert_eq!(
                scope.matches(Some("tenant-a"), team, project, user, budget),
                scope.matches_rust(Some("tenant-a"), team, project, user, budget),
            );
        }
        for name in ["team-a-key", "shared-key", "team-b-key", ""] {
            assert_eq!(
                scope.matches_resource_name(name),
                scope.matches_resource_name_rust(name),
            );
        }
        for prefixes in [
            vec!["team-a-child".to_string()],
            vec!["team-b-child".to_string()],
            Vec::new(),
        ] {
            assert_eq!(
                scope.allows_resource_prefixes(&prefixes),
                scope.allows_resource_prefixes_rust(&prefixes),
            );
        }
    }
}
