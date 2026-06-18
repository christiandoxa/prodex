use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub(super) struct RuntimeGatewayVirtualKeyEntry {
    pub(super) key: runtime_proxy_crate::RuntimeGatewayVirtualKey,
    pub(super) source: RuntimeGatewayVirtualKeySource,
    pub(super) tenant_id: Option<String>,
    pub(super) created_at_epoch: Option<u64>,
    pub(super) updated_at_epoch: Option<u64>,
    pub(super) disabled: bool,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct RuntimeGatewayVirtualKeyStoreFile {
    #[serde(default = "runtime_gateway_virtual_key_store_version")]
    pub(super) version: u32,
    #[serde(default)]
    pub(super) keys: Vec<RuntimeGatewayStoredVirtualKey>,
    #[serde(default)]
    pub(super) scim_users: Vec<RuntimeGatewayScimUser>,
}

impl RuntimeGatewayVirtualKeyStoreFile {
    pub(super) fn sort_for_rendering(&mut self) {
        self.sort_keys();
        self.scim_users
            .sort_by(|left, right| left.user_name.cmp(&right.user_name));
    }

    pub(super) fn sort_keys(&mut self) {
        self.keys.sort_by(|left, right| left.name.cmp(&right.name));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct RuntimeGatewayStoredVirtualKey {
    pub(super) name: String,
    pub(super) token_hash_base64: String,
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
    pub(super) allowed_models: Vec<String>,
    #[serde(default)]
    pub(super) budget_microusd: Option<u64>,
    #[serde(default)]
    pub(super) request_budget: Option<u64>,
    #[serde(default)]
    pub(super) rpm_limit: Option<u64>,
    #[serde(default)]
    pub(super) tpm_limit: Option<u64>,
    #[serde(default)]
    pub(super) disabled: Option<bool>,
    pub(super) created_at_epoch: u64,
    pub(super) updated_at_epoch: u64,
}

pub(super) fn runtime_gateway_virtual_key_entry_from_stored(
    record: &RuntimeGatewayStoredVirtualKey,
) -> Option<RuntimeGatewayVirtualKeyEntry> {
    let token_hash = runtime_proxy_crate::LocalBridgeBearerTokenHash::from_hash_base64(
        &record.token_hash_base64,
    )?;
    Some(RuntimeGatewayVirtualKeyEntry {
        key: runtime_proxy_crate::RuntimeGatewayVirtualKey {
            name: record.name.trim().to_string(),
            tenant_id: record.tenant_id.clone(),
            team_id: record.team_id.clone(),
            project_id: record.project_id.clone(),
            user_id: record.user_id.clone(),
            budget_id: record.budget_id.clone(),
            token_hash,
            allowed_models: record.allowed_models.clone(),
            budget_microusd: record.budget_microusd,
            request_budget: record.request_budget,
            rpm_limit: record.rpm_limit,
            tpm_limit: record.tpm_limit,
        },
        source: RuntimeGatewayVirtualKeySource::Admin,
        tenant_id: record.tenant_id.clone(),
        created_at_epoch: Some(record.created_at_epoch),
        updated_at_epoch: Some(record.updated_at_epoch),
        disabled: record.disabled.unwrap_or(false),
    })
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

    #[test]
    fn stored_key_converts_to_admin_entry_with_trimmed_name() {
        let token_hash =
            runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("secret").hash_base64();
        let record = RuntimeGatewayStoredVirtualKey {
            name: " alpha ".to_string(),
            token_hash_base64: token_hash,
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
        assert_eq!(entry.source, RuntimeGatewayVirtualKeySource::Admin);
        assert!(entry.disabled);
        assert_eq!(entry.created_at_epoch, Some(1));
        assert_eq!(entry.updated_at_epoch, Some(2));
    }
}
