use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;
use std::path::PathBuf;

use serde::de::{SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub(crate) const PROFILE_EXPORT_FORMAT: &str = "prodex_profile_export";
pub const PROFILE_EXPORT_VERSION_V1: u32 = 1;
pub const PROFILE_EXPORT_VERSION_V2: u32 = 2;
pub const PROFILE_EXPORT_CIPHER: &str = "aes_256_gcm_siv";
pub const PROFILE_EXPORT_KDF: &str = "pbkdf2_sha256";
pub const PROFILE_EXPORT_ARGON2ID_KDF: &str = "argon2id";
pub(crate) const PROFILE_EXPORT_NONCE_BYTES: usize = 12;
pub(crate) const PROFILE_EXPORT_SALT_BYTES: usize = 16;
pub(crate) const PROFILE_EXPORT_KEY_BYTES: usize = 32;
pub const PROFILE_EXPORT_PBKDF2_ITERATIONS: u32 = 100_000;
pub const PROFILE_EXPORT_PBKDF2_MIN_ITERATIONS: u32 = 50_000;
pub const PROFILE_EXPORT_PBKDF2_MAX_ITERATIONS: u32 = 2_000_000;
pub const PROFILE_EXPORT_ARGON2_VERSION: u32 = 0x13;
pub const PROFILE_EXPORT_ARGON2_MEMORY_KIB: u32 = 64 * 1024;
pub const PROFILE_EXPORT_ARGON2_MIN_MEMORY_KIB: u32 = 8 * 1024;
pub const PROFILE_EXPORT_ARGON2_MAX_MEMORY_KIB: u32 = 128 * 1024;
pub const PROFILE_EXPORT_ARGON2_ITERATIONS: u32 = 3;
pub const PROFILE_EXPORT_ARGON2_MIN_ITERATIONS: u32 = 1;
pub const PROFILE_EXPORT_ARGON2_MAX_ITERATIONS: u32 = 6;
pub const PROFILE_EXPORT_ARGON2_PARALLELISM: u32 = 1;
pub const PROFILE_EXPORT_ARGON2_MIN_PARALLELISM: u32 = 1;
pub const PROFILE_EXPORT_ARGON2_MAX_PARALLELISM: u32 = 4;
pub const PROFILE_EXPORT_PASSWORD_MAX_BYTES: usize = 4 * 1024;
pub const PROFILE_EXPORT_PLAINTEXT_MAX_BYTES: usize = 32 * 1024 * 1024;
pub const PROFILE_EXPORT_NESTED_JSON_MAX_BYTES: usize = 2 * 1024 * 1024;
pub const PROFILE_EXPORT_MAX_PROFILES: usize = 256;
pub const PROFILE_EXPORT_MAX_SECRET_FILES_PER_PROFILE: usize = 16;
pub const PROFILE_EXPORT_MAX_SECRET_FILES: usize =
    PROFILE_EXPORT_MAX_PROFILES * PROFILE_EXPORT_MAX_SECRET_FILES_PER_PROFILE;
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

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(bound(
    serialize = "Provider: Serialize",
    deserialize = "Provider: Deserialize<'de> + Default"
))]
pub struct ProfileExportPayload<Provider> {
    pub exported_at: String,
    pub source_prodex_version: String,
    pub active_profile: Option<String>,
    #[serde(
        serialize_with = "serialize_profile_export_profiles",
        deserialize_with = "deserialize_profile_export_profiles"
    )]
    pub profiles: Vec<ExportedProfile<Provider>>,
}

impl<Provider> fmt::Debug for ProfileExportPayload<Provider> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProfileExportPayload")
            .field("exported_at", &self.exported_at)
            .field("source_prodex_version", &self.source_prodex_version)
            .field("active_profile", &self.active_profile)
            .field("profile_count", &self.profiles.len())
            .field("profiles", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    #[serde(
        serialize_with = "serialize_bounded_secret_text",
        deserialize_with = "deserialize_bounded_secret_text"
    )]
    pub auth_json: String,
    #[serde(
        default,
        serialize_with = "serialize_exported_secret_files",
        deserialize_with = "deserialize_exported_secret_files"
    )]
    pub secret_files: Vec<ExportedSecretFile>,
}

impl<Provider> fmt::Debug for ExportedProfile<Provider> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExportedProfile")
            .field("name", &self.name)
            .field("email", &self.email)
            .field("source_managed", &self.source_managed)
            .field("provider", &"<redacted>")
            .field("auth_json", &"<redacted>")
            .field("secret_file_count", &self.secret_files.len())
            .field("secret_files", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportedSecretFile {
    pub path: String,
    #[serde(
        serialize_with = "serialize_bounded_secret_text",
        deserialize_with = "deserialize_bounded_secret_text"
    )]
    pub text: String,
}

impl fmt::Debug for ExportedSecretFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExportedSecretFile")
            .field("path", &self.path)
            .field("text", &"<redacted>")
            .finish()
    }
}

fn serialize_profile_export_profiles<S, Provider>(
    profiles: &[ExportedProfile<Provider>],
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    Provider: Serialize,
{
    if profiles.len() > PROFILE_EXPORT_MAX_PROFILES {
        return Err(serde::ser::Error::custom(format_args!(
            "profile export exceeds profile count limit ({PROFILE_EXPORT_MAX_PROFILES})"
        )));
    }
    let secret_file_count = profiles
        .iter()
        .map(|profile| profile.secret_files.len())
        .sum::<usize>();
    if secret_file_count > PROFILE_EXPORT_MAX_SECRET_FILES {
        return Err(serde::ser::Error::custom(format_args!(
            "profile export exceeds secret-file count limit ({PROFILE_EXPORT_MAX_SECRET_FILES})"
        )));
    }
    let mut sequence = serializer.serialize_seq(Some(profiles.len()))?;
    for profile in profiles {
        sequence.serialize_element(profile)?;
    }
    sequence.end()
}

fn deserialize_profile_export_profiles<'de, D, Provider>(
    deserializer: D,
) -> Result<Vec<ExportedProfile<Provider>>, D::Error>
where
    D: Deserializer<'de>,
    Provider: Deserialize<'de> + Default,
{
    struct ProfilesVisitor<Provider>(PhantomData<Provider>);

    impl<'de, Provider> Visitor<'de> for ProfilesVisitor<Provider>
    where
        Provider: Deserialize<'de> + Default,
    {
        type Value = Vec<ExportedProfile<Provider>>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a bounded profile array")
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let capacity = sequence
                .size_hint()
                .unwrap_or_default()
                .min(PROFILE_EXPORT_MAX_PROFILES);
            let mut profiles = Vec::with_capacity(capacity);
            while let Some(profile) = sequence.next_element()? {
                if profiles.len() == PROFILE_EXPORT_MAX_PROFILES {
                    return Err(serde::de::Error::custom(format_args!(
                        "profile export exceeds profile count limit ({PROFILE_EXPORT_MAX_PROFILES})"
                    )));
                }
                profiles.push(profile);
            }
            let secret_file_count = profiles
                .iter()
                .map(|profile: &ExportedProfile<Provider>| profile.secret_files.len())
                .sum::<usize>();
            if secret_file_count > PROFILE_EXPORT_MAX_SECRET_FILES {
                return Err(serde::de::Error::custom(format_args!(
                    "profile export exceeds secret-file count limit ({PROFILE_EXPORT_MAX_SECRET_FILES})"
                )));
            }
            Ok(profiles)
        }
    }

    deserializer.deserialize_seq(ProfilesVisitor(PhantomData))
}

fn serialize_exported_secret_files<S>(
    secret_files: &[ExportedSecretFile],
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if secret_files.len() > PROFILE_EXPORT_MAX_SECRET_FILES_PER_PROFILE {
        return Err(serde::ser::Error::custom(format_args!(
            "profile export exceeds per-profile secret-file count limit ({PROFILE_EXPORT_MAX_SECRET_FILES_PER_PROFILE})"
        )));
    }
    let mut sequence = serializer.serialize_seq(Some(secret_files.len()))?;
    for secret_file in secret_files {
        sequence.serialize_element(secret_file)?;
    }
    sequence.end()
}

fn deserialize_exported_secret_files<'de, D>(
    deserializer: D,
) -> Result<Vec<ExportedSecretFile>, D::Error>
where
    D: Deserializer<'de>,
{
    struct SecretFilesVisitor;

    impl<'de> Visitor<'de> for SecretFilesVisitor {
        type Value = Vec<ExportedSecretFile>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a bounded secret-file array")
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let capacity = sequence
                .size_hint()
                .unwrap_or_default()
                .min(PROFILE_EXPORT_MAX_SECRET_FILES_PER_PROFILE);
            let mut secret_files = Vec::with_capacity(capacity);
            while let Some(secret_file) = sequence.next_element()? {
                if secret_files.len() == PROFILE_EXPORT_MAX_SECRET_FILES_PER_PROFILE {
                    return Err(serde::de::Error::custom(format_args!(
                        "profile export exceeds per-profile secret-file count limit ({PROFILE_EXPORT_MAX_SECRET_FILES_PER_PROFILE})"
                    )));
                }
                secret_files.push(secret_file);
            }
            Ok(secret_files)
        }
    }

    deserializer.deserialize_seq(SecretFilesVisitor)
}

fn serialize_bounded_secret_text<S>(value: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if value.len() > PROFILE_EXPORT_NESTED_JSON_MAX_BYTES {
        return Err(serde::ser::Error::custom(format_args!(
            "profile export nested secret exceeds size limit ({PROFILE_EXPORT_NESTED_JSON_MAX_BYTES} bytes)"
        )));
    }
    serializer.serialize_str(value)
}

fn deserialize_bounded_secret_text<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    if value.len() > PROFILE_EXPORT_NESTED_JSON_MAX_BYTES {
        return Err(serde::de::Error::custom(format_args!(
            "profile export nested secret exceeds size limit ({PROFILE_EXPORT_NESTED_JSON_MAX_BYTES} bytes)"
        )));
    }
    Ok(value)
}

#[derive(Clone, PartialEq, Eq)]
pub struct ProfileImportAuthUpdatePlan {
    pub target_profile_name: String,
    pub email: Option<String>,
    pub auth_json: String,
}

impl fmt::Debug for ProfileImportAuthUpdatePlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProfileImportAuthUpdatePlan")
            .field("target_profile_name", &self.target_profile_name)
            .field("email", &self.email)
            .field("auth_json", &"<redacted>")
            .finish()
    }
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

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "algorithm", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProfileExportKdfParameters {
    Argon2id {
        version: u32,
        memory_kib: u32,
        iterations: u32,
        parallelism: u32,
    },
}

impl fmt::Debug for ProfileExportKdfParameters {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Argon2id {
                version,
                memory_kib,
                iterations,
                parallelism,
            } => formatter
                .debug_struct("Argon2id")
                .field("version", version)
                .field("memory_kib", memory_kib)
                .field("iterations", iterations)
                .field("parallelism", parallelism)
                .finish(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
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
    EncryptedV2 {
        format: String,
        version: u32,
        cipher: String,
        kdf: ProfileExportKdfParameters,
        salt_base64: String,
        nonce_base64: String,
        ciphertext_base64: String,
    },
}

impl<T> fmt::Debug for ProfileExportEnvelope<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plain {
                format, version, ..
            } => formatter
                .debug_struct("Plain")
                .field("format", format)
                .field("version", version)
                .field("payload", &"<redacted>")
                .finish(),
            Self::Encrypted {
                format,
                version,
                cipher,
                kdf,
                iterations,
                ..
            } => formatter
                .debug_struct("EncryptedV1")
                .field("format", format)
                .field("version", version)
                .field("cipher", cipher)
                .field("kdf", kdf)
                .field("iterations", iterations)
                .field("salt_base64", &"<redacted>")
                .field("nonce_base64", &"<redacted>")
                .field("ciphertext_base64", &"<redacted>")
                .finish(),
            Self::EncryptedV2 {
                format,
                version,
                cipher,
                kdf,
                ..
            } => formatter
                .debug_struct("EncryptedV2")
                .field("format", format)
                .field("version", version)
                .field("cipher", cipher)
                .field("kdf", kdf)
                .field("salt_base64", &"<redacted>")
                .field("nonce_base64", &"<redacted>")
                .field("ciphertext_base64", &"<redacted>")
                .finish(),
        }
    }
}
