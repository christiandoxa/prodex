use anyhow::{Context, Result, bail};
use dirs::home_dir;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::absolutize;
use prodex_profile_export::{copilot_platform_label, parse_copilot_version};

use super::COPILOT_KEYCHAIN_SERVICE;

pub(super) fn read_copilot_keychain_token(account_key: &str) -> Result<Option<String>> {
    let keytar_path = discover_copilot_keytar_path()?;
    let node_script = r#"
const keytar = require(process.argv[1]);
keytar.getPassword(process.argv[2], process.argv[3]).then(
  token => process.stdout.write(token || ''),
  err => { console.error(String(err)); process.exit(1); }
);
"#;
    let output = Command::new("node")
        .arg("-e")
        .arg(node_script)
        .arg(&keytar_path)
        .arg(COPILOT_KEYCHAIN_SERVICE)
        .arg(account_key)
        .output()
        .with_context(|| format!("failed to execute node for {}", keytar_path.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            bail!("node keychain lookup failed for {}", keytar_path.display());
        }
        bail!(
            "node keychain lookup failed for {}: {}",
            keytar_path.display(),
            stderr
        );
    }
    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok((!token.is_empty()).then_some(token))
}

/// Read a Copilot OAuth token from GNOME keyring via `secret-tool` (libsecret).
///
/// Copilot CLI v1.0.65+ stores OAuth tokens through the `rust-keyring` crate,
/// which writes into the system keyring (GNOME keyring on Linux via libsecret).
pub(super) fn read_copilot_libsecret_token(account_key: &str) -> Result<Option<String>> {
    match Command::new("secret-tool")
        .arg("lookup")
        .arg("service")
        .arg(COPILOT_KEYCHAIN_SERVICE)
        .arg("username")
        .arg(account_key)
        .output()
    {
        Ok(output) if output.status.success() => {
            let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !token.is_empty() {
                return Ok(Some(token));
            }
        }
        _ => {}
    }

    // Copilot CLI ≤1.0.31 stored tokens with the `account` attribute
    // (keytar getPassword(service, account)).  Later versions use
    // `username` (rust-keyring).  Try both so old entries are not missed.
    match Command::new("secret-tool")
        .arg("lookup")
        .arg("service")
        .arg(COPILOT_KEYCHAIN_SERVICE)
        .arg("account")
        .arg(account_key)
        .output()
    {
        Ok(output) if output.status.success() => {
            let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !token.is_empty() {
                return Ok(Some(token));
            }
        }
        _ => {}
    }

    Ok(None)
}

fn discover_copilot_keytar_path() -> Result<PathBuf> {
    let mut candidates = Vec::new();
    let keytar_suffix = PathBuf::from("prebuilds")
        .join(copilot_platform_label())
        .join("keytar.node");
    for root in copilot_package_roots()? {
        if !root.exists() {
            continue;
        }
        for entry in
            fs::read_dir(&root).with_context(|| format!("failed to read {}", root.display()))?
        {
            let entry = entry.with_context(|| format!("failed to read {}", root.display()))?;
            let path = entry.path();
            if !copilot_path_is_regular_dir(&path) {
                continue;
            }
            let keytar_path = path.join(&keytar_suffix);
            if !copilot_keytar_path_is_regular_file(&path, &keytar_suffix) {
                continue;
            }
            let version = path
                .file_name()
                .and_then(|value| value.to_str())
                .map(parse_copilot_version)
                .unwrap_or((0, 0, 0));
            candidates.push((version, keytar_path));
        }
    }

    candidates.sort_by_key(|(version, _)| *version);
    candidates
        .pop()
        .map(|(_, path)| path)
        .context("failed to locate the Copilot CLI keychain helper")
}

fn copilot_path_is_regular_dir(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| !metadata.file_type().is_symlink() && metadata.is_dir())
        .unwrap_or(false)
}

fn copilot_keytar_path_is_regular_file(version_dir: &Path, keytar_suffix: &Path) -> bool {
    let mut current = version_dir.to_path_buf();
    for component in keytar_suffix.components() {
        current.push(component.as_os_str());
        if fs::symlink_metadata(&current)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return false;
        }
    }
    fs::symlink_metadata(version_dir.join(keytar_suffix))
        .map(|metadata| !metadata.file_type().is_symlink() && metadata.is_file())
        .unwrap_or(false)
}

fn copilot_package_roots() -> Result<Vec<PathBuf>> {
    let mut roots = BTreeSet::new();
    let platform = copilot_platform_label();

    if let Some(path) = env::var_os("COPILOT_CACHE_HOME") {
        roots.insert(absolutize(PathBuf::from(path))?.join("pkg").join(platform));
    }

    let cache_home = env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|home| home.join(".cache")))
        .context("failed to determine cache directory")?;
    roots.insert(cache_home.join("copilot").join("pkg").join(platform));

    if let Some(path) = env::var_os("COPILOT_HOME") {
        roots.insert(absolutize(PathBuf::from(path))?.join("pkg").join(platform));
    }
    if let Some(home) = home_dir() {
        roots.insert(home.join(".copilot").join("pkg").join(platform));
    }

    Ok(roots.into_iter().collect())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!(
            "prodex-copilot-keychain-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn isolate_copilot_env(root: &Path) -> Vec<crate::TestEnvVarGuard> {
        vec![
            crate::TestEnvVarGuard::set("HOME", root.join("home").to_str().unwrap()),
            crate::TestEnvVarGuard::set("XDG_CACHE_HOME", root.join("xdg-cache").to_str().unwrap()),
            crate::TestEnvVarGuard::set(
                "COPILOT_CACHE_HOME",
                root.join("copilot-cache").to_str().unwrap(),
            ),
            crate::TestEnvVarGuard::set(
                "COPILOT_HOME",
                root.join("copilot-home").to_str().unwrap(),
            ),
        ]
    }

    #[test]
    fn copilot_keytar_discovery_rejects_symlink_version_dir() {
        let _lock = crate::TestEnvVarGuard::lock();
        let root = temp_dir("symlink-version-dir");
        let _env = isolate_copilot_env(&root);
        let package_root = root
            .join("copilot-cache")
            .join("pkg")
            .join(copilot_platform_label());
        let outside_version = root.join("outside-version");
        fs::create_dir_all(
            outside_version
                .join("prebuilds")
                .join(copilot_platform_label()),
        )
        .unwrap();
        fs::write(
            outside_version
                .join("prebuilds")
                .join(copilot_platform_label())
                .join("keytar.node"),
            "",
        )
        .unwrap();
        fs::create_dir_all(&package_root).unwrap();
        std::os::unix::fs::symlink(&outside_version, package_root.join("99.0.0")).unwrap();

        assert!(discover_copilot_keytar_path().is_err());
    }

    #[test]
    fn copilot_keytar_discovery_rejects_symlink_prebuild_component() {
        let _lock = crate::TestEnvVarGuard::lock();
        let root = temp_dir("symlink-prebuild");
        let _env = isolate_copilot_env(&root);
        let version_dir = root
            .join("copilot-cache")
            .join("pkg")
            .join(copilot_platform_label())
            .join("99.0.0");
        let outside_prebuilds = root.join("outside-prebuilds");
        fs::create_dir_all(outside_prebuilds.join(copilot_platform_label())).unwrap();
        fs::write(
            outside_prebuilds
                .join(copilot_platform_label())
                .join("keytar.node"),
            "",
        )
        .unwrap();
        fs::create_dir_all(&version_dir).unwrap();
        std::os::unix::fs::symlink(&outside_prebuilds, version_dir.join("prebuilds")).unwrap();

        assert!(discover_copilot_keytar_path().is_err());
    }
}
