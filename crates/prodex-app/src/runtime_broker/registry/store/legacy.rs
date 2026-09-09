use anyhow::{Context, Result, bail};
use prodex_runtime_broker::RuntimeBrokerSecret;
use serde::Deserialize;
use std::fs;
use std::io::Read as _;
use std::path::Path;
use zeroize::Zeroizing;

use super::{
    load_runtime_broker_capability_record, remove_runtime_broker_capability_unlocked,
    remove_runtime_broker_registry_files_checked,
};
use crate::{
    AppPaths, RuntimeBrokerRegistry, delete_runtime_secret, runtime_broker_capability_file_path,
};

const RUNTIME_BROKER_REGISTRY_MAX_BYTES: u64 = crate::runtime_store::RUNTIME_STORE_JSON_MAX_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RegistryLegacyStatus {
    NotLegacy,
    ValidLegacy,
    Malformed,
    TooLarge,
}

enum RegistryBytes {
    Unavailable,
    Present(Zeroizing<Vec<u8>>),
    TooLarge,
}

#[derive(Deserialize)]
struct LegacyRuntimeBrokerRegistry {
    #[serde(deserialize_with = "deserialize_runtime_broker_secret")]
    instance_token: RuntimeBrokerSecret,
    #[serde(deserialize_with = "deserialize_runtime_broker_secret")]
    admin_token: RuntimeBrokerSecret,
}

fn deserialize_runtime_broker_secret<'de, D>(
    deserializer: D,
) -> std::result::Result<RuntimeBrokerSecret, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct SecretVisitor;

    impl serde::de::Visitor<'_> for SecretVisitor {
        type Value = RuntimeBrokerSecret;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a valid runtime broker secret")
        }

        fn visit_string<E>(self, value: String) -> std::result::Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            RuntimeBrokerSecret::new(value).map_err(E::custom)
        }

        fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            RuntimeBrokerSecret::new(value).map_err(E::custom)
        }
    }

    deserializer.deserialize_string(SecretVisitor)
}

pub(super) fn remove_artifacts_unlocked(
    paths: &AppPaths,
    broker_key: &str,
    path: &Path,
    backup_path: &Path,
) -> Result<()> {
    let legacy = load_legacy_registry_with_backup_unlocked(path, backup_path)
        .context("legacy runtime broker registry is invalid")?;
    let LegacyRuntimeBrokerRegistry {
        instance_token,
        admin_token,
    } = legacy;
    drop(instance_token);
    drop(admin_token);
    remove_runtime_broker_registry_files_checked(paths, broker_key)?;
    let capability_path = runtime_broker_capability_file_path(paths, broker_key);
    match fs::symlink_metadata(&capability_path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // Keyring-backed capabilities have no file to signal their presence.
            delete_runtime_secret(paths, &capability_path);
            return Ok(());
        }
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to inspect {}", capability_path.display()));
        }
    }
    if load_runtime_broker_capability_record(paths, broker_key).is_ok() {
        remove_runtime_broker_capability_unlocked(paths, broker_key);
    }
    if fs::symlink_metadata(&capability_path).is_ok() {
        anyhow::bail!("failed to remove legacy runtime broker capability");
    }
    Ok(())
}

fn load_legacy_registry_with_backup_unlocked(
    path: &Path,
    backup_path: &Path,
) -> Result<LegacyRuntimeBrokerRegistry> {
    match read_legacy_registry(path) {
        Ok(legacy) => Ok(legacy),
        Err(primary_error) => read_legacy_registry(backup_path).with_context(|| {
            format!(
                "failed to parse {} after primary load error: {primary_error:#}",
                backup_path.display()
            )
        }),
    }
}

fn read_legacy_registry(path: &Path) -> Result<LegacyRuntimeBrokerRegistry> {
    let bytes = match read_registry_bytes(path)? {
        RegistryBytes::Unavailable => bail!("failed to read {}", path.display()),
        RegistryBytes::TooLarge => bail!(
            "runtime broker registry {} exceeds safe size limit ({})",
            path.display(),
            RUNTIME_BROKER_REGISTRY_MAX_BYTES
        ),
        RegistryBytes::Present(bytes) => bytes,
    };
    serde_json::from_slice(&bytes).with_context(|| format!("failed to parse {}", path.display()))
}

pub(super) fn registry_legacy_status(path: &Path) -> Result<RegistryLegacyStatus> {
    let bytes = match read_registry_bytes(path)? {
        RegistryBytes::Unavailable => return Ok(RegistryLegacyStatus::NotLegacy),
        RegistryBytes::TooLarge => return Ok(RegistryLegacyStatus::TooLarge),
        RegistryBytes::Present(bytes) => bytes,
    };
    let contains_legacy_keys =
        match prodex_runtime_broker::runtime_broker_registry_contains_legacy_secrets(&bytes) {
            Ok(contains) => contains,
            Err(_) => return Ok(RegistryLegacyStatus::Malformed),
        };
    if !contains_legacy_keys {
        return Ok(RegistryLegacyStatus::NotLegacy);
    }
    match serde_json::from_slice::<LegacyRuntimeBrokerRegistry>(&bytes) {
        Ok(LegacyRuntimeBrokerRegistry {
            instance_token,
            admin_token,
        }) => {
            drop(instance_token);
            drop(admin_token);
            Ok(RegistryLegacyStatus::ValidLegacy)
        }
        Err(_) => Ok(RegistryLegacyStatus::Malformed),
    }
}

pub(super) fn registry_file_is_current(path: &Path) -> bool {
    matches!(
        read_registry_bytes(path).ok(),
        Some(RegistryBytes::Present(bytes)) if parse_current_registry_bytes(&bytes).is_ok()
    )
}

pub(super) fn parse_current_registry(content: &str) -> Result<RuntimeBrokerRegistry> {
    parse_current_registry_bytes(content.as_bytes())
}

fn parse_current_registry_bytes(bytes: &[u8]) -> Result<RuntimeBrokerRegistry> {
    let contains_legacy_keys =
        prodex_runtime_broker::runtime_broker_registry_contains_legacy_secrets(bytes)
            .context("failed to inspect runtime broker registry")?;
    if contains_legacy_keys {
        bail!("runtime broker registry contains legacy secret fields");
    }
    Ok(serde_json::from_slice(bytes)?)
}

fn read_registry_bytes(path: &Path) -> Result<RegistryBytes> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(RegistryBytes::Unavailable);
        }
        Err(_) => return Ok(RegistryBytes::Unavailable),
    };
    if !metadata.file_type().is_file() {
        return Ok(RegistryBytes::Unavailable);
    }
    if metadata.len() > RUNTIME_BROKER_REGISTRY_MAX_BYTES {
        return Ok(RegistryBytes::TooLarge);
    }
    let file = match prodex_core::open_regular_file_no_follow(path) {
        Ok(file) => file,
        Err(_) => return Ok(RegistryBytes::Unavailable),
    };
    if !prodex_core::opened_file_matches_path(&metadata, path, &file).unwrap_or(false) {
        return Ok(RegistryBytes::Unavailable);
    }
    let mut bytes = Zeroizing::new(Vec::new());
    if file
        .take(RUNTIME_BROKER_REGISTRY_MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return Ok(RegistryBytes::Unavailable);
    }
    if bytes.len() as u64 > RUNTIME_BROKER_REGISTRY_MAX_BYTES {
        return Ok(RegistryBytes::TooLarge);
    }
    Ok(RegistryBytes::Present(bytes))
}
