use anyhow::{Context, Result, bail};
use std::ffi::OsString;
use std::io::{self, IsTerminal, Write};
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
#[cfg(windows)]
use std::os::windows::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::time::{SystemTime, UNIX_EPOCH};

use super::write_secret_text_file;
use crate::{
    AppPaths, AppState, AppStateIoExt, CodexPassthroughArgs, LogoutArgs, ProfileEntry,
    ProfileProvider, codex_child_plan, create_codex_home_if_missing, ensure_managed_profiles_root,
    exit_with_status, fetch_profile_email, fetch_profile_identity, find_profile_by_identity,
    managed_profile_home_path, persist_login_home, prepare_managed_codex_home, print_panel,
    read_auth_summary, remove_dir_if_exists, required_auth_json_text, resolve_profile_name,
    run_child_plan, unique_profile_name_for_email, update_existing_profile_auth,
    write_profile_openai_compatible_base_url,
};
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoginMethod {
    ChatGpt,
    DeviceCode,
    ApiKey,
    AccessToken,
    Status,
}

#[derive(Debug)]
struct LoginRequest {
    method: LoginMethod,
    codex_args: Vec<OsString>,
    api_key: Option<String>,
    openai_base_url: Option<String>,
    openai_base_url_specified: bool,
    api_key_profile_name: Option<String>,
}

pub(crate) fn handle_codex_login(args: CodexPassthroughArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;
    let login_request = resolve_login_request(args.profile.as_deref(), args.codex_args)?;
    let status = if let Some(profile_name) = args.profile.as_deref() {
        login_into_profile(&paths, &mut state, profile_name, &login_request)?
    } else {
        login_with_auto_profile(&paths, &mut state, &login_request)?
    };
    exit_with_status(status)
}

fn login_into_profile(
    paths: &AppPaths,
    state: &mut AppState,
    profile_name: &str,
    login_request: &LoginRequest,
) -> Result<ExitStatus> {
    let profile_name = resolve_profile_name(state, Some(profile_name))?;
    let codex_home = prepare_profile_login_home(paths, state, &profile_name)?;
    let status = run_codex_login(&codex_home, login_request)?;
    if !status.success() {
        return Ok(status);
    }
    if login_request.method == LoginMethod::Status {
        return Ok(status);
    }

    finish_named_profile_login(
        paths,
        state,
        &profile_name,
        &codex_home,
        login_request.openai_base_url.as_deref(),
        login_request.openai_base_url_specified,
    )?;
    Ok(status)
}

fn prepare_profile_login_home(
    paths: &AppPaths,
    state: &AppState,
    profile_name: &str,
) -> Result<PathBuf> {
    let profile = state
        .profiles
        .get(profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    if !profile.provider.supports_codex_runtime() {
        bail!(
            "profile '{}' uses {}. `prodex login --profile` currently supports OpenAI/Codex profiles only.",
            profile_name,
            profile.provider.display_name()
        );
    }
    let codex_home = profile.codex_home.clone();
    if profile.managed {
        prepare_managed_codex_home(paths, &codex_home)?;
    } else {
        create_codex_home_if_missing(&codex_home)?;
    }
    Ok(codex_home)
}

fn finish_named_profile_login(
    paths: &AppPaths,
    state: &mut AppState,
    profile_name: &str,
    codex_home: &Path,
    openai_base_url: Option<&str>,
    openai_base_url_specified: bool,
) -> Result<()> {
    let auth_label = read_auth_summary(codex_home).label;
    if auth_label == "api-key" {
        if let Some(profile) = state.profiles.get_mut(profile_name) {
            profile.email = None;
        }
        if openai_base_url_specified {
            write_profile_openai_compatible_base_url(codex_home, openai_base_url)?;
        }
    } else {
        refresh_profile_email_from_home(state, profile_name, codex_home);
    }
    let account_email = profile_email_label(state, profile_name);
    state.active_profile = Some(profile_name.to_string());
    state.save(paths)?;

    let result = if auth_label == "api-key" {
        format!("Logged in with API key for profile '{profile_name}'.")
    } else {
        format!("Logged in successfully for profile '{profile_name}'.")
    };
    let fields = vec![
        ("Result".to_string(), result),
        ("Account".to_string(), account_email),
        ("Profile".to_string(), profile_name.to_string()),
        ("CODEX_HOME".to_string(), codex_home.display().to_string()),
    ];
    print_panel("Login", &fields);
    Ok(())
}

fn refresh_profile_email_from_home(state: &mut AppState, profile_name: &str, codex_home: &Path) {
    if let Ok(email) = fetch_profile_email(codex_home)
        && let Some(profile) = state.profiles.get_mut(profile_name)
    {
        profile.email = Some(email);
    }
}

fn profile_email_label(state: &AppState, profile_name: &str) -> String {
    state
        .profiles
        .get(profile_name)
        .and_then(|profile| profile.email.clone())
        .unwrap_or_else(|| "-".to_string())
}

fn login_with_auto_profile(
    paths: &AppPaths,
    state: &mut AppState,
    login_request: &LoginRequest,
) -> Result<ExitStatus> {
    let login_home = create_temporary_login_home(paths)?;
    let status = run_codex_login(&login_home, login_request)?;
    if !status.success() {
        remove_dir_if_exists(&login_home)?;
        return Ok(status);
    }
    if login_request.method == LoginMethod::Status {
        remove_dir_if_exists(&login_home)?;
        return Ok(status);
    }

    let auth_json = required_auth_json_text(&login_home)?;
    if read_auth_summary(&login_home).label == "api-key" {
        finish_auto_login_for_api_key_profile(
            paths,
            state,
            &login_home,
            login_request.api_key_profile_name.as_deref(),
            login_request.openai_base_url.as_deref(),
            login_request.openai_base_url_specified,
            &auth_json,
        )?;
        return Ok(status);
    }

    let identity = fetch_profile_identity(&login_home).with_context(|| {
        format!(
            "failed to resolve the logged-in account identity from {}",
            login_home.display()
        )
    })?;
    let email = identity
        .email
        .as_deref()
        .context("logged-in account identity did not include an email")?;

    if let Some(profile_name) = find_profile_by_identity(state, &identity)? {
        finish_auto_login_for_existing_profile(
            paths,
            state,
            &login_home,
            &profile_name,
            email,
            &auth_json,
        )?;
        return Ok(status);
    }

    finish_auto_login_for_new_profile(paths, state, &login_home, email)?;
    Ok(status)
}

fn finish_auto_login_for_existing_profile(
    paths: &AppPaths,
    state: &mut AppState,
    login_home: &Path,
    profile_name: &str,
    email: &str,
    auth_json: &str,
) -> Result<()> {
    let updated =
        update_existing_profile_auth(paths, state, profile_name, Some(email), auth_json, true)?;
    remove_dir_if_exists(login_home)?;
    state.save(paths)?;

    let fields = vec![
        (
            "Result".to_string(),
            format!(
                "Logged in as {email}. Updated auth token for existing profile '{}'.",
                updated.profile_name
            ),
        ),
        ("Account".to_string(), email.to_string()),
        ("Profile".to_string(), updated.profile_name),
        (
            "CODEX_HOME".to_string(),
            updated.codex_home.display().to_string(),
        ),
    ];
    print_panel("Login", &fields);
    Ok(())
}

fn finish_auto_login_for_api_key_profile(
    paths: &AppPaths,
    state: &mut AppState,
    login_home: &Path,
    requested_profile_name: Option<&str>,
    openai_base_url: Option<&str>,
    openai_base_url_specified: bool,
    auth_json: &str,
) -> Result<()> {
    let profile_name = requested_profile_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| default_api_key_profile_name(openai_base_url));

    if state.profiles.contains_key(&profile_name) {
        finish_api_key_login_for_existing_profile(
            paths,
            state,
            login_home,
            &profile_name,
            openai_base_url,
            openai_base_url_specified,
            auth_json,
        )?;
        return Ok(());
    }

    finish_api_key_login_for_new_profile(
        paths,
        state,
        login_home,
        &profile_name,
        openai_base_url,
        openai_base_url_specified,
    )
}

fn finish_api_key_login_for_existing_profile(
    paths: &AppPaths,
    state: &mut AppState,
    login_home: &Path,
    profile_name: &str,
    openai_base_url: Option<&str>,
    openai_base_url_specified: bool,
    auth_json: &str,
) -> Result<()> {
    let provider = state
        .profiles
        .get(profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?
        .provider
        .clone();
    if !provider.supports_codex_runtime() {
        bail!(
            "profile '{}' uses {}. API key login supports OpenAI/Codex profiles only.",
            profile_name,
            provider.display_name()
        );
    }

    let updated = update_existing_profile_auth(paths, state, profile_name, None, auth_json, true)?;
    if let Some(profile) = state.profiles.get_mut(profile_name) {
        profile.email = None;
    }
    if openai_base_url_specified {
        write_profile_openai_compatible_base_url(&updated.codex_home, openai_base_url)?;
    }
    remove_dir_if_exists(login_home)?;
    state.save(paths)?;

    let fields = vec![
        (
            "Result".to_string(),
            format!(
                "Logged in with API key. Updated profile '{}'.",
                updated.profile_name
            ),
        ),
        ("Account".to_string(), "api-key".to_string()),
        ("Profile".to_string(), updated.profile_name),
        (
            "CODEX_HOME".to_string(),
            updated.codex_home.display().to_string(),
        ),
    ];
    print_panel("Login", &fields);
    Ok(())
}

fn finish_api_key_login_for_new_profile(
    paths: &AppPaths,
    state: &mut AppState,
    login_home: &Path,
    requested_profile_name: &str,
    openai_base_url: Option<&str>,
    openai_base_url_specified: bool,
) -> Result<()> {
    let profile_name = unique_profile_name_for_slug(paths, state, requested_profile_name);
    let codex_home = managed_profile_home_path(paths, &profile_name)?;
    if openai_base_url_specified {
        write_profile_openai_compatible_base_url(login_home, openai_base_url)?;
    }
    persist_login_home(login_home, &codex_home)?;
    prepare_managed_codex_home(paths, &codex_home)?;

    state.profiles.insert(
        profile_name.clone(),
        ProfileEntry {
            codex_home: codex_home.clone(),
            managed: true,
            email: None,
            provider: ProfileProvider::Openai,
        },
    );
    state.active_profile = Some(profile_name.clone());
    state.save(paths)?;

    let fields = vec![
        (
            "Result".to_string(),
            format!("Logged in with API key. Created profile '{profile_name}'."),
        ),
        ("Account".to_string(), "api-key".to_string()),
        ("Profile".to_string(), profile_name),
        ("CODEX_HOME".to_string(), codex_home.display().to_string()),
    ];
    print_panel("Login", &fields);
    Ok(())
}

fn finish_auto_login_for_new_profile(
    paths: &AppPaths,
    state: &mut AppState,
    login_home: &Path,
    email: &str,
) -> Result<()> {
    let profile_name = unique_profile_name_for_email(paths, state, email);
    let codex_home = managed_profile_home_path(paths, &profile_name)?;
    persist_login_home(login_home, &codex_home)?;
    prepare_managed_codex_home(paths, &codex_home)?;

    state.profiles.insert(
        profile_name.clone(),
        ProfileEntry {
            codex_home: codex_home.clone(),
            managed: true,
            email: Some(email.to_string()),
            provider: ProfileProvider::Openai,
        },
    );
    state.active_profile = Some(profile_name.clone());
    state.save(paths)?;

    let fields = vec![
        (
            "Result".to_string(),
            format!("Logged in as {email}. Created profile '{profile_name}'."),
        ),
        ("Account".to_string(), email.to_string()),
        ("Profile".to_string(), profile_name),
        ("CODEX_HOME".to_string(), codex_home.display().to_string()),
    ];
    print_panel("Login", &fields);
    Ok(())
}

fn run_codex_login(codex_home: &Path, login_request: &LoginRequest) -> Result<ExitStatus> {
    if login_request.method == LoginMethod::ApiKey
        && let Some(api_key) = login_request.api_key.as_deref()
    {
        write_api_key_auth_json(codex_home, api_key)?;
        if login_request.openai_base_url_specified {
            write_profile_openai_compatible_base_url(
                codex_home,
                login_request.openai_base_url.as_deref(),
            )?;
        }
        return Ok(success_exit_status());
    }

    let mut command_args = vec![OsString::from("login")];
    command_args.extend(login_request.codex_args.iter().cloned());
    let status = run_child_plan(
        &codex_child_plan(codex_home.to_path_buf(), command_args),
        None,
    )?;
    if status.success()
        && login_request.method == LoginMethod::ApiKey
        && login_request.openai_base_url_specified
    {
        write_profile_openai_compatible_base_url(
            codex_home,
            login_request.openai_base_url.as_deref(),
        )?;
    }
    Ok(status)
}

fn write_api_key_auth_json(codex_home: &Path, api_key: &str) -> Result<()> {
    create_codex_home_if_missing(codex_home)?;
    let auth_json = serde_json::to_string_pretty(&json!({
        "auth_mode": "apikey",
        "OPENAI_API_KEY": api_key,
    }))
    .context("failed to serialize API key auth JSON")?;
    write_secret_text_file(&secret_store::auth_json_path(codex_home), &auth_json)
}

fn resolve_login_request(
    selected_profile: Option<&str>,
    codex_args: Vec<OsString>,
) -> Result<LoginRequest> {
    let (openai_base_url, openai_base_url_specified, codex_args) =
        extract_login_base_url(codex_args)?;
    let inferred_method = infer_login_method(&codex_args);
    if inferred_method == LoginMethod::ChatGpt && login_prompt_is_interactive() {
        return prompt_login_request(
            openai_base_url,
            openai_base_url_specified,
            selected_profile.is_some(),
        );
    }

    if openai_base_url_specified && inferred_method != LoginMethod::ApiKey {
        bail!("--base-url is only supported for API key login");
    }

    Ok(LoginRequest {
        method: inferred_method,
        codex_args,
        api_key: None,
        openai_base_url,
        openai_base_url_specified,
        api_key_profile_name: None,
    })
}

fn prompt_login_request(
    openai_base_url: Option<String>,
    openai_base_url_specified: bool,
    has_selected_profile: bool,
) -> Result<LoginRequest> {
    let method = prompt_login_method()?;
    match method {
        LoginMethod::ChatGpt => {
            if openai_base_url_specified {
                bail!("--base-url is only supported for API key login");
            }
            Ok(LoginRequest {
                method,
                codex_args: Vec::new(),
                api_key: None,
                openai_base_url: None,
                openai_base_url_specified: false,
                api_key_profile_name: None,
            })
        }
        LoginMethod::DeviceCode => {
            if openai_base_url_specified {
                bail!("--base-url is only supported for API key login");
            }
            Ok(LoginRequest {
                method,
                codex_args: vec![OsString::from("--device-auth")],
                api_key: None,
                openai_base_url: None,
                openai_base_url_specified: false,
                api_key_profile_name: None,
            })
        }
        LoginMethod::ApiKey => {
            let api_key = prompt_api_key()?;
            let (openai_base_url, openai_base_url_specified) = match openai_base_url {
                Some(base_url) => (Some(base_url), true),
                None => (prompt_openai_compatible_base_url()?, true),
            };
            let api_key_profile_name = if has_selected_profile {
                None
            } else {
                let default_profile_name = default_api_key_profile_name(openai_base_url.as_deref());
                Some(prompt_profile_name(&default_profile_name)?)
            };
            Ok(LoginRequest {
                method,
                codex_args: Vec::new(),
                api_key: Some(api_key),
                openai_base_url,
                openai_base_url_specified,
                api_key_profile_name,
            })
        }
        LoginMethod::AccessToken | LoginMethod::Status => Ok(LoginRequest {
            method,
            codex_args: Vec::new(),
            api_key: None,
            openai_base_url: None,
            openai_base_url_specified: false,
            api_key_profile_name: None,
        }),
    }
}

fn prompt_login_method() -> Result<LoginMethod> {
    let mut stderr = io::stderr();
    writeln!(stderr, "Choose login method:")?;
    writeln!(
        stderr,
        "  1. Sign in with ChatGPT\n     Usage included with Plus, Pro, Business, and Enterprise plans"
    )?;
    writeln!(
        stderr,
        "  2. Sign in with Device Code\n     Sign in from another device with a one-time code"
    )?;
    writeln!(
        stderr,
        "  3. Provide your own API key\n     Pay for what you use"
    )?;
    loop {
        write!(stderr, "Select login method [1]: ")?;
        stderr.flush()?;
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .context("failed to read login method")?;
        match input.trim() {
            "" | "1" => return Ok(LoginMethod::ChatGpt),
            "2" => return Ok(LoginMethod::DeviceCode),
            "3" => return Ok(LoginMethod::ApiKey),
            _ => writeln!(stderr, "Enter 1, 2, or 3.")?,
        }
    }
}

fn prompt_api_key() -> Result<String> {
    let api_key = rpassword::prompt_password("OpenAI API key: ")
        .context("failed to read OpenAI API key")?
        .trim()
        .to_string();
    if api_key.is_empty() {
        bail!("API key cannot be empty");
    }
    Ok(api_key)
}

fn prompt_openai_compatible_base_url() -> Result<Option<String>> {
    let mut stderr = io::stderr();
    write!(
        stderr,
        "OpenAI-compatible base URL, e.g. http://localhost:11434/v1 [default OpenAI]: "
    )?;
    stderr.flush()?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read base URL")?;
    normalize_optional_base_url(input.trim())
}

fn prompt_profile_name(default_profile_name: &str) -> Result<String> {
    let mut stderr = io::stderr();
    write!(stderr, "Profile name [{default_profile_name}]: ")?;
    stderr.flush()?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read profile name")?;
    let profile_name = input.trim();
    Ok(if profile_name.is_empty() {
        default_profile_name.to_string()
    } else {
        sanitize_profile_slug(profile_name)
    })
}

fn extract_login_base_url(
    codex_args: Vec<OsString>,
) -> Result<(Option<String>, bool, Vec<OsString>)> {
    let mut base_url = None;
    let mut specified = false;
    let mut filtered = Vec::with_capacity(codex_args.len());
    let mut index = 0;
    while index < codex_args.len() {
        let Some(arg) = codex_args[index].to_str() else {
            filtered.push(codex_args[index].clone());
            index += 1;
            continue;
        };

        if matches!(arg, "--base-url" | "--openai-base-url") {
            index += 1;
            let value = codex_args
                .get(index)
                .and_then(|value| value.to_str())
                .context("--base-url requires a URL value")?;
            base_url = normalize_optional_base_url(value)?;
            specified = true;
            index += 1;
            continue;
        }

        if let Some(value) = arg
            .strip_prefix("--base-url=")
            .or_else(|| arg.strip_prefix("--openai-base-url="))
        {
            base_url = normalize_optional_base_url(value)?;
            specified = true;
            index += 1;
            continue;
        }

        filtered.push(codex_args[index].clone());
        index += 1;
    }
    Ok((base_url, specified, filtered))
}

fn normalize_optional_base_url(value: &str) -> Result<Option<String>> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let parsed =
        reqwest::Url::parse(value).with_context(|| format!("invalid base URL: {value}"))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        bail!("base URL must use http or https");
    }
    Ok(Some(value.trim_end_matches('/').to_string()))
}

fn infer_login_method(codex_args: &[OsString]) -> LoginMethod {
    if codex_args
        .first()
        .and_then(|arg| arg.to_str())
        .is_some_and(|arg| arg == "status")
    {
        return LoginMethod::Status;
    }
    if codex_args.iter().any(|arg| arg == "--with-api-key") {
        return LoginMethod::ApiKey;
    }
    if codex_args.iter().any(|arg| arg == "--with-access-token") {
        return LoginMethod::AccessToken;
    }
    if codex_args.iter().any(|arg| arg == "--device-auth") {
        return LoginMethod::DeviceCode;
    }
    LoginMethod::ChatGpt
}

fn login_prompt_is_interactive() -> bool {
    io::stdin().is_terminal() && io::stderr().is_terminal()
}

fn success_exit_status() -> ExitStatus {
    ExitStatus::from_raw(0)
}

fn create_temporary_login_home(paths: &AppPaths) -> Result<PathBuf> {
    ensure_managed_profiles_root(paths)?;

    for attempt in 0..100 {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let candidate = paths
            .managed_profiles_root
            .join(format!(".login-{}-{stamp}-{attempt}", std::process::id()));
        if candidate.exists() {
            continue;
        }
        create_codex_home_if_missing(&candidate)?;
        return Ok(candidate);
    }

    bail!("failed to allocate a temporary CODEX_HOME for login")
}

fn default_api_key_profile_name(openai_base_url: Option<&str>) -> String {
    openai_base_url
        .and_then(|base_url| reqwest::Url::parse(base_url).ok())
        .and_then(|url| url.host_str().map(ToOwned::to_owned))
        .map(|host| sanitize_profile_slug(&format!("api_key_{host}")))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "api_key".to_string())
}

fn unique_profile_name_for_slug(paths: &AppPaths, state: &AppState, slug: &str) -> String {
    let base = sanitize_profile_slug(slug);
    if profile_slug_is_available(paths, state, &base) {
        return base;
    }
    for suffix in 2.. {
        let candidate = format!("{base}-{suffix}");
        if profile_slug_is_available(paths, state, &candidate) {
            return candidate;
        }
    }
    unreachable!("unbounded profile suffix search should always return")
}

fn profile_slug_is_available(paths: &AppPaths, state: &AppState, candidate: &str) -> bool {
    !state.profiles.contains_key(candidate) && !paths.managed_profiles_root.join(candidate).exists()
}

fn sanitize_profile_slug(value: &str) -> String {
    let mut slug = String::new();
    for ch in value.trim().to_ascii_lowercase().chars() {
        match ch {
            'a'..='z' | '0'..='9' | '.' | '_' | '-' => slug.push(ch),
            '@' => slug.push('_'),
            _ => slug.push('-'),
        }
    }
    let slug = slug.trim_matches(|ch| matches!(ch, '.' | '_' | '-'));
    if slug.is_empty() || slug == "." || slug == ".." {
        "api_key".to_string()
    } else {
        slug.to_string()
    }
}

pub(crate) fn handle_codex_logout(args: LogoutArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let profile_name = resolve_profile_name(&state, args.selected_profile())?;
    let codex_home = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    if !codex_home.provider.supports_codex_runtime() {
        bail!(
            "profile '{}' uses {}. `prodex logout` currently supports OpenAI/Codex profiles only.",
            profile_name,
            codex_home.provider.display_name()
        );
    }
    let codex_home = codex_home.codex_home.clone();

    let status = run_child_plan(
        &codex_child_plan(codex_home.clone(), vec![OsString::from("logout")]),
        None,
    )?;
    exit_with_status(status)
}
