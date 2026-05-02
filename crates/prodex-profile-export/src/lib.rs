use std::collections::{BTreeMap, BTreeSet};

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
