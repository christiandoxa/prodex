use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use aes_gcm_siv::{
    Aes256GcmSiv, Nonce,
    aead::{Aead, KeyInit},
};
use anyhow::{Context, Result, bail};
use base64::Engine;
use pbkdf2::pbkdf2_hmac;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::Sha256;

const PROFILE_EXPORT_FORMAT: &str = "prodex_profile_export";
const PROFILE_EXPORT_VERSION: u32 = 1;
pub const PROFILE_EXPORT_CIPHER: &str = "aes_256_gcm_siv";
pub const PROFILE_EXPORT_KDF: &str = "pbkdf2_sha256";
const PROFILE_EXPORT_NONCE_BYTES: usize = 12;
const PROFILE_EXPORT_SALT_BYTES: usize = 16;
const PROFILE_EXPORT_KEY_BYTES: usize = 32;
const PROFILE_EXPORT_PBKDF2_ITERATIONS: u32 = if cfg!(test) { 1_000 } else { 600_000 };
pub const IMPORT_AUTH_UPDATE_JOURNAL_DIR: &str = "profile-import-auth-journal";
pub const IMPORT_AUTH_UPDATE_JOURNAL_VERSION: u32 = 1;

pub type SummaryFields = Vec<(String, String)>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileImportIdentity {
    pub email: Option<String>,
    pub account_id: Option<String>,
}

impl ProfileImportIdentity {
    pub fn target_key(&self) -> Option<String> {
        profile_import_identity_parts_target_key(self.account_id.as_deref(), self.email.as_deref())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(bound(
    serialize = "Provider: Serialize",
    deserialize = "Provider: Deserialize<'de> + Default"
))]
pub struct ProfileExportPayload<Provider> {
    pub exported_at: String,
    pub source_prodex_version: String,
    pub active_profile: Option<String>,
    pub profiles: Vec<ExportedProfile<Provider>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(bound(
    serialize = "Provider: Serialize",
    deserialize = "Provider: Deserialize<'de> + Default"
))]
pub struct ExportedProfile<Provider> {
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    pub source_managed: bool,
    #[serde(default)]
    pub provider: Provider,
    pub auth_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileImportAuthUpdatePlan {
    pub target_profile_name: String,
    pub email: Option<String>,
    pub auth_json: String,
}

pub trait ProfileImportPlanProfile {
    fn profile_name(&self) -> &str;
    fn supports_codex_runtime(&self) -> bool;
    fn import_identity(&self) -> ProfileImportIdentity;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileImportPlanInput {
    pub profile_name: String,
    pub supports_codex_runtime: bool,
    pub identity: ProfileImportIdentity,
}

impl ProfileImportPlanProfile for ProfileImportPlanInput {
    fn profile_name(&self) -> &str {
        &self.profile_name
    }

    fn supports_codex_runtime(&self) -> bool {
        self.supports_codex_runtime
    }

    fn import_identity(&self) -> ProfileImportIdentity {
        self.identity.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileImportPlan {
    pub actions: Vec<ProfileImportPlanAction>,
    pub resolved_profile_names: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileImportPlanAction {
    UpdateExisting {
        source_index: usize,
        target_profile_name: String,
    },
    StageNew {
        source_index: usize,
        staged_index: usize,
    },
    RewriteStagedAuth {
        source_index: usize,
        staged_index: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ImportIdentityTarget {
    Existing(String),
    PendingNew(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedImportedProfile<Provider> {
    pub name: String,
    pub email: Option<String>,
    pub staging_home: PathBuf,
    pub final_home: PathBuf,
    pub provider: Provider,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedImportedProfiles<Provider> {
    pub staged_profiles: Vec<StagedImportedProfile<Provider>>,
    pub auth_updates: Vec<ProfileImportAuthUpdatePlan>,
    pub resolved_profile_names: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedExistingProfileAuthUpdate {
    pub profile_name: String,
    pub codex_home: PathBuf,
    pub previous_auth_json: Option<String>,
    pub previous_email: Option<String>,
    pub journal_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedProfilesCommit {
    pub imported_names: Vec<String>,
    pub updated_existing_names: Vec<String>,
    pub committed_homes: Vec<PathBuf>,
    pub auth_updates: Vec<ImportedExistingProfileAuthUpdate>,
    pub previous_active_profile: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedProfilesTransaction {
    pub imported_names: Vec<String>,
    pub updated_existing_names: Vec<String>,
    pub committed_homes: Vec<PathBuf>,
    pub auth_updates: Vec<ImportedExistingProfileAuthUpdate>,
    pub previous_active_profile: Option<String>,
}

impl ImportedProfilesTransaction {
    pub fn new(
        previous_active_profile: Option<String>,
        staged_profile_count: usize,
        auth_update_count: usize,
    ) -> Self {
        Self {
            imported_names: Vec::with_capacity(staged_profile_count),
            updated_existing_names: Vec::with_capacity(auth_update_count),
            committed_homes: Vec::with_capacity(staged_profile_count),
            auth_updates: Vec::with_capacity(auth_update_count),
            previous_active_profile,
        }
    }

    pub fn record_existing_auth_update(&mut self, update: ImportedExistingProfileAuthUpdate) {
        self.updated_existing_names
            .push(update.profile_name.clone());
        self.auth_updates.push(update);
    }

    pub fn record_imported_profile(&mut self, name: String, final_home: PathBuf) {
        self.committed_homes.push(final_home);
        self.imported_names.push(name);
    }

    pub fn into_commit(self) -> ImportedProfilesCommit {
        ImportedProfilesCommit {
            imported_names: self.imported_names,
            updated_existing_names: self.updated_existing_names,
            committed_homes: self.committed_homes,
            auth_updates: self.auth_updates,
            previous_active_profile: self.previous_active_profile,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileExportSummary {
    pub profile_count: usize,
    pub path: String,
    pub encrypted: bool,
    pub active_profile: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileImportSummary {
    pub imported_count: usize,
    pub updated_existing_count: usize,
    pub path: String,
    pub encrypted: bool,
    pub source_active_profile: Option<String>,
    pub active_profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "payload_kind", rename_all = "snake_case")]
pub enum ProfileExportEnvelope<T> {
    Plain {
        format: String,
        version: u32,
        payload: T,
    },
    Encrypted {
        format: String,
        version: u32,
        cipher: String,
        kdf: String,
        iterations: u32,
        salt_base64: String,
        nonce_base64: String,
        ciphertext_base64: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedExistingProfileAuthUpdateJournal {
    pub version: u32,
    pub profile_name: String,
    pub codex_home: String,
    pub previous_email: Option<String>,
    pub previous_auth_json: Option<String>,
    pub created_at: String,
}

impl ImportedExistingProfileAuthUpdateJournal {
    pub fn new(
        profile_name: String,
        codex_home: String,
        previous_email: Option<String>,
        previous_auth_json: Option<String>,
        created_at: String,
    ) -> Self {
        Self {
            version: IMPORT_AUTH_UPDATE_JOURNAL_VERSION,
            profile_name,
            codex_home,
            previous_email,
            previous_auth_json,
            created_at,
        }
    }
}

pub fn resolve_requested_profile_names(
    available_names: &BTreeSet<String>,
    requested_names: &[String],
) -> Result<Vec<String>> {
    if available_names.is_empty() {
        bail!("no profiles configured");
    }

    if requested_names.is_empty() {
        return Ok(available_names.iter().cloned().collect());
    }

    let mut names = Vec::new();
    let mut seen = BTreeSet::new();
    for name in requested_names {
        if !seen.insert(name.clone()) {
            continue;
        }
        if !available_names.contains(name) {
            bail!("profile '{}' does not exist", name);
        }
        names.push(name.clone());
    }
    Ok(names)
}

pub fn profile_export_summary_fields(summary: ProfileExportSummary) -> SummaryFields {
    let mut fields = vec![
        (
            "Result".to_string(),
            format!("Exported {} profile(s).", summary.profile_count),
        ),
        ("Path".to_string(), summary.path),
        ("Encrypted".to_string(), yes_no(summary.encrypted)),
    ];
    if let Some(active_profile) = summary.active_profile {
        fields.push(("Active".to_string(), active_profile));
    }
    fields
}

pub fn profile_import_summary_fields(summary: ProfileImportSummary) -> SummaryFields {
    let mut fields = vec![
        (
            "Result".to_string(),
            profile_import_result_message(summary.imported_count, summary.updated_existing_count),
        ),
        ("Path".to_string(), summary.path),
        ("Encrypted".to_string(), yes_no(summary.encrypted)),
        ("Imported".to_string(), summary.imported_count.to_string()),
        (
            "Updated duplicates".to_string(),
            summary.updated_existing_count.to_string(),
        ),
    ];
    if let Some(active_profile) = summary.source_active_profile {
        fields.push(("Source active".to_string(), active_profile));
    }
    if let Some(active_profile) = summary.active_profile {
        fields.push(("Active".to_string(), active_profile));
    }
    fields
}

pub fn profile_import_result_message(
    imported_count: usize,
    updated_existing_count: usize,
) -> String {
    match (imported_count, updated_existing_count) {
        (0, updated) => format!("Updated {updated} existing profile(s)."),
        (imported, 0) => format!("Imported {imported} profile(s)."),
        (imported, updated) => {
            format!("Imported {imported} profile(s) and updated {updated} existing profile(s).")
        }
    }
}

pub fn resolve_profile_export_active_profile<'a>(
    active_profile: Option<&str>,
    selected_profile_names: impl IntoIterator<Item = &'a str>,
) -> Option<String> {
    let active_profile = active_profile?;
    selected_profile_names
        .into_iter()
        .any(|name| name == active_profile)
        .then(|| active_profile.to_string())
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

pub fn resolve_imported_active_profile(
    existing_active_profile: Option<&str>,
    source_active_profile: Option<&str>,
    resolved_profile_names: &BTreeMap<String, String>,
) -> Option<String> {
    existing_active_profile.map(ToOwned::to_owned).or_else(|| {
        source_active_profile.and_then(|active| resolved_profile_names.get(active).cloned())
    })
}

pub fn validate_import_auth_update_journal_version(version: u32) -> Result<()> {
    if version != IMPORT_AUTH_UPDATE_JOURNAL_VERSION {
        bail!("unsupported auth update journal version {}", version);
    }
    Ok(())
}

pub fn plan_profile_import<P>(
    profiles: &[P],
    existing_profile_supports_codex_runtime: impl Fn(&str) -> Option<bool>,
    mut find_existing_profile_by_identity: impl FnMut(&ProfileImportIdentity) -> Result<Option<String>>,
) -> Result<ProfileImportPlan>
where
    P: ProfileImportPlanProfile,
{
    if profiles.is_empty() {
        bail!("profile export bundle does not contain any profiles");
    }

    let mut seen_names = BTreeSet::new();
    let mut identity_targets = BTreeMap::new();
    let mut actions = Vec::with_capacity(profiles.len());
    let mut resolved_profile_names = BTreeMap::new();
    let mut staged_profile_names = Vec::new();
    let mut staged_count = 0_usize;

    for (source_index, profile) in profiles.iter().enumerate() {
        let source_profile_name = profile.profile_name();
        if !seen_names.insert(source_profile_name.to_string()) {
            bail!(
                "profile export bundle contains duplicate profile '{}'",
                source_profile_name
            );
        }

        let provider_supports_codex_runtime = profile.supports_codex_runtime();
        let resolved_identity = profile.import_identity();
        let identity_key = resolved_identity.target_key();

        if let Some(existing_supports_codex_runtime) =
            existing_profile_supports_codex_runtime(source_profile_name)
        {
            if !provider_supports_codex_runtime || !existing_supports_codex_runtime {
                bail!("profile '{}' already exists", source_profile_name);
            }

            actions.push(ProfileImportPlanAction::UpdateExisting {
                source_index,
                target_profile_name: source_profile_name.to_string(),
            });
            resolved_profile_names.insert(
                source_profile_name.to_string(),
                source_profile_name.to_string(),
            );
            if let Some(identity_key) = identity_key {
                identity_targets.insert(
                    identity_key,
                    ImportIdentityTarget::Existing(source_profile_name.to_string()),
                );
            }
            continue;
        }

        if provider_supports_codex_runtime {
            if let Some(identity_key) = identity_key.as_deref()
                && let Some(target) = identity_targets.get(identity_key)
            {
                match target {
                    ImportIdentityTarget::Existing(profile_name) => {
                        actions.push(ProfileImportPlanAction::UpdateExisting {
                            source_index,
                            target_profile_name: profile_name.clone(),
                        });
                        resolved_profile_names
                            .insert(source_profile_name.to_string(), profile_name.clone());
                        continue;
                    }
                    ImportIdentityTarget::PendingNew(staged_index) => {
                        actions.push(ProfileImportPlanAction::RewriteStagedAuth {
                            source_index,
                            staged_index: *staged_index,
                        });
                        let target_profile_name = staged_profile_names
                            .get(*staged_index)
                            .cloned()
                            .with_context(|| {
                                format!(
                                    "staged import profile index {} is missing for '{}'",
                                    staged_index, source_profile_name
                                )
                            })?;
                        resolved_profile_names
                            .insert(source_profile_name.to_string(), target_profile_name);
                        continue;
                    }
                }
            }

            if identity_key.is_some()
                && let Some(existing_profile_name) =
                    find_existing_profile_by_identity(&resolved_identity)?
            {
                if let Some(identity_key) = identity_key {
                    identity_targets.insert(
                        identity_key,
                        ImportIdentityTarget::Existing(existing_profile_name.clone()),
                    );
                }
                actions.push(ProfileImportPlanAction::UpdateExisting {
                    source_index,
                    target_profile_name: existing_profile_name.clone(),
                });
                resolved_profile_names
                    .insert(source_profile_name.to_string(), existing_profile_name);
                continue;
            }
        }

        let staged_index = staged_count;
        staged_count += 1;
        staged_profile_names.push(source_profile_name.to_string());
        actions.push(ProfileImportPlanAction::StageNew {
            source_index,
            staged_index,
        });
        resolved_profile_names.insert(
            source_profile_name.to_string(),
            source_profile_name.to_string(),
        );
        if provider_supports_codex_runtime && let Some(identity_key) = identity_key {
            identity_targets.insert(identity_key, ImportIdentityTarget::PendingNew(staged_index));
        }
    }

    Ok(ProfileImportPlan {
        actions,
        resolved_profile_names,
    })
}

pub fn profile_import_identity_target_key(identity: &ProfileImportIdentity) -> Option<String> {
    identity.target_key()
}

pub fn profile_import_identity_parts_target_key(
    account_id: Option<&str>,
    email: Option<&str>,
) -> Option<String> {
    account_id
        .map(str::trim)
        .filter(|account_id| !account_id.is_empty())
        .map(|account_id| format!("account:{account_id}"))
        .or_else(|| {
            email
                .map(str::trim)
                .filter(|email| !email.is_empty())
                .map(|email| format!("email:{}", email.to_ascii_lowercase()))
        })
}

pub fn validate_profile_import_source_names<'a>(
    profile_names: impl IntoIterator<Item = &'a str>,
) -> Result<()> {
    let mut seen_names = BTreeSet::new();
    for profile_name in profile_names {
        if !seen_names.insert(profile_name.to_string()) {
            bail!(
                "profile export bundle contains duplicate profile '{}'",
                profile_name
            );
        }
    }
    Ok(())
}

pub fn resolve_profile_import_identity(
    mut auth_identity: ProfileImportIdentity,
    fallback_email: Option<&str>,
) -> ProfileImportIdentity {
    if auth_identity.email.is_none() {
        auth_identity.email = fallback_email
            .map(str::trim)
            .filter(|email| !email.is_empty())
            .map(ToOwned::to_owned);
    }
    auth_identity
}

pub fn queue_profile_import_auth_update(
    auth_updates: &mut Vec<ProfileImportAuthUpdatePlan>,
    target_profile_name: &str,
    email: Option<String>,
    auth_json: String,
) {
    if let Some(existing) = auth_updates
        .iter_mut()
        .find(|update| update.target_profile_name == target_profile_name)
    {
        existing.auth_json = auth_json;
        if email.is_some() {
            existing.email = email;
        }
        return;
    }

    auth_updates.push(ProfileImportAuthUpdatePlan {
        target_profile_name: target_profile_name.to_string(),
        email,
        auth_json,
    });
}

pub fn parse_copilot_config_file(raw: &str) -> Result<CopilotConfigFile> {
    serde_json::from_str(raw).context("failed to parse Copilot config")
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

pub fn serialize_profile_export_payload<T>(payload: &T, password: Option<&str>) -> Result<Vec<u8>>
where
    T: Clone + Serialize,
{
    let envelope = match password {
        Some(password) => encrypt_profile_export_payload(payload, password)?,
        None => ProfileExportEnvelope::Plain {
            format: PROFILE_EXPORT_FORMAT.to_string(),
            version: PROFILE_EXPORT_VERSION,
            payload: payload.clone(),
        },
    };
    serde_json::to_vec_pretty(&envelope).context("failed to serialize profile export bundle")
}

pub fn decode_profile_export_envelope<T>(
    envelope: ProfileExportEnvelope<T>,
    resolve_password: impl FnOnce() -> Result<String>,
) -> Result<T>
where
    T: DeserializeOwned,
{
    match envelope {
        ProfileExportEnvelope::Plain {
            format,
            version,
            payload,
        } => {
            validate_profile_export_header(&format, version)?;
            Ok(payload)
        }
        ProfileExportEnvelope::Encrypted {
            format,
            version,
            cipher,
            kdf,
            iterations,
            salt_base64,
            nonce_base64,
            ciphertext_base64,
        } => {
            validate_profile_export_header(&format, version)?;
            if cipher != PROFILE_EXPORT_CIPHER {
                bail!("unsupported profile export cipher '{}'", cipher);
            }
            if kdf != PROFILE_EXPORT_KDF {
                bail!("unsupported profile export KDF '{}'", kdf);
            }
            let password = resolve_password()?;
            let salt = base64::engine::general_purpose::STANDARD
                .decode(salt_base64)
                .context("failed to decode encrypted export salt")?;
            let nonce = base64::engine::general_purpose::STANDARD
                .decode(nonce_base64)
                .context("failed to decode encrypted export nonce")?;
            let ciphertext = base64::engine::general_purpose::STANDARD
                .decode(ciphertext_base64)
                .context("failed to decode encrypted export payload")?;
            if nonce.len() != PROFILE_EXPORT_NONCE_BYTES {
                bail!("invalid encrypted export nonce length");
            }
            let key = derive_profile_export_key(&password, &salt, iterations);
            let cipher = Aes256GcmSiv::new_from_slice(&key)
                .map_err(|_| anyhow::anyhow!("failed to initialize import cipher"))?;
            let plaintext = cipher
                .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
                .map_err(|_| anyhow::anyhow!("failed to decrypt profile export bundle"))?;
            serde_json::from_slice(&plaintext)
                .context("failed to parse decrypted profile export payload")
        }
    }
}

pub fn derive_profile_export_key(
    password: &str,
    salt: &[u8],
    iterations: u32,
) -> [u8; PROFILE_EXPORT_KEY_BYTES] {
    let mut key = [0_u8; PROFILE_EXPORT_KEY_BYTES];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, iterations, &mut key);
    key
}

pub fn validate_profile_export_header(format: &str, version: u32) -> Result<()> {
    if format != PROFILE_EXPORT_FORMAT {
        bail!("unsupported profile export format '{}'", format);
    }
    if version != PROFILE_EXPORT_VERSION {
        bail!(
            "unsupported profile export version {} (expected {})",
            version,
            PROFILE_EXPORT_VERSION
        );
    }
    Ok(())
}

fn encrypt_profile_export_payload<T>(
    payload: &T,
    password: &str,
) -> Result<ProfileExportEnvelope<T>>
where
    T: Clone + Serialize,
{
    let payload_json =
        serde_json::to_vec(payload).context("failed to serialize profile export payload")?;
    let mut salt = [0_u8; PROFILE_EXPORT_SALT_BYTES];
    getrandom::fill(&mut salt)
        .map_err(|err| anyhow::anyhow!("failed to generate export salt: {err}"))?;
    let mut nonce = [0_u8; PROFILE_EXPORT_NONCE_BYTES];
    getrandom::fill(&mut nonce)
        .map_err(|err| anyhow::anyhow!("failed to generate export nonce: {err}"))?;
    let key = derive_profile_export_key(password, &salt, PROFILE_EXPORT_PBKDF2_ITERATIONS);
    let cipher = Aes256GcmSiv::new_from_slice(&key)
        .map_err(|_| anyhow::anyhow!("failed to initialize export cipher"))?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), payload_json.as_ref())
        .map_err(|_| anyhow::anyhow!("failed to encrypt profile export payload"))?;

    Ok(ProfileExportEnvelope::Encrypted {
        format: PROFILE_EXPORT_FORMAT.to_string(),
        version: PROFILE_EXPORT_VERSION,
        cipher: PROFILE_EXPORT_CIPHER.to_string(),
        kdf: PROFILE_EXPORT_KDF.to_string(),
        iterations: PROFILE_EXPORT_PBKDF2_ITERATIONS,
        salt_base64: base64::engine::general_purpose::STANDARD.encode(salt),
        nonce_base64: base64::engine::general_purpose::STANDARD.encode(nonce),
        ciphertext_base64: base64::engine::general_purpose::STANDARD.encode(ciphertext),
    })
}

fn yes_no(value: bool) -> String {
    if value {
        "Yes".to_string()
    } else {
        "No".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct PlanProfile {
        name: &'static str,
        supports_codex_runtime: bool,
        email: Option<&'static str>,
        account_id: Option<&'static str>,
    }

    impl ProfileImportPlanProfile for PlanProfile {
        fn profile_name(&self) -> &str {
            self.name
        }

        fn supports_codex_runtime(&self) -> bool {
            self.supports_codex_runtime
        }

        fn import_identity(&self) -> ProfileImportIdentity {
            ProfileImportIdentity {
                email: self.email.map(ToOwned::to_owned),
                account_id: self.account_id.map(ToOwned::to_owned),
            }
        }
    }

    fn available(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }

    #[test]
    fn requested_profile_names_default_to_available_names() {
        let names = resolve_requested_profile_names(&available(&["second", "main"]), &[])
            .expect("profile names should resolve");

        assert_eq!(names, vec!["main".to_string(), "second".to_string()]);
    }

    #[test]
    fn requested_profile_names_deduplicate_and_preserve_request_order() {
        let requested = vec![
            "second".to_string(),
            "main".to_string(),
            "second".to_string(),
        ];
        let names = resolve_requested_profile_names(&available(&["main", "second"]), &requested)
            .expect("profile names should resolve");

        assert_eq!(names, vec!["second".to_string(), "main".to_string()]);
    }

    #[test]
    fn requested_profile_names_reject_missing_profiles() {
        let requested = vec!["missing".to_string()];
        let err = resolve_requested_profile_names(&available(&["main"]), &requested)
            .expect_err("missing profile should fail");

        assert_eq!(err.to_string(), "profile 'missing' does not exist");
    }

    #[test]
    fn export_summary_fields_match_cli_panel_fields() {
        let fields = profile_export_summary_fields(ProfileExportSummary {
            profile_count: 2,
            path: "/tmp/profiles.json".to_string(),
            encrypted: true,
            active_profile: Some("main".to_string()),
        });

        assert_eq!(
            fields,
            vec![
                ("Result".to_string(), "Exported 2 profile(s).".to_string()),
                ("Path".to_string(), "/tmp/profiles.json".to_string()),
                ("Encrypted".to_string(), "Yes".to_string()),
                ("Active".to_string(), "main".to_string()),
            ]
        );
    }

    #[test]
    fn import_summary_fields_match_cli_panel_fields() {
        let fields = profile_import_summary_fields(ProfileImportSummary {
            imported_count: 2,
            updated_existing_count: 1,
            path: "/tmp/profiles.json".to_string(),
            encrypted: false,
            source_active_profile: Some("source".to_string()),
            active_profile: Some("target".to_string()),
        });

        assert_eq!(
            fields,
            vec![
                (
                    "Result".to_string(),
                    "Imported 2 profile(s) and updated 1 existing profile(s).".to_string(),
                ),
                ("Path".to_string(), "/tmp/profiles.json".to_string()),
                ("Encrypted".to_string(), "No".to_string()),
                ("Imported".to_string(), "2".to_string()),
                ("Updated duplicates".to_string(), "1".to_string()),
                ("Source active".to_string(), "source".to_string()),
                ("Active".to_string(), "target".to_string()),
            ]
        );
    }

    #[test]
    fn imported_active_profile_uses_existing_active_profile_first() {
        let resolved = BTreeMap::from([("source".to_string(), "target".to_string())]);

        assert_eq!(
            resolve_imported_active_profile(Some("current"), Some("source"), &resolved),
            Some("current".to_string())
        );
        assert_eq!(
            resolve_imported_active_profile(None, Some("source"), &resolved),
            Some("target".to_string())
        );
        assert_eq!(
            resolve_imported_active_profile(None, Some("missing"), &resolved),
            None
        );
    }

    #[test]
    fn export_active_profile_only_survives_when_selected() {
        assert_eq!(
            resolve_profile_export_active_profile(Some("main"), ["main", "second"]),
            Some("main".to_string())
        );
        assert_eq!(
            resolve_profile_export_active_profile(Some("other"), ["main", "second"]),
            None
        );
        assert_eq!(
            resolve_profile_export_active_profile(None, ["main", "second"]),
            None
        );
    }

    #[test]
    fn import_auth_update_journal_constructor_sets_current_version() {
        let journal = ImportedExistingProfileAuthUpdateJournal::new(
            "main".to_string(),
            "/tmp/main".to_string(),
            Some("old@example.com".to_string()),
            Some("{}".to_string()),
            "2026-05-02T00:00:00+00:00".to_string(),
        );

        assert_eq!(journal.version, IMPORT_AUTH_UPDATE_JOURNAL_VERSION);
        validate_import_auth_update_journal_version(journal.version)
            .expect("current journal version should validate");
        assert!(validate_import_auth_update_journal_version(journal.version + 1).is_err());
    }

    #[test]
    fn import_transaction_records_commit_order_and_previous_active() {
        let mut transaction = ImportedProfilesTransaction::new(Some("previous".to_string()), 2, 1);

        transaction.record_imported_profile("main".to_string(), PathBuf::from("/tmp/main"));
        transaction.record_existing_auth_update(ImportedExistingProfileAuthUpdate {
            profile_name: "existing".to_string(),
            codex_home: PathBuf::from("/tmp/existing"),
            previous_auth_json: Some("old-auth".to_string()),
            previous_email: Some("old@example.com".to_string()),
            journal_path: Some(PathBuf::from("/tmp/journal.json")),
        });

        let commit = transaction.into_commit();

        assert_eq!(commit.imported_names, vec!["main".to_string()]);
        assert_eq!(commit.updated_existing_names, vec!["existing".to_string()]);
        assert_eq!(commit.committed_homes, vec![PathBuf::from("/tmp/main")]);
        assert_eq!(commit.previous_active_profile.as_deref(), Some("previous"));
        assert_eq!(commit.auth_updates[0].profile_name, "existing");
    }

    #[test]
    fn import_plan_updates_same_name_runtime_profile() {
        let profiles = [PlanProfile {
            name: "main",
            supports_codex_runtime: true,
            email: Some("user@example.com"),
            account_id: Some("acct-main"),
        }];

        let plan = plan_profile_import(
            &profiles,
            |name| (name == "main").then_some(true),
            |_| Ok(None),
        )
        .expect("plan should resolve");

        assert_eq!(
            plan.actions,
            vec![ProfileImportPlanAction::UpdateExisting {
                source_index: 0,
                target_profile_name: "main".to_string(),
            }]
        );
        assert_eq!(
            plan.resolved_profile_names,
            BTreeMap::from([("main".to_string(), "main".to_string())])
        );
    }

    #[test]
    fn import_plan_rewrites_pending_new_profile_for_duplicate_identity() {
        let profiles = [
            PlanProfile {
                name: "first",
                supports_codex_runtime: true,
                email: Some("user@example.com"),
                account_id: Some("acct-main"),
            },
            PlanProfile {
                name: "second",
                supports_codex_runtime: true,
                email: Some("other@example.com"),
                account_id: Some("acct-main"),
            },
        ];

        let plan =
            plan_profile_import(&profiles, |_| None, |_| Ok(None)).expect("plan should resolve");

        assert_eq!(
            plan.actions,
            vec![
                ProfileImportPlanAction::StageNew {
                    source_index: 0,
                    staged_index: 0,
                },
                ProfileImportPlanAction::RewriteStagedAuth {
                    source_index: 1,
                    staged_index: 0,
                },
            ]
        );
        assert_eq!(
            plan.resolved_profile_names,
            BTreeMap::from([
                ("first".to_string(), "first".to_string()),
                ("second".to_string(), "first".to_string()),
            ])
        );
    }

    #[test]
    fn import_plan_uses_external_identity_match() {
        let profiles = [PlanProfile {
            name: "incoming",
            supports_codex_runtime: true,
            email: Some("user@example.com"),
            account_id: Some("acct-main"),
        }];

        let plan = plan_profile_import(&profiles, |_| None, |_| Ok(Some("existing".to_string())))
            .expect("plan should resolve");

        assert_eq!(
            plan.actions,
            vec![ProfileImportPlanAction::UpdateExisting {
                source_index: 0,
                target_profile_name: "existing".to_string(),
            }]
        );
        assert_eq!(
            plan.resolved_profile_names,
            BTreeMap::from([("incoming".to_string(), "existing".to_string())])
        );
    }

    #[test]
    fn import_source_name_validation_rejects_duplicates() {
        let err = validate_profile_import_source_names(["main", "backup", "main"])
            .expect_err("duplicate profile names should fail");

        assert_eq!(
            err.to_string(),
            "profile export bundle contains duplicate profile 'main'"
        );
    }

    #[test]
    fn import_identity_falls_back_to_exported_email() {
        assert_eq!(
            resolve_profile_import_identity(
                ProfileImportIdentity {
                    email: None,
                    account_id: Some("acct-main".to_string()),
                },
                Some(" User@Example.com "),
            ),
            ProfileImportIdentity {
                email: Some("User@Example.com".to_string()),
                account_id: Some("acct-main".to_string()),
            }
        );
        assert_eq!(
            resolve_profile_import_identity(
                ProfileImportIdentity {
                    email: Some("auth@example.com".to_string()),
                    account_id: None,
                },
                Some("fallback@example.com"),
            )
            .email
            .as_deref(),
            Some("auth@example.com")
        );
    }

    #[test]
    fn queued_auth_update_keeps_last_token_and_non_empty_email() {
        let mut updates = Vec::new();

        queue_profile_import_auth_update(
            &mut updates,
            "main",
            Some("first@example.com".to_string()),
            "first-auth".to_string(),
        );
        queue_profile_import_auth_update(&mut updates, "main", None, "second-auth".to_string());

        assert_eq!(
            updates,
            vec![ProfileImportAuthUpdatePlan {
                target_profile_name: "main".to_string(),
                email: Some("first@example.com".to_string()),
                auth_json: "second-auth".to_string(),
            }]
        );
    }

    #[test]
    fn copilot_config_helpers_select_user_and_token() {
        let config = parse_copilot_config_file(
            r#"{
                "loggedInUsers": [{"host": "https://github.com", "login": "fallback"}],
                "lastLoggedInUser": {"host": "https://ghe.example", "login": "main"},
                "copilotTokens": {
                    "https://ghe.example:main": " token-value "
                }
            }"#,
        )
        .expect("config should parse");

        let user = select_copilot_logged_in_user(&config).expect("user should resolve");

        assert_eq!(user.host, "https://ghe.example");
        assert_eq!(user.login, "main");
        assert_eq!(
            copilot_token_from_config(&config, "https://ghe.example", "main").as_deref(),
            Some("token-value")
        );
        assert_eq!(
            copilot_account_key(" https://ghe.example ", " main "),
            "https://ghe.example:main"
        );
    }

    #[test]
    fn copilot_url_helpers_match_import_expectations() {
        assert_eq!(parse_copilot_version("1.2.3-beta"), (1, 2, 3));
        assert_eq!(copilot_platform_label_for("linux", "x86_64"), "linux-x64");
        assert_eq!(
            copilot_user_api_origin("github.com").unwrap(),
            "https://api.github.com"
        );
        assert_eq!(
            copilot_user_api_origin("http://127.0.0.1:1234/path").unwrap(),
            "http://127.0.0.1:1234"
        );
        assert_eq!(
            default_copilot_models_api_url("github.com"),
            "https://api.githubcopilot.com"
        );
        assert_eq!(
            default_copilot_models_api_url("https://enterprise.ghe.com"),
            "https://copilot-api.enterprise.ghe.com"
        );
    }

    #[test]
    fn copilot_user_info_response_parses_metadata() {
        let info = parse_copilot_user_info_response(
            br#"{
                "login": "copilot-user",
                "access_type_sku": "copilot_standalone_seat_quota",
                "copilot_plan": "business",
                "endpoints": { "api": "https://api.example.githubcopilot.test" }
            }"#,
            "https://api.github.com/copilot_internal/user",
        )
        .expect("Copilot response should parse");

        assert_eq!(info.login.as_deref(), Some("copilot-user"));
        assert_eq!(
            info.access_type_sku.as_deref(),
            Some("copilot_standalone_seat_quota")
        );
        assert_eq!(
            info.endpoints
                .and_then(|endpoints| endpoints.api)
                .as_deref(),
            Some("https://api.example.githubcopilot.test")
        );
    }

    #[test]
    fn copilot_import_plan_uses_user_info_with_api_fallback() {
        let plan = plan_copilot_profile_import(
            "https://github.example.test",
            "config-login",
            &CopilotUserInfo {
                login: Some("api-login".to_string()),
                access_type_sku: Some("sku".to_string()),
                copilot_plan: Some("business".to_string()),
                endpoints: Some(CopilotUserEndpoints {
                    api: Some("https://api.github.example.test".to_string()),
                }),
                limited_user_quotas: BTreeMap::new(),
                monthly_quotas: BTreeMap::new(),
                limited_user_reset_date: None,
            },
        );

        assert_eq!(
            plan,
            CopilotProfileImportPlan {
                host: "https://github.example.test".to_string(),
                login: "api-login".to_string(),
                api_url: "https://api.github.example.test".to_string(),
                access_type_sku: Some("sku".to_string()),
                copilot_plan: Some("business".to_string()),
            }
        );

        let fallback_plan = plan_copilot_profile_import(
            "https://github.enterprise.test",
            "config-login",
            &CopilotUserInfo {
                login: None,
                access_type_sku: None,
                copilot_plan: None,
                endpoints: None,
                limited_user_quotas: BTreeMap::new(),
                monthly_quotas: BTreeMap::new(),
                limited_user_reset_date: None,
            },
        );

        assert_eq!(fallback_plan.login, "config-login");
        assert_eq!(fallback_plan.api_url, "https://api.github.enterprise.test");
    }

    #[test]
    fn copilot_state_plan_updates_existing_or_adds_new_profile() {
        assert_eq!(
            plan_copilot_profile_import_state(
                "octo",
                Some("copilot-main"),
                Some("copilot-main"),
                true,
                false,
                |_| false,
                || "unused".to_string(),
            )
            .unwrap(),
            CopilotProfileImportStatePlan::UpdateExisting {
                profile_name: "copilot-main".to_string(),
                activate: false,
            }
        );

        assert_eq!(
            plan_copilot_profile_import_state(
                "octo",
                None,
                None,
                false,
                false,
                |_| false,
                || "copilot-octo".to_string(),
            )
            .unwrap(),
            CopilotProfileImportStatePlan::AddNew {
                profile_name: "copilot-octo".to_string(),
                activate: true,
            }
        );

        assert_eq!(
            plan_copilot_profile_import_state(
                "octo",
                Some("other"),
                Some("copilot-main"),
                true,
                true,
                |_| false,
                || "unused".to_string(),
            )
            .unwrap_err()
            .to_string(),
            "Copilot account 'octo' is already imported as profile 'copilot-main'"
        );
        assert_eq!(
            plan_copilot_profile_import_state(
                "octo",
                Some("taken"),
                None,
                true,
                false,
                |name| name == "taken",
                || "unused".to_string(),
            )
            .unwrap_err()
            .to_string(),
            "profile 'taken' already exists"
        );
    }

    #[test]
    fn copilot_import_summary_fields_render_new_and_updated_profiles() {
        assert_eq!(
            copilot_profile_import_summary_fields(CopilotProfileImportSummary {
                profile_name: "copilot-main".to_string(),
                provider: "GitHub Copilot".to_string(),
                identity: "octo".to_string(),
                github_host: "https://github.com".to_string(),
                api_url: Some("https://api.githubcopilot.com".to_string()),
                codex_home: Some("/tmp/prodex/copilot-main".to_string()),
                active: true,
                updated_existing: false,
            }),
            vec![
                (
                    "Result".to_string(),
                    "Imported Copilot profile 'copilot-main'.".to_string()
                ),
                ("Profile".to_string(), "copilot-main".to_string()),
                ("Provider".to_string(), "GitHub Copilot".to_string()),
                ("Identity".to_string(), "octo".to_string()),
                ("GitHub host".to_string(), "https://github.com".to_string()),
                (
                    "CODEX_HOME".to_string(),
                    "/tmp/prodex/copilot-main".to_string()
                ),
                ("API".to_string(), "https://api.githubcopilot.com".to_string()),
                (
                    "Storage".to_string(),
                    "Managed profile home created; token remains in Copilot's keychain/config store."
                        .to_string()
                ),
                ("Active".to_string(), "copilot-main".to_string()),
            ]
        );

        assert_eq!(
            copilot_profile_import_summary_fields(CopilotProfileImportSummary {
                profile_name: "copilot-main".to_string(),
                provider: "GitHub Copilot".to_string(),
                identity: "octo".to_string(),
                github_host: "https://github.com".to_string(),
                api_url: None,
                codex_home: None,
                active: false,
                updated_existing: true,
            })
            .first()
            .map(|(_, value)| value.as_str()),
            Some("Updated imported Copilot profile 'copilot-main'.")
        );
    }
}
