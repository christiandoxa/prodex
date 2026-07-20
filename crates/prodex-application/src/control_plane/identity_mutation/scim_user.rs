use std::fmt;

use prodex_control_plane::{ControlPlaneActionPlan, ControlPlaneOperation};
use prodex_domain::{PrincipalId, ResourceKind};

use super::*;

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationGatewayScimUserRecord {
    pub id: PrincipalId,
    pub tenant_id: Option<String>,
    pub user_name: String,
    pub external_id: Option<String>,
    pub display_name: Option<String>,
    pub active: bool,
    pub role: Option<String>,
    pub team_id: Option<String>,
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    pub group_ids: Vec<String>,
    pub department_id: Option<String>,
    pub budget_id: Option<String>,
    pub allowed_key_prefixes: Vec<String>,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
}

impl fmt::Debug for ApplicationGatewayScimUserRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationGatewayScimUserRecord")
            .field("id", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("identity", &"<redacted>")
            .field("governance", &"<redacted>")
            .field("timestamps", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationScimUserUpdateMode {
    Patch,
    Replace,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct ApplicationGatewayScimUserPatch {
    pub tenant_id: ApplicationPatchValue<String>,
    pub user_name: ApplicationPatchValue<String>,
    pub external_id: ApplicationPatchValue<String>,
    pub display_name: ApplicationPatchValue<String>,
    pub active: ApplicationPatchValue<bool>,
    pub role: ApplicationPatchValue<String>,
    pub team_id: ApplicationPatchValue<String>,
    pub project_id: ApplicationPatchValue<String>,
    pub user_id: ApplicationPatchValue<String>,
    pub group_ids: ApplicationPatchValue<Vec<String>>,
    pub department_id: ApplicationPatchValue<String>,
    pub budget_id: ApplicationPatchValue<String>,
    pub allowed_key_prefixes: ApplicationPatchValue<Vec<String>>,
}

impl fmt::Debug for ApplicationGatewayScimUserPatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationGatewayScimUserPatch")
            .field("fields", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationGatewayScimUserMutation {
    Create {
        id: PrincipalId,
        patch: ApplicationGatewayScimUserPatch,
    },
    Update {
        id: PrincipalId,
        mode: ApplicationScimUserUpdateMode,
        patch: ApplicationGatewayScimUserPatch,
    },
    Delete {
        id: PrincipalId,
    },
}

impl fmt::Debug for ApplicationGatewayScimUserMutation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Create { .. } => "Create(<redacted>)",
            Self::Update { .. } => "Update(<redacted>)",
            Self::Delete { .. } => "Delete(<redacted>)",
        })
    }
}

pub struct ApplicationGatewayScimUserMutationRequest<'a> {
    pub authorized_action: &'a ControlPlaneActionPlan,
    pub governance: &'a ApplicationControlPlaneGovernanceScope,
    pub current_records: &'a [ApplicationGatewayScimUserRecord],
    pub mutation: ApplicationGatewayScimUserMutation,
    pub now_unix_ms: u64,
}

impl fmt::Debug for ApplicationGatewayScimUserMutationRequest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationGatewayScimUserMutationRequest")
            .field("authorized_action", &"<redacted>")
            .field("governance", &self.governance)
            .field("current_records", &"<redacted>")
            .field("mutation", &self.mutation)
            .field("now_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationGatewayScimUserMutationPlan {
    pub projection: ApplicationGatewayIdentityProjection<ApplicationGatewayScimUserRecord>,
    pub authorized_action: ControlPlaneActionPlan,
}

impl fmt::Debug for ApplicationGatewayScimUserMutationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationGatewayScimUserMutationPlan")
            .field("projection", &self.projection)
            .field("authorized_action", &"<redacted>")
            .finish()
    }
}

pub fn plan_application_gateway_scim_user_mutation(
    request: ApplicationGatewayScimUserMutationRequest<'_>,
) -> Result<ApplicationGatewayScimUserMutationPlan, ApplicationGatewayIdentityMutationError> {
    let ApplicationGatewayScimUserMutationRequest {
        authorized_action,
        governance,
        current_records,
        mutation,
        now_unix_ms,
    } = request;
    if now_unix_ms == 0 {
        return Err(ApplicationGatewayIdentityMutationError::InvalidTimestamp);
    }
    match mutation {
        ApplicationGatewayScimUserMutation::Create { id, patch } => plan_create(
            authorized_action,
            governance,
            current_records,
            now_unix_ms,
            id,
            patch,
        ),
        ApplicationGatewayScimUserMutation::Update { id, mode, patch } => plan_update(
            authorized_action,
            governance,
            current_records,
            now_unix_ms,
            id,
            mode,
            patch,
        ),
        ApplicationGatewayScimUserMutation::Delete { id } => plan_delete(
            authorized_action,
            governance,
            current_records,
            now_unix_ms,
            id,
        ),
    }
}

fn plan_create(
    authorized_action: &ControlPlaneActionPlan,
    governance: &ApplicationControlPlaneGovernanceScope,
    current_records: &[ApplicationGatewayScimUserRecord],
    now_unix_ms: u64,
    id: PrincipalId,
    patch: ApplicationGatewayScimUserPatch,
) -> Result<ApplicationGatewayScimUserMutationPlan, ApplicationGatewayIdentityMutationError> {
    let resource_id = id.to_string();
    let authorized_action = validate_action(
        authorized_action,
        ControlPlaneOperation::ScimUserCreate,
        ResourceKind::User,
        "user",
        &resource_id,
        true,
    )?;
    let mut record = ApplicationGatewayScimUserRecord {
        id,
        tenant_id: governance.tenant_id().map(str::to_string),
        user_name: String::new(),
        external_id: None,
        display_name: None,
        active: true,
        role: None,
        team_id: governance.team_id().map(str::to_string),
        project_id: governance.project_id().map(str::to_string),
        user_id: Some(
            governance
                .user_id()
                .unwrap_or(resource_id.as_str())
                .to_string(),
        ),
        group_ids: Vec::new(),
        department_id: None,
        budget_id: governance.budget_id().map(str::to_string),
        allowed_key_prefixes: governance.allowed_resource_prefixes().to_vec(),
        created_at_unix_ms: now_unix_ms,
        updated_at_unix_ms: now_unix_ms,
    };
    apply_scim_patch(
        &mut record,
        patch,
        ApplicationScimUserUpdateMode::Patch,
        true,
    )?;
    validate_scim_user_record(&record)?;
    if !governance_allows_scim_user(governance, &record) {
        return Err(ApplicationGatewayIdentityMutationError::GovernanceDenied);
    }
    if current_records
        .iter()
        .any(|current| current.id == record.id)
    {
        return Err(ApplicationGatewayIdentityMutationError::DuplicateIdentity);
    }
    if current_records
        .iter()
        .any(|current| current.user_name.eq_ignore_ascii_case(&record.user_name))
    {
        return Err(ApplicationGatewayIdentityMutationError::DuplicateName);
    }
    Ok(ApplicationGatewayScimUserMutationPlan {
        projection: ApplicationGatewayIdentityProjection::Insert { record },
        authorized_action,
    })
}

fn plan_update(
    authorized_action: &ControlPlaneActionPlan,
    governance: &ApplicationControlPlaneGovernanceScope,
    current_records: &[ApplicationGatewayScimUserRecord],
    now_unix_ms: u64,
    id: PrincipalId,
    mode: ApplicationScimUserUpdateMode,
    patch: ApplicationGatewayScimUserPatch,
) -> Result<ApplicationGatewayScimUserMutationPlan, ApplicationGatewayIdentityMutationError> {
    let (index, current) = find_scim_user(current_records, id)?;
    let authorized_action = validate_existing_action(
        authorized_action,
        ControlPlaneOperation::ScimUserUpdate,
        current,
    )?;
    validate_current(governance, now_unix_ms, current)?;
    let mut record = current.clone();
    apply_scim_patch(&mut record, patch, mode, false)?;
    record.updated_at_unix_ms = now_unix_ms;
    validate_scim_user_record(&record)?;
    if !governance_allows_scim_user(governance, &record) {
        return Err(ApplicationGatewayIdentityMutationError::GovernanceDenied);
    }
    if current_records.iter().enumerate().any(|(other, value)| {
        other != index && value.user_name.eq_ignore_ascii_case(&record.user_name)
    }) {
        return Err(ApplicationGatewayIdentityMutationError::DuplicateName);
    }
    Ok(ApplicationGatewayScimUserMutationPlan {
        projection: ApplicationGatewayIdentityProjection::Replace { index, record },
        authorized_action,
    })
}

fn plan_delete(
    authorized_action: &ControlPlaneActionPlan,
    governance: &ApplicationControlPlaneGovernanceScope,
    current_records: &[ApplicationGatewayScimUserRecord],
    now_unix_ms: u64,
    id: PrincipalId,
) -> Result<ApplicationGatewayScimUserMutationPlan, ApplicationGatewayIdentityMutationError> {
    let (index, current) = find_scim_user(current_records, id)?;
    let authorized_action = validate_existing_action(
        authorized_action,
        ControlPlaneOperation::ScimUserDelete,
        current,
    )?;
    validate_current(governance, now_unix_ms, current)?;
    Ok(ApplicationGatewayScimUserMutationPlan {
        projection: ApplicationGatewayIdentityProjection::Delete {
            index,
            record: current.clone(),
        },
        authorized_action,
    })
}

fn validate_existing_action(
    action: &ControlPlaneActionPlan,
    operation: ControlPlaneOperation,
    record: &ApplicationGatewayScimUserRecord,
) -> Result<ControlPlaneActionPlan, ApplicationGatewayIdentityMutationError> {
    validate_action(
        action,
        operation,
        ResourceKind::User,
        "user",
        &record.id.to_string(),
        false,
    )
}

fn validate_current(
    governance: &ApplicationControlPlaneGovernanceScope,
    now_unix_ms: u64,
    record: &ApplicationGatewayScimUserRecord,
) -> Result<(), ApplicationGatewayIdentityMutationError> {
    validate_scim_user_record(record)?;
    if now_unix_ms < record.updated_at_unix_ms {
        return Err(ApplicationGatewayIdentityMutationError::InvalidTimestamp);
    }
    if !governance_allows_scim_user(governance, record) {
        return Err(ApplicationGatewayIdentityMutationError::GovernanceDenied);
    }
    Ok(())
}

fn find_scim_user(
    records: &[ApplicationGatewayScimUserRecord],
    id: PrincipalId,
) -> Result<(usize, &ApplicationGatewayScimUserRecord), ApplicationGatewayIdentityMutationError> {
    let mut matches = records
        .iter()
        .enumerate()
        .filter(|(_, record)| record.id == id);
    let found = matches
        .next()
        .ok_or(ApplicationGatewayIdentityMutationError::NotFound)?;
    if matches.next().is_some() {
        return Err(ApplicationGatewayIdentityMutationError::DuplicateIdentity);
    }
    Ok(found)
}

fn apply_scim_patch(
    record: &mut ApplicationGatewayScimUserRecord,
    patch: ApplicationGatewayScimUserPatch,
    mode: ApplicationScimUserUpdateMode,
    creating: bool,
) -> Result<(), ApplicationGatewayIdentityMutationError> {
    match patch.user_name {
        ApplicationPatchValue::Set(user_name) => record.user_name = user_name,
        ApplicationPatchValue::Unchanged
            if !creating && mode == ApplicationScimUserUpdateMode::Patch => {}
        ApplicationPatchValue::Unchanged | ApplicationPatchValue::Clear => {
            return Err(ApplicationGatewayIdentityMutationError::RequiredFieldMissing);
        }
    }
    let reset_unchanged = !creating && mode == ApplicationScimUserUpdateMode::Replace;
    apply_optional(&mut record.tenant_id, patch.tenant_id, reset_unchanged);
    apply_optional(&mut record.external_id, patch.external_id, reset_unchanged);
    apply_optional(
        &mut record.display_name,
        patch.display_name,
        reset_unchanged,
    );
    apply_optional(&mut record.team_id, patch.team_id, reset_unchanged);
    apply_optional(&mut record.project_id, patch.project_id, reset_unchanged);
    apply_optional(&mut record.user_id, patch.user_id, reset_unchanged);
    apply_optional(
        &mut record.department_id,
        patch.department_id,
        reset_unchanged,
    );
    apply_optional(&mut record.budget_id, patch.budget_id, reset_unchanged);
    match patch.active {
        ApplicationPatchValue::Unchanged if !reset_unchanged => {}
        ApplicationPatchValue::Unchanged | ApplicationPatchValue::Clear => record.active = true,
        ApplicationPatchValue::Set(active) => record.active = active,
    }
    match patch.role {
        ApplicationPatchValue::Unchanged if !reset_unchanged => {}
        ApplicationPatchValue::Unchanged | ApplicationPatchValue::Clear => record.role = None,
        ApplicationPatchValue::Set(role) => record.role = Some(normalize_role(&role)?),
    }
    match patch.allowed_key_prefixes {
        ApplicationPatchValue::Unchanged if !reset_unchanged => {}
        ApplicationPatchValue::Unchanged | ApplicationPatchValue::Clear => {
            record.allowed_key_prefixes.clear();
        }
        ApplicationPatchValue::Set(prefixes) => record.allowed_key_prefixes = prefixes,
    }
    match patch.group_ids {
        ApplicationPatchValue::Unchanged if !reset_unchanged => {}
        ApplicationPatchValue::Unchanged | ApplicationPatchValue::Clear => {
            record.group_ids.clear();
        }
        ApplicationPatchValue::Set(group_ids) => record.group_ids = group_ids,
    }
    Ok(())
}

fn apply_optional<T>(
    target: &mut Option<T>,
    patch: ApplicationPatchValue<T>,
    reset_unchanged: bool,
) {
    match patch {
        ApplicationPatchValue::Unchanged if !reset_unchanged => {}
        ApplicationPatchValue::Unchanged | ApplicationPatchValue::Clear => *target = None,
        ApplicationPatchValue::Set(value) => *target = Some(value),
    }
}

fn validate_scim_user_record(
    record: &ApplicationGatewayScimUserRecord,
) -> Result<(), ApplicationGatewayIdentityMutationError> {
    if record.user_name.is_empty()
        || record.user_name.len() > 320
        || record
            .user_name
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err(ApplicationGatewayIdentityMutationError::InvalidScimUserName);
    }
    if !validate_optional_text(&record.external_id) || !validate_optional_text(&record.display_name)
    {
        return Err(ApplicationGatewayIdentityMutationError::InvalidScimField);
    }
    if record
        .role
        .as_deref()
        .is_some_and(|role| !matches!(role, "admin" | "viewer"))
    {
        return Err(ApplicationGatewayIdentityMutationError::InvalidScimRole);
    }
    if [
        &record.tenant_id,
        &record.team_id,
        &record.project_id,
        &record.user_id,
        &record.department_id,
        &record.budget_id,
    ]
    .into_iter()
    .any(|value| !validate_scope_value(value))
    {
        return Err(ApplicationGatewayIdentityMutationError::InvalidScopeValue);
    }
    if record.group_ids.len() > prodex_domain::MAX_POLICY_PRINCIPAL_GROUPS
        || record
            .group_ids
            .iter()
            .any(|group_id| !valid_policy_attribute(group_id))
        || record
            .group_ids
            .iter()
            .enumerate()
            .any(|(index, group_id)| {
                record.group_ids[index + 1..]
                    .iter()
                    .any(|other| group_id.eq_ignore_ascii_case(other))
            })
        || record
            .department_id
            .as_deref()
            .is_some_and(|value| !valid_policy_attribute(value))
    {
        return Err(ApplicationGatewayIdentityMutationError::InvalidScopeValue);
    }
    if record.allowed_key_prefixes.len() > 128
        || record
            .allowed_key_prefixes
            .iter()
            .any(|prefix| !valid_key_prefix(prefix))
    {
        return Err(ApplicationGatewayIdentityMutationError::InvalidKeyPrefix);
    }
    for (index, prefix) in record.allowed_key_prefixes.iter().enumerate() {
        if record.allowed_key_prefixes[index + 1..]
            .iter()
            .any(|other| prefix.eq_ignore_ascii_case(other))
        {
            return Err(ApplicationGatewayIdentityMutationError::DuplicateKeyPrefix);
        }
    }
    if !timestamp_is_valid(record.created_at_unix_ms, record.updated_at_unix_ms) {
        return Err(ApplicationGatewayIdentityMutationError::InvalidTimestamp);
    }
    Ok(())
}

fn governance_allows_scim_user(
    governance: &ApplicationControlPlaneGovernanceScope,
    record: &ApplicationGatewayScimUserRecord,
) -> bool {
    governance.matches(
        record.tenant_id.as_deref(),
        record.team_id.as_deref(),
        record.project_id.as_deref(),
        record.user_id.as_deref(),
        record.budget_id.as_deref(),
    ) && governance.allows_resource_prefixes(&record.allowed_key_prefixes)
}

fn normalize_role(role: &str) -> Result<String, ApplicationGatewayIdentityMutationError> {
    if role.eq_ignore_ascii_case("admin") {
        Ok("admin".to_string())
    } else if role.eq_ignore_ascii_case("viewer") {
        Ok("viewer".to_string())
    } else {
        Err(ApplicationGatewayIdentityMutationError::InvalidScimRole)
    }
}

fn valid_key_prefix(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn valid_policy_attribute(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/')
        })
}
