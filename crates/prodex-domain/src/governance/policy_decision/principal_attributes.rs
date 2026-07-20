use super::{GovernancePolicyError, PolicySelector};
use std::fmt;

pub const MAX_POLICY_PRINCIPAL_GROUPS: usize = 128;

#[derive(Clone, Default, PartialEq, Eq)]
pub struct PrincipalPolicyAttributes {
    team_id: Option<PolicySelector>,
    project_id: Option<PolicySelector>,
    user_id: Option<PolicySelector>,
    group_ids: Vec<PolicySelector>,
    department_id: Option<PolicySelector>,
}

impl PrincipalPolicyAttributes {
    pub fn new(
        team_id: Option<&str>,
        project_id: Option<&str>,
        user_id: Option<&str>,
    ) -> Result<Self, GovernancePolicyError> {
        Self::new_with_organization(team_id, project_id, user_id, &[], None)
    }

    pub fn new_with_organization(
        team_id: Option<&str>,
        project_id: Option<&str>,
        user_id: Option<&str>,
        group_ids: &[String],
        department_id: Option<&str>,
    ) -> Result<Self, GovernancePolicyError> {
        if group_ids.len() > MAX_POLICY_PRINCIPAL_GROUPS {
            return Err(GovernancePolicyError::AttributeLimitExceeded);
        }
        Ok(Self {
            team_id: team_id.map(PolicySelector::new).transpose()?,
            project_id: project_id.map(PolicySelector::new).transpose()?,
            user_id: user_id.map(PolicySelector::new).transpose()?,
            group_ids: group_ids
                .iter()
                .map(|value| PolicySelector::new(value.clone()))
                .collect::<Result<_, _>>()?,
            department_id: department_id.map(PolicySelector::new).transpose()?,
        })
    }

    pub fn team_id(&self) -> Option<&str> {
        self.team_id.as_ref().map(PolicySelector::as_str)
    }

    pub fn project_id(&self) -> Option<&str> {
        self.project_id.as_ref().map(PolicySelector::as_str)
    }

    pub fn user_id(&self) -> Option<&str> {
        self.user_id.as_ref().map(PolicySelector::as_str)
    }

    pub fn group_ids(&self) -> impl Iterator<Item = &str> {
        self.group_ids.iter().map(PolicySelector::as_str)
    }

    pub fn department_id(&self) -> Option<&str> {
        self.department_id.as_ref().map(PolicySelector::as_str)
    }
}

impl fmt::Debug for PrincipalPolicyAttributes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrincipalPolicyAttributes")
            .field("team_id", &self.team_id.as_ref().map(|_| "<redacted>"))
            .field(
                "project_id",
                &self.project_id.as_ref().map(|_| "<redacted>"),
            )
            .field("user_id", &self.user_id.as_ref().map(|_| "<redacted>"))
            .field(
                "group_ids",
                &(!self.group_ids.is_empty()).then_some("<redacted>"),
            )
            .field(
                "department_id",
                &self.department_id.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}
