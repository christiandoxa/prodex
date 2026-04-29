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
