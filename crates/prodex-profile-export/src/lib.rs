use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

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

static PROFILE_EXPORT_BUNDLE_WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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

pub fn profile_import_auth_update_journal_root(prodex_root: impl AsRef<Path>) -> PathBuf {
    prodex_root.as_ref().join(IMPORT_AUTH_UPDATE_JOURNAL_DIR)
}

pub fn profile_import_auth_update_journal_paths(
    prodex_root: impl AsRef<Path>,
) -> Result<Vec<PathBuf>> {
    let journal_root = profile_import_auth_update_journal_root(prodex_root);
    let entries = match fs::read_dir(&journal_root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read {}", journal_root.display()));
        }
    };
    let mut journal_paths = entries
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .with_context(|| format!("failed to read entry in {}", journal_root.display()))
        })
        .collect::<Result<Vec<_>>>()?;
    journal_paths.retain(|path| path.is_file());
    journal_paths.sort();
    Ok(journal_paths)
}

pub fn ensure_profile_import_auth_update_journal_root(
    prodex_root: impl AsRef<Path>,
) -> Result<PathBuf> {
    let journal_root = profile_import_auth_update_journal_root(prodex_root);
    fs::create_dir_all(&journal_root)
        .with_context(|| format!("failed to create {}", journal_root.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o700);
        fs::set_permissions(&journal_root, permissions)
            .with_context(|| format!("failed to secure {}", journal_root.display()))?;
    }
    Ok(journal_root)
}

pub fn unique_profile_import_auth_update_journal_path(
    prodex_root: impl AsRef<Path>,
    profile_name: &str,
    token: &str,
) -> Result<PathBuf> {
    let journal_root = ensure_profile_import_auth_update_journal_root(prodex_root)?;
    Ok(journal_root.join(format!("{profile_name}-{token}.json")))
}

pub fn profile_import_staging_home(
    managed_profiles_root: impl AsRef<Path>,
    profile_name: &str,
    token: &str,
) -> PathBuf {
    managed_profiles_root
        .as_ref()
        .join(format!(".import-{profile_name}-{token}"))
}

pub fn cleanup_imported_auth_update_journals(commit: &ImportedProfilesCommit) {
    for update in &commit.auth_updates {
        let Some(journal_path) = update.journal_path.as_deref() else {
            continue;
        };
        let _ = fs::remove_file(journal_path);
        if let Some(parent) = journal_path.parent() {
            let _ = fs::remove_dir(parent);
        }
    }
}

pub fn remove_committed_import_homes(committed_homes: &[PathBuf]) {
    for home in committed_homes.iter().rev() {
        let _ = fs::remove_dir_all(home);
    }
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

pub fn read_profile_export_envelope<T>(path: &Path) -> Result<(ProfileExportEnvelope<T>, bool)>
where
    T: DeserializeOwned,
{
    let content = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let envelope: ProfileExportEnvelope<T> = serde_json::from_slice(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let encrypted = matches!(envelope, ProfileExportEnvelope::Encrypted { .. });
    Ok((envelope, encrypted))
}

pub fn write_profile_export_bundle(path: &Path, content: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let temp_path = unique_profile_export_temp_file_path(path);
    write_profile_export_temp_file(&temp_path, content)?;
    fs::rename(&temp_path, path)
        .with_context(|| format!("failed to replace {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o600);
        fs::set_permissions(path, permissions)
            .with_context(|| format!("failed to secure {}", path.display()))?;
    }
    Ok(())
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

fn unique_profile_export_temp_file_path(path: &Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = PROFILE_EXPORT_BUNDLE_WRITE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let file_name = format!(
        "{}.{}.{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("profile-export.json"),
        std::process::id(),
        nanos,
        sequence
    );
    path.with_file_name(file_name)
}

fn write_profile_export_temp_file(path: &Path, content: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options
        .open(path)
        .with_context(|| format!("failed to create {}", path.display()))?;
    file.write_all(content)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn yes_no(value: bool) -> String {
    if value {
        "Yes".to_string()
    } else {
        "No".to_string()
    }
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
