use std::fmt;

use prodex_control_plane::{ControlPlaneActionPlan, ControlPlaneOperation};
use prodex_domain::{ResourceKind, VirtualKeyId};

use super::*;

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationGatewayVirtualKeyRecord {
    pub id: VirtualKeyId,
    pub tenant_id: Option<String>,
    pub name: String,
    pub source: ApplicationGatewayRecordSource,
    pub team_id: Option<String>,
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    pub budget_id: Option<String>,
    pub allowed_models: Vec<String>,
    pub budget_microusd: Option<u64>,
    pub request_budget: Option<u64>,
    pub rpm_limit: Option<u64>,
    pub tpm_limit: Option<u64>,
    pub disabled: bool,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
}

impl fmt::Debug for ApplicationGatewayVirtualKeyRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationGatewayVirtualKeyRecord")
            .field("id", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("name", &"<redacted>")
            .field("source", &self.source)
            .field("policy", &"<redacted>")
            .field("timestamps", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct ApplicationGatewayVirtualKeyPatch {
    pub tenant_id: ApplicationPatchValue<String>,
    pub team_id: ApplicationPatchValue<String>,
    pub project_id: ApplicationPatchValue<String>,
    pub user_id: ApplicationPatchValue<String>,
    pub budget_id: ApplicationPatchValue<String>,
    pub allowed_models: ApplicationPatchValue<Vec<String>>,
    pub budget_microusd: ApplicationPatchValue<u64>,
    pub request_budget: ApplicationPatchValue<u64>,
    pub rpm_limit: ApplicationPatchValue<u64>,
    pub tpm_limit: ApplicationPatchValue<u64>,
    pub disabled: ApplicationPatchValue<bool>,
}

impl fmt::Debug for ApplicationGatewayVirtualKeyPatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationGatewayVirtualKeyPatch")
            .field("fields", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationGatewayVirtualKeyMutation {
    Create {
        id: VirtualKeyId,
        name: String,
        patch: ApplicationGatewayVirtualKeyPatch,
        secret_fingerprint: ApplicationSecretFingerprint,
    },
    Update {
        id: VirtualKeyId,
        patch: ApplicationGatewayVirtualKeyPatch,
    },
    RotateSecret {
        id: VirtualKeyId,
        secret_fingerprint: ApplicationSecretFingerprint,
    },
    Delete {
        id: VirtualKeyId,
    },
}

impl fmt::Debug for ApplicationGatewayVirtualKeyMutation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Create { .. } => "Create(<redacted>)",
            Self::Update { .. } => "Update(<redacted>)",
            Self::RotateSecret { .. } => "RotateSecret(<redacted>)",
            Self::Delete { .. } => "Delete(<redacted>)",
        })
    }
}

pub struct ApplicationGatewayVirtualKeyMutationRequest<'a> {
    pub authorized_action: &'a ControlPlaneActionPlan,
    pub governance: &'a ApplicationControlPlaneGovernanceScope,
    pub current_records: &'a [ApplicationGatewayVirtualKeyRecord],
    pub mutation: ApplicationGatewayVirtualKeyMutation,
    pub now_unix_ms: u64,
}

impl fmt::Debug for ApplicationGatewayVirtualKeyMutationRequest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationGatewayVirtualKeyMutationRequest")
            .field("authorized_action", &"<redacted>")
            .field("governance", &self.governance)
            .field("current_records", &"<redacted>")
            .field("mutation", &self.mutation)
            .field("now_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationGatewayVirtualKeyMutationPlan {
    pub projection: ApplicationGatewayIdentityProjection<ApplicationGatewayVirtualKeyRecord>,
    pub secret_mutation: ApplicationSecretMutation,
    pub authorized_action: ControlPlaneActionPlan,
}

impl fmt::Debug for ApplicationGatewayVirtualKeyMutationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationGatewayVirtualKeyMutationPlan")
            .field("projection", &self.projection)
            .field("secret_mutation", &self.secret_mutation)
            .field("authorized_action", &"<redacted>")
            .finish()
    }
}

pub fn plan_application_gateway_virtual_key_mutation(
    request: ApplicationGatewayVirtualKeyMutationRequest<'_>,
) -> Result<ApplicationGatewayVirtualKeyMutationPlan, ApplicationGatewayIdentityMutationError> {
    let ApplicationGatewayVirtualKeyMutationRequest {
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
        ApplicationGatewayVirtualKeyMutation::Create {
            id,
            name,
            patch,
            secret_fingerprint,
        } => plan_create(
            authorized_action,
            governance,
            current_records,
            now_unix_ms,
            (id, name, patch, secret_fingerprint),
        ),
        ApplicationGatewayVirtualKeyMutation::Update { id, patch } => plan_update(
            authorized_action,
            governance,
            current_records,
            now_unix_ms,
            id,
            patch,
        ),
        ApplicationGatewayVirtualKeyMutation::RotateSecret {
            id,
            secret_fingerprint,
        } => plan_rotate(
            authorized_action,
            governance,
            current_records,
            now_unix_ms,
            id,
            secret_fingerprint,
        ),
        ApplicationGatewayVirtualKeyMutation::Delete { id } => plan_delete(
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
    current_records: &[ApplicationGatewayVirtualKeyRecord],
    now_unix_ms: u64,
    create: (
        VirtualKeyId,
        String,
        ApplicationGatewayVirtualKeyPatch,
        ApplicationSecretFingerprint,
    ),
) -> Result<ApplicationGatewayVirtualKeyMutationPlan, ApplicationGatewayIdentityMutationError> {
    let (id, name, patch, secret_fingerprint) = create;
    let authorized_action = validate_action(
        authorized_action,
        ControlPlaneOperation::VirtualKeyCreate,
        ResourceKind::VirtualKey,
        "virtual_key",
        &name,
        true,
    )?;
    let mut record = ApplicationGatewayVirtualKeyRecord {
        id,
        tenant_id: governance.tenant_id().map(str::to_string),
        name,
        source: ApplicationGatewayRecordSource::Editable,
        team_id: governance.team_id().map(str::to_string),
        project_id: governance.project_id().map(str::to_string),
        user_id: governance.user_id().map(str::to_string),
        budget_id: governance.budget_id().map(str::to_string),
        allowed_models: Vec::new(),
        budget_microusd: None,
        request_budget: None,
        rpm_limit: None,
        tpm_limit: None,
        disabled: false,
        created_at_unix_ms: now_unix_ms,
        updated_at_unix_ms: now_unix_ms,
    };
    apply_virtual_key_patch(&mut record, patch);
    validate_virtual_key_record(&record)?;
    if !governance_allows_virtual_key(governance, &record) {
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
        .any(|current| current.name.eq_ignore_ascii_case(&record.name))
    {
        return Err(ApplicationGatewayIdentityMutationError::DuplicateName);
    }
    Ok(ApplicationGatewayVirtualKeyMutationPlan {
        projection: ApplicationGatewayIdentityProjection::Insert { record },
        secret_mutation: ApplicationSecretMutation::Create(secret_fingerprint),
        authorized_action,
    })
}

fn plan_update(
    authorized_action: &ControlPlaneActionPlan,
    governance: &ApplicationControlPlaneGovernanceScope,
    current_records: &[ApplicationGatewayVirtualKeyRecord],
    now_unix_ms: u64,
    id: VirtualKeyId,
    patch: ApplicationGatewayVirtualKeyPatch,
) -> Result<ApplicationGatewayVirtualKeyMutationPlan, ApplicationGatewayIdentityMutationError> {
    let (index, current) = find_virtual_key(current_records, id)?;
    let authorized_action = validate_existing_action(
        authorized_action,
        ControlPlaneOperation::VirtualKeyUpdate,
        current,
    )?;
    validate_mutable_current(governance, now_unix_ms, current)?;
    let mut record = current.clone();
    apply_virtual_key_patch(&mut record, patch);
    record.updated_at_unix_ms = now_unix_ms;
    validate_virtual_key_record(&record)?;
    if !governance_allows_virtual_key(governance, &record) {
        return Err(ApplicationGatewayIdentityMutationError::GovernanceDenied);
    }
    Ok(ApplicationGatewayVirtualKeyMutationPlan {
        projection: ApplicationGatewayIdentityProjection::Replace { index, record },
        secret_mutation: ApplicationSecretMutation::None,
        authorized_action,
    })
}

fn plan_rotate(
    authorized_action: &ControlPlaneActionPlan,
    governance: &ApplicationControlPlaneGovernanceScope,
    current_records: &[ApplicationGatewayVirtualKeyRecord],
    now_unix_ms: u64,
    id: VirtualKeyId,
    secret_fingerprint: ApplicationSecretFingerprint,
) -> Result<ApplicationGatewayVirtualKeyMutationPlan, ApplicationGatewayIdentityMutationError> {
    let (index, current) = find_virtual_key(current_records, id)?;
    let authorized_action = validate_existing_action(
        authorized_action,
        ControlPlaneOperation::VirtualKeyRotateSecret,
        current,
    )?;
    validate_mutable_current(governance, now_unix_ms, current)?;
    let mut record = current.clone();
    record.updated_at_unix_ms = now_unix_ms;
    validate_virtual_key_record(&record)?;
    if !governance_allows_virtual_key(governance, &record) {
        return Err(ApplicationGatewayIdentityMutationError::GovernanceDenied);
    }
    Ok(ApplicationGatewayVirtualKeyMutationPlan {
        projection: ApplicationGatewayIdentityProjection::Replace { index, record },
        secret_mutation: ApplicationSecretMutation::Rotate(secret_fingerprint),
        authorized_action,
    })
}

fn plan_delete(
    authorized_action: &ControlPlaneActionPlan,
    governance: &ApplicationControlPlaneGovernanceScope,
    current_records: &[ApplicationGatewayVirtualKeyRecord],
    now_unix_ms: u64,
    id: VirtualKeyId,
) -> Result<ApplicationGatewayVirtualKeyMutationPlan, ApplicationGatewayIdentityMutationError> {
    let (index, current) = find_virtual_key(current_records, id)?;
    let authorized_action = validate_existing_action(
        authorized_action,
        ControlPlaneOperation::VirtualKeyDelete,
        current,
    )?;
    validate_mutable_current(governance, now_unix_ms, current)?;
    Ok(ApplicationGatewayVirtualKeyMutationPlan {
        projection: ApplicationGatewayIdentityProjection::Delete {
            index,
            record: current.clone(),
        },
        secret_mutation: ApplicationSecretMutation::None,
        authorized_action,
    })
}

fn validate_existing_action(
    action: &ControlPlaneActionPlan,
    operation: ControlPlaneOperation,
    record: &ApplicationGatewayVirtualKeyRecord,
) -> Result<ControlPlaneActionPlan, ApplicationGatewayIdentityMutationError> {
    validate_action(
        action,
        operation,
        ResourceKind::VirtualKey,
        "virtual_key",
        &record.name,
        false,
    )
}

fn validate_mutable_current(
    governance: &ApplicationControlPlaneGovernanceScope,
    now_unix_ms: u64,
    record: &ApplicationGatewayVirtualKeyRecord,
) -> Result<(), ApplicationGatewayIdentityMutationError> {
    validate_virtual_key_record(record)?;
    if now_unix_ms < record.updated_at_unix_ms {
        return Err(ApplicationGatewayIdentityMutationError::InvalidTimestamp);
    }
    if !governance_allows_virtual_key(governance, record) {
        return Err(ApplicationGatewayIdentityMutationError::GovernanceDenied);
    }
    if record.source != ApplicationGatewayRecordSource::Editable {
        return Err(ApplicationGatewayIdentityMutationError::ReadOnly);
    }
    Ok(())
}

fn find_virtual_key(
    records: &[ApplicationGatewayVirtualKeyRecord],
    id: VirtualKeyId,
) -> Result<(usize, &ApplicationGatewayVirtualKeyRecord), ApplicationGatewayIdentityMutationError> {
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

fn validate_virtual_key_record(
    record: &ApplicationGatewayVirtualKeyRecord,
) -> Result<(), ApplicationGatewayIdentityMutationError> {
    if !valid_key_name(&record.name) {
        return Err(ApplicationGatewayIdentityMutationError::InvalidKeyName);
    }
    if [
        &record.tenant_id,
        &record.team_id,
        &record.project_id,
        &record.user_id,
        &record.budget_id,
    ]
    .into_iter()
    .any(|value| !validate_scope_value(value))
    {
        return Err(ApplicationGatewayIdentityMutationError::InvalidScopeValue);
    }
    if record.allowed_models.len() > 128
        || record
            .allowed_models
            .iter()
            .any(|model| !valid_model(model))
    {
        return Err(ApplicationGatewayIdentityMutationError::InvalidModel);
    }
    for (index, model) in record.allowed_models.iter().enumerate() {
        if record.allowed_models[index + 1..]
            .iter()
            .any(|other| model.eq_ignore_ascii_case(other))
        {
            return Err(ApplicationGatewayIdentityMutationError::DuplicateModel);
        }
    }
    if [
        record.budget_microusd,
        record.request_budget,
        record.rpm_limit,
        record.tpm_limit,
    ]
    .into_iter()
    .flatten()
    .any(|value| value > i64::MAX as u64)
    {
        return Err(ApplicationGatewayIdentityMutationError::InvalidNumericValue);
    }
    if !timestamp_is_valid(record.created_at_unix_ms, record.updated_at_unix_ms) {
        return Err(ApplicationGatewayIdentityMutationError::InvalidTimestamp);
    }
    Ok(())
}

fn governance_allows_virtual_key(
    governance: &ApplicationControlPlaneGovernanceScope,
    record: &ApplicationGatewayVirtualKeyRecord,
) -> bool {
    governance.matches(
        record.tenant_id.as_deref(),
        record.team_id.as_deref(),
        record.project_id.as_deref(),
        record.user_id.as_deref(),
        record.budget_id.as_deref(),
    ) && governance.matches_resource_name(&record.name)
}

fn valid_key_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn valid_model(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && !value
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
}

fn apply_virtual_key_patch(
    record: &mut ApplicationGatewayVirtualKeyRecord,
    patch: ApplicationGatewayVirtualKeyPatch,
) {
    apply_optional(&mut record.tenant_id, patch.tenant_id);
    apply_optional(&mut record.team_id, patch.team_id);
    apply_optional(&mut record.project_id, patch.project_id);
    apply_optional(&mut record.user_id, patch.user_id);
    apply_optional(&mut record.budget_id, patch.budget_id);
    match patch.allowed_models {
        ApplicationPatchValue::Unchanged => {}
        ApplicationPatchValue::Set(models) => record.allowed_models = models,
        ApplicationPatchValue::Clear => record.allowed_models.clear(),
    }
    apply_optional(&mut record.budget_microusd, patch.budget_microusd);
    apply_optional(&mut record.request_budget, patch.request_budget);
    apply_optional(&mut record.rpm_limit, patch.rpm_limit);
    apply_optional(&mut record.tpm_limit, patch.tpm_limit);
    match patch.disabled {
        ApplicationPatchValue::Unchanged => {}
        ApplicationPatchValue::Set(disabled) => record.disabled = disabled,
        ApplicationPatchValue::Clear => record.disabled = false,
    }
}

fn apply_optional<T>(target: &mut Option<T>, patch: ApplicationPatchValue<T>) {
    match patch {
        ApplicationPatchValue::Unchanged => {}
        ApplicationPatchValue::Set(value) => *target = Some(value),
        ApplicationPatchValue::Clear => *target = None,
    }
}
