use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;
use zeroize::Zeroizing;

use crate::{
    AppPaths, RuntimeBrokerRegistry, load_json_file_with_backup,
    runtime_broker_capability_file_path, runtime_broker_registry_file_path,
    runtime_broker_registry_last_good_file_path, terminate_runtime_process,
    write_json_file_with_backup,
};
use prodex_runtime_broker::{RuntimeBrokerCapability, RuntimeBrokerSecret};
use secret_store::{FileSecretBackend, SecretBackend as _, SecretLocation, SecretValue};

const RUNTIME_BROKER_CAPABILITY_MAX_BYTES: u64 =
    prodex_runtime_broker::RUNTIME_BROKER_CAPABILITY_MAX_BYTES as u64;
const RUNTIME_BROKER_REGISTRY_MAX_BYTES: u64 = 64 * 1024;

#[derive(Deserialize)]
struct LegacyRuntimeBrokerRegistry {
    pid: u32,
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

pub(crate) fn load_runtime_broker_registry(
    paths: &AppPaths,
    broker_key: &str,
) -> Result<Option<RuntimeBrokerRegistry>> {
    let path = runtime_broker_registry_file_path(paths, broker_key);
    let backup_path = runtime_broker_registry_last_good_file_path(paths, broker_key);
    if !path.exists() && !backup_path.exists() {
        return Ok(None);
    }
    let primary_has_legacy_secrets = runtime_broker_registry_has_legacy_secrets(&path);
    let backup_has_legacy_secrets = runtime_broker_registry_has_legacy_secrets(&backup_path);
    if backup_has_legacy_secrets
        && !primary_has_legacy_secrets
        && runtime_broker_registry_file_is_current(&path)
    {
        remove_runtime_broker_file_checked(&backup_path)?;
    } else if primary_has_legacy_secrets || backup_has_legacy_secrets {
        if let Ok(legacy) =
            load_json_file_with_backup::<LegacyRuntimeBrokerRegistry>(&path, &backup_path)
        {
            let LegacyRuntimeBrokerRegistry {
                pid,
                instance_token,
                admin_token,
            } = legacy.value;
            terminate_runtime_process(pid);
            drop(instance_token);
            drop(admin_token);
        }
        remove_runtime_broker_registry_files_checked(paths, broker_key)?;
        remove_runtime_broker_capability(paths, broker_key);
        if fs::symlink_metadata(runtime_broker_capability_file_path(paths, broker_key)).is_ok() {
            anyhow::bail!("failed to remove legacy runtime broker capability");
        }
        return Ok(None);
    }
    let current = load_json_file_with_backup::<RuntimeBrokerRegistry>(&path, &backup_path);
    match current {
        Ok(loaded) => Ok(Some(loaded.value)),
        Err(_err) if !path.exists() && !backup_path.exists() => Ok(None),
        Err(err) => Err(err),
    }
}

fn runtime_broker_registry_has_legacy_secrets(path: &Path) -> bool {
    let Some(bytes) = read_runtime_broker_registry_bytes(path) else {
        return false;
    };
    prodex_runtime_broker::runtime_broker_registry_contains_legacy_secrets(bytes)
}

fn runtime_broker_registry_file_is_current(path: &Path) -> bool {
    read_runtime_broker_registry_bytes(path)
        .and_then(|bytes| serde_json::from_slice::<RuntimeBrokerRegistry>(&bytes).ok())
        .is_some()
}

fn read_runtime_broker_registry_bytes(path: &Path) -> Option<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > RUNTIME_BROKER_REGISTRY_MAX_BYTES {
        return None;
    }
    let file = fs::File::open(path).ok()?;
    if !runtime_broker_same_file_metadata(&metadata, &file.metadata().ok()?) {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(RUNTIME_BROKER_REGISTRY_MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= RUNTIME_BROKER_REGISTRY_MAX_BYTES).then_some(bytes)
}

#[cfg(unix)]
fn runtime_broker_same_file_metadata(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    before.dev() == after.dev() && before.ino() == after.ino()
}

#[cfg(not(unix))]
fn runtime_broker_same_file_metadata(_before: &fs::Metadata, _after: &fs::Metadata) -> bool {
    true
}

pub(crate) fn save_runtime_broker_registry(
    paths: &AppPaths,
    broker_key: &str,
    registry: &RuntimeBrokerRegistry,
) -> Result<()> {
    let path = runtime_broker_registry_file_path(paths, broker_key);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(registry)
        .context("failed to serialize runtime broker registry")?;
    write_json_file_with_backup(
        &path,
        &runtime_broker_registry_last_good_file_path(paths, broker_key),
        &json,
        |content| {
            let _: RuntimeBrokerRegistry = serde_json::from_str(content)
                .context("failed to validate runtime broker registry")?;
            Ok(())
        },
    )
}

pub(crate) fn save_runtime_broker_capability(
    paths: &AppPaths,
    broker_key: &str,
    instance_id: &str,
    capability: &RuntimeBrokerSecret,
) -> Result<()> {
    let path = runtime_broker_capability_file_path(paths, broker_key);
    let location = SecretLocation::file(&path);
    let mut payload = Zeroizing::new(Vec::new());
    prodex_runtime_broker::write_runtime_broker_capability(&mut *payload, instance_id, capability)
        .context("failed to encode runtime broker capability")?;
    FileSecretBackend::new()
        .write_bounded(
            &location,
            SecretValue::bytes(std::mem::take(&mut *payload)),
            RUNTIME_BROKER_CAPABILITY_MAX_BYTES,
        )
        .with_context(|| format!("failed to write {}", path.display()))
}

pub(crate) fn load_runtime_broker_capability(
    paths: &AppPaths,
    broker_key: &str,
    expected_instance_id: &str,
) -> Result<RuntimeBrokerSecret> {
    let capability = load_runtime_broker_capability_record(paths, broker_key)?;
    if capability.instance_id != expected_instance_id {
        anyhow::bail!("runtime broker capability belongs to another instance");
    }
    Ok(capability.admin_token)
}

fn load_runtime_broker_capability_record(
    paths: &AppPaths,
    broker_key: &str,
) -> Result<RuntimeBrokerCapability> {
    let path = runtime_broker_capability_file_path(paths, broker_key);
    let value = FileSecretBackend::new()
        .read_bounded(
            &SecretLocation::file(&path),
            RUNTIME_BROKER_CAPABILITY_MAX_BYTES,
        )
        .with_context(|| format!("failed to read {}", path.display()))?
        .context("runtime broker capability is missing")?;
    value
        .with_bytes(|bytes| {
            prodex_runtime_broker::read_runtime_broker_capability(Cursor::new(bytes))
        })
        .context("runtime broker capability is invalid")
}

pub(crate) fn remove_runtime_broker_registry_if_instance_matches(
    paths: &AppPaths,
    broker_key: &str,
    instance_id: &str,
) {
    let Ok(Some(existing)) = load_runtime_broker_registry(paths, broker_key) else {
        return;
    };
    if existing.instance_id != instance_id {
        return;
    }
    remove_runtime_broker_registry_files(paths, broker_key);
    remove_runtime_broker_capability_if_instance_matches(paths, broker_key, instance_id);
}

pub(crate) fn remove_runtime_broker_capability_if_instance_matches(
    paths: &AppPaths,
    broker_key: &str,
    expected_instance_id: &str,
) {
    match load_runtime_broker_capability_record(paths, broker_key) {
        Ok(existing) if existing.instance_id == expected_instance_id => {
            remove_runtime_broker_capability(paths, broker_key);
        }
        Ok(_) => {}
        Err(_) => remove_runtime_broker_capability(paths, broker_key),
    }
}

pub(crate) fn remove_runtime_broker_capability_if_matches(
    paths: &AppPaths,
    broker_key: &str,
    expected_instance_id: &str,
    expected: &RuntimeBrokerSecret,
) {
    let Ok(existing) = load_runtime_broker_capability_record(paths, broker_key) else {
        return;
    };
    if existing.instance_id == expected_instance_id
        && existing.admin_token.matches(expected.expose())
    {
        remove_runtime_broker_capability(paths, broker_key);
    }
}

pub(crate) fn remove_runtime_broker_capability(paths: &AppPaths, broker_key: &str) {
    let path = runtime_broker_capability_file_path(paths, broker_key);
    let location = SecretLocation::file(&path);
    let backend = FileSecretBackend::new();
    if backend.delete(&location).is_err() {
        let _ = backend.remove_untrusted_entry(&location);
    }
}

fn remove_runtime_broker_registry_files(paths: &AppPaths, broker_key: &str) {
    for path in [
        runtime_broker_registry_file_path(paths, broker_key),
        runtime_broker_registry_last_good_file_path(paths, broker_key),
    ] {
        let _ = fs::remove_file(path);
    }
}

fn remove_runtime_broker_registry_files_checked(paths: &AppPaths, broker_key: &str) -> Result<()> {
    for path in [
        runtime_broker_registry_file_path(paths, broker_key),
        runtime_broker_registry_last_good_file_path(paths, broker_key),
    ] {
        remove_runtime_broker_file_checked(&path)?;
    }
    Ok(())
}

fn remove_runtime_broker_file_checked(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("failed to remove {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RUNTIME_PROXY_OPENAI_MOUNT_PATH;
    use prodex_runtime_broker::RuntimeBrokerRegistry;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_paths(label: &str) -> AppPaths {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "prodex-broker-capability-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        }
        AppPaths {
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("shared"),
            legacy_shared_codex_root: root.join("legacy-shared"),
            root,
        }
    }

    fn test_registry(instance_id: &str) -> RuntimeBrokerRegistry {
        RuntimeBrokerRegistry {
            pid: std::process::id(),
            listen_addr: "127.0.0.1:4567".to_string(),
            started_at: 100,
            upstream_base_url: "https://upstream.example".to_string(),
            include_code_review: false,
            upstream_no_proxy: false,
            smart_context_enabled: false,
            current_profile: "main".to_string(),
            instance_id: instance_id.to_string(),
            prodex_version: None,
            executable_path: None,
            executable_sha256: None,
            openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
        }
    }

    #[test]
    fn capability_is_private_unbacked_and_absent_from_registry_files() {
        let paths = test_paths("private");
        let broker_key = "broker";
        let secret = RuntimeBrokerSecret::new("raw-admin-capability").unwrap();
        let registry = test_registry("public-instance-id");

        save_runtime_broker_capability(&paths, broker_key, "public-instance-id", &secret).unwrap();
        save_runtime_broker_registry(&paths, broker_key, &registry).unwrap();
        save_runtime_broker_registry(&paths, broker_key, &registry).unwrap();

        let capability_path = runtime_broker_capability_file_path(&paths, broker_key);
        let capability_body = fs::read_to_string(&capability_path).unwrap();
        assert!(capability_body.contains(secret.expose()));
        assert!(capability_body.contains("public-instance-id"));
        let loaded =
            load_runtime_broker_capability(&paths, broker_key, "public-instance-id").unwrap();
        assert!(loaded.matches(secret.expose()));
        assert!(
            !capability_path
                .with_extension("capability.last-good")
                .exists()
        );
        assert_eq!(
            fs::read_dir(&paths.root)
                .unwrap()
                .flatten()
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with("runtime-broker-broker.capability")
                })
                .count(),
            1
        );
        for path in [
            runtime_broker_registry_file_path(&paths, broker_key),
            runtime_broker_registry_last_good_file_path(&paths, broker_key),
        ] {
            let body = fs::read_to_string(path).unwrap();
            assert!(!body.contains(secret.expose()));
            assert!(!body.contains("admin_token"));
            assert!(!body.contains("instance_token"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&capability_path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        assert!(!format!("{secret:?}").contains(secret.expose()));

        let _ = fs::remove_dir_all(paths.root);
    }

    #[test]
    fn capability_rotation_and_instance_scoped_removal_are_safe() {
        let paths = test_paths("rotate");
        let broker_key = "broker";
        let old = RuntimeBrokerSecret::new("old-admin-capability").unwrap();
        let new = RuntimeBrokerSecret::new("new-admin-capability").unwrap();
        let registry = test_registry("current-instance");

        save_runtime_broker_registry(&paths, broker_key, &registry).unwrap();
        save_runtime_broker_capability(&paths, broker_key, "current-instance", &old).unwrap();
        assert!(
            load_runtime_broker_capability(&paths, broker_key, "current-instance")
                .unwrap()
                .matches(old.expose())
        );

        save_runtime_broker_capability(&paths, broker_key, "current-instance", &new).unwrap();
        let loaded =
            load_runtime_broker_capability(&paths, broker_key, "current-instance").unwrap();
        assert!(loaded.matches(new.expose()));
        assert!(!loaded.matches(old.expose()));

        remove_runtime_broker_registry_if_instance_matches(&paths, broker_key, "other-instance");
        assert!(runtime_broker_capability_file_path(&paths, broker_key).exists());
        remove_runtime_broker_registry_if_instance_matches(&paths, broker_key, "current-instance");
        assert!(!runtime_broker_capability_file_path(&paths, broker_key).exists());
        assert!(!runtime_broker_registry_file_path(&paths, broker_key).exists());
        assert!(!runtime_broker_registry_last_good_file_path(&paths, broker_key).exists());

        let _ = fs::remove_dir_all(paths.root);
    }

    #[test]
    fn capability_value_scoped_removal_preserves_rotated_secrets() {
        let paths = test_paths("value-remove");
        let broker_key = "broker";
        let current = RuntimeBrokerSecret::new("current-admin-capability").unwrap();
        let stale = RuntimeBrokerSecret::new("stale-admin-capability").unwrap();
        save_runtime_broker_capability(&paths, broker_key, "current-instance", &current).unwrap();

        remove_runtime_broker_capability_if_matches(&paths, broker_key, "current-instance", &stale);
        assert!(runtime_broker_capability_file_path(&paths, broker_key).exists());
        remove_runtime_broker_capability_if_matches(&paths, broker_key, "other-instance", &current);
        assert!(runtime_broker_capability_file_path(&paths, broker_key).exists());
        remove_runtime_broker_capability_if_matches(
            &paths,
            broker_key,
            "current-instance",
            &current,
        );
        assert!(!runtime_broker_capability_file_path(&paths, broker_key).exists());

        let _ = fs::remove_dir_all(paths.root);
    }

    #[test]
    fn old_instance_cleanup_preserves_a_new_capability_generation() {
        let paths = test_paths("generation-race");
        let broker_key = "broker";
        let capability = RuntimeBrokerSecret::new("new-admin-capability").unwrap();
        save_runtime_broker_registry(&paths, broker_key, &test_registry("old-instance")).unwrap();
        save_runtime_broker_capability(&paths, broker_key, "new-instance", &capability).unwrap();

        remove_runtime_broker_registry_if_instance_matches(&paths, broker_key, "old-instance");

        assert!(!runtime_broker_registry_file_path(&paths, broker_key).exists());
        assert!(
            load_runtime_broker_capability(&paths, broker_key, "new-instance")
                .unwrap()
                .matches(capability.expose())
        );

        let _ = fs::remove_dir_all(paths.root);
    }

    #[test]
    fn legacy_registry_secrets_are_removed_instead_of_reused() {
        let paths = test_paths("legacy");
        let broker_key = "broker";
        fs::create_dir_all(&paths.root).unwrap();
        let legacy = r#"{
            "pid": 999999999,
            "listen_addr": "127.0.0.1:4567",
            "started_at": 100,
            "upstream_base_url": "https://upstream.example",
            "include_code_review": false,
            "current_profile": "main",
            "instance_token": "legacy-instance-secret",
            "admin_token": "legacy-admin-secret"
        }"#;
        let registry_path = runtime_broker_registry_file_path(&paths, broker_key);
        let backup_path = runtime_broker_registry_last_good_file_path(&paths, broker_key);
        fs::write(&registry_path, legacy).unwrap();
        fs::write(&backup_path, legacy).unwrap();

        assert!(
            load_runtime_broker_registry(&paths, broker_key)
                .unwrap()
                .is_none()
        );
        assert!(!registry_path.exists());
        assert!(!backup_path.exists());
        assert!(!runtime_broker_capability_file_path(&paths, broker_key).exists());

        let _ = fs::remove_dir_all(paths.root);
    }

    #[test]
    fn current_registry_survives_a_legacy_backup_cleanup() {
        let paths = test_paths("legacy-backup");
        let broker_key = "broker";
        fs::create_dir_all(&paths.root).unwrap();
        let registry_path = runtime_broker_registry_file_path(&paths, broker_key);
        let backup_path = runtime_broker_registry_last_good_file_path(&paths, broker_key);
        fs::write(
            &registry_path,
            serde_json::to_vec(&test_registry("current-instance")).unwrap(),
        )
        .unwrap();
        fs::write(
            &backup_path,
            r#"{"pid":999999999,"instance_token":"legacy-instance","admin_token":"legacy-admin"}"#,
        )
        .unwrap();

        let loaded = load_runtime_broker_registry(&paths, broker_key)
            .unwrap()
            .unwrap();

        assert_eq!(loaded.instance_id, "current-instance");
        assert!(registry_path.exists());
        assert!(!backup_path.exists());

        let _ = fs::remove_dir_all(paths.root);
    }

    #[cfg(unix)]
    #[test]
    fn capability_loader_rejects_symlinks_and_broad_permissions() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let paths = test_paths("validation");
        fs::create_dir_all(&paths.root).unwrap();
        let capability_path = runtime_broker_capability_file_path(&paths, "broker");
        let target = paths.root.join("outside-secret");
        fs::write(&target, "secret").unwrap();
        symlink(&target, &capability_path).unwrap();
        assert!(load_runtime_broker_capability(&paths, "broker", "instance").is_err());
        remove_runtime_broker_capability(&paths, "broker");
        assert!(!capability_path.exists());
        assert_eq!(fs::read_to_string(&target).unwrap(), "secret");

        let capability = RuntimeBrokerSecret::new("secret").unwrap();
        save_runtime_broker_capability(&paths, "broker", "instance", &capability).unwrap();
        fs::set_permissions(&capability_path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(load_runtime_broker_capability(&paths, "broker", "instance").is_err());

        let _ = fs::remove_dir_all(paths.root);
    }

    #[cfg(unix)]
    #[test]
    fn capability_writer_replaces_only_the_final_symlink() {
        use std::os::unix::fs::symlink;

        let paths = test_paths("replace-final-symlink");
        let capability_path = runtime_broker_capability_file_path(&paths, "broker");
        let target = paths.root.join("outside-secret");
        fs::write(&target, "outside").unwrap();
        symlink(&target, &capability_path).unwrap();
        let capability = RuntimeBrokerSecret::new("inside-capability").unwrap();

        save_runtime_broker_capability(&paths, "broker", "instance", &capability).unwrap();

        assert!(!capability_path.is_symlink());
        assert_eq!(fs::read_to_string(&target).unwrap(), "outside");
        assert!(
            load_runtime_broker_capability(&paths, "broker", "instance")
                .unwrap()
                .matches(capability.expose())
        );

        let _ = fs::remove_dir_all(paths.root);
    }

    #[cfg(unix)]
    #[test]
    fn capability_writer_rejects_a_symlinked_parent() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};

        let paths = test_paths("reject-parent-symlink");
        fs::remove_dir_all(&paths.root).unwrap();
        let outside = paths.root.with_extension("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(outside.join("marker"), "outside").unwrap();
        symlink(&outside, &paths.root).unwrap();
        let capability = RuntimeBrokerSecret::new("inside-capability").unwrap();

        assert!(save_runtime_broker_capability(&paths, "broker", "instance", &capability).is_err());
        assert_eq!(
            fs::read_to_string(outside.join("marker")).unwrap(),
            "outside"
        );
        assert!(!outside.join("runtime-broker-broker.capability").exists());

        fs::remove_file(&paths.root).unwrap();
        let _ = fs::remove_dir_all(outside);
    }
}
