use super::*;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct UpdateCheckCache {
    #[serde(default)]
    source: ProdexReleaseSource,
    latest_version: String,
    checked_at: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ProdexReleaseSource {
    Npm,
    #[default]
    CratesIo,
}

#[derive(Debug, Deserialize)]
struct CratesIoVersionResponse {
    #[serde(rename = "crate")]
    crate_info: CratesIoCrateInfo,
}

#[derive(Debug, Deserialize)]
struct CratesIoCrateInfo {
    max_version: String,
}

#[derive(Debug, Deserialize)]
struct NpmLatestVersionResponse {
    version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProdexVersionStatus {
    UpToDate,
    UpdateAvailable(String),
    Unknown,
}

pub(crate) fn show_update_notice_if_available(command: &Commands) -> Result<()> {
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
    }

    Ok(())
}

pub(crate) fn should_emit_update_notice(command: &Commands) -> bool {
    match command {
        Commands::Info(_) => false,
        Commands::Doctor(args) => !args.json,
        Commands::Quota(args) => !args.raw,
        _ => true,
    }
}

pub(crate) fn current_prodex_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub(crate) fn current_prodex_release_source() -> ProdexReleaseSource {
    if env::var("npm_package_name").ok().as_deref() == Some("@christiandoxa/prodex") {
        return ProdexReleaseSource::Npm;
    }

    if env::current_exe().ok().is_some_and(|path| {
        path.to_string_lossy()
            .contains("node_modules/@christiandoxa/prodex-")
    }) {
        return ProdexReleaseSource::Npm;
    }

    ProdexReleaseSource::CratesIo
}

pub(crate) fn prodex_update_command_for_version(latest_version: &str) -> String {
    match current_prodex_release_source() {
        ProdexReleaseSource::Npm => format!(
            "npm install -g @christiandoxa/prodex@{latest_version} or npm install -g @christiandoxa/prodex@latest"
        ),
        ProdexReleaseSource::CratesIo => {
            format!("cargo install prodex --force --version {latest_version}")
        }
    }
}

pub(crate) fn prodex_version_status(paths: &AppPaths) -> Result<ProdexVersionStatus> {
    Ok(match latest_prodex_version(paths)? {
        Some(latest_version) if version_is_newer(&latest_version, current_prodex_version()) => {
            ProdexVersionStatus::UpdateAvailable(latest_version)
        }
        Some(_) => ProdexVersionStatus::UpToDate,
        None => ProdexVersionStatus::Unknown,
    })
}

pub(crate) fn format_info_prodex_version(paths: &AppPaths) -> Result<String> {
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

pub(crate) fn should_use_cached_update_version(
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

pub(crate) fn update_check_cache_ttl_seconds(
    cached_latest_version: &str,
    current_version: &str,
) -> i64 {
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
    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let cache = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(Some(cache))
}

fn save_update_check_cache(paths: &AppPaths, cache: &UpdateCheckCache) -> Result<()> {
    fs::create_dir_all(&paths.root)
        .with_context(|| format!("failed to create {}", paths.root.display()))?;
    let path = update_check_cache_file_path(paths);
    let temp_file = unique_state_temp_file_path(&path);
    let json =
        serde_json::to_string_pretty(cache).context("failed to serialize update check cache")?;
    fs::write(&temp_file, json)
        .with_context(|| format!("failed to write {}", temp_file.display()))?;
    fs::rename(&temp_file, &path)
        .with_context(|| format!("failed to replace update cache file {}", path.display()))?;
    Ok(())
}

fn fetch_latest_prodex_version(source: ProdexReleaseSource) -> Result<String> {
    match source {
        ProdexReleaseSource::Npm => fetch_latest_prodex_npm_version(),
        ProdexReleaseSource::CratesIo => fetch_latest_prodex_crates_version(),
    }
}

fn fetch_latest_prodex_crates_version() -> Result<String> {
    let client = Client::builder()
        .connect_timeout(Duration::from_millis(UPDATE_CHECK_HTTP_CONNECT_TIMEOUT_MS))
        .timeout(Duration::from_millis(UPDATE_CHECK_HTTP_READ_TIMEOUT_MS))
        .build()
        .context("failed to build update-check HTTP client")?;
    let response = client
        .get("https://crates.io/api/v1/crates/prodex")
        .header(
            "User-Agent",
            format!("prodex/{}", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .context("failed to request crates.io prodex metadata")?;
    let status = response.status();
    let body = response
        .bytes()
        .context("failed to read crates.io prodex metadata")?;
    if !status.is_success() {
        bail!("crates.io returned HTTP {}", status.as_u16());
    }
    let payload: CratesIoVersionResponse =
        serde_json::from_slice(&body).context("failed to parse crates.io prodex metadata")?;
    Ok(payload.crate_info.max_version)
}

fn fetch_latest_prodex_npm_version() -> Result<String> {
    let client = Client::builder()
        .connect_timeout(Duration::from_millis(UPDATE_CHECK_HTTP_CONNECT_TIMEOUT_MS))
        .timeout(Duration::from_millis(UPDATE_CHECK_HTTP_READ_TIMEOUT_MS))
        .build()
        .context("failed to build update-check HTTP client")?;
    let response = client
        .get("https://registry.npmjs.org/%40christiandoxa%2Fprodex/latest")
        .header(
            "User-Agent",
            format!("prodex/{}", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .context("failed to request npm prodex metadata")?;
    let status = response.status();
    let body = response
        .bytes()
        .context("failed to read npm prodex metadata")?;
    if !status.is_success() {
        bail!("npm registry returned HTTP {}", status.as_u16());
    }
    let payload: NpmLatestVersionResponse =
        serde_json::from_slice(&body).context("failed to parse npm prodex metadata")?;
    Ok(payload.version)
}

pub(crate) fn version_is_newer(candidate: &str, current: &str) -> bool {
    parse_release_version(candidate) > parse_release_version(current)
}

fn parse_release_version(version: &str) -> Vec<u64> {
    version
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}
