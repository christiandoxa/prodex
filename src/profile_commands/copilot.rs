use super::*;

const COPILOT_KEYCHAIN_SERVICE: &str = "copilot-cli";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CopilotConfigFile {
    #[serde(default)]
    last_logged_in_user: Option<CopilotConfigUser>,
    #[serde(default)]
    logged_in_users: Vec<CopilotConfigUser>,
    #[serde(default)]
    copilot_tokens: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CopilotConfigUser {
    host: String,
    login: String,
}

#[derive(Debug, Deserialize)]
struct CopilotUserInfo {
    #[serde(default)]
    login: Option<String>,
    #[serde(default)]
    access_type_sku: Option<String>,
    #[serde(default)]
    copilot_plan: Option<String>,
    #[serde(default)]
    endpoints: Option<CopilotUserEndpoints>,
}

#[derive(Debug, Deserialize)]
struct CopilotUserEndpoints {
    #[serde(default)]
    api: Option<String>,
}

#[derive(Debug)]
struct CopilotImportContext {
    host: String,
    login: String,
    token: String,
}

pub(super) fn is_copilot_import_source(path: &Path) -> bool {
    path.components().count() == 1
        && path
            .to_str()
            .is_some_and(|value| value.eq_ignore_ascii_case("copilot"))
        && !path.exists()
}

pub(super) fn handle_import_copilot_profile(args: &ImportProfileArgs) -> Result<()> {
    let context = resolve_copilot_import_context()?;
    let user_info = fetch_copilot_user_info(&context)?;
    let provider = ProfileProvider::Copilot {
        host: context.host.clone(),
        login: user_info
            .login
            .clone()
            .unwrap_or_else(|| context.login.clone()),
        api_url: user_info
            .endpoints
            .as_ref()
            .and_then(|endpoints| endpoints.api.clone())
            .unwrap_or_else(|| default_copilot_models_api_url(&context.host)),
        access_type_sku: user_info.access_type_sku.clone(),
        copilot_plan: user_info.copilot_plan.clone(),
    };

    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;

    if let Some(existing_name) =
        find_copilot_profile_by_identity(&state, &context.host, &context.login)
    {
        if let Some(requested_name) = args.name.as_deref()
            && requested_name != existing_name
        {
            bail!(
                "Copilot account '{}' is already imported as profile '{}'",
                context.login,
                existing_name
            );
        }

        let profile = state
            .profiles
            .get_mut(&existing_name)
            .with_context(|| format!("profile '{}' is missing", existing_name))?;
        profile.provider = provider.clone();
        profile.email = Some(context.login.clone());
        if state.active_profile.is_none() || args.activate {
            state.active_profile = Some(existing_name.clone());
        }
        state.save(&paths)?;

        audit_log_event_best_effort(
            "profile",
            "import_copilot",
            "success",
            serde_json::json!({
                "profile_name": existing_name,
                "provider": provider.label(),
                "github_host": context.host,
                "github_login": context.login,
                "api_url": provider_api_url(&provider),
                "activated": state.active_profile.as_deref() == Some(existing_name.as_str()),
                "updated_existing": true,
            }),
        );

        let mut fields = vec![
            (
                "Result".to_string(),
                format!("Updated imported Copilot profile '{}'.", existing_name),
            ),
            ("Profile".to_string(), existing_name.clone()),
            ("Provider".to_string(), provider.display_name().to_string()),
            ("Identity".to_string(), context.login.clone()),
            ("GitHub host".to_string(), context.host.clone()),
        ];
        if let Some(api_url) = provider_api_url(&provider) {
            fields.push(("API".to_string(), api_url.to_string()));
        }
        fields.push((
            "Storage".to_string(),
            "Token remains in Copilot's keychain/config store.".to_string(),
        ));
        if state.active_profile.as_deref() == Some(existing_name.as_str()) {
            fields.push(("Active".to_string(), existing_name));
        }
        print_panel("Profile Updated", &fields);
        return Ok(());
    }

    let profile_name = match args.name.as_deref() {
        Some(requested) => {
            validate_profile_name(requested)?;
            if state.profiles.contains_key(requested) {
                bail!("profile '{}' already exists", requested);
            }
            requested.to_string()
        }
        None => default_copilot_profile_name(&paths, &state, &context.login),
    };

    let codex_home = managed_profile_home_path(&paths, &profile_name)?;
    ensure_path_is_unique(&state, &codex_home)?;
    if codex_home.exists() {
        bail!(
            "managed profile home {} already exists",
            codex_home.display()
        );
    }
    create_codex_home_if_missing(&codex_home)?;
    prepare_managed_codex_home(&paths, &codex_home)?;

    state.profiles.insert(
        profile_name.clone(),
        ProfileEntry {
            codex_home: codex_home.clone(),
            managed: true,
            email: Some(context.login.clone()),
            provider: provider.clone(),
        },
    );
    if state.active_profile.is_none() || args.activate {
        state.active_profile = Some(profile_name.clone());
    }
    state.save(&paths)?;

    audit_log_event_best_effort(
        "profile",
        "import_copilot",
        "success",
        serde_json::json!({
            "profile_name": profile_name.clone(),
            "provider": provider.label(),
            "github_host": context.host.clone(),
            "github_login": context.login.clone(),
            "api_url": provider_api_url(&provider),
            "activated": state.active_profile.as_deref() == Some(profile_name.as_str()),
            "codex_home": codex_home.display().to_string(),
            "updated_existing": false,
        }),
    );

    let mut fields = vec![
        (
            "Result".to_string(),
            format!("Imported Copilot profile '{}'.", profile_name),
        ),
        ("Profile".to_string(), profile_name.clone()),
        ("Provider".to_string(), provider.display_name().to_string()),
        ("Identity".to_string(), context.login.clone()),
        ("GitHub host".to_string(), context.host),
        ("CODEX_HOME".to_string(), codex_home.display().to_string()),
    ];
    if let Some(api_url) = provider_api_url(&provider) {
        fields.push(("API".to_string(), api_url.to_string()));
    }
    fields.push((
        "Storage".to_string(),
        "Managed profile home created; token remains in Copilot's keychain/config store."
            .to_string(),
    ));
    if state.active_profile.as_deref() == Some(profile_name.as_str()) {
        fields.push(("Active".to_string(), profile_name));
    }
    print_panel("Profile Added", &fields);
    Ok(())
}

fn provider_api_url(provider: &ProfileProvider) -> Option<&str> {
    match provider {
        ProfileProvider::Openai => None,
        ProfileProvider::Copilot { api_url, .. } => Some(api_url.as_str()),
    }
}

fn find_copilot_profile_by_identity(state: &AppState, host: &str, login: &str) -> Option<String> {
    state.profiles.iter().find_map(|(name, profile)| {
        profile
            .provider
            .copilot_matches(host, login)
            .then_some(name.clone())
    })
}

fn default_copilot_profile_name(paths: &AppPaths, state: &AppState, login: &str) -> String {
    let base_name = profile_name_from_email(&format!("copilot-{login}"));
    unique_profile_name_from_seed(paths, state, &base_name)
}

fn unique_profile_name_from_seed(paths: &AppPaths, state: &AppState, base_name: &str) -> String {
    let base_name = if base_name.trim().is_empty() {
        "copilot".to_string()
    } else {
        base_name.to_string()
    };
    if is_available_profile_name(paths, state, &base_name) {
        return base_name;
    }

    for suffix in 2.. {
        let candidate = format!("{base_name}-{suffix}");
        if is_available_profile_name(paths, state, &candidate) {
            return candidate;
        }
    }

    unreachable!("integer suffix space should not be exhausted")
}

fn is_available_profile_name(paths: &AppPaths, state: &AppState, candidate: &str) -> bool {
    !state.profiles.contains_key(candidate) && !paths.managed_profiles_root.join(candidate).exists()
}

fn resolve_copilot_import_context() -> Result<CopilotImportContext> {
    let config_root = discover_copilot_config_root()?;
    let config_path = config_root.join("config.json");
    let raw = fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    let config: CopilotConfigFile = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", config_path.display()))?;
    let user = config
        .last_logged_in_user
        .or_else(|| config.logged_in_users.into_iter().next())
        .context("no logged-in Copilot user found in config.json")?;
    let account_key = copilot_account_key(&user.host, &user.login);

    let token = config
        .copilot_tokens
        .get(&account_key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| read_copilot_keychain_token(&account_key).ok().flatten())
        .context("failed to resolve the stored Copilot token from config or keychain")?;

    Ok(CopilotImportContext {
        host: user.host,
        login: user.login,
        token,
    })
}

fn discover_copilot_config_root() -> Result<PathBuf> {
    Ok(match env::var_os("COPILOT_HOME") {
        Some(path) => absolutize(PathBuf::from(path))?,
        None => home_dir()
            .context("failed to determine home directory")?
            .join(".copilot"),
    })
}

fn read_copilot_keychain_token(account_key: &str) -> Result<Option<String>> {
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

fn parse_copilot_version(raw: &str) -> (u64, u64, u64) {
    let mut parts = raw.split('.');
    let parse_part = |value: Option<&str>| {
        value
            .and_then(|part| {
                part.chars()
                    .take_while(|ch| ch.is_ascii_digit())
                    .collect::<String>()
                    .parse::<u64>()
                    .ok()
            })
            .unwrap_or(0)
    };
    (
        parse_part(parts.next()),
        parse_part(parts.next()),
        parse_part(parts.next()),
    )
}

fn copilot_platform_label() -> &'static str {
    match (env::consts::OS, env::consts::ARCH) {
        ("linux", "x86_64") => "linux-x64",
        ("linux", "aarch64") => "linux-arm64",
        ("macos", "x86_64") => "darwin-x64",
        ("macos", "aarch64") => "darwin-arm64",
        ("windows", "x86_64") => "win32-x64",
        ("windows", "aarch64") => "win32-arm64",
        _ => "linux-x64",
    }
}

fn copilot_account_key(host: &str, login: &str) -> String {
    format!("{}:{}", host.trim(), login.trim())
}

fn fetch_copilot_user_info(context: &CopilotImportContext) -> Result<CopilotUserInfo> {
    let client = Client::builder()
        .connect_timeout(Duration::from_millis(QUOTA_HTTP_CONNECT_TIMEOUT_MS))
        .timeout(Duration::from_millis(QUOTA_HTTP_READ_TIMEOUT_MS))
        .build()
        .context("failed to build Copilot import HTTP client")?;
    let user_url = format!(
        "{}/copilot_internal/user",
        copilot_user_api_origin(&context.host)?
    );
    let response = client
        .get(&user_url)
        .header("Authorization", format!("Bearer {}", context.token))
        .header("Accept", "application/json")
        .header(
            "User-Agent",
            format!("prodex/{}", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .with_context(|| format!("failed to query {}", user_url))?;
    let status = response.status();
    let body = response
        .bytes()
        .with_context(|| format!("failed to read {}", user_url))?;
    if !status.is_success() {
        let body_text = format_response_body(&body);
        if body_text.is_empty() {
            bail!(
                "Copilot import failed (HTTP {}) at {}",
                status.as_u16(),
                user_url
            );
        }
        bail!(
            "Copilot import failed (HTTP {}) at {}: {}",
            status.as_u16(),
            user_url,
            body_text
        );
    }
    serde_json::from_slice(&body).with_context(|| format!("failed to parse {}", user_url))
}

fn copilot_user_api_origin(host: &str) -> Result<String> {
    let mut url = if host.contains("://") {
        reqwest::Url::parse(host).with_context(|| format!("invalid Copilot host '{}'", host))?
    } else {
        reqwest::Url::parse(&format!("https://{host}"))
            .with_context(|| format!("invalid Copilot host '{}'", host))?
    };
    let is_local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    let has_explicit_port = url.port().is_some();
    if !has_explicit_port
        && !is_local
        && let Some(hostname) = url.host_str()
        && !hostname.starts_with("api.")
    {
        url.set_host(Some(&format!("api.{hostname}")))
            .map_err(|_| {
                anyhow::anyhow!("failed to derive Copilot API hostname from '{}'", host)
            })?;
    }
    url.set_path("");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string().trim_end_matches('/').to_string())
}

fn default_copilot_models_api_url(host: &str) -> String {
    let normalized = host.trim().trim_end_matches('/');
    if normalized.eq_ignore_ascii_case("https://github.com")
        || normalized.eq_ignore_ascii_case("http://github.com")
        || normalized.eq_ignore_ascii_case("github.com")
    {
        return "https://api.githubcopilot.com".to_string();
    }

    let fallback_host = normalized
        .strip_prefix("https://")
        .or_else(|| normalized.strip_prefix("http://"))
        .unwrap_or(normalized);
    if let Some(subdomain) = fallback_host.strip_suffix(".ghe.com") {
        return format!("https://copilot-api.{subdomain}.ghe.com");
    }

    format!("https://api.{fallback_host}")
}
