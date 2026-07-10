use super::local_rewrite_gateway_admin_fields::runtime_gateway_validate_virtual_key_name;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Clone)]
pub(super) struct RuntimeGatewayVirtualKeyEntry {
    pub(super) virtual_key_id: Option<prodex_domain::VirtualKeyId>,
    pub(super) key: runtime_proxy_crate::RuntimeGatewayVirtualKey,
    pub(super) source: RuntimeGatewayVirtualKeySource,
    pub(super) tenant_id: Option<String>,
    pub(super) created_at_epoch: Option<u64>,
    pub(super) updated_at_epoch: Option<u64>,
    pub(super) disabled: bool,
}

impl fmt::Debug for RuntimeGatewayVirtualKeyEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGatewayVirtualKeyEntry")
            .field("virtual_key_id", &redacted_option(&self.virtual_key_id))
            .field("key", &"<redacted>")
            .field("source", &self.source)
            .field("tenant_id", &redacted_option(&self.tenant_id))
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
}

impl fmt::Debug for RuntimeGatewayVirtualKeyStoreFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGatewayVirtualKeyStoreFile")
            .field("version", &self.version)
            .field("keys", &redacted_len(self.keys.len()))
            .field("scim_users", &redacted_len(self.scim_users.len()))
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
}

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
            .field("budget_id", &redacted_option(&self.budget_id))
            .field("allowed_key_prefixes", &"<redacted>")
            .field("created_at_epoch", &"<redacted>")
            .field("updated_at_epoch", &"<redacted>")
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

fn redacted_option<T>(value: &Option<T>) -> Option<&'static str> {
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
        created_at_epoch: Some(record.created_at_epoch),
        updated_at_epoch: Some(record.updated_at_epoch),
        disabled: record.disabled.unwrap_or(false),
    })
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
        budget_id,
        allowed_key_prefixes: user.allowed_key_prefixes.clone(),
        created_at_epoch: user.created_at_epoch,
        updated_at_epoch: user.updated_at_epoch,
    })
}

fn runtime_gateway_exact_virtual_key_id(value: &str) -> Option<prodex_domain::VirtualKeyId> {
    let id = prodex_domain::VirtualKeyId::from_str(value).ok()?;
    (id.to_string() == value).then_some(id)
}

fn runtime_gateway_exact_stored_string(value: &str) -> Option<&str> {
    (!value.is_empty() && !value.chars().any(char::is_whitespace)).then_some(value)
}

fn runtime_gateway_exact_optional_stored_scope(value: &Option<String>) -> Option<Option<String>> {
    match value.as_deref() {
        None | Some("") => Some(None),
        Some(value) => {
            runtime_gateway_exact_stored_string(value).map(|value| Some(value.to_string()))
        }
    }
}

pub(super) fn runtime_gateway_virtual_key_store_version() -> u32 {
    1
}

fn runtime_gateway_scim_user_active_default() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored_scim_user_for_tests() -> RuntimeGatewayScimUser {
        RuntimeGatewayScimUser {
            id: "user-1".to_string(),
            user_name: "alice@example.com".to_string(),
            external_id: None,
            display_name: None,
            active: true,
            role: Some("admin".to_string()),
            tenant_id: Some("tenant-a".to_string()),
            team_id: None,
            project_id: None,
            user_id: Some("user-1".to_string()),
            budget_id: None,
            allowed_key_prefixes: vec!["tenant-a-".to_string()],
            created_at_epoch: 1,
            updated_at_epoch: 2,
        }
    }

    #[test]
    fn stored_key_converts_to_admin_entry_with_exact_name() {
        let token_hash =
            runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("secret").hash_base64();
        let record = RuntimeGatewayStoredVirtualKey {
            name: "alpha".to_string(),
            token_hash_base64: token_hash,
            virtual_key_id: Some(prodex_domain::VirtualKeyId::new().to_string()),
            tenant_id: Some("tenant-a".to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_models: vec!["gpt-5".to_string()],
            budget_microusd: Some(1_000),
            request_budget: Some(10),
            rpm_limit: Some(5),
            tpm_limit: Some(500),
            disabled: Some(true),
            created_at_epoch: 1,
            updated_at_epoch: 2,
        };

        let entry = runtime_gateway_virtual_key_entry_from_stored(&record).unwrap();

        assert_eq!(entry.key.name, "alpha");
        assert!(entry.virtual_key_id.is_some());
        assert_eq!(entry.source, RuntimeGatewayVirtualKeySource::Admin);
        assert!(entry.disabled);
        assert_eq!(entry.created_at_epoch, Some(1));
        assert_eq!(entry.updated_at_epoch, Some(2));
    }

    #[test]
    fn stored_key_rejects_padded_name() {
        let token_hash =
            runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("secret").hash_base64();
        let record = RuntimeGatewayStoredVirtualKey {
            name: " alpha ".to_string(),
            token_hash_base64: token_hash,
            virtual_key_id: Some(prodex_domain::VirtualKeyId::new().to_string()),
            tenant_id: Some("tenant-a".to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_models: vec!["gpt-5".to_string()],
            budget_microusd: Some(1_000),
            request_budget: Some(10),
            rpm_limit: Some(5),
            tpm_limit: Some(500),
            disabled: Some(false),
            created_at_epoch: 1,
            updated_at_epoch: 2,
        };

        assert!(runtime_gateway_virtual_key_entry_from_stored(&record).is_none());
    }

    #[test]
    fn stored_key_rejects_padded_token_hash() {
        let token_hash =
            runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("secret").hash_base64();
        let record = RuntimeGatewayStoredVirtualKey {
            name: "alpha".to_string(),
            token_hash_base64: format!(" {token_hash} "),
            virtual_key_id: Some(prodex_domain::VirtualKeyId::new().to_string()),
            tenant_id: Some("tenant-a".to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_models: vec!["gpt-5".to_string()],
            budget_microusd: Some(1_000),
            request_budget: Some(10),
            rpm_limit: Some(5),
            tpm_limit: Some(500),
            disabled: Some(false),
            created_at_epoch: 1,
            updated_at_epoch: 2,
        };

        assert!(runtime_gateway_virtual_key_entry_from_stored(&record).is_none());
    }

    #[test]
    fn stored_key_rejects_padded_governance_scope() {
        let token_hash =
            runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("secret").hash_base64();
        let record = RuntimeGatewayStoredVirtualKey {
            name: "alpha".to_string(),
            token_hash_base64: token_hash,
            virtual_key_id: Some(prodex_domain::VirtualKeyId::new().to_string()),
            tenant_id: Some(" tenant-a ".to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_models: vec!["gpt-5".to_string()],
            budget_microusd: Some(1_000),
            request_budget: Some(10),
            rpm_limit: Some(5),
            tpm_limit: Some(500),
            disabled: Some(false),
            created_at_epoch: 1,
            updated_at_epoch: 2,
        };

        assert!(runtime_gateway_virtual_key_entry_from_stored(&record).is_none());
    }

    #[test]
    fn stored_key_rejects_padded_allowed_model_scope() {
        let token_hash =
            runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("secret").hash_base64();
        let record = RuntimeGatewayStoredVirtualKey {
            name: "alpha".to_string(),
            token_hash_base64: token_hash,
            virtual_key_id: Some(prodex_domain::VirtualKeyId::new().to_string()),
            tenant_id: Some("tenant-a".to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_models: vec![" gpt-5 ".to_string()],
            budget_microusd: Some(1_000),
            request_budget: Some(10),
            rpm_limit: Some(5),
            tpm_limit: Some(500),
            disabled: Some(false),
            created_at_epoch: 1,
            updated_at_epoch: 2,
        };

        assert!(runtime_gateway_virtual_key_entry_from_stored(&record).is_none());
    }

    #[test]
    fn stored_scim_user_auth_entry_rejects_padded_authz_fields() {
        let mut user = stored_scim_user_for_tests();
        user.tenant_id = Some(" tenant-a ".to_string());
        assert!(runtime_gateway_scim_user_auth_entry_from_stored(&user).is_none());

        let mut user = stored_scim_user_for_tests();
        user.allowed_key_prefixes = vec!["tenant-a- ".to_string()];
        assert!(runtime_gateway_scim_user_auth_entry_from_stored(&user).is_none());
    }

    #[test]
    fn stored_scim_user_auth_entry_normalizes_empty_optional_scope_absent() {
        let mut user = stored_scim_user_for_tests();
        user.tenant_id = Some(String::new());

        let auth_user = runtime_gateway_scim_user_auth_entry_from_stored(&user).unwrap();

        assert_eq!(auth_user.tenant_id, None);
    }

    #[test]
    fn stored_key_rejects_non_canonical_virtual_key_id() {
        let token_hash =
            runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("secret").hash_base64();
        let record = RuntimeGatewayStoredVirtualKey {
            name: "alpha".to_string(),
            token_hash_base64: token_hash,
            virtual_key_id: Some(" not-a-uuid ".to_string()),
            tenant_id: Some("tenant-a".to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_models: vec!["gpt-5".to_string()],
            budget_microusd: Some(1_000),
            request_budget: Some(10),
            rpm_limit: Some(5),
            tpm_limit: Some(500),
            disabled: Some(false),
            created_at_epoch: 1,
            updated_at_epoch: 2,
        };

        assert!(runtime_gateway_virtual_key_entry_from_stored(&record).is_none());
    }

    #[test]
    fn virtual_key_store_debug_output_redacts_sensitive_fields() {
        let id = prodex_domain::VirtualKeyId::new();
        let id_string = id.to_string();
        let stored = RuntimeGatewayStoredVirtualKey {
            name: "sk-store-secret".to_string(),
            token_hash_base64: "hash-store-secret".to_string(),
            virtual_key_id: Some(id_string.clone()),
            tenant_id: Some("tenant-store-secret".to_string()),
            team_id: Some("team-store-secret".to_string()),
            project_id: Some("project-store-secret".to_string()),
            user_id: Some("user-store-secret".to_string()),
            budget_id: Some("budget-store-secret".to_string()),
            allowed_models: vec!["model-store-secret".to_string()],
            budget_microusd: Some(10),
            request_budget: Some(20),
            rpm_limit: Some(30),
            tpm_limit: Some(40),
            disabled: Some(false),
            created_at_epoch: 50,
            updated_at_epoch: 60,
        };
        let scim_user = RuntimeGatewayScimUser {
            id: "scim-id-secret".to_string(),
            user_name: "scim-user-secret".to_string(),
            external_id: Some("external-secret".to_string()),
            display_name: Some("display-secret".to_string()),
            active: true,
            role: Some("admin".to_string()),
            tenant_id: Some("tenant-user-secret".to_string()),
            team_id: Some("team-user-secret".to_string()),
            project_id: Some("project-user-secret".to_string()),
            user_id: Some("user-user-secret".to_string()),
            budget_id: Some("budget-user-secret".to_string()),
            allowed_key_prefixes: vec!["prefix-secret".to_string()],
            created_at_epoch: 70,
            updated_at_epoch: 80,
        };
        let entry = RuntimeGatewayVirtualKeyEntry {
            virtual_key_id: Some(id),
            key: runtime_proxy_crate::RuntimeGatewayVirtualKey {
                name: "sk-entry-secret".to_string(),
                tenant_id: Some("tenant-entry-secret".to_string()),
                team_id: Some("team-entry-secret".to_string()),
                project_id: Some("project-entry-secret".to_string()),
                user_id: Some("user-entry-secret".to_string()),
                budget_id: Some("budget-entry-secret".to_string()),
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                    "entry-token-secret",
                ),
                allowed_models: vec!["model-entry-secret".to_string()],
                budget_microusd: Some(90),
                request_budget: Some(100),
                rpm_limit: Some(110),
                tpm_limit: Some(120),
            },
            source: RuntimeGatewayVirtualKeySource::Admin,
            tenant_id: Some("tenant-entry-secret".to_string()),
            created_at_epoch: Some(130),
            updated_at_epoch: Some(140),
            disabled: false,
        };
        let store = RuntimeGatewayVirtualKeyStoreFile {
            version: runtime_gateway_virtual_key_store_version(),
            keys: vec![stored.clone()],
            scim_users: vec![scim_user.clone()],
        };
        let rendered = format!("{stored:?}\n{scim_user:?}\n{entry:?}\n{store:?}");

        assert!(rendered.contains("RuntimeGatewayStoredVirtualKey"));
        assert!(rendered.contains("RuntimeGatewayScimUser"));
        assert!(rendered.contains("RuntimeGatewayVirtualKeyEntry"));
        assert!(rendered.contains("RuntimeGatewayVirtualKeyStoreFile"));
        assert!(rendered.contains("source: Admin"));
        assert!(rendered.contains("<redacted>"));
        for raw in [
            "sk-store-secret",
            "hash-store-secret",
            "tenant-store-secret",
            "team-store-secret",
            "project-store-secret",
            "user-store-secret",
            "budget-store-secret",
            "model-store-secret",
            "scim-id-secret",
            "scim-user-secret",
            "external-secret",
            "display-secret",
            "tenant-user-secret",
            "team-user-secret",
            "project-user-secret",
            "user-user-secret",
            "budget-user-secret",
            "prefix-secret",
            "sk-entry-secret",
            "tenant-entry-secret",
            "team-entry-secret",
            "project-entry-secret",
            "user-entry-secret",
            "budget-entry-secret",
            "entry-token-secret",
            "model-entry-secret",
            id_string.as_str(),
        ] {
            assert!(!rendered.contains(raw), "{rendered}");
        }
    }
}
