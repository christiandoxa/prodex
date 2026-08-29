use super::local_rewrite_gateway_admin_fields::runtime_gateway_validate_virtual_key_name;
pub(super) use super::local_rewrite_gateway_store_scim::{
    RuntimeGatewayScimUser, runtime_gateway_apply_scim_policy_attributes,
    runtime_gateway_principal_policy_attributes, runtime_gateway_scim_user_auth_entry_from_stored,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::str::FromStr;

#[derive(Clone)]
pub(super) struct RuntimeGatewayVirtualKeyEntry {
    pub(super) virtual_key_id: Option<prodex_domain::VirtualKeyId>,
    pub(super) key: runtime_proxy_crate::RuntimeGatewayVirtualKey,
    pub(super) source: RuntimeGatewayVirtualKeySource,
    pub(super) tenant_id: Option<String>,
    pub(super) group_ids: Vec<String>,
    pub(super) department_id: Option<String>,
    pub(super) created_at_epoch: Option<u64>,
    pub(super) updated_at_epoch: Option<u64>,
    pub(super) disabled: bool,
}

pub(super) fn runtime_gateway_virtual_key_effective_id(
    entry: &RuntimeGatewayVirtualKeyEntry,
) -> Option<prodex_domain::VirtualKeyId> {
    if let Some(virtual_key_id) = entry.virtual_key_id {
        return Some(virtual_key_id);
    }
    let tenant_id = entry
        .tenant_id
        .as_deref()
        .and_then(|value| value.parse::<prodex_domain::TenantId>().ok())?;
    let mut digest = Sha256::new();
    digest.update(b"prodex-policy-virtual-key-v1");
    digest.update(tenant_id.as_uuid().as_bytes());
    digest.update(entry.key.name.to_ascii_lowercase().as_bytes());
    let digest = digest.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Some(prodex_domain::VirtualKeyId::from_uuid(
        uuid::Uuid::from_bytes(bytes),
    ))
}

impl fmt::Debug for RuntimeGatewayVirtualKeyEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGatewayVirtualKeyEntry")
            .field("virtual_key_id", &redacted_option(&self.virtual_key_id))
            .field("key", &"<redacted>")
            .field("source", &self.source)
            .field("tenant_id", &redacted_option(&self.tenant_id))
            .field("group_ids", &"<redacted>")
            .field("department_id", &redacted_option(&self.department_id))
            .field("created_at_epoch", &redacted_option(&self.created_at_epoch))
            .field("updated_at_epoch", &redacted_option(&self.updated_at_epoch))
            .field("disabled", &self.disabled)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RuntimeGatewayVirtualKeySource {
    Policy,
    Admin,
}

impl RuntimeGatewayVirtualKeySource {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Policy => "policy",
            Self::Admin => "admin",
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub(super) struct RuntimeGatewayVirtualKeyStoreFile {
    #[serde(default = "runtime_gateway_virtual_key_store_version")]
    pub(super) version: u32,
    #[serde(default)]
    pub(super) keys: Vec<RuntimeGatewayStoredVirtualKey>,
    #[serde(default)]
    pub(super) scim_users: Vec<RuntimeGatewayScimUser>,
    #[serde(default)]
    pub(super) admin_idempotency: Vec<RuntimeGatewayAdminIdempotencyRecord>,
    #[serde(default)]
    pub(super) admin_audit: Vec<prodex_domain::AuditEnvelope>,
}

impl fmt::Debug for RuntimeGatewayVirtualKeyStoreFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGatewayVirtualKeyStoreFile")
            .field("version", &self.version)
            .field("keys", &redacted_len(self.keys.len()))
            .field("scim_users", &redacted_len(self.scim_users.len()))
            .field(
                "admin_idempotency",
                &redacted_len(self.admin_idempotency.len()),
            )
            .field("admin_audit", &redacted_len(self.admin_audit.len()))
            .finish()
    }
}

impl RuntimeGatewayVirtualKeyStoreFile {
    pub(super) fn canonicalize_for_active_state(&mut self) {
        self.scim_users = self
            .scim_users
            .iter()
            .filter_map(runtime_gateway_scim_user_auth_entry_from_stored)
            .collect();
    }

    pub(super) fn sort_for_rendering(&mut self) {
        self.sort_keys();
        self.scim_users
            .sort_by(|left, right| left.user_name.cmp(&right.user_name));
    }

    pub(super) fn sort_keys(&mut self) {
        self.keys.sort_by(|left, right| left.name.cmp(&right.name));
    }

    pub(super) fn bound_admin_history(&mut self) {
        const MAX_RECORDS: usize = 4_096;
        if self.admin_idempotency.len() > MAX_RECORDS {
            self.admin_idempotency
                .drain(..self.admin_idempotency.len() - MAX_RECORDS);
        }
        if self.admin_audit.len() > MAX_RECORDS {
            self.admin_audit
                .drain(..self.admin_audit.len() - MAX_RECORDS);
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct RuntimeGatewayAdminIdempotencyRecord {
    pub(super) entry: prodex_domain::IdempotencyEntry<()>,
    pub(super) completed_at_unix_ms: u64,
}

impl fmt::Debug for RuntimeGatewayAdminIdempotencyRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGatewayAdminIdempotencyRecord")
            .field("entry", &"<redacted>")
            .field("completed_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct RuntimeGatewayStoredVirtualKey {
    pub(super) name: String,
    pub(super) token_hash_base64: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "runtime_gateway_optional_string_no_null"
    )]
    pub(super) virtual_key_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "runtime_gateway_optional_string_no_null"
    )]
    pub(super) tenant_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "runtime_gateway_optional_string_no_null"
    )]
    pub(super) team_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "runtime_gateway_optional_string_no_null"
    )]
    pub(super) project_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "runtime_gateway_optional_string_no_null"
    )]
    pub(super) user_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "runtime_gateway_optional_string_no_null"
    )]
    pub(super) budget_id: Option<String>,
    #[serde(default)]
    pub(super) allowed_models: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "runtime_gateway_optional_budget_microusd_no_null"
    )]
    pub(super) budget_microusd: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "runtime_gateway_optional_request_budget_no_null"
    )]
    pub(super) request_budget: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "runtime_gateway_optional_rpm_limit_no_null"
    )]
    pub(super) rpm_limit: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "runtime_gateway_optional_tpm_limit_no_null"
    )]
    pub(super) tpm_limit: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "runtime_gateway_optional_bool_no_null"
    )]
    pub(super) disabled: Option<bool>,
    pub(super) created_at_epoch: u64,
    pub(super) updated_at_epoch: u64,
}

impl fmt::Debug for RuntimeGatewayStoredVirtualKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGatewayStoredVirtualKey")
            .field("name", &"<redacted>")
            .field("token_hash_base64", &"<redacted>")
            .field("virtual_key_id", &redacted_option(&self.virtual_key_id))
            .field("tenant_id", &redacted_option(&self.tenant_id))
            .field("team_id", &redacted_option(&self.team_id))
            .field("project_id", &redacted_option(&self.project_id))
            .field("user_id", &redacted_option(&self.user_id))
            .field("budget_id", &redacted_option(&self.budget_id))
            .field("allowed_models", &"<redacted>")
            .field("budget_microusd", &redacted_option(&self.budget_microusd))
            .field("request_budget", &redacted_option(&self.request_budget))
            .field("rpm_limit", &redacted_option(&self.rpm_limit))
            .field("tpm_limit", &redacted_option(&self.tpm_limit))
            .field("disabled", &self.disabled)
            .field("created_at_epoch", &"<redacted>")
            .field("updated_at_epoch", &"<redacted>")
            .finish()
    }
}

pub(super) fn redacted_option<T>(value: &Option<T>) -> Option<&'static str> {
    value.as_ref().map(|_| "<redacted>")
}

fn redacted_len(len: usize) -> String {
    format!("<redacted:{len}>")
}

fn runtime_gateway_optional_string_no_null<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<serde_json::Value>::deserialize(deserializer)? {
        Some(serde_json::Value::String(value)) => Ok(Some(value)),
        _ => Err(serde::de::Error::custom(
            "optional string field must be a string",
        )),
    }
}

fn runtime_gateway_optional_bool_no_null<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<serde_json::Value>::deserialize(deserializer)? {
        Some(serde_json::Value::Bool(value)) => Ok(Some(value)),
        _ => Err(serde::de::Error::custom("disabled must be a boolean")),
    }
}

fn runtime_gateway_optional_budget_microusd_no_null<'de, D>(
    deserializer: D,
) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    runtime_gateway_optional_u64_no_null(deserializer, "budget_microusd")
}

fn runtime_gateway_optional_request_budget_no_null<'de, D>(
    deserializer: D,
) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    runtime_gateway_optional_u64_no_null(deserializer, "request_budget")
}

fn runtime_gateway_optional_rpm_limit_no_null<'de, D>(
    deserializer: D,
) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    runtime_gateway_optional_u64_no_null(deserializer, "rpm_limit")
}

fn runtime_gateway_optional_tpm_limit_no_null<'de, D>(
    deserializer: D,
) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    runtime_gateway_optional_u64_no_null(deserializer, "tpm_limit")
}

fn runtime_gateway_optional_u64_no_null<'de, D>(
    deserializer: D,
    field: &'static str,
) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<serde_json::Value>::deserialize(deserializer)? {
        Some(serde_json::Value::Number(value)) => value.as_u64().map(Some).ok_or_else(|| {
            serde::de::Error::custom(format!("{field} must be an unsigned integer"))
        }),
        _ => Err(serde::de::Error::custom(format!(
            "{field} must be an unsigned integer"
        ))),
    }
}

pub(super) fn runtime_gateway_virtual_key_entry_from_stored(
    record: &RuntimeGatewayStoredVirtualKey,
) -> Option<RuntimeGatewayVirtualKeyEntry> {
    let token_hash = runtime_proxy_crate::LocalBridgeBearerTokenHash::from_hash_base64(
        &record.token_hash_base64,
    )?;
    let virtual_key_id = record
        .virtual_key_id
        .as_deref()
        .and_then(runtime_gateway_exact_virtual_key_id)?;
    runtime_gateway_validate_virtual_key_name(&record.name).ok()?;
    let tenant_id = runtime_gateway_exact_optional_stored_scope(&record.tenant_id)?;
    let team_id = runtime_gateway_exact_optional_stored_scope(&record.team_id)?;
    let project_id = runtime_gateway_exact_optional_stored_scope(&record.project_id)?;
    let user_id = runtime_gateway_exact_optional_stored_scope(&record.user_id)?;
    let budget_id = runtime_gateway_exact_optional_stored_scope(&record.budget_id)?;
    if !record
        .allowed_models
        .iter()
        .all(|model| runtime_gateway_exact_stored_string(model).is_some())
    {
        return None;
    }
    Some(RuntimeGatewayVirtualKeyEntry {
        virtual_key_id: Some(virtual_key_id),
        key: runtime_proxy_crate::RuntimeGatewayVirtualKey {
            name: record.name.clone(),
            tenant_id: tenant_id.clone(),
            team_id,
            project_id,
            user_id,
            budget_id,
            token_hash,
            allowed_models: record.allowed_models.clone(),
            budget_microusd: record.budget_microusd,
            request_budget: record.request_budget,
            rpm_limit: record.rpm_limit,
            tpm_limit: record.tpm_limit,
        },
        source: RuntimeGatewayVirtualKeySource::Admin,
        tenant_id,
        group_ids: Vec::new(),
        department_id: None,
        created_at_epoch: Some(record.created_at_epoch),
        updated_at_epoch: Some(record.updated_at_epoch),
        disabled: record.disabled.unwrap_or(false),
    })
}

fn runtime_gateway_exact_virtual_key_id(value: &str) -> Option<prodex_domain::VirtualKeyId> {
    let id = prodex_domain::VirtualKeyId::from_str(value).ok()?;
    (id.to_string() == value).then_some(id)
}

pub(super) fn runtime_gateway_exact_stored_string(value: &str) -> Option<&str> {
    (!value.is_empty() && !value.chars().any(char::is_whitespace)).then_some(value)
}

pub(super) fn runtime_gateway_exact_optional_stored_scope(
    value: &Option<String>,
) -> Option<Option<String>> {
    match value.as_deref() {
        None | Some("") => Some(None),
        Some(value) => {
            runtime_gateway_exact_stored_string(value).map(|value| Some(value.to_string()))
        }
    }
}

pub(super) fn runtime_gateway_virtual_key_store_version() -> u32 {
    3
}

#[cfg(test)]
#[path = "local_rewrite_gateway_store_types/tests.rs"]
mod tests;
