use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::SummaryFields;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotConfigFile {
    #[serde(default)]
    pub last_logged_in_user: Option<CopilotConfigUser>,
    #[serde(default)]
    pub logged_in_users: Vec<CopilotConfigUser>,
    #[serde(default)]
    pub copilot_tokens: BTreeMap<String, String>,
}

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
            let without_comments = strip_json_line_comments(raw);
            if without_comments == raw || without_comments.trim().is_empty() {
                return Err(original_error).context("failed to parse Copilot config");
            }
            serde_json::from_str(&without_comments).context("failed to parse Copilot config")
        }
    }
}

fn strip_json_line_comments(raw: &str) -> String {
    let mut output = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            output.push(ch);
            continue;
        }

        if ch == '/' && chars.peek() == Some(&'/') {
            let _ = chars.next();
            for comment_ch in chars.by_ref() {
                if comment_ch == '\n' {
                    output.push('\n');
                    break;
                }
            }
            continue;
        }

        output.push(ch);
    }

    output
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
    let mut parts = raw.split('.');
    let parse_part = |value: Option<&str>| {
        value
            .and_then(|part| {
                part.chars()
                    .take_while(|ch| ch.is_ascii_digit())
                    .collect::<String>()
                    .parse::<u64>()
                    .ok()
            })
            .unwrap_or(0)
    };
    (
        parse_part(parts.next()),
        parse_part(parts.next()),
        parse_part(parts.next()),
    )
}

pub fn copilot_platform_label() -> &'static str {
    copilot_platform_label_for(std::env::consts::OS, std::env::consts::ARCH)
}

pub fn copilot_platform_label_for(os: &str, arch: &str) -> &'static str {
    match (os, arch) {
        ("linux", "x86_64") => "linux-x64",
        ("linux", "aarch64") => "linux-arm64",
        ("macos", "x86_64") => "darwin-x64",
        ("macos", "aarch64") => "darwin-arm64",
        ("windows", "x86_64") => "win32-x64",
        ("windows", "aarch64") => "win32-arm64",
        _ => "linux-x64",
    }
}

pub fn copilot_user_api_origin(host: &str) -> Result<String> {
    let trimmed = host.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        bail!("invalid Copilot host '{}'", host);
    }

    let (scheme, rest) = if let Some((scheme, rest)) = trimmed.split_once("://") {
        if scheme.is_empty() || rest.is_empty() {
            bail!("invalid Copilot host '{}'", host);
        }
        (scheme, rest)
    } else {
        ("https", trimmed)
    };

    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .filter(|authority| !authority.is_empty())
        .with_context(|| format!("invalid Copilot host '{}'", host))?;
    let (hostname, has_explicit_port) = authority_host_and_port(authority)
        .with_context(|| format!("invalid Copilot host '{}'", host))?;
    let is_local = matches!(hostname, "localhost" | "127.0.0.1" | "::1");

    let authority = if has_explicit_port || is_local || hostname.starts_with("api.") {
        authority.to_string()
    } else {
        format!("api.{authority}")
    };

    Ok(format!("{scheme}://{authority}"))
}

pub fn default_copilot_models_api_url(host: &str) -> String {
    let normalized = host.trim().trim_end_matches('/');
    if normalized.eq_ignore_ascii_case("https://github.com")
        || normalized.eq_ignore_ascii_case("http://github.com")
        || normalized.eq_ignore_ascii_case("github.com")
    {
        return "https://api.githubcopilot.com".to_string();
    }

    let fallback_host = normalized
        .strip_prefix("https://")
        .or_else(|| normalized.strip_prefix("http://"))
        .unwrap_or(normalized);
    if let Some(subdomain) = fallback_host.strip_suffix(".ghe.com") {
        return format!("https://copilot-api.{subdomain}.ghe.com");
    }

    format!("https://api.{fallback_host}")
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

fn authority_host_and_port(authority: &str) -> Option<(&str, bool)> {
    if let Some(rest) = authority.strip_prefix('[') {
        let closing = rest.find(']')?;
        let hostname = &rest[..closing];
        let after = &rest[closing + 1..];
        return Some((hostname, after.starts_with(':')));
    }

    let mut parts = authority.rsplitn(2, ':');
    let last = parts.next()?;
    let maybe_host = parts.next();
    match maybe_host {
        Some(host) if !host.is_empty() && last.chars().all(|ch| ch.is_ascii_digit()) => {
            Some((host, true))
        }
        _ => Some((authority, false)),
    }
}
