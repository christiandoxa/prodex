use std::fs;
use std::fs::OpenOptions;
use std::io::{Read as _, Write as _};
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

use crate::data_model::{
    PROFILE_EXPORT_FORMAT, PROFILE_EXPORT_KEY_BYTES, PROFILE_EXPORT_NONCE_BYTES,
    PROFILE_EXPORT_PBKDF2_ITERATIONS, PROFILE_EXPORT_SALT_BYTES, PROFILE_EXPORT_VERSION,
};
use crate::{PROFILE_EXPORT_CIPHER, PROFILE_EXPORT_KDF, ProfileExportEnvelope};

static PROFILE_EXPORT_BUNDLE_WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub(crate) const PROFILE_EXPORT_BUNDLE_MAX_BYTES: u64 = 64 * 1024 * 1024;

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
    let content = read_profile_export_bundle(path)?;
    let envelope: ProfileExportEnvelope<T> = serde_json::from_slice(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let encrypted = matches!(envelope, ProfileExportEnvelope::Encrypted { .. });
    Ok((envelope, encrypted))
}

fn read_profile_export_bundle(path: &Path) -> Result<Vec<u8>> {
    let metadata =
        fs::metadata(path).with_context(|| format!("failed to inspect {}", path.display()))?;
    if !metadata.file_type().is_file() {
        bail!("profile export bundle {} is not a file", path.display());
    }
    if metadata.len() > PROFILE_EXPORT_BUNDLE_MAX_BYTES {
        bail!(
            "profile export bundle {} exceeds safe size limit ({} bytes)",
            path.display(),
            PROFILE_EXPORT_BUNDLE_MAX_BYTES
        );
    }
    let file =
        fs::File::open(path).with_context(|| format!("failed to read {}", path.display()))?;
    if !profile_export_same_file_metadata(&metadata, &file.metadata()?) {
        bail!(
            "profile export bundle changed while opening {}",
            path.display()
        );
    }
    let mut bytes = Vec::new();
    file.take(PROFILE_EXPORT_BUNDLE_MAX_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read {}", path.display()))?;
    if bytes.len() as u64 > PROFILE_EXPORT_BUNDLE_MAX_BYTES {
        bail!(
            "profile export bundle {} exceeds safe size limit ({} bytes)",
            path.display(),
            PROFILE_EXPORT_BUNDLE_MAX_BYTES
        );
    }
    Ok(bytes)
}

#[cfg(unix)]
fn profile_export_same_file_metadata(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn profile_export_same_file_metadata(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    true
}

pub fn write_profile_export_bundle(path: &Path, content: &[u8]) -> Result<()> {
    if content.len() as u64 > PROFILE_EXPORT_BUNDLE_MAX_BYTES {
        bail!(
            "profile export bundle {} exceeds safe size limit ({} bytes)",
            path.display(),
            PROFILE_EXPORT_BUNDLE_MAX_BYTES
        );
    }
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
