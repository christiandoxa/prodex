use super::{
    acquire_update_check_lock, cached_latest_prodex_version, current_prodex_release_source,
    fetch_latest_prodex_version, load_update_check_cache, parse_release_version,
    save_update_check_cache,
};
use anyhow::{Context, Result};
use chrono::Local;
use prodex_core::AppPaths;
use std::cmp::Ordering as VersionOrdering;
use std::fs::{self, OpenOptions};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProdexUpdateDecision {
    UpToDate,
    UpdateAvailable(String),
    LocalNewer(String),
}

pub fn latest_prodex_version_for_update(paths: &AppPaths) -> Result<String> {
    let source = current_prodex_release_source();
    if let Some(latest_version) = cached_latest_prodex_version(paths, source) {
        return Ok(latest_version);
    }

    let _lock = acquire_update_check_lock(paths);
    if let Some(latest_version) = cached_latest_prodex_version(paths, source) {
        return Ok(latest_version);
    }

    let latest_version = fetch_latest_prodex_version(source)
        .context("failed to resolve the latest Prodex release before updating")?;
    let mut cache = load_update_check_cache(paths)
        .ok()
        .flatten()
        .unwrap_or_default();
    cache.source = source;
    cache.latest_version = latest_version.clone();
    cache.checked_at = Local::now().timestamp();
    let _ = save_update_check_cache(paths, &cache);
    Ok(latest_version)
}

pub fn prodex_update_decision(
    current_version: &str,
    target_version: &str,
) -> Result<ProdexUpdateDecision> {
    let current = parse_release_version(current_version)
        .with_context(|| format!("invalid installed Prodex version: {current_version}"))?;
    let target = parse_release_version(target_version)
        .with_context(|| format!("invalid target Prodex version: {target_version}"))?;
    Ok(match current.cmp_precedence(&target) {
        VersionOrdering::Less => ProdexUpdateDecision::UpdateAvailable(target_version.to_string()),
        VersionOrdering::Equal => ProdexUpdateDecision::UpToDate,
        VersionOrdering::Greater => ProdexUpdateDecision::LocalNewer(target_version.to_string()),
    })
}

pub fn acquire_prodex_update_lock(paths: &AppPaths) -> Result<fs::File> {
    fs::create_dir_all(&paths.root).context("failed to create Prodex update state directory")?;
    let path = paths.root.join("update-install.lock");

    #[cfg(unix)]
    let file = {
        use std::os::unix::fs::OpenOptionsExt;

        OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)
            .with_context(|| format!("failed to open {}", path.display()))?
    };

    #[cfg(not(unix))]
    let file = {
        if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            anyhow::bail!("refusing Prodex update lock symlink");
        }
        OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)
            .with_context(|| format!("failed to open {}", path.display()))?
    };

    file.lock()
        .context("failed to acquire Prodex update lock")?;
    Ok(file)
}
