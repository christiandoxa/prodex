use anyhow::{Context, Result, bail};
use chrono::Local;
use prodex_cli::{Commands, ContextCommands};
use prodex_core::AppPaths;
use reqwest::blocking::Client;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use terminal_ui::{print_wrapped_stderr, section_header};

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

pub fn format_info_codex_version(
    paths: &AppPaths,
    current_version: Option<&str>,
) -> Result<String> {
    let latest_version = latest_codex_version(paths, current_version.unwrap_or("0.0.0"))?;
    Ok(match (current_version, latest_version) {
        (Some(current), Some(latest)) if version_is_newer(&latest, current) => {
            format!("{current} (update available: {latest})")
        }
        (Some(current), Some(_)) => format!("{current} (up to date)"),
        (Some(current), None) => format!("{current} (update check unavailable)"),
        (None, Some(latest)) => format!("not detected (latest release: {latest})"),
        (None, None) => "not detected (update check unavailable)".to_string(),
    })
}

fn latest_prodex_version(paths: &AppPaths) -> Result<Option<String>> {
    let source = current_prodex_release_source();
    if let Some(latest_version) = cached_latest_prodex_version(paths, source) {
        return Ok(Some(latest_version));
    }

    let _lock = acquire_update_check_lock(paths);
    if let Some(latest_version) = cached_latest_prodex_version(paths, source) {
        return Ok(Some(latest_version));
    }

    let latest_version = match fetch_latest_prodex_version(source) {
        Ok(version) => version,
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

fn cached_latest_prodex_version(paths: &AppPaths, source: ProdexReleaseSource) -> Option<String> {
    let cached = load_update_check_cache(paths).ok().flatten()?;
    should_use_cached_update_version(
        cached.source,
        &cached.latest_version,
        cached.checked_at,
        source,
        current_prodex_version(),
        Local::now().timestamp(),
    )
    .then_some(cached.latest_version)
}

fn latest_codex_version(paths: &AppPaths, current_version: &str) -> Result<Option<String>> {
    if let Some(latest_version) = cached_latest_codex_version(paths, current_version) {
        return Ok(Some(latest_version));
    }

    let _lock = acquire_update_check_lock(paths);
    if let Some(latest_version) = cached_latest_codex_version(paths, current_version) {
        return Ok(Some(latest_version));
    }

    let latest_version = match fetch_latest_codex_github_version() {
        Ok(version) => version,
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

fn cached_latest_codex_version(paths: &AppPaths, current_version: &str) -> Option<String> {
    let cached = load_update_check_cache(paths).ok().flatten()?;
    let latest_version = cached.codex_latest_version?;
    let checked_at = cached.codex_checked_at?;
    (Local::now().timestamp().saturating_sub(checked_at)
        < update_check_cache_ttl_seconds(&latest_version, current_version))
    .then_some(latest_version)
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
    let version = tag.strip_prefix('v').unwrap_or(tag);
    parse_release_version(version).context("invalid GitHub release version")?;
    Ok(version.to_string())
}

fn latest_codex_release_version_from_url(url: &reqwest::Url) -> Result<String> {
    let version = url
        .path()
        .strip_prefix("/openai/codex/releases/tag/rust-v")
        .context("GitHub latest Codex release redirect did not contain a version")?;
    parse_release_version(version).context("invalid GitHub Codex release version")?;
    Ok(version.to_string())
}

pub fn version_is_newer(candidate: &str, current: &str) -> bool {
    match (
        parse_release_version(candidate),
        parse_release_version(current),
    ) {
        (Some(candidate), Some(current)) => candidate > current,
        _ => false,
    }
}

fn parse_release_version(version: &str) -> Option<Version> {
    let version = version.trim();
    Version::parse(version.strip_prefix('v').unwrap_or(version)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

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

    #[test]
    fn version_comparison_uses_semver_and_rejects_invalid_versions() {
        assert!(version_is_newer("v0.297.0", "0.296.0"));
        assert!(version_is_newer("1.0.0", "1.0.0-rc.1"));
        assert!(!version_is_newer("1.0.0-rc.1", "1.0.0"));
        assert!(!version_is_newer("1.invalid.0", "1.0.0"));
        assert!(!version_is_newer("1.0.0", "invalid"));
    }

    #[test]
    fn cached_update_refreshes_within_five_minutes() {
        assert_eq!(update_check_cache_ttl_seconds("0.3.1", "0.3.0"), 300);
        assert_eq!(update_check_cache_ttl_seconds("0.3.0", "0.3.0"), 300);
    }

    #[test]
    fn latest_release_version_comes_from_redirect_url() {
        let url =
            reqwest::Url::parse("https://github.com/christiandoxa/prodex/releases/tag/v0.300.0")
                .unwrap();

        assert_eq!(latest_release_version_from_url(&url).unwrap(), "0.300.0");
        assert!(
            latest_release_version_from_url(
                &reqwest::Url::parse("https://github.com/christiandoxa/prodex/releases/latest")
                    .unwrap()
            )
            .is_err()
        );
    }

    #[test]
    fn latest_codex_release_version_comes_from_rust_tag() {
        let url = reqwest::Url::parse("https://github.com/openai/codex/releases/tag/rust-v0.144.6")
            .unwrap();

        assert_eq!(
            latest_codex_release_version_from_url(&url).unwrap(),
            "0.144.6"
        );
    }

    #[test]
    fn codex_info_version_uses_cached_github_release() {
        let paths = test_paths("codex-version-cache");
        let cache = UpdateCheckCache {
            source: ProdexReleaseSource::GitHub,
            latest_version: current_prodex_version().to_string(),
            checked_at: Local::now().timestamp(),
            codex_latest_version: Some("0.144.6".to_string()),
            codex_checked_at: Some(Local::now().timestamp()),
        };
        save_update_check_cache(&paths, &cache).unwrap();

        assert_eq!(
            format_info_codex_version(&paths, Some("0.144.5")).unwrap(),
            "0.144.5 (update available: 0.144.6)"
        );
        assert_eq!(
            format_info_codex_version(&paths, Some("0.144.6")).unwrap(),
            "0.144.6 (up to date)"
        );

        let _ = fs::remove_dir_all(paths.root);
    }

    #[test]
    fn update_cache_write_is_private_and_bounded() {
        let paths = test_paths("private-cache");
        let cache = UpdateCheckCache {
            source: ProdexReleaseSource::GitHub,
            latest_version: "0.297.0".to_string(),
            checked_at: 1,
            ..UpdateCheckCache::default()
        };
        save_update_check_cache(&paths, &cache).unwrap();
        assert_eq!(
            load_update_check_cache(&paths)
                .unwrap()
                .unwrap()
                .latest_version,
            "0.297.0"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            assert_eq!(
                fs::metadata(update_check_cache_file_path(&paths))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }

        fs::write(
            update_check_cache_file_path(&paths),
            vec![b'x'; UPDATE_CHECK_CACHE_MAX_BYTES as usize + 1],
        )
        .unwrap();
        assert!(load_update_check_cache(&paths).is_err());

        let _ = fs::remove_dir_all(paths.root);
    }

    #[test]
    fn update_cache_reader_rejects_growth_past_limit() {
        let error = read_update_check_cache_content(Cursor::new(b"123456789"), 8).unwrap_err();

        assert!(error.to_string().contains("exceeds read limit"));
    }

    #[cfg(unix)]
    #[test]
    fn update_cache_reader_rejects_symlink() {
        use std::os::unix::fs::symlink;

        let paths = test_paths("symlink-cache");
        fs::create_dir_all(&paths.root).unwrap();
        let target = paths.root.join("outside.json");
        fs::write(
            &target,
            r#"{"source":"GitHub","latest_version":"0.297.0","checked_at":1}"#,
        )
        .unwrap();
        symlink(&target, update_check_cache_file_path(&paths)).unwrap();

        let error = load_update_check_cache(&paths).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("failed to open update check cache")
        );

        let _ = fs::remove_dir_all(paths.root);
    }
}
