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
use serde::{Serialize, de::DeserializeOwned};
use sha2::Sha256;

mod copilot;
mod data_model;
mod transaction;

pub use copilot::*;
pub use data_model::*;
use data_model::{
    ImportIdentityTarget, PROFILE_EXPORT_FORMAT, PROFILE_EXPORT_KEY_BYTES,
    PROFILE_EXPORT_NONCE_BYTES, PROFILE_EXPORT_PBKDF2_ITERATIONS, PROFILE_EXPORT_SALT_BYTES,
    PROFILE_EXPORT_VERSION,
};
pub use transaction::*;

static PROFILE_EXPORT_BUNDLE_WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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
    let account_id = account_id
        .map(str::trim)
        .filter(|account_id| !account_id.is_empty());
    let email = email
        .map(str::trim)
        .filter(|email| !email.is_empty())
        .map(str::to_ascii_lowercase);

    match (account_id, email) {
        (Some(account_id), Some(email)) => Some(format!("account:{account_id}|email:{email}")),
        (Some(account_id), None) => Some(format!("account:{account_id}")),
        (None, Some(email)) => Some(format!("email:{email}")),
        (None, None) => None,
    }
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
