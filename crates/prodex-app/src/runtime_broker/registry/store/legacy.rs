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
    AppPaths, RuntimeBrokerRegistry, delete_runtime_secret, load_json_file_with_backup_unlocked,
    runtime_broker_capability_file_path,
};

const RUNTIME_BROKER_REGISTRY_MAX_BYTES: u64 = crate::runtime_store::RUNTIME_STORE_JSON_MAX_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RegistryLegacyStatus {
    NotLegacy,
    ValidLegacy,
    Malformed,
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
    let value = String::deserialize(deserializer)?;
    RuntimeBrokerSecret::new(value).map_err(serde::de::Error::custom)
}

pub(super) fn remove_artifacts_unlocked(
    paths: &AppPaths,
    broker_key: &str,
    path: &Path,
    backup_path: &Path,
) -> Result<()> {
    let legacy =
        load_json_file_with_backup_unlocked::<LegacyRuntimeBrokerRegistry>(path, backup_path)
            .context("legacy runtime broker registry is invalid")?;
    let LegacyRuntimeBrokerRegistry {
        instance_token,
        admin_token,
    } = legacy.value;
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

pub(super) fn registry_legacy_status(path: &Path) -> Result<RegistryLegacyStatus> {
    let Some(bytes) = read_registry_bytes(path)? else {
        return Ok(RegistryLegacyStatus::NotLegacy);
    };
    let contains_legacy_keys =
        match prodex_runtime_broker::runtime_broker_registry_contains_legacy_secrets(&bytes) {
            Ok(contains) => contains,
            Err(_) => return Ok(RegistryLegacyStatus::Malformed),
        };
    if !contains_legacy_keys {
        return Ok(RegistryLegacyStatus::NotLegacy);
    }
    if serde_json::from_slice::<LegacyRuntimeBrokerRegistry>(&bytes).is_ok() {
        Ok(RegistryLegacyStatus::ValidLegacy)
    } else {
        Ok(RegistryLegacyStatus::Malformed)
    }
}

pub(super) fn registry_file_is_current(path: &Path) -> bool {
    read_registry_bytes(path)
        .ok()
        .flatten()
        .and_then(|bytes| serde_json::from_slice::<RuntimeBrokerRegistry>(&bytes).ok())
        .is_some()
}

fn read_registry_bytes(path: &Path) -> Result<Option<Zeroizing<Vec<u8>>>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Ok(None),
    };
    if !metadata.file_type().is_file() {
        return Ok(None);
    }
    if metadata.len() > RUNTIME_BROKER_REGISTRY_MAX_BYTES {
        bail!(
            "runtime broker registry {} exceeds safe size limit ({} bytes)",
            path.display(),
            RUNTIME_BROKER_REGISTRY_MAX_BYTES
        );
    }
    let file = match prodex_core::open_regular_file_no_follow(path) {
        Ok(file) => file,
        Err(_) => return Ok(None),
    };
    if !prodex_core::opened_file_matches_path(&metadata, path, &file).unwrap_or(false) {
        return Ok(None);
    }
    let mut bytes = Zeroizing::new(Vec::new());
    if file
        .take(RUNTIME_BROKER_REGISTRY_MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return Ok(None);
    }
    if bytes.len() as u64 > RUNTIME_BROKER_REGISTRY_MAX_BYTES {
        bail!(
            "runtime broker registry {} exceeds safe size limit ({} bytes)",
            path.display(),
            RUNTIME_BROKER_REGISTRY_MAX_BYTES
        );
    }
    Ok(Some(bytes))
}
