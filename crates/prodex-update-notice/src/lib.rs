use anyhow::{Context, Result, bail};
use chrono::Local;
use prodex_cli::Commands;
use prodex_core::AppPaths;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::time::Duration;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};
use terminal_ui::{print_wrapped_stderr, section_header};

mod release_version;
mod updater;

use release_version::{
    is_update_notice_mojo_error, map_update_notice_mojo, release_version_is_valid,
};

pub use release_version::version_is_newer;
pub use updater::{
    ProdexUpdateDecision, acquire_prodex_update_lock, latest_prodex_version_for_update,
    prodex_update_decision,
};

pub const UPDATE_CHECK_CACHE_TTL_SECONDS: i64 = 300;
pub const UPDATE_CHECK_STALE_CURRENT_TTL_SECONDS: i64 = 300;
const UPDATE_CHECK_HTTP_CONNECT_TIMEOUT_MS: u64 = if cfg!(test) { 200 } else { 800 };
const UPDATE_CHECK_HTTP_READ_TIMEOUT_MS: u64 = if cfg!(test) { 400 } else { 1200 };
const UPDATE_CHECK_CACHE_MAX_BYTES: u64 = 64 * 1024;
static UPDATE_CHECK_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct UpdateCheckCache {
    #[serde(default)]
    source: ProdexReleaseSource,
    latest_version: String,
    checked_at: i64,
    #[serde(default)]
    codex_latest_version: Option<String>,
    #[serde(default)]
    codex_checked_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProdexReleaseSource {
    #[default]
    GitHub,
    Npm,
    CratesIo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProdexInstallChannel {
    Standalone,
    Npm,
    Cargo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProdexVersionStatus {
    UpToDate,
    UpdateAvailable(String),
    Unknown,
}

pub fn show_update_notice_if_available(command: &Commands) -> Result<()> {
    if !should_emit_update_notice(command) {
        return Ok(());
    }

    let paths = AppPaths::discover()?;
    if let ProdexVersionStatus::UpdateAvailable(latest_version) = prodex_version_status(&paths)? {
        print_wrapped_stderr(&section_header("Update Available"))?;
        print_wrapped_stderr(&format!(
            "A newer prodex release is available: {} -> {}",
            current_prodex_version(),
            latest_version
        ))?;
        print_wrapped_stderr(&format!(
            "Update with: {}",
            prodex_update_command_for_version(&latest_version)
        ))?;
        if let Some(warning) = current_prodex_install_warning() {
            print_wrapped_stderr(&format!("WARNING: {warning}"))?;
        }
    }

    Ok(())
}

pub fn should_emit_update_notice(command: &Commands) -> bool {
    use prodex_mojo_core::update_notice_policy::UpdateNoticeCommandClass;

    let (class, doctor_json, doctor_bundle_present, quota_raw) = match command {
        Commands::Doctor(args) => (
            UpdateNoticeCommandClass::Doctor,
            args.json,
            args.bundle.is_some(),
            false,
        ),
        Commands::Update(_) => (UpdateNoticeCommandClass::Update, false, false, false),
        Commands::Quota(args) => (UpdateNoticeCommandClass::Quota, false, false, args.raw),
        _ => (UpdateNoticeCommandClass::Other, false, false, false),
    };
    prodex_mojo_core::update_notice_policy::should_emit_notice(
        class,
        doctor_json,
        doctor_bundle_present,
        quota_raw,
    )
    .expect("Mojo update-notice command policy returned invalid output")
}

pub fn current_prodex_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn current_prodex_release_source() -> ProdexReleaseSource {
    ProdexReleaseSource::GitHub
}

pub fn current_prodex_install_channel() -> ProdexInstallChannel {
    prodex_install_channel(
        env::var("npm_package_name").ok().as_deref(),
        env::current_exe().ok().as_deref(),
    )
}

pub fn prodex_install_channel(
    npm_package_name: Option<&str>,
    executable_path: Option<&Path>,
) -> ProdexInstallChannel {
    use prodex_mojo_core::update_notice_policy::UpdateInstallChannelClass;

    let executable_path = executable_path.map(Path::to_string_lossy);
    match prodex_mojo_core::update_notice_policy::install_channel(
        npm_package_name,
        executable_path.as_deref(),
    )
    .expect("Mojo update-notice install-channel policy returned invalid output")
    {
        UpdateInstallChannelClass::Standalone => ProdexInstallChannel::Standalone,
        UpdateInstallChannelClass::Npm => ProdexInstallChannel::Npm,
        UpdateInstallChannelClass::Cargo => ProdexInstallChannel::Cargo,
    }
}

pub fn current_prodex_install_warning() -> Option<&'static str> {
    prodex_install_warning(current_prodex_install_channel())
}

pub fn prodex_install_warning(channel: ProdexInstallChannel) -> Option<&'static str> {
    match channel {
        ProdexInstallChannel::Npm => Some(
            "npm installations are no longer supported; run `prodex update` to migrate to standalone.",
        ),
        ProdexInstallChannel::Cargo => Some(
            "Cargo installations are no longer supported; run `prodex update` to migrate to standalone.",
        ),
        ProdexInstallChannel::Standalone => None,
    }
}

pub fn prodex_update_command_for_version(_latest_version: &str) -> String {
    "prodex update".to_string()
}

pub fn prodex_version_status(paths: &AppPaths) -> Result<ProdexVersionStatus> {
    use prodex_mojo_core::update_notice_policy::UpdateVersionStatusClass;

    let latest_version = latest_prodex_version(paths)?;
    let status = map_update_notice_mojo(prodex_mojo_core::update_notice_policy::version_status(
        latest_version.as_deref(),
        current_prodex_version(),
    ))?;
    Ok(match (status, latest_version) {
        (UpdateVersionStatusClass::Unknown, _) => ProdexVersionStatus::Unknown,
        (UpdateVersionStatusClass::UpToDate, _) => ProdexVersionStatus::UpToDate,
        (UpdateVersionStatusClass::UpdateAvailable, Some(latest_version)) => {
            ProdexVersionStatus::UpdateAvailable(latest_version)
        }
        (UpdateVersionStatusClass::UpdateAvailable, None) => {
            bail!("Mojo reported an update without a latest version")
        }
    })
}

pub fn format_info_codex_version(
    paths: &AppPaths,
    current_version: Option<&str>,
) -> Result<String> {
    let latest_version = latest_codex_version(paths, current_version.unwrap_or("0.0.0"))?;
    map_update_notice_mojo(
        prodex_mojo_core::update_notice_policy::format_codex_version(
            current_version,
            latest_version.as_deref(),
        ),
    )
}

fn latest_prodex_version(paths: &AppPaths) -> Result<Option<String>> {
    let source = current_prodex_release_source();
    if let Some(latest_version) = cached_latest_prodex_version(paths, source)? {
        return Ok(Some(latest_version));
    }

    let _lock = acquire_update_check_lock(paths);
    if let Some(latest_version) = cached_latest_prodex_version(paths, source)? {
        return Ok(Some(latest_version));
    }

    let latest_version = match fetch_latest_prodex_version(source) {
        Ok(version) => version,
        Err(error) if is_update_notice_mojo_error(&error) => return Err(error),
        Err(_) => return Ok(None),
    };
    let mut cache = load_update_check_cache(paths)
        .ok()
        .flatten()
        .unwrap_or_default();
    cache.source = source;
    cache.latest_version = latest_version.clone();
    cache.checked_at = Local::now().timestamp();
    let _ = save_update_check_cache(paths, &cache);
    Ok(Some(latest_version))
}

fn cached_latest_prodex_version(
    paths: &AppPaths,
    source: ProdexReleaseSource,
) -> Result<Option<String>> {
    let Some(cached) = load_update_check_cache(paths).ok().flatten() else {
        return Ok(None);
    };
    Ok(should_use_cached_update_version(
        cached.source,
        &cached.latest_version,
        cached.checked_at,
        source,
        current_prodex_version(),
        Local::now().timestamp(),
    )?
    .then_some(cached.latest_version))
}

fn latest_codex_version(paths: &AppPaths, current_version: &str) -> Result<Option<String>> {
    if let Some(latest_version) = cached_latest_codex_version(paths, current_version)? {
        return Ok(Some(latest_version));
    }

    let _lock = acquire_update_check_lock(paths);
    if let Some(latest_version) = cached_latest_codex_version(paths, current_version)? {
        return Ok(Some(latest_version));
    }

    let latest_version = match fetch_latest_codex_github_version() {
        Ok(version) => version,
        Err(error) if is_update_notice_mojo_error(&error) => return Err(error),
        Err(_) => return Ok(None),
    };
    let mut cache = load_update_check_cache(paths)
        .ok()
        .flatten()
        .unwrap_or_default();
    cache.codex_latest_version = Some(latest_version.clone());
    cache.codex_checked_at = Some(Local::now().timestamp());
    let _ = save_update_check_cache(paths, &cache);
    Ok(Some(latest_version))
}

fn cached_latest_codex_version(paths: &AppPaths, current_version: &str) -> Result<Option<String>> {
    let Some(cached) = load_update_check_cache(paths).ok().flatten() else {
        return Ok(None);
    };
    let (Some(latest_version), Some(checked_at)) =
        (cached.codex_latest_version, cached.codex_checked_at)
    else {
        return Ok(None);
    };
    Ok((Local::now().timestamp().saturating_sub(checked_at)
        < update_check_cache_ttl_seconds(&latest_version, current_version)?)
    .then_some(latest_version))
}

pub fn should_use_cached_update_version(
    cached_source: ProdexReleaseSource,
    cached_latest_version: &str,
    cached_checked_at: i64,
    current_source: ProdexReleaseSource,
    current_version: &str,
    now: i64,
) -> Result<bool> {
    map_update_notice_mojo(prodex_mojo_core::update_notice_policy::cache_is_fresh(
        cached_source == current_source,
        now,
        cached_checked_at,
        update_check_cache_ttl_seconds(cached_latest_version, current_version)?,
    ))
}

pub fn update_check_cache_ttl_seconds(
    cached_latest_version: &str,
    current_version: &str,
) -> Result<i64> {
    // Both release-cache TTLs are currently identical. Preserve the prior
    // version-validation error contract without a redundant Rust branch or a
    // speculative duplicate Mojo planner.
    let _ = version_is_newer(cached_latest_version, current_version)?;
    Ok(UPDATE_CHECK_CACHE_TTL_SECONDS)
}

fn load_update_check_cache(paths: &AppPaths) -> Result<Option<UpdateCheckCache>> {
    let path = update_check_cache_file_path(paths);
    let Some(file) = open_update_check_cache(&path)? else {
        return Ok(None);
    };
    let metadata = file
        .metadata()
        .context("failed to stat update check cache")?;
    if !metadata.is_file() || metadata.len() > UPDATE_CHECK_CACHE_MAX_BYTES {
        bail!("update check cache exceeds read limit");
    }
    let content = read_update_check_cache_content(file, UPDATE_CHECK_CACHE_MAX_BYTES)?;
    let cache = serde_json::from_str(&content).context("failed to parse update check cache")?;
    Ok(Some(cache))
}

fn open_update_check_cache(path: &Path) -> Result<Option<fs::File>> {
    #[cfg(unix)]
    let opened = {
        use std::os::unix::fs::OpenOptionsExt;

        OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
    };

    #[cfg(not(unix))]
    let opened = {
        if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            bail!("refusing update check cache symlink");
        }
        OpenOptions::new().read(true).open(path)
    };

    match opened {
        Ok(file) => Ok(Some(file)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).context("failed to open update check cache"),
    }
}

fn read_update_check_cache_content(reader: impl Read, max_bytes: u64) -> Result<String> {
    let mut bytes = Vec::new();
    reader
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .context("failed to read update check cache")?;
    if bytes.len() as u64 > max_bytes {
        bail!("update check cache exceeds read limit");
    }
    String::from_utf8(bytes).context("failed to decode update check cache")
}

fn acquire_update_check_lock(paths: &AppPaths) -> Option<fs::File> {
    fs::create_dir_all(&paths.root).ok()?;
    let path = paths.root.join("update-check.lock");

    #[cfg(unix)]
    let file = {
        use std::os::unix::fs::OpenOptionsExt;

        OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .ok()?
    };

    #[cfg(not(unix))]
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(path)
        .ok()?;

    file.lock().ok()?;
    Some(file)
}

fn update_check_cache_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("update-check.json")
}

fn save_update_check_cache(paths: &AppPaths, cache: &UpdateCheckCache) -> Result<()> {
    fs::create_dir_all(&paths.root).context("failed to create prodex state directory")?;
    let path = update_check_cache_file_path(paths);
    let temp_file = prodex_core::unique_root_temp_file_path(
        &path,
        "update-check.json",
        &UPDATE_CHECK_TEMP_SEQUENCE,
    );
    let json =
        serde_json::to_string_pretty(cache).context("failed to serialize update check cache")?;
    let result = (|| {
        #[cfg(unix)]
        let mut file = {
            use std::os::unix::fs::OpenOptionsExt;

            OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&temp_file)
                .context("failed to create update check cache")?
        };
        #[cfg(not(unix))]
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_file)
            .context("failed to create update check cache")?;
        file.write_all(json.as_bytes())
            .and_then(|()| file.sync_all())
            .context("failed to write update check cache")?;
        drop(file);
        fs::rename(&temp_file, &path).context("failed to replace update check cache")?;
        #[cfg(unix)]
        fs::File::open(&paths.root)
            .and_then(|directory| directory.sync_all())
            .context("failed to sync update check cache directory")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp_file);
    }
    result
}

fn fetch_latest_prodex_version(_source: ProdexReleaseSource) -> Result<String> {
    fetch_latest_prodex_github_version()
}

fn fetch_latest_prodex_github_version() -> Result<String> {
    fetch_latest_github_version(
        "https://github.com/christiandoxa/prodex/releases/latest",
        latest_release_version_from_url,
    )
}

fn fetch_latest_codex_github_version() -> Result<String> {
    fetch_latest_github_version(
        "https://github.com/openai/codex/releases/latest",
        latest_codex_release_version_from_url,
    )
}

fn fetch_latest_github_version(
    latest_release_url: &str,
    version_from_url: fn(&reqwest::Url) -> Result<String>,
) -> Result<String> {
    let client = Client::builder()
        .connect_timeout(Duration::from_millis(UPDATE_CHECK_HTTP_CONNECT_TIMEOUT_MS))
        .timeout(Duration::from_millis(UPDATE_CHECK_HTTP_READ_TIMEOUT_MS))
        .build()
        .context("failed to build update-check HTTP client")?;
    let response = client
        .head(latest_release_url)
        .header(
            "User-Agent",
            format!("prodex/{}", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .context("failed to request latest GitHub release")?;
    let status = response.status();
    if !status.is_success() {
        bail!("GitHub releases returned HTTP {}", status.as_u16());
    }
    version_from_url(response.url())
}

fn latest_release_version_from_url(url: &reqwest::Url) -> Result<String> {
    let tag = url
        .path()
        .strip_prefix("/christiandoxa/prodex/releases/tag/")
        .context("GitHub latest release redirect did not contain a version")?;
    if !release_version_is_valid(tag)? {
        bail!("invalid GitHub release version");
    }
    Ok(tag.strip_prefix('v').unwrap_or(tag).to_string())
}

fn latest_codex_release_version_from_url(url: &reqwest::Url) -> Result<String> {
    let version = url
        .path()
        .strip_prefix("/openai/codex/releases/tag/rust-v")
        .context("GitHub latest Codex release redirect did not contain a version")?;
    if !release_version_is_valid(version)? {
        bail!("invalid GitHub Codex release version");
    }
    Ok(version.to_string())
}

#[cfg(test)]
#[path = "../tests/src/notice.rs"]
mod tests;
