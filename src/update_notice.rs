use super::*;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct UpdateCheckCache {
    latest_version: String,
    checked_at: i64,
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

pub(crate) fn show_update_notice_if_available(command: &Commands) -> Result<()> {
    if !should_emit_update_notice(command) {
        return Ok(());
    }

    let paths = AppPaths::discover()?;
    if let Some(latest_version) = latest_prodex_version(&paths)?
        && version_is_newer(&latest_version, env!("CARGO_PKG_VERSION"))
    {
        print_wrapped_stderr(&section_header("Update Available"));
        print_wrapped_stderr(&format!(
            "A newer prodex release is available: {} -> {}",
            env!("CARGO_PKG_VERSION"),
            latest_version
        ));
        print_wrapped_stderr("Update with: cargo install prodex --force");
    }

    Ok(())
}

pub(crate) fn should_emit_update_notice(command: &Commands) -> bool {
    match command {
        Commands::Doctor(args) => !args.json,
        Commands::Quota(args) => !args.raw,
        _ => true,
    }
}

fn latest_prodex_version(paths: &AppPaths) -> Result<Option<String>> {
    if let Some(cached) = load_update_check_cache(paths)?
        && Local::now().timestamp().saturating_sub(cached.checked_at)
            < update_check_cache_ttl_seconds(&cached.latest_version, env!("CARGO_PKG_VERSION"))
    {
        return Ok(Some(cached.latest_version));
    }

    let latest_version = match fetch_latest_prodex_version() {
        Ok(version) => version,
        Err(_) => return Ok(None),
    };
    save_update_check_cache(
        paths,
        &UpdateCheckCache {
            latest_version: latest_version.clone(),
            checked_at: Local::now().timestamp(),
        },
    )?;
    Ok(Some(latest_version))
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

fn fetch_latest_prodex_version() -> Result<String> {
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

pub(crate) fn version_is_newer(candidate: &str, current: &str) -> bool {
    parse_release_version(candidate) > parse_release_version(current)
}

fn parse_release_version(version: &str) -> Vec<u64> {
    version
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}
