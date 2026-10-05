use std::{collections::BTreeMap, fmt};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::SummaryFields;

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotConfigFile {
    #[serde(default)]
    pub last_logged_in_user: Option<CopilotConfigUser>,
    #[serde(default)]
    pub logged_in_users: Vec<CopilotConfigUser>,
    #[serde(default)]
    pub copilot_tokens: BTreeMap<String, String>,
}

impl fmt::Debug for CopilotConfigFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CopilotConfigFile")
            .field("last_logged_in_user", &self.last_logged_in_user)
            .field("logged_in_user_count", &self.logged_in_users.len())
            .field("copilot_token_count", &self.copilot_tokens.len())
            .field("copilot_tokens", &"<redacted>")
            .finish()
    }
}

impl Zeroize for CopilotConfigFile {
    fn zeroize(&mut self) {
        for token in self.copilot_tokens.values_mut() {
            token.zeroize();
        }
        self.copilot_tokens.clear();
    }
}

impl Drop for CopilotConfigFile {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for CopilotConfigFile {}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CopilotConfigUser {
    pub host: String,
    pub login: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopilotUserInfo {
    #[serde(default)]
    pub login: Option<String>,
    #[serde(default)]
    pub access_type_sku: Option<String>,
    #[serde(default)]
    pub copilot_plan: Option<String>,
    #[serde(default)]
    pub endpoints: Option<CopilotUserEndpoints>,
    #[serde(default)]
    pub limited_user_quotas: BTreeMap<String, i64>,
    #[serde(default)]
    pub monthly_quotas: BTreeMap<String, i64>,
    #[serde(default)]
    pub limited_user_reset_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopilotUserEndpoints {
    #[serde(default)]
    pub api: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopilotProfileImportPlan {
    pub host: String,
    pub login: String,
    pub api_url: String,
    pub access_type_sku: Option<String>,
    pub copilot_plan: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopilotProfileImportStatePlan {
    UpdateExisting {
        profile_name: String,
        activate: bool,
    },
    AddNew {
        profile_name: String,
        activate: bool,
    },
}

impl CopilotProfileImportStatePlan {
    pub fn profile_name(&self) -> &str {
        match self {
            Self::UpdateExisting { profile_name, .. } | Self::AddNew { profile_name, .. } => {
                profile_name
            }
        }
    }

    pub fn activate(&self) -> bool {
        match self {
            Self::UpdateExisting { activate, .. } | Self::AddNew { activate, .. } => *activate,
        }
    }

    pub fn updated_existing(&self) -> bool {
        matches!(self, Self::UpdateExisting { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopilotProfileImportSummary {
    pub profile_name: String,
    pub provider: String,
    pub identity: String,
    pub github_host: String,
    pub api_url: Option<String>,
    pub codex_home: Option<String>,
    pub active: bool,
    pub updated_existing: bool,
}

pub fn copilot_profile_import_summary_fields(
    summary: CopilotProfileImportSummary,
) -> SummaryFields {
    let result = if summary.updated_existing {
        format!(
            "Updated imported Copilot profile '{}'.",
            summary.profile_name
        )
    } else {
        format!("Imported Copilot profile '{}'.", summary.profile_name)
    };
    let storage = if summary.updated_existing {
        "Token remains in Copilot's keychain/config store."
    } else {
        "Managed profile home created; token remains in Copilot's keychain/config store."
    };

    let mut fields = vec![
        ("Result".to_string(), result),
        ("Profile".to_string(), summary.profile_name.clone()),
        ("Provider".to_string(), summary.provider),
        ("Identity".to_string(), summary.identity),
        ("GitHub host".to_string(), summary.github_host),
    ];
    if let Some(codex_home) = summary.codex_home {
        fields.push(("CODEX_HOME".to_string(), codex_home));
    }
    if let Some(api_url) = summary.api_url {
        fields.push(("API".to_string(), api_url));
    }
    fields.push(("Storage".to_string(), storage.to_string()));
    if summary.active {
        fields.push(("Active".to_string(), summary.profile_name));
    }
    fields
}

pub fn parse_copilot_config_file(raw: &str) -> Result<CopilotConfigFile> {
    if raw.trim().is_empty() {
        return Ok(CopilotConfigFile::default());
    }

    match serde_json::from_str(raw) {
        Ok(config) => Ok(config),
        Err(original_error) => {
            let without_comments = Zeroizing::new(
                prodex_mojo_core::profile_export::strip_copilot_json_line_comments(raw).map_err(
                    |error| anyhow::anyhow!("Mojo Copilot JSONC normalization failed: {error:?}"),
                )?,
            );
            if without_comments.as_str() == raw || without_comments.trim().is_empty() {
                return Err(original_error).context("failed to parse Copilot config");
            }
            serde_json::from_str(without_comments.as_str())
                .context("failed to parse Copilot config")
        }
    }
}

pub fn select_copilot_logged_in_user(config: &CopilotConfigFile) -> Option<CopilotConfigUser> {
    config
        .last_logged_in_user
        .clone()
        .or_else(|| config.logged_in_users.first().cloned())
}

pub fn copilot_account_key(host: &str, login: &str) -> String {
    format!("{}:{}", host.trim(), login.trim())
}

pub fn copilot_token_from_config(
    config: &CopilotConfigFile,
    host: &str,
    login: &str,
) -> Option<String> {
    let account_key = copilot_account_key(host, login);
    config
        .copilot_tokens
        .get(&account_key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
}

pub fn parse_copilot_version(raw: &str) -> (u64, u64, u64) {
    prodex_mojo_core::profile_export::copilot_version_triplet(raw)
        .expect("Mojo Copilot version parser returned invalid output")
}

pub fn copilot_platform_label() -> &'static str {
    copilot_platform_label_for(std::env::consts::OS, std::env::consts::ARCH)
}

pub fn copilot_platform_label_for(os: &str, arch: &str) -> &'static str {
    prodex_mojo_core::profile_export::copilot_platform_label(os, arch)
        .expect("Mojo Copilot platform planner returned invalid output")
}

pub fn copilot_user_api_origin(host: &str) -> Result<String> {
    prodex_mojo_core::profile_export::copilot_user_api_origin(host)
        .map_err(|error| anyhow::anyhow!("Mojo Copilot user API origin failed: {error:?}"))?
        .ok_or_else(|| anyhow::anyhow!("invalid Copilot host '{}'", host))
}

pub fn default_copilot_models_api_url(host: &str) -> String {
    prodex_mojo_core::profile_export::copilot_models_api_url(host)
        .expect("Mojo Copilot models API URL planner returned invalid output")
}

pub fn parse_copilot_user_info_json_response(
    body: &[u8],
    source_label: &str,
) -> Result<serde_json::Value> {
    serde_json::from_slice(body).with_context(|| format!("failed to parse {source_label}"))
}

pub fn parse_copilot_user_info_value(
    value: serde_json::Value,
    source_label: &str,
) -> Result<CopilotUserInfo> {
    serde_json::from_value(value).with_context(|| format!("failed to parse {source_label}"))
}

pub fn parse_copilot_user_info_response(
    body: &[u8],
    source_label: &str,
) -> Result<CopilotUserInfo> {
    let value = parse_copilot_user_info_json_response(body, source_label)?;
    parse_copilot_user_info_value(value, source_label)
}

pub fn plan_copilot_profile_import(
    host: &str,
    config_login: &str,
    user_info: &CopilotUserInfo,
) -> CopilotProfileImportPlan {
    CopilotProfileImportPlan {
        host: host.to_string(),
        login: user_info
            .login
            .clone()
            .unwrap_or_else(|| config_login.to_string()),
        api_url: user_info
            .endpoints
            .as_ref()
            .and_then(|endpoints| endpoints.api.clone())
            .unwrap_or_else(|| default_copilot_models_api_url(host)),
        access_type_sku: user_info.access_type_sku.clone(),
        copilot_plan: user_info.copilot_plan.clone(),
    }
}

pub fn plan_copilot_profile_import_state(
    login: &str,
    requested_name: Option<&str>,
    existing_profile_name: Option<&str>,
    has_active_profile: bool,
    activate_requested: bool,
    mut profile_name_exists: impl FnMut(&str) -> bool,
    default_profile_name: impl FnOnce() -> String,
) -> Result<CopilotProfileImportStatePlan> {
    let activate = !has_active_profile || activate_requested;

    if let Some(existing_name) = existing_profile_name {
        if let Some(requested_name) = requested_name
            && requested_name != existing_name
        {
            bail!(
                "Copilot account '{}' is already imported as profile '{}'",
                login,
                existing_name
            );
        }

        return Ok(CopilotProfileImportStatePlan::UpdateExisting {
            profile_name: existing_name.to_string(),
            activate,
        });
    }

    let profile_name = match requested_name {
        Some(requested_name) => {
            if profile_name_exists(requested_name) {
                bail!("profile '{}' already exists", requested_name);
            }
            requested_name.to_string()
        }
        None => default_profile_name(),
    };

    Ok(CopilotProfileImportStatePlan::AddNew {
        profile_name,
        activate,
    })
}
