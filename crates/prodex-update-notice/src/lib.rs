use anyhow::{Context, Result, bail};
use chrono::Local;
use prodex_cli::{Commands, ContextCommands};
use prodex_core::AppPaths;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use terminal_ui::{print_wrapped_stderr, section_header};

pub const UPDATE_CHECK_CACHE_TTL_SECONDS: i64 = if cfg!(test) { 5 } else { 21_600 };
pub const UPDATE_CHECK_STALE_CURRENT_TTL_SECONDS: i64 = if cfg!(test) { 1 } else { 300 };
const UPDATE_CHECK_HTTP_CONNECT_TIMEOUT_MS: u64 = if cfg!(test) { 200 } else { 800 };
const UPDATE_CHECK_HTTP_READ_TIMEOUT_MS: u64 = if cfg!(test) { 400 } else { 1200 };
static UPDATE_CHECK_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct UpdateCheckCache {
    #[serde(default)]
    source: ProdexReleaseSource,
    latest_version: String,
    checked_at: i64,
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

#[derive(Debug, Deserialize)]
struct GitHubLatestReleaseResponse {
    tag_name: String,
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
        print_wrapped_stderr(&section_header("Update Available"));
        print_wrapped_stderr(&format!(
            "A newer prodex release is available: {} -> {}",
            current_prodex_version(),
            latest_version
        ));
        print_wrapped_stderr(&format!(
            "Update with: {}",
            prodex_update_command_for_version(&latest_version)
        ));
        if let Some(warning) = current_prodex_install_warning() {
            print_wrapped_stderr(&format!("WARNING: {warning}"));
        }
    }

    Ok(())
}

pub fn should_emit_update_notice(command: &Commands) -> bool {
    match command {
        Commands::Info(_) => false,
        Commands::Log(_) => false,
        Commands::Doctor(args) => !args.json && args.bundle.is_none(),
        Commands::Audit(args) => !args.json,
        Commands::Context(ContextCommands::Audit(args)) => !args.json,
        Commands::Context(ContextCommands::Compress(args)) => !args.json,
        Commands::Update(_) => false,
        Commands::Quota(args) => !args.raw,
        _ => true,
    }
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
    if npm_package_name == Some("@christiandoxa/prodex") {
        return ProdexInstallChannel::Npm;
    }

    let normalized_path = executable_path
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    if normalized_path.contains("/node_modules/@christiandoxa/prodex-")
        || normalized_path.contains("/node_modules/@christiandoxa/prodex/")
    {
        return ProdexInstallChannel::Npm;
    }
    if normalized_path.ends_with("/.cargo/bin/prodex")
        || normalized_path.ends_with("/.cargo/bin/prodex.exe")
    {
        return ProdexInstallChannel::Cargo;
    }
    ProdexInstallChannel::Standalone
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
    Ok(match latest_prodex_version(paths)? {
        Some(latest_version) if version_is_newer(&latest_version, current_prodex_version()) => {
            ProdexVersionStatus::UpdateAvailable(latest_version)
        }
        Some(_) => ProdexVersionStatus::UpToDate,
        None => ProdexVersionStatus::Unknown,
    })
}

pub fn format_info_prodex_version(paths: &AppPaths) -> Result<String> {
    let current_version = current_prodex_version();
    Ok(match prodex_version_status(paths)? {
        ProdexVersionStatus::UpToDate => format!("{current_version} (up to date)"),
        ProdexVersionStatus::UpdateAvailable(latest_version) => {
            format!("{current_version} (update available: {latest_version})")
        }
        ProdexVersionStatus::Unknown => format!("{current_version} (update check unavailable)"),
    })
}

fn latest_prodex_version(paths: &AppPaths) -> Result<Option<String>> {
    let source = current_prodex_release_source();
    if let Some(cached) = load_update_check_cache(paths)?
        && should_use_cached_update_version(
            cached.source,
            &cached.latest_version,
            cached.checked_at,
            source,
            current_prodex_version(),
            Local::now().timestamp(),
        )
    {
        return Ok(Some(cached.latest_version));
    }

    let latest_version = match fetch_latest_prodex_version(source) {
        Ok(version) => version,
        Err(_) => return Ok(None),
    };
    save_update_check_cache(
        paths,
        &UpdateCheckCache {
            source,
            latest_version: latest_version.clone(),
            checked_at: Local::now().timestamp(),
        },
    )?;
    Ok(Some(latest_version))
}

pub fn should_use_cached_update_version(
    cached_source: ProdexReleaseSource,
    cached_latest_version: &str,
    cached_checked_at: i64,
    current_source: ProdexReleaseSource,
    current_version: &str,
    now: i64,
) -> bool {
    cached_source == current_source
        && now.saturating_sub(cached_checked_at)
            < update_check_cache_ttl_seconds(cached_latest_version, current_version)
}

pub fn update_check_cache_ttl_seconds(cached_latest_version: &str, current_version: &str) -> i64 {
    if version_is_newer(cached_latest_version, current_version) {
        UPDATE_CHECK_CACHE_TTL_SECONDS
    } else {
        UPDATE_CHECK_STALE_CURRENT_TTL_SECONDS
    }
}

fn load_update_check_cache(paths: &AppPaths) -> Result<Option<UpdateCheckCache>> {
    let path = update_check_cache_file_path(paths);
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path).context("failed to read update check cache")?;
    let cache = serde_json::from_str(&content).context("failed to parse update check cache")?;
    Ok(Some(cache))
}

fn update_check_cache_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("update-check.json")
}

fn unique_state_temp_file_path(state_file: &Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = UPDATE_CHECK_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let file_name = format!(
        "{}.{}.{}.{}.tmp",
        state_file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("update-check.json"),
        std::process::id(),
        nanos,
        sequence
    );

    state_file.with_file_name(file_name)
}

fn save_update_check_cache(paths: &AppPaths, cache: &UpdateCheckCache) -> Result<()> {
    fs::create_dir_all(&paths.root).context("failed to create prodex state directory")?;
    let path = update_check_cache_file_path(paths);
    let temp_file = unique_state_temp_file_path(&path);
    let json =
        serde_json::to_string_pretty(cache).context("failed to serialize update check cache")?;
    fs::write(&temp_file, json).context("failed to write update check cache")?;
    fs::rename(&temp_file, &path).context("failed to replace update check cache")?;
    Ok(())
}

fn fetch_latest_prodex_version(_source: ProdexReleaseSource) -> Result<String> {
    fetch_latest_prodex_github_version()
}

fn fetch_latest_prodex_github_version() -> Result<String> {
    let client = Client::builder()
        .connect_timeout(Duration::from_millis(UPDATE_CHECK_HTTP_CONNECT_TIMEOUT_MS))
        .timeout(Duration::from_millis(UPDATE_CHECK_HTTP_READ_TIMEOUT_MS))
        .build()
        .context("failed to build update-check HTTP client")?;
    let response = client
        .get("https://api.github.com/repos/christiandoxa/prodex/releases/latest")
        .header("Accept", "application/vnd.github+json")
        .header(
            "User-Agent",
            format!("prodex/{}", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .context("failed to request GitHub release metadata")?;
    let status = response.status();
    let body = response
        .bytes()
        .context("failed to read GitHub release metadata")?;
    if !status.is_success() {
        bail!("GitHub API returned HTTP {}", status.as_u16());
    }
    let payload: GitHubLatestReleaseResponse =
        serde_json::from_slice(&body).context("failed to parse GitHub release metadata")?;
    Ok(payload.tag_name.trim_start_matches('v').to_string())
}

pub fn version_is_newer(candidate: &str, current: &str) -> bool {
    parse_release_version(candidate) > parse_release_version(current)
}

fn parse_release_version(version: &str) -> Vec<u64> {
    version
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_paths(name: &str) -> AppPaths {
        let root = env::temp_dir().join(format!(
            "prodex-update-notice-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        AppPaths {
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("shared-codex"),
            legacy_shared_codex_root: root.join("legacy-shared"),
            root,
        }
    }

    #[test]
    fn update_check_cache_parse_error_redacts_cache_path() {
        let paths = test_paths("parse-redaction");
        fs::create_dir_all(&paths.root).expect("prodex root should be created");
        fs::write(update_check_cache_file_path(&paths), "{not-json")
            .expect("cache should be written");

        let error = load_update_check_cache(&paths).unwrap_err().to_string();

        assert_eq!(error, "failed to parse update check cache");
        assert!(!error.contains(paths.root.to_string_lossy().as_ref()));

        let _ = fs::remove_dir_all(paths.root);
    }

    #[test]
    fn install_channel_warnings_cover_legacy_managers() {
        assert_eq!(
            prodex_install_channel(
                None,
                Some(Path::new(
                    r"C:\Users\test-user\AppData\Roaming\npm\node_modules\@christiandoxa\prodex-win32-x64\vendor\prodex.exe"
                ))
            ),
            ProdexInstallChannel::Npm
        );
        assert_eq!(
            prodex_install_channel(None, Some(Path::new("/home/test-user/.cargo/bin/prodex"))),
            ProdexInstallChannel::Cargo
        );
        assert_eq!(
            prodex_install_channel(None, Some(Path::new("/home/test-user/.local/bin/prodex"))),
            ProdexInstallChannel::Standalone
        );
        assert!(prodex_install_warning(ProdexInstallChannel::Npm).is_some());
        assert!(prodex_install_warning(ProdexInstallChannel::Cargo).is_some());
        assert!(prodex_install_warning(ProdexInstallChannel::Standalone).is_none());
    }
}
