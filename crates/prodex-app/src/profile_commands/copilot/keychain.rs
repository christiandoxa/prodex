use anyhow::{Context, Result, bail};
use dirs::home_dir;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::PathBuf;
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
            if !path.is_dir() {
                continue;
            }
            let keytar_path = path.join(&keytar_suffix);
            if !keytar_path.is_file() {
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
