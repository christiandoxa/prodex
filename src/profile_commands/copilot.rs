use super::*;

pub(crate) use prodex_profile_export::CopilotUserInfo;
use prodex_profile_export::{
    CopilotConfigFile, CopilotProfileImportStatePlan, CopilotProfileImportSummary,
    copilot_account_key, copilot_platform_label, copilot_profile_import_summary_fields,
    copilot_token_from_config, copilot_user_api_origin, parse_copilot_config_file,
    parse_copilot_user_info_json_response, parse_copilot_user_info_value, parse_copilot_version,
    plan_copilot_profile_import, plan_copilot_profile_import_state, select_copilot_logged_in_user,
};

const COPILOT_KEYCHAIN_SERVICE: &str = "copilot-cli";

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

pub(crate) fn handle_import_copilot_profile(args: &ImportProfileArgs) -> Result<()> {
    let context = resolve_copilot_import_context()?;
    let user_info = fetch_copilot_user_info(&context)?;
    let import_plan = plan_copilot_profile_import(&context.host, &context.login, &user_info);
    let provider = ProfileProvider::Copilot {
        host: import_plan.host.clone(),
        login: import_plan.login.clone(),
        api_url: import_plan.api_url.clone(),
        access_type_sku: import_plan.access_type_sku.clone(),
        copilot_plan: import_plan.copilot_plan.clone(),
    };

    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;
    let existing_profile_name =
        find_copilot_profile_by_identity(&state, &context.host, &context.login);
    let import_state_plan = plan_copilot_profile_import_state(
        &context.login,
        args.name.as_deref(),
        existing_profile_name.as_deref(),
        state.active_profile.is_some(),
        args.activate,
        |profile_name| state.profiles.contains_key(profile_name),
        || default_copilot_profile_name(&paths, &state, &context.login),
    )?;

    let (profile_name, activate) = match import_state_plan {
        CopilotProfileImportStatePlan::UpdateExisting {
            profile_name: existing_name,
            activate,
        } => {
            let profile = state
                .profiles
                .get_mut(&existing_name)
                .with_context(|| format!("profile '{}' is missing", existing_name))?;
            profile.provider = provider.clone();
            profile.email = Some(context.login.clone());
            if activate {
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
                    "api_url": import_plan.api_url,
                    "activated": state.active_profile.as_deref() == Some(existing_name.as_str()),
                    "updated_existing": true,
                }),
            );

            let fields = copilot_profile_import_summary_fields(CopilotProfileImportSummary {
                profile_name: existing_name.clone(),
                provider: provider.display_name().to_string(),
                identity: context.login.clone(),
                github_host: context.host.clone(),
                api_url: Some(import_plan.api_url.clone()),
                codex_home: None,
                active: state.active_profile.as_deref() == Some(existing_name.as_str()),
                updated_existing: true,
            });
            print_panel("Profile Updated", &fields);
            return Ok(());
        }
        CopilotProfileImportStatePlan::AddNew {
            profile_name,
            activate,
        } => (profile_name, activate),
    };
    if args.name.is_some() {
        prodex_profile_identity::validate_profile_name(&profile_name)?;
    }

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
    if activate {
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
            "api_url": import_plan.api_url,
            "activated": state.active_profile.as_deref() == Some(profile_name.as_str()),
            "codex_home": codex_home.display().to_string(),
            "updated_existing": false,
        }),
    );

    let fields = copilot_profile_import_summary_fields(CopilotProfileImportSummary {
        profile_name: profile_name.clone(),
        provider: provider.display_name().to_string(),
        identity: context.login.clone(),
        github_host: context.host,
        api_url: Some(import_plan.api_url.clone()),
        codex_home: Some(codex_home.display().to_string()),
        active: state.active_profile.as_deref() == Some(profile_name.as_str()),
        updated_existing: false,
    });
    print_panel("Profile Added", &fields);
    Ok(())
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
    prodex_profile_identity::unique_copilot_profile_name(login, |candidate| {
        is_available_profile_name(paths, state, candidate)
    })
}

fn is_available_profile_name(paths: &AppPaths, state: &AppState, candidate: &str) -> bool {
    !state.profiles.contains_key(candidate) && !paths.managed_profiles_root.join(candidate).exists()
}

fn resolve_copilot_import_context() -> Result<CopilotImportContext> {
    let config = read_copilot_config()?;
    let user = select_copilot_logged_in_user(&config)
        .context("no logged-in Copilot user found in config.json")?;
    let token = resolve_copilot_account_token_from_config(&config, &user.host, &user.login)?;

    Ok(CopilotImportContext {
        host: user.host,
        login: user.login,
        token,
    })
}

fn read_copilot_config() -> Result<CopilotConfigFile> {
    let config_root = discover_copilot_config_root()?;
    let config_path = config_root.join("config.json");
    let raw = fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    parse_copilot_config_file(&raw)
        .with_context(|| format!("failed to parse {}", config_path.display()))
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

fn resolve_copilot_account_token_from_config(
    config: &CopilotConfigFile,
    host: &str,
    login: &str,
) -> Result<String> {
    let account_key = copilot_account_key(host, login);
    copilot_token_from_config(config, host, login)
        .or_else(|| read_copilot_keychain_token(&account_key).ok().flatten())
        .context(format!(
            "failed to resolve the stored Copilot token for {} from config or keychain",
            account_key
        ))
}

fn resolve_copilot_account_token(host: &str, login: &str) -> Result<String> {
    let config = read_copilot_config()?;
    resolve_copilot_account_token_from_config(&config, host, login)
}

fn fetch_copilot_user_info(context: &CopilotImportContext) -> Result<CopilotUserInfo> {
    fetch_copilot_user_info_with_token(&context.host, &context.token)
}

pub(crate) fn fetch_copilot_user_info_for_account(
    host: &str,
    login: &str,
) -> Result<CopilotUserInfo> {
    let token = resolve_copilot_account_token(host, login)?;
    fetch_copilot_user_info_with_token(host, &token)
}

pub(crate) fn fetch_copilot_user_info_json_for_account(
    host: &str,
    login: &str,
) -> Result<serde_json::Value> {
    let token = resolve_copilot_account_token(host, login)?;
    fetch_copilot_user_info_json_with_token(host, &token)
}

fn fetch_copilot_user_info_with_token(host: &str, token: &str) -> Result<CopilotUserInfo> {
    let value = fetch_copilot_user_info_json_with_token(host, token)?;
    parse_copilot_user_info_value(
        value,
        &format!("{}/copilot_internal/user", host.trim_end_matches('/')),
    )
}

fn fetch_copilot_user_info_json_with_token(host: &str, token: &str) -> Result<serde_json::Value> {
    let client = Client::builder()
        .connect_timeout(Duration::from_millis(QUOTA_HTTP_CONNECT_TIMEOUT_MS))
        .timeout(Duration::from_millis(QUOTA_HTTP_READ_TIMEOUT_MS))
        .build()
        .context("failed to build Copilot account HTTP client")?;
    let user_url = format!("{}/copilot_internal/user", copilot_user_api_origin(host)?);
    let response = client
        .get(&user_url)
        .header("Authorization", format!("Bearer {}", token))
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
                "Copilot account query failed (HTTP {}) at {}",
                status.as_u16(),
                user_url
            );
        }
        bail!(
            "Copilot account query failed (HTTP {}) at {}: {}",
            status.as_u16(),
            user_url,
            body_text
        );
    }
    parse_copilot_user_info_json_response(&body, &user_url)
}
