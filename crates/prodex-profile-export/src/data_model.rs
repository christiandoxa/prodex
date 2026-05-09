use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub(crate) const PROFILE_EXPORT_FORMAT: &str = "prodex_profile_export";
pub(crate) const PROFILE_EXPORT_VERSION: u32 = 1;
pub const PROFILE_EXPORT_CIPHER: &str = "aes_256_gcm_siv";
pub const PROFILE_EXPORT_KDF: &str = "pbkdf2_sha256";
pub(crate) const PROFILE_EXPORT_NONCE_BYTES: usize = 12;
pub(crate) const PROFILE_EXPORT_SALT_BYTES: usize = 16;
pub(crate) const PROFILE_EXPORT_KEY_BYTES: usize = 32;
pub(crate) const PROFILE_EXPORT_PBKDF2_ITERATIONS: u32 = if cfg!(test) { 1_000 } else { 600_000 };
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
        crate::profile_import_identity_parts_target_key(
            self.account_id.as_deref(),
            self.email.as_deref(),
        )
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
pub(crate) enum ImportIdentityTarget {
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
