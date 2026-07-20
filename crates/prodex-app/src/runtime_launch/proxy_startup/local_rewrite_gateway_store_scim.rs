use super::local_rewrite_gateway_store_types::{
    RuntimeGatewayVirtualKeyEntry, redacted_option, runtime_gateway_exact_optional_stored_scope,
    runtime_gateway_exact_stored_string,
};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct RuntimeGatewayScimUser {
    pub(super) id: String,
    pub(super) user_name: String,
    #[serde(default)]
    pub(super) external_id: Option<String>,
    #[serde(default)]
    pub(super) display_name: Option<String>,
    #[serde(default = "runtime_gateway_scim_user_active_default")]
    pub(super) active: bool,
    #[serde(default)]
    pub(super) role: Option<String>,
    #[serde(default)]
    pub(super) tenant_id: Option<String>,
    #[serde(default)]
    pub(super) team_id: Option<String>,
    #[serde(default)]
    pub(super) project_id: Option<String>,
    #[serde(default)]
    pub(super) user_id: Option<String>,
    #[serde(default)]
    pub(super) group_ids: Vec<String>,
    #[serde(default)]
    pub(super) department_id: Option<String>,
    #[serde(default)]
    pub(super) budget_id: Option<String>,
    #[serde(default)]
    pub(super) allowed_key_prefixes: Vec<String>,
    pub(super) created_at_epoch: u64,
    pub(super) updated_at_epoch: u64,
}

impl fmt::Debug for RuntimeGatewayScimUser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGatewayScimUser")
            .field("id", &"<redacted>")
            .field("user_name", &"<redacted>")
            .field("external_id", &redacted_option(&self.external_id))
            .field("display_name", &redacted_option(&self.display_name))
            .field("active", &self.active)
            .field("role", &redacted_option(&self.role))
            .field("tenant_id", &redacted_option(&self.tenant_id))
            .field("team_id", &redacted_option(&self.team_id))
            .field("project_id", &redacted_option(&self.project_id))
            .field("user_id", &redacted_option(&self.user_id))
            .field("group_ids", &"<redacted>")
            .field("department_id", &redacted_option(&self.department_id))
            .field("budget_id", &redacted_option(&self.budget_id))
            .field("allowed_key_prefixes", &"<redacted>")
            .field("created_at_epoch", &"<redacted>")
            .field("updated_at_epoch", &"<redacted>")
            .finish()
    }
}

pub(super) fn runtime_gateway_scim_user_auth_entry_from_stored(
    user: &RuntimeGatewayScimUser,
) -> Option<RuntimeGatewayScimUser> {
    runtime_gateway_exact_stored_string(&user.id)?;
    runtime_gateway_exact_stored_string(&user.user_name)?;
    let role = runtime_gateway_exact_optional_stored_scope(&user.role)?;
    let tenant_id = runtime_gateway_exact_optional_stored_scope(&user.tenant_id)?;
    let team_id = runtime_gateway_exact_optional_stored_scope(&user.team_id)?;
    let project_id = runtime_gateway_exact_optional_stored_scope(&user.project_id)?;
    let user_id = runtime_gateway_exact_optional_stored_scope(&user.user_id)?;
    if user.group_ids.len() > prodex_domain::MAX_POLICY_PRINCIPAL_GROUPS
        || user
            .group_ids
            .iter()
            .any(|value| prodex_domain::PolicySelector::new(value.clone()).is_err() || value == "*")
        || user.group_ids.iter().enumerate().any(|(index, value)| {
            user.group_ids[index + 1..]
                .iter()
                .any(|other| value.eq_ignore_ascii_case(other))
        })
    {
        return None;
    }
    let department_id = runtime_gateway_exact_optional_stored_scope(&user.department_id)?;
    if department_id
        .as_deref()
        .is_some_and(|value| prodex_domain::PolicySelector::new(value).is_err() || value == "*")
    {
        return None;
    }
    let budget_id = runtime_gateway_exact_optional_stored_scope(&user.budget_id)?;
    if !user
        .allowed_key_prefixes
        .iter()
        .all(|prefix| runtime_gateway_exact_stored_string(prefix).is_some())
    {
        return None;
    }
    Some(RuntimeGatewayScimUser {
        id: user.id.clone(),
        user_name: user.user_name.clone(),
        external_id: user.external_id.clone(),
        display_name: user.display_name.clone(),
        active: user.active,
        role,
        tenant_id,
        team_id,
        project_id,
        user_id,
        group_ids: user.group_ids.clone(),
        department_id,
        budget_id,
        allowed_key_prefixes: user.allowed_key_prefixes.clone(),
        created_at_epoch: user.created_at_epoch,
        updated_at_epoch: user.updated_at_epoch,
    })
}

pub(super) fn runtime_gateway_apply_scim_policy_attributes(
    entries: &mut [RuntimeGatewayVirtualKeyEntry],
    users: &[RuntimeGatewayScimUser],
) {
    for entry in entries {
        let (group_ids, department_id) = runtime_gateway_scim_policy_attributes(entry, users);
        entry.group_ids = group_ids;
        entry.department_id = department_id;
    }
}

pub(super) fn runtime_gateway_principal_policy_attributes(
    key: &runtime_proxy_crate::RuntimeGatewayVirtualKey,
    entry: Option<&RuntimeGatewayVirtualKeyEntry>,
) -> Result<prodex_domain::PrincipalPolicyAttributes, prodex_domain::GovernancePolicyError> {
    prodex_domain::PrincipalPolicyAttributes::new_with_organization(
        key.team_id.as_deref(),
        key.project_id.as_deref(),
        key.user_id.as_deref(),
        entry.map(|entry| entry.group_ids.as_slice()).unwrap_or(&[]),
        entry.and_then(|entry| entry.department_id.as_deref()),
    )
}

fn runtime_gateway_scim_policy_attributes(
    entry: &RuntimeGatewayVirtualKeyEntry,
    users: &[RuntimeGatewayScimUser],
) -> (Vec<String>, Option<String>) {
    let (Some(tenant_id), Some(user_id)) =
        (entry.tenant_id.as_deref(), entry.key.user_id.as_deref())
    else {
        return (Vec::new(), None);
    };
    let mut matches = users
        .iter()
        .filter_map(runtime_gateway_scim_user_auth_entry_from_stored)
        .filter(|user| {
            user.active
                && user.tenant_id.as_deref() == Some(tenant_id)
                && user.user_id.as_deref() == Some(user_id)
        });
    let Some(user) = matches.next() else {
        return (Vec::new(), None);
    };
    if matches.next().is_some() {
        return (Vec::new(), None);
    }
    (user.group_ids, user.department_id)
}

fn runtime_gateway_scim_user_active_default() -> bool {
    true
}
