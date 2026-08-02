use anyhow::Result;
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
    AppPaths, RuntimeBrokerRegistry, load_json_file_with_backup,
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
    if let Ok(legacy) = load_json_file_with_backup::<LegacyRuntimeBrokerRegistry>(path, backup_path)
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

pub(super) fn registry_has_legacy_secrets(path: &Path) -> bool {
    read_registry_bytes(path).is_some_and(|bytes| {
        prodex_runtime_broker::runtime_broker_registry_contains_legacy_secrets(bytes)
    })
}

pub(super) fn registry_file_is_current(path: &Path) -> bool {
    read_registry_bytes(path)
        .and_then(|bytes| serde_json::from_slice::<RuntimeBrokerRegistry>(&bytes).ok())
        .is_some()
}

fn read_registry_bytes(path: &Path) -> Option<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > RUNTIME_BROKER_REGISTRY_MAX_BYTES {
        return None;
    }
    let file = prodex_core::open_regular_file_no_follow(path).ok()?;
    if !prodex_core::opened_file_matches_path(&metadata, path, &file).ok()? {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(RUNTIME_BROKER_REGISTRY_MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= RUNTIME_BROKER_REGISTRY_MAX_BYTES).then_some(bytes)
}
