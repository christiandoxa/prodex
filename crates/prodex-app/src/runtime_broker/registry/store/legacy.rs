use anyhow::{Context, Result};
use prodex_runtime_broker::RuntimeBrokerSecret;
use serde::Deserialize;
use std::fs;
use std::io::Read as _;
use std::path::Path;

use super::{
    load_runtime_broker_capability_record, remove_runtime_broker_capability_unlocked,
    remove_runtime_broker_registry_files_checked,
};
use crate::{
    AppPaths, RuntimeBrokerRegistry, load_json_file_with_backup_unlocked,
    runtime_broker_capability_file_path,
};

const RUNTIME_BROKER_REGISTRY_MAX_BYTES: u64 = 64 * 1024;

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
    if let Ok(legacy) =
        load_json_file_with_backup_unlocked::<LegacyRuntimeBrokerRegistry>(path, backup_path)
    {
        let LegacyRuntimeBrokerRegistry {
            instance_token,
            admin_token,
        } = legacy.value;
        drop(instance_token);
        drop(admin_token);
    }
    remove_runtime_broker_registry_files_checked(paths, broker_key)?;
    let capability_path = runtime_broker_capability_file_path(paths, broker_key);
    if fs::symlink_metadata(&capability_path).is_err() {
        return Ok(());
    }
    if load_runtime_broker_capability_record(paths, broker_key).is_ok() {
        remove_runtime_broker_capability_unlocked(paths, broker_key);
    }
    if fs::symlink_metadata(&capability_path).is_ok() {
        anyhow::bail!("failed to remove legacy runtime broker capability");
    }
    Ok(())
}

pub(super) fn registry_has_legacy_secrets(path: &Path) -> Result<bool> {
    Ok(read_registry_bytes(path)?.is_some_and(|bytes| {
        prodex_runtime_broker::runtime_broker_registry_contains_legacy_secrets(bytes)
    }))
}

pub(super) fn registry_file_is_current(path: &Path) -> Result<bool> {
    Ok(read_registry_bytes(path)?
        .is_some_and(|bytes| serde_json::from_slice::<RuntimeBrokerRegistry>(&bytes).is_ok()))
}

fn read_registry_bytes(path: &Path) -> Result<Option<Vec<u8>>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to inspect {}", path.display()));
        }
    };
    anyhow::ensure!(
        metadata.file_type().is_file(),
        "runtime broker registry is not a regular file: {}",
        path.display()
    );
    anyhow::ensure!(
        metadata.len() <= RUNTIME_BROKER_REGISTRY_MAX_BYTES,
        "runtime broker registry exceeds legacy scan size limit: {}",
        path.display()
    );
    let file = prodex_core::open_regular_file_no_follow(path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    anyhow::ensure!(
        prodex_core::opened_file_matches_path(&metadata, path, &file)
            .with_context(|| format!("failed to inspect {}", path.display()))?,
        "runtime broker registry changed while reading: {}",
        path.display()
    );
    let mut bytes = Vec::new();
    file.take(RUNTIME_BROKER_REGISTRY_MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read {}", path.display()))?;
    anyhow::ensure!(
        bytes.len() as u64 <= RUNTIME_BROKER_REGISTRY_MAX_BYTES,
        "runtime broker registry exceeds legacy scan size limit: {}",
        path.display()
    );
    Ok(Some(bytes))
}
