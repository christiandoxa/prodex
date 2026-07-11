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
use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine;
use pbkdf2::pbkdf2_hmac;
use serde::{Serialize, de::DeserializeOwned};
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::data_model::{
    PROFILE_EXPORT_ARGON2_ITERATIONS, PROFILE_EXPORT_ARGON2_MAX_ITERATIONS,
    PROFILE_EXPORT_ARGON2_MAX_MEMORY_KIB, PROFILE_EXPORT_ARGON2_MAX_PARALLELISM,
    PROFILE_EXPORT_ARGON2_MEMORY_KIB, PROFILE_EXPORT_ARGON2_MIN_ITERATIONS,
    PROFILE_EXPORT_ARGON2_MIN_MEMORY_KIB, PROFILE_EXPORT_ARGON2_MIN_PARALLELISM,
    PROFILE_EXPORT_ARGON2_PARALLELISM, PROFILE_EXPORT_ARGON2_VERSION, PROFILE_EXPORT_FORMAT,
    PROFILE_EXPORT_KEY_BYTES, PROFILE_EXPORT_NONCE_BYTES, PROFILE_EXPORT_PASSWORD_MAX_BYTES,
    PROFILE_EXPORT_PBKDF2_MAX_ITERATIONS, PROFILE_EXPORT_PBKDF2_MIN_ITERATIONS,
    PROFILE_EXPORT_PLAINTEXT_MAX_BYTES, PROFILE_EXPORT_SALT_BYTES, PROFILE_EXPORT_VERSION_V1,
    PROFILE_EXPORT_VERSION_V2,
};
use crate::{
    PROFILE_EXPORT_CIPHER, PROFILE_EXPORT_KDF, ProfileExportEnvelope, ProfileExportKdfParameters,
};

static PROFILE_EXPORT_BUNDLE_WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub const PROFILE_EXPORT_BUNDLE_MAX_BYTES: u64 = 64 * 1024 * 1024;
pub const PROFILE_EXPORT_CIPHERTEXT_MAX_BYTES: usize =
    PROFILE_EXPORT_PLAINTEXT_MAX_BYTES + PROFILE_EXPORT_AUTH_TAG_BYTES;
const PROFILE_EXPORT_AUTH_TAG_BYTES: usize = 16;

#[derive(Serialize)]
struct PlainProfileExportEnvelope<'a, T> {
    payload_kind: &'static str,
    format: &'static str,
    version: u32,
    payload: &'a T,
}

pub fn serialize_profile_export_payload<T>(payload: &T, password: Option<&str>) -> Result<Vec<u8>>
where
    T: Serialize,
{
    let encoded = match password {
        Some(password) => {
            serde_json::to_vec_pretty(&encrypt_profile_export_payload(payload, password)?)
        }
        None => serde_json::to_vec_pretty(&PlainProfileExportEnvelope {
            payload_kind: "plain",
            format: PROFILE_EXPORT_FORMAT,
            version: PROFILE_EXPORT_VERSION_V1,
            payload,
        }),
    };
    let encoded = encoded.context("failed to serialize profile export bundle")?;
    if encoded.len() as u64 > PROFILE_EXPORT_BUNDLE_MAX_BYTES {
        bail!(
            "profile export bundle exceeds safe size limit ({} bytes)",
            PROFILE_EXPORT_BUNDLE_MAX_BYTES
        );
    }
    Ok(encoded)
}

pub fn read_profile_export_envelope<T>(path: &Path) -> Result<(ProfileExportEnvelope<T>, bool)>
where
    T: DeserializeOwned,
{
    let content = Zeroizing::new(read_profile_export_bundle(path)?);
    let envelope = parse_profile_export_envelope(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let encrypted = matches!(
        envelope,
        ProfileExportEnvelope::Encrypted { .. } | ProfileExportEnvelope::EncryptedV2 { .. }
    );
    Ok((envelope, encrypted))
}

pub fn parse_profile_export_envelope<T>(content: &[u8]) -> Result<ProfileExportEnvelope<T>>
where
    T: DeserializeOwned,
{
    if content.len() as u64 > PROFILE_EXPORT_BUNDLE_MAX_BYTES {
        bail!(
            "profile export bundle exceeds safe size limit ({} bytes)",
            PROFILE_EXPORT_BUNDLE_MAX_BYTES
        );
    }
    let envelope: ProfileExportEnvelope<T> =
        serde_json::from_slice(content).map_err(redacted_profile_export_parse_error)?;
    validate_profile_export_envelope(&envelope)?;
    Ok(envelope)
}

fn redacted_profile_export_parse_error(error: serde_json::Error) -> anyhow::Error {
    let detail = error.to_string();
    for stable_error in [
        "profile export exceeds profile count limit",
        "profile export exceeds secret-file count limit",
        "profile export exceeds per-profile secret-file count limit",
        "profile export nested secret exceeds size limit",
    ] {
        if detail.contains(stable_error) {
            return anyhow::anyhow!(stable_error);
        }
    }
    anyhow::anyhow!("invalid profile export envelope")
}

fn redacted_decrypted_payload_parse_error(error: serde_json::Error) -> anyhow::Error {
    let error = redacted_profile_export_parse_error(error);
    if error.to_string() == "invalid profile export envelope" {
        anyhow::anyhow!("failed to parse decrypted profile export payload")
    } else {
        error
    }
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
    validate_profile_export_envelope(&envelope)?;
    match envelope {
        ProfileExportEnvelope::Plain { payload, .. } => Ok(payload),
        ProfileExportEnvelope::Encrypted {
            iterations,
            salt_base64,
            nonce_base64,
            ciphertext_base64,
            ..
        } => decrypt_profile_export_payload(
            salt_base64,
            nonce_base64,
            ciphertext_base64,
            ProfileExportKdfPlan::Pbkdf2Sha256 { iterations },
            resolve_password,
        ),
        ProfileExportEnvelope::EncryptedV2 {
            kdf:
                ProfileExportKdfParameters::Argon2id {
                    memory_kib,
                    iterations,
                    parallelism,
                    ..
                },
            salt_base64,
            nonce_base64,
            ciphertext_base64,
            ..
        } => decrypt_profile_export_payload(
            salt_base64,
            nonce_base64,
            ciphertext_base64,
            ProfileExportKdfPlan::Argon2id {
                memory_kib,
                iterations,
                parallelism,
            },
            resolve_password,
        ),
    }
}

#[derive(Clone, Copy)]
enum ProfileExportKdfPlan {
    Pbkdf2Sha256 {
        iterations: u32,
    },
    Argon2id {
        memory_kib: u32,
        iterations: u32,
        parallelism: u32,
    },
}

fn decrypt_profile_export_payload<T>(
    salt_base64: String,
    nonce_base64: String,
    ciphertext_base64: String,
    kdf: ProfileExportKdfPlan,
    resolve_password: impl FnOnce() -> Result<String>,
) -> Result<T>
where
    T: DeserializeOwned,
{
    let salt = Zeroizing::new(
        base64::engine::general_purpose::STANDARD
            .decode(salt_base64)
            .context("failed to decode encrypted export salt")?,
    );
    let nonce = Zeroizing::new(
        base64::engine::general_purpose::STANDARD
            .decode(nonce_base64)
            .context("failed to decode encrypted export nonce")?,
    );
    let ciphertext = Zeroizing::new(
        base64::engine::general_purpose::STANDARD
            .decode(ciphertext_base64)
            .context("failed to decode encrypted export payload")?,
    );
    let password = Zeroizing::new(resolve_password()?);
    validate_profile_export_password(password.as_bytes())?;
    let mut key = Zeroizing::new([0_u8; PROFILE_EXPORT_KEY_BYTES]);
    derive_profile_export_key_into(password.as_str(), &salt, kdf, &mut key)?;
    let cipher = Aes256GcmSiv::new_from_slice(key.as_ref())
        .map_err(|_| anyhow::anyhow!("failed to initialize import cipher"))?;
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(Nonce::from_slice(&nonce), ciphertext.as_slice())
            .map_err(|_| anyhow::anyhow!("failed to decrypt profile export bundle"))?,
    );
    if plaintext.len() > PROFILE_EXPORT_PLAINTEXT_MAX_BYTES {
        bail!(
            "decrypted profile export payload exceeds safe size limit ({} bytes)",
            PROFILE_EXPORT_PLAINTEXT_MAX_BYTES
        );
    }
    serde_json::from_slice(&plaintext).map_err(redacted_decrypted_payload_parse_error)
}

/// Derives a legacy version-1 PBKDF2 key.
///
/// This remains source-compatible for callers that inspect old envelopes. New
/// code should use the encrypt/decode APIs, whose key buffers zeroize on drop.
#[deprecated(
    note = "legacy v1 compatibility helper returns a raw key; use profile export encrypt/decode APIs"
)]
pub fn derive_profile_export_key(
    password: &str,
    salt: &[u8],
    iterations: u32,
) -> [u8; PROFILE_EXPORT_KEY_BYTES] {
    let mut key = [0_u8; PROFILE_EXPORT_KEY_BYTES];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, iterations, &mut key);
    key
}

fn derive_profile_export_key_into(
    password: &str,
    salt: &[u8],
    kdf: ProfileExportKdfPlan,
    key: &mut [u8; PROFILE_EXPORT_KEY_BYTES],
) -> Result<()> {
    match kdf {
        ProfileExportKdfPlan::Pbkdf2Sha256 { iterations } => {
            pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, iterations, key);
            Ok(())
        }
        ProfileExportKdfPlan::Argon2id {
            memory_kib,
            iterations,
            parallelism,
        } => {
            let params = Params::new(
                memory_kib,
                iterations,
                parallelism,
                Some(PROFILE_EXPORT_KEY_BYTES),
            )
            .map_err(|_| anyhow::anyhow!("invalid profile export Argon2id parameters"))?;
            Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
                .hash_password_into(password.as_bytes(), salt, key)
                .map_err(|_| anyhow::anyhow!("failed to derive profile export key"))
        }
    }
}

pub fn validate_profile_export_header(format: &str, version: u32) -> Result<()> {
    if format != PROFILE_EXPORT_FORMAT {
        bail!("unsupported profile export format");
    }
    if !matches!(
        version,
        PROFILE_EXPORT_VERSION_V1 | PROFILE_EXPORT_VERSION_V2
    ) {
        bail!(
            "unsupported profile export version {} (supported: {}, {})",
            version,
            PROFILE_EXPORT_VERSION_V1,
            PROFILE_EXPORT_VERSION_V2
        );
    }
    Ok(())
}

fn validate_profile_export_header_version(format: &str, version: u32, expected: u32) -> Result<()> {
    validate_profile_export_header(format, version)?;
    if version != expected {
        bail!(
            "profile export envelope kind requires version {} (received {})",
            expected,
            version
        );
    }
    Ok(())
}

fn validate_profile_export_envelope<T>(envelope: &ProfileExportEnvelope<T>) -> Result<()> {
    match envelope {
        ProfileExportEnvelope::Plain {
            format, version, ..
        } => validate_profile_export_header_version(format, *version, PROFILE_EXPORT_VERSION_V1),
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
            validate_profile_export_header_version(format, *version, PROFILE_EXPORT_VERSION_V1)?;
            validate_profile_export_cipher(cipher)?;
            if kdf != PROFILE_EXPORT_KDF {
                bail!("unsupported profile export KDF");
            }
            validate_profile_export_pbkdf2_iterations(*iterations)?;
            validate_encrypted_profile_export_fields(salt_base64, nonce_base64, ciphertext_base64)
        }
        ProfileExportEnvelope::EncryptedV2 {
            format,
            version,
            cipher,
            kdf,
            salt_base64,
            nonce_base64,
            ciphertext_base64,
        } => {
            validate_profile_export_header_version(format, *version, PROFILE_EXPORT_VERSION_V2)?;
            validate_profile_export_cipher(cipher)?;
            match kdf {
                ProfileExportKdfParameters::Argon2id {
                    version,
                    memory_kib,
                    iterations,
                    parallelism,
                } => validate_profile_export_argon2id_parameters(
                    *version,
                    *memory_kib,
                    *iterations,
                    *parallelism,
                )?,
            }
            validate_encrypted_profile_export_fields(salt_base64, nonce_base64, ciphertext_base64)
        }
    }
}

fn validate_profile_export_cipher(cipher: &str) -> Result<()> {
    if cipher != PROFILE_EXPORT_CIPHER {
        bail!("unsupported profile export cipher");
    }
    Ok(())
}

fn validate_profile_export_pbkdf2_iterations(iterations: u32) -> Result<()> {
    if !(PROFILE_EXPORT_PBKDF2_MIN_ITERATIONS..=PROFILE_EXPORT_PBKDF2_MAX_ITERATIONS)
        .contains(&iterations)
    {
        bail!(
            "profile export PBKDF2 iteration count {} is outside safe range {}..={}",
            iterations,
            PROFILE_EXPORT_PBKDF2_MIN_ITERATIONS,
            PROFILE_EXPORT_PBKDF2_MAX_ITERATIONS
        );
    }
    Ok(())
}

fn validate_profile_export_argon2id_parameters(
    version: u32,
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
) -> Result<()> {
    if version != PROFILE_EXPORT_ARGON2_VERSION {
        bail!("unsupported profile export Argon2id version {}", version);
    }
    if !(PROFILE_EXPORT_ARGON2_MIN_MEMORY_KIB..=PROFILE_EXPORT_ARGON2_MAX_MEMORY_KIB)
        .contains(&memory_kib)
    {
        bail!("profile export Argon2id memory cost is outside safe range");
    }
    if !(PROFILE_EXPORT_ARGON2_MIN_ITERATIONS..=PROFILE_EXPORT_ARGON2_MAX_ITERATIONS)
        .contains(&iterations)
    {
        bail!("profile export Argon2id iteration count is outside safe range");
    }
    if !(PROFILE_EXPORT_ARGON2_MIN_PARALLELISM..=PROFILE_EXPORT_ARGON2_MAX_PARALLELISM)
        .contains(&parallelism)
    {
        bail!("profile export Argon2id parallelism is outside safe range");
    }
    Ok(())
}

fn validate_encrypted_profile_export_fields(
    salt_base64: &str,
    nonce_base64: &str,
    ciphertext_base64: &str,
) -> Result<()> {
    validate_base64_field("salt", salt_base64, PROFILE_EXPORT_SALT_BYTES, true)?;
    validate_base64_field("nonce", nonce_base64, PROFILE_EXPORT_NONCE_BYTES, true)?;
    validate_base64_field(
        "payload",
        ciphertext_base64,
        PROFILE_EXPORT_CIPHERTEXT_MAX_BYTES,
        false,
    )?;
    let ciphertext_len = decoded_base64_length(ciphertext_base64)?;
    if ciphertext_len < PROFILE_EXPORT_AUTH_TAG_BYTES {
        bail!("invalid encrypted export payload length");
    }
    Ok(())
}

fn validate_base64_field(label: &str, value: &str, limit: usize, exact: bool) -> Result<()> {
    let decoded_len = decoded_base64_length(value)
        .with_context(|| format!("invalid encrypted export {label} encoding"))?;
    if (exact && decoded_len != limit) || (!exact && decoded_len > limit) {
        bail!("invalid encrypted export {label} length");
    }
    Ok(())
}

fn decoded_base64_length(value: &str) -> Result<usize> {
    let bytes = value.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        bail!("invalid base64 length");
    }
    let padding = bytes.iter().rev().take_while(|byte| **byte == b'=').count();
    if padding > 2
        || bytes[..bytes.len().saturating_sub(padding)]
            .iter()
            .any(|byte| !byte.is_ascii_alphanumeric() && !matches!(*byte, b'+' | b'/'))
        || bytes[..bytes.len().saturating_sub(padding)].contains(&b'=')
    {
        bail!("invalid base64 encoding");
    }
    Ok(bytes.len() / 4 * 3 - padding)
}

fn validate_profile_export_password(password: &[u8]) -> Result<()> {
    if password.is_empty() || password.len() > PROFILE_EXPORT_PASSWORD_MAX_BYTES {
        bail!(
            "profile export password must contain 1..={} bytes",
            PROFILE_EXPORT_PASSWORD_MAX_BYTES
        );
    }
    Ok(())
}

fn encrypt_profile_export_payload<T>(
    payload: &T,
    password: &str,
) -> Result<ProfileExportEnvelope<()>>
where
    T: Serialize,
{
    validate_profile_export_password(password.as_bytes())?;
    let payload_json = Zeroizing::new(
        serde_json::to_vec(payload).context("failed to serialize profile export payload")?,
    );
    if payload_json.len() > PROFILE_EXPORT_PLAINTEXT_MAX_BYTES {
        bail!(
            "profile export payload exceeds safe size limit ({} bytes)",
            PROFILE_EXPORT_PLAINTEXT_MAX_BYTES
        );
    }
    let mut salt = Zeroizing::new([0_u8; PROFILE_EXPORT_SALT_BYTES]);
    getrandom::fill(&mut salt[..])
        .map_err(|err| anyhow::anyhow!("failed to generate export salt: {err}"))?;
    let mut nonce = Zeroizing::new([0_u8; PROFILE_EXPORT_NONCE_BYTES]);
    getrandom::fill(&mut nonce[..])
        .map_err(|err| anyhow::anyhow!("failed to generate export nonce: {err}"))?;
    let password = Zeroizing::new(password.to_string());
    let mut key = Zeroizing::new([0_u8; PROFILE_EXPORT_KEY_BYTES]);
    derive_profile_export_key_into(
        password.as_str(),
        &salt[..],
        ProfileExportKdfPlan::Argon2id {
            memory_kib: PROFILE_EXPORT_ARGON2_MEMORY_KIB,
            iterations: PROFILE_EXPORT_ARGON2_ITERATIONS,
            parallelism: PROFILE_EXPORT_ARGON2_PARALLELISM,
        },
        &mut key,
    )?;
    let cipher = Aes256GcmSiv::new_from_slice(key.as_ref())
        .map_err(|_| anyhow::anyhow!("failed to initialize export cipher"))?;
    let ciphertext = Zeroizing::new(
        cipher
            .encrypt(Nonce::from_slice(&nonce[..]), payload_json.as_slice())
            .map_err(|_| anyhow::anyhow!("failed to encrypt profile export payload"))?,
    );

    Ok(ProfileExportEnvelope::EncryptedV2 {
        format: PROFILE_EXPORT_FORMAT.to_string(),
        version: PROFILE_EXPORT_VERSION_V2,
        cipher: PROFILE_EXPORT_CIPHER.to_string(),
        kdf: ProfileExportKdfParameters::Argon2id {
            version: PROFILE_EXPORT_ARGON2_VERSION,
            memory_kib: PROFILE_EXPORT_ARGON2_MEMORY_KIB,
            iterations: PROFILE_EXPORT_ARGON2_ITERATIONS,
            parallelism: PROFILE_EXPORT_ARGON2_PARALLELISM,
        },
        salt_base64: base64::engine::general_purpose::STANDARD.encode(salt.as_ref()),
        nonce_base64: base64::engine::general_purpose::STANDARD.encode(nonce.as_ref()),
        ciphertext_base64: base64::engine::general_purpose::STANDARD.encode(ciphertext.as_slice()),
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

#[cfg(test)]
#[path = "envelope/tests.rs"]
mod tests;
