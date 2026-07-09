use anyhow::{Context, Result};
use base64::Engine;
use std::fs;
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
    let path = lease_dir.join(format!("{}-{}.lease", pid, runtime_random_token("lease")?));
    fs::write(&path, format!("pid={pid}\n"))
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(RuntimeBrokerLease { path })
}

pub(crate) fn cleanup_runtime_broker_stale_leases(paths: &AppPaths, broker_key: &str) -> usize {
    let lease_dir = runtime_broker_lease_dir(paths, broker_key);
    let Ok(entries) = fs::read_dir(&lease_dir) else {
        return 0;
    };
    let mut live = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
