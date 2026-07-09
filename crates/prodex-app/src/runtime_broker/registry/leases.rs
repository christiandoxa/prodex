use anyhow::{Context, Result};
use base64::Engine;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use crate::{AppPaths, RuntimeBrokerLease, runtime_broker_lease_dir, runtime_process_pid_alive};

pub(crate) fn runtime_random_token(prefix: &str) -> Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).context("failed to generate runtime token")?;
    Ok(format!(
        "{prefix}-{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    ))
}

pub(crate) fn create_runtime_broker_lease(
    paths: &AppPaths,
    broker_key: &str,
) -> Result<RuntimeBrokerLease> {
    let lease_dir = runtime_broker_lease_dir(paths, broker_key);
    create_runtime_broker_lease_in_dir_for_pid(&lease_dir, std::process::id())
}

pub(crate) fn create_runtime_broker_lease_in_dir_for_pid(
    lease_dir: &Path,
    pid: u32,
) -> Result<RuntimeBrokerLease> {
    fs::create_dir_all(lease_dir)
        .with_context(|| format!("failed to create {}", lease_dir.display()))?;
    anyhow::ensure!(
        runtime_broker_lease_dir_is_regular_dir(lease_dir),
        "refusing to use symlinked runtime broker lease dir {}",
        lease_dir.display()
    );
    let path = lease_dir.join(format!("{}-{}.lease", pid, runtime_random_token("lease")?));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&path)
        .with_context(|| format!("failed to write {}", path.display()))?;
    file.write_all(format!("pid={pid}\n").as_bytes())
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(RuntimeBrokerLease { path })
}

pub(crate) fn cleanup_runtime_broker_stale_leases(paths: &AppPaths, broker_key: &str) -> usize {
    let lease_dir = runtime_broker_lease_dir(paths, broker_key);
    if !runtime_broker_lease_dir_is_regular_dir(&lease_dir) {
        return 0;
    }
    let Ok(entries) = fs::read_dir(&lease_dir) else {
        return 0;
    };
    let mut live = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !runtime_broker_lease_path_is_regular_file(&path) {
            continue;
        }
        let pid = file_name
            .split('-')
            .next()
            .and_then(|value| value.parse::<u32>().ok());
        if pid.is_some_and(runtime_process_pid_alive) {
            live += 1;
        } else {
            let _ = fs::remove_file(path);
        }
    }
    live
}

pub(crate) fn runtime_broker_lease_dir_is_regular_dir(lease_dir: &Path) -> bool {
    fs::symlink_metadata(lease_dir)
        .map(|metadata| !metadata.file_type().is_symlink() && metadata.is_dir())
        .unwrap_or(false)
}

pub(crate) fn runtime_broker_lease_path_is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| !metadata.file_type().is_symlink() && metadata.is_file())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn runtime_random_token_uses_url_safe_random_payload() {
        let first = runtime_random_token("admin").expect("token should generate");
        let second = runtime_random_token("admin").expect("token should generate");

        assert_ne!(first, second);
        assert!(first.starts_with("admin-"));
        assert!(second.starts_with("admin-"));
        let payload = first.strip_prefix("admin-").unwrap();
        assert_eq!(payload.len(), 43);
        assert!(
            payload
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        );
    }

    #[cfg(unix)]
    #[test]
    fn runtime_broker_lease_create_rejects_symlink_dir() {
        let root = std::env::temp_dir().join(format!(
            "prodex-broker-lease-create-symlink-{}",
            std::process::id()
        ));
        let lease_dir = root.join("leases");
        let outside = root.join("outside");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, &lease_dir).unwrap();

        let err = create_runtime_broker_lease_in_dir_for_pid(&lease_dir, 12345)
            .expect_err("symlink lease dir should be rejected");

        assert!(
            err.to_string()
                .contains("symlinked runtime broker lease dir")
        );
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn runtime_broker_stale_lease_cleanup_skips_symlink_dir() {
        let root = std::env::temp_dir().join(format!(
            "prodex-broker-lease-cleanup-symlink-{}",
            std::process::id()
        ));
        let prodex_root = root.join("prodex");
        let outside = root.join("outside");
        let paths = test_paths(prodex_root.clone());
        let lease_dir = runtime_broker_lease_dir(&paths, "key");
        let outside_lease = outside.join("999999-lease-stale.lease");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&prodex_root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(&outside_lease, "pid=999999\n").unwrap();
        std::os::unix::fs::symlink(&outside, &lease_dir).unwrap();

        let live = cleanup_runtime_broker_stale_leases(&paths, "key");

        assert_eq!(live, 0);
        assert_eq!(fs::read_to_string(&outside_lease).unwrap(), "pid=999999\n");
        fs::remove_dir_all(root).unwrap();
    }

    fn test_paths(root: PathBuf) -> AppPaths {
        AppPaths {
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("shared-codex-home"),
            legacy_shared_codex_root: root.join("shared"),
            root,
        }
    }
}
