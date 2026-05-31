use anyhow::{Context, Result, bail};
use std::ffi::OsString;
use std::io::{self, IsTerminal, Read, Write};
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
#[cfg(windows)]
use std::os::windows::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

mod api_key;
mod claude;
mod google;

use self::api_key::*;
use self::claude::*;
use self::google::*;
use super::write_secret_text_file;
use crate::{
    AppPaths, AppState, AppStateIoExt, CodexPassthroughArgs, LogoutArgs, ProfileEntry,
    ProfileProvider, codex_child_plan, create_codex_home_if_missing, current_cli_width,
    ensure_managed_profiles_root, exit_with_status, fetch_profile_email, fetch_profile_identity,
    find_profile_by_identity, login_with_claude_oauth, login_with_google_oauth,
    managed_profile_home_path, persist_login_home, prepare_managed_codex_home, print_panel,
    read_auth_summary, read_gemini_oauth_secret, remove_dir_if_exists, required_auth_json_text,
    resolve_profile_name, run_child_plan, terminal_height_lines, unique_profile_name_for_email,
    update_existing_profile_auth, write_gemini_oauth_secret,
    write_profile_openai_compatible_base_url,
};

const LOGIN_MENU_MIN_VISIBLE_ITEMS: usize = 3;
const LOGIN_MENU_FALLBACK_ROWS: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoginMethod {
    ChatGpt,
    DeviceCode,
    ApiKey,
    AccessToken,
    Google,
    Claude,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoginMenuAction {
    Method(LoginMethod),
    Guidance(LoginGuidanceKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoginGuidanceKind {
    GeminiApiKey,
    AnthropicApiKey,
    DeepSeekApiKey,
    CopilotImport,
}

#[derive(Debug, Clone, Copy)]
struct LoginMenuEntry {
    title: &'static str,
    provider: &'static str,
    auth: &'static str,
    usage: &'static str,
    command: &'static str,
    action: LoginMenuAction,
}

#[derive(Debug, Clone, Copy)]
struct LoginMenuLayout {
    visible_items: usize,
    compact: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoginMenuKey {
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Enter,
    Cancel,
    Digit(usize),
    Ignore,
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

    // Validate the profile exists and supports codex runtime before creating
    // a temporary login home (non-destructive check, no home prep needed yet).
    {
        let profile = state
            .profiles
            .get(&profile_name)
            .with_context(|| format!("profile '{}' is missing", profile_name))?;
        if login_request.method == LoginMethod::Google {
            if matches!(profile.provider, ProfileProvider::Copilot { .. }) {
                bail!(
                    "profile '{}' uses {}. Google sign-in supports OpenAI/Codex placeholders or Google Gemini profiles.",
                    profile_name,
                    profile.provider.display_name()
                );
            }
        } else if login_request.method == LoginMethod::Claude {
            if matches!(
                profile.provider,
                ProfileProvider::Gemini { .. } | ProfileProvider::Copilot { .. }
            ) {
                bail!(
                    "profile '{}' uses {}. Claude sign-in supports OpenAI/Codex placeholders or Anthropic Claude profiles.",
                    profile_name,
                    profile.provider.display_name()
                );
            }
        } else if !profile.provider.supports_codex_runtime() {
            bail!(
                "profile '{}' uses {}. `prodex login --profile` currently supports OpenAI/Codex profiles only.",
                profile_name,
                profile.provider.display_name()
            );
        }
    }

    // Run codex login in a temporary home so the existing auth.json is
    // preserved when login fails or is cancelled.
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
    if login_request.method == LoginMethod::Google {
        let secret = read_gemini_oauth_secret(&login_home)?;
        let codex_home = prepare_gemini_profile_login_home(paths, state, &profile_name)?;
        write_gemini_oauth_secret(&codex_home, &secret)?;
        remove_dir_if_exists(&login_home)?;
        finish_named_gemini_profile_login(paths, state, &profile_name, &codex_home, &secret)?;
        return Ok(status);
    }
    if login_request.method == LoginMethod::Claude {
        let codex_home = prepare_anthropic_profile_login_home(paths, state, &profile_name)?;
        crate::copy_claude_oauth_credentials(&login_home, &codex_home)?;
        remove_dir_if_exists(&login_home)?;
        finish_named_anthropic_profile_login(paths, state, &profile_name, &codex_home)?;
        return Ok(status);
    }

    // Login succeeded — prepare the real profile home (dirs only, never
    // deletes auth.json) and copy the new auth across.
    let codex_home = prepare_profile_login_home(paths, state, &profile_name)?;
    let auth_json = required_auth_json_text(&login_home)?;
    write_secret_text_file(&secret_store::auth_json_path(&codex_home), &auth_json)?;
    remove_dir_if_exists(&login_home)?;

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
    if login_request.method == LoginMethod::Google {
        let secret = read_gemini_oauth_secret(&login_home)?;
        finish_auto_login_for_gemini_profile(paths, state, &login_home, &secret)?;
        return Ok(status);
    }
    if login_request.method == LoginMethod::Claude {
        finish_auto_login_for_anthropic_profile(paths, state, &login_home)?;
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
    if login_request.method == LoginMethod::Google {
        login_with_google_oauth(codex_home)?;
        return Ok(success_exit_status());
    }
    if login_request.method == LoginMethod::Claude {
        return login_with_claude_oauth(codex_home, None);
    }

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
    let method = loop {
        match prompt_login_menu_action()? {
            LoginMenuAction::Method(method) => break method,
            LoginMenuAction::Guidance(kind) => show_login_guidance(kind)?,
        }
    };
    login_request_for_method(
        method,
        openai_base_url,
        openai_base_url_specified,
        has_selected_profile,
    )
}

fn login_request_for_method(
    method: LoginMethod,
    openai_base_url: Option<String>,
    openai_base_url_specified: bool,
    has_selected_profile: bool,
) -> Result<LoginRequest> {
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
        LoginMethod::Google => {
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
        LoginMethod::Claude => {
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

fn login_menu_entries() -> &'static [LoginMenuEntry] {
    const ENTRIES: &[LoginMenuEntry] = &[
        LoginMenuEntry {
            title: "Sign in with ChatGPT (OpenAI OAuth)",
            provider: "OpenAI / Codex",
            auth: "ChatGPT OAuth",
            usage: "Default quota-aware profile pool for prodex run, prodex s, and prodex caveman.",
            command: "prodex login",
            action: LoginMenuAction::Method(LoginMethod::ChatGpt),
        },
        LoginMenuEntry {
            title: "OpenAI device code",
            provider: "OpenAI / Codex",
            auth: "Device-code OAuth",
            usage: "Same OpenAI/Codex profile type, useful on a terminal without a local browser.",
            command: "prodex login --device-auth",
            action: LoginMenuAction::Method(LoginMethod::DeviceCode),
        },
        LoginMenuEntry {
            title: "Provide your own API key (OpenAI/API-compatible)",
            provider: "OpenAI / local / OpenAI-compatible endpoint",
            auth: "API key stored in the selected Prodex profile",
            usage: "Use OpenAI API billing, or provide a compatible base URL for local/custom endpoints.",
            command: "prodex login --with-api-key [--base-url URL]",
            action: LoginMenuAction::Method(LoginMethod::ApiKey),
        },
        LoginMenuEntry {
            title: "Google Gemini OAuth",
            provider: "Google Gemini",
            auth: "Google OAuth profile",
            usage: "Reusable Gemini profile for prodex s --provider gemini, including OAuth rotation.",
            command: "prodex login --with-google",
            action: LoginMenuAction::Method(LoginMethod::Google),
        },
        LoginMenuEntry {
            title: "Google Gemini API key",
            provider: "Google Gemini",
            auth: "Runtime API key only",
            usage: "Not a persisted login profile; pass GEMINI_API_KEY or --api-key when launching Gemini.",
            command: "GEMINI_API_KEY=... prodex s --provider gemini --model gemini-2.5-pro",
            action: LoginMenuAction::Guidance(LoginGuidanceKind::GeminiApiKey),
        },
        LoginMenuEntry {
            title: "Anthropic Claude OAuth",
            provider: "Anthropic Claude",
            auth: "Claude Code OAuth profile",
            usage: "Reusable Anthropic profile for prodex s --provider anthropic without API-key storage.",
            command: "prodex login --with-claude",
            action: LoginMenuAction::Method(LoginMethod::Claude),
        },
        LoginMenuEntry {
            title: "Anthropic API key",
            provider: "Anthropic Claude",
            auth: "Runtime API key only",
            usage: "Not a persisted login profile; pass ANTHROPIC_API_KEY, ANTHROPIC_API_KEYS, or --api-key.",
            command: "ANTHROPIC_API_KEY=... prodex s --provider anthropic --model claude-sonnet-4-6",
            action: LoginMenuAction::Guidance(LoginGuidanceKind::AnthropicApiKey),
        },
        LoginMenuEntry {
            title: "DeepSeek API key",
            provider: "DeepSeek",
            auth: "Runtime API key only",
            usage: "DeepSeek has no OAuth login in Prodex; use an API key for the provider adapter.",
            command: "DEEPSEEK_API_KEY=... prodex s --provider deepseek --model deepseek-v4-pro",
            action: LoginMenuAction::Guidance(LoginGuidanceKind::DeepSeekApiKey),
        },
        LoginMenuEntry {
            title: "GitHub Copilot import",
            provider: "GitHub Copilot",
            auth: "Existing Copilot CLI account import",
            usage: "Record the Copilot identity from local Copilot CLI state, then launch with --provider copilot.",
            command: "prodex profile import copilot",
            action: LoginMenuAction::Guidance(LoginGuidanceKind::CopilotImport),
        },
    ];
    ENTRIES
}

fn prompt_login_menu_action() -> Result<LoginMenuAction> {
    if let Some(action) = prompt_login_menu_action_raw()? {
        return Ok(action);
    }
    prompt_login_menu_action_numbered()
}

fn prompt_login_menu_action_raw() -> Result<Option<LoginMenuAction>> {
    let entries = login_menu_entries();
    let _raw_mode = match LoginRawMode::enter() {
        Ok(raw_mode) => raw_mode,
        Err(_) => return Ok(None),
    };
    let _screen = match LoginMenuScreen::enter() {
        Ok(screen) => screen,
        Err(_) => return Ok(None),
    };
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let mut selected = 0usize;
    let mut offset = 0usize;

    loop {
        let layout = login_menu_layout_for_rows(
            terminal_height_lines().unwrap_or(LOGIN_MENU_FALLBACK_ROWS),
            entries.len(),
        );
        offset = login_menu_window_offset(selected, offset, layout.visible_items, entries.len());
        render_login_menu_to_stderr(entries, selected, offset, layout)?;
        match read_login_menu_key(&mut stdin).context("failed to read login menu key")? {
            LoginMenuKey::Up => {
                selected = selected.saturating_sub(1);
            }
            LoginMenuKey::Down => {
                selected = (selected + 1).min(entries.len().saturating_sub(1));
            }
            LoginMenuKey::PageUp => {
                let step = layout.visible_items.saturating_sub(1).max(1);
                selected = selected.saturating_sub(step);
            }
            LoginMenuKey::PageDown => {
                let step = layout.visible_items.saturating_sub(1).max(1);
                selected = (selected + step).min(entries.len().saturating_sub(1));
            }
            LoginMenuKey::Home => {
                selected = 0;
            }
            LoginMenuKey::End => {
                selected = entries.len().saturating_sub(1);
            }
            LoginMenuKey::Enter => return Ok(Some(entries[selected].action)),
            LoginMenuKey::Digit(index) => {
                if let Some(entry) = entries.get(index.saturating_sub(1)) {
                    return Ok(Some(entry.action));
                }
            }
            LoginMenuKey::Cancel => bail!("login cancelled"),
            LoginMenuKey::Ignore => {}
        }
    }
}

fn prompt_login_menu_action_numbered() -> Result<LoginMenuAction> {
    let entries = login_menu_entries();
    let mut stderr = io::stderr();
    writeln!(stderr, "Choose login method:")?;
    for (index, entry) in entries.iter().enumerate() {
        writeln!(stderr, "  {}. {}", index + 1, entry.title)?;
        writeln!(stderr, "     Provider: {}", entry.provider)?;
        writeln!(stderr, "     Auth: {}", entry.auth)?;
        writeln!(stderr, "     Use: {}", entry.usage)?;
        writeln!(stderr, "     Command: {}", entry.command)?;
    }
    loop {
        write!(stderr, "Select login method [1]: ")?;
        stderr.flush()?;
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .context("failed to read login method")?;
        let selected = input.trim();
        if selected.is_empty() {
            return Ok(entries[0].action);
        }
        if let Ok(index) = selected.parse::<usize>()
            && (1..=entries.len()).contains(&index)
        {
            return Ok(entries[index - 1].action);
        }
        writeln!(stderr, "Enter 1 through {}.", entries.len())?;
    }
}

fn show_login_guidance(kind: LoginGuidanceKind) -> Result<()> {
    let entry = login_menu_entries()
        .iter()
        .find(|entry| entry.action == LoginMenuAction::Guidance(kind))
        .context("login guidance entry is missing")?;
    let mut stderr = io::stderr();
    writeln!(stderr)?;
    writeln!(stderr, "Provider guidance: {}", entry.title)?;
    writeln!(stderr, "  Provider: {}", entry.provider)?;
    writeln!(stderr, "  Auth: {}", entry.auth)?;
    writeln!(stderr, "  Use: {}", entry.usage)?;
    writeln!(stderr, "  Command: {}", entry.command)?;
    writeln!(
        stderr,
        "  Note: this path is selected at runtime, not stored by prodex login."
    )?;
    writeln!(stderr)?;
    write!(
        stderr,
        "Press Enter to return to login methods, or Ctrl-C to exit."
    )?;
    stderr.flush()?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read login guidance acknowledgement")?;
    Ok(())
}

fn login_menu_layout_for_rows(rows: usize, entry_count: usize) -> LoginMenuLayout {
    if entry_count == 0 {
        return LoginMenuLayout {
            visible_items: 0,
            compact: true,
        };
    }

    let rows = rows.max(8);
    let compact = rows < 16;
    let reserved_rows = if compact { 5 } else { 7 };
    let min_visible = LOGIN_MENU_MIN_VISIBLE_ITEMS.min(entry_count);
    let visible_items = rows
        .saturating_sub(reserved_rows)
        .max(min_visible)
        .min(entry_count);
    LoginMenuLayout {
        visible_items,
        compact,
    }
}

fn login_menu_window_offset(
    selected: usize,
    current_offset: usize,
    visible_items: usize,
    entry_count: usize,
) -> usize {
    if entry_count == 0 || visible_items == 0 {
        return 0;
    }
    let max_offset = entry_count.saturating_sub(visible_items);
    if selected < current_offset {
        selected.min(max_offset)
    } else if selected >= current_offset.saturating_add(visible_items) {
        selected
            .saturating_add(1)
            .saturating_sub(visible_items)
            .min(max_offset)
    } else {
        current_offset.min(max_offset)
    }
}

fn render_login_menu_to_stderr(
    entries: &[LoginMenuEntry],
    selected: usize,
    offset: usize,
    layout: LoginMenuLayout,
) -> Result<()> {
    let lines = render_login_menu(entries, selected, offset, layout, current_cli_width());
    let mut stderr = io::stderr();
    write!(stderr, "\x1b[H\x1b[2J")?;
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            write!(stderr, "\r\n")?;
        }
        write!(stderr, "{line}")?;
    }
    stderr.flush()?;
    Ok(())
}

fn render_login_menu(
    entries: &[LoginMenuEntry],
    selected: usize,
    offset: usize,
    layout: LoginMenuLayout,
    width: usize,
) -> Vec<String> {
    if entries.is_empty() {
        return vec![fit_login_menu_line(
            "Prodex login: no login methods available",
            width,
        )];
    }

    let width = width.max(40);
    let selected = selected.min(entries.len().saturating_sub(1));
    let offset = offset.min(entries.len().saturating_sub(1));
    let visible_items = layout.visible_items.max(1).min(entries.len());
    let end = offset.saturating_add(visible_items).min(entries.len());
    let mut lines = Vec::new();
    lines.push(fit_login_menu_line(
        &format!(
            "Select login method - showing {}-{} of {}",
            offset + 1,
            end,
            entries.len()
        ),
        width,
    ));
    lines.push(fit_login_menu_line(
        "Up/Down move, PageUp/PageDown scroll, Enter select, 1-9 quick select, q cancel",
        width,
    ));
    for (index, entry) in entries.iter().enumerate().take(end).skip(offset) {
        let marker = if index == selected {
            ">"
        } else if index == offset && offset > 0 {
            "^"
        } else if index + 1 == end && end < entries.len() {
            "v"
        } else {
            " "
        };
        lines.push(fit_login_menu_line(
            &format!(
                "{marker} {:>2}. {} [{}]",
                index + 1,
                entry.title,
                entry.auth
            ),
            width,
        ));
    }

    let entry = &entries[selected];
    lines.push(String::new());
    if layout.compact {
        lines.push(fit_login_menu_line(
            &format!("{} | {}", entry.provider, entry.auth),
            width,
        ));
        lines.push(fit_login_menu_line(
            &format!("Use/Cmd: {}; {}", entry.usage, entry.command),
            width,
        ));
    } else {
        lines.push(fit_login_menu_line(
            &format!("Provider: {}", entry.provider),
            width,
        ));
        lines.push(fit_login_menu_line(&format!("Auth: {}", entry.auth), width));
        lines.push(fit_login_menu_line(&format!("Use: {}", entry.usage), width));
        lines.push(fit_login_menu_line(
            &format!("Command: {}", entry.command),
            width,
        ));
    }
    lines
}

fn fit_login_menu_line(value: &str, width: usize) -> String {
    let width = width.max(4);
    let char_count = value.chars().count();
    if char_count <= width {
        return value.to_string();
    }
    let mut output: String = value.chars().take(width.saturating_sub(3)).collect();
    output.push_str("...");
    output
}

fn read_login_menu_key<R: Read>(reader: &mut R) -> io::Result<LoginMenuKey> {
    let first = loop {
        if let Some(byte) = read_login_menu_byte(reader)? {
            break byte;
        }
    };
    match first {
        b'\r' | b'\n' => Ok(LoginMenuKey::Enter),
        3 | 4 => Ok(LoginMenuKey::Cancel),
        b'q' | b'Q' => Ok(LoginMenuKey::Cancel),
        b'k' | b'K' => Ok(LoginMenuKey::Up),
        b'j' | b'J' => Ok(LoginMenuKey::Down),
        b'u' | b'U' => Ok(LoginMenuKey::PageUp),
        b'd' | b'D' => Ok(LoginMenuKey::PageDown),
        b'g' => Ok(LoginMenuKey::Home),
        b'G' => Ok(LoginMenuKey::End),
        b'1'..=b'9' => Ok(LoginMenuKey::Digit((first - b'0') as usize)),
        27 => read_login_menu_escape_key(reader),
        _ => Ok(LoginMenuKey::Ignore),
    }
}

fn read_login_menu_escape_key<R: Read>(reader: &mut R) -> io::Result<LoginMenuKey> {
    let Some(first) = read_login_menu_byte(reader)? else {
        return Ok(LoginMenuKey::Cancel);
    };
    if first != b'[' && first != b'O' {
        return Ok(LoginMenuKey::Cancel);
    }
    let Some(second) = read_login_menu_byte(reader)? else {
        return Ok(LoginMenuKey::Cancel);
    };
    match second {
        b'A' => Ok(LoginMenuKey::Up),
        b'B' => Ok(LoginMenuKey::Down),
        b'H' => Ok(LoginMenuKey::Home),
        b'F' => Ok(LoginMenuKey::End),
        b'5' => {
            let _ = read_login_menu_byte(reader)?;
            Ok(LoginMenuKey::PageUp)
        }
        b'6' => {
            let _ = read_login_menu_byte(reader)?;
            Ok(LoginMenuKey::PageDown)
        }
        b'1' | b'7' => {
            let _ = read_login_menu_byte(reader)?;
            Ok(LoginMenuKey::Home)
        }
        b'4' | b'8' => {
            let _ = read_login_menu_byte(reader)?;
            Ok(LoginMenuKey::End)
        }
        _ => Ok(LoginMenuKey::Ignore),
    }
}

fn read_login_menu_byte<R: Read>(reader: &mut R) -> io::Result<Option<u8>> {
    let mut byte = [0u8; 1];
    match reader.read(&mut byte) {
        Ok(0) => Ok(None),
        Ok(_) => Ok(Some(byte[0])),
        Err(err) if err.kind() == io::ErrorKind::Interrupted => Ok(None),
        Err(err) => Err(err),
    }
}

struct LoginRawMode {
    saved_mode: String,
}

impl LoginRawMode {
    fn enter() -> Result<Self> {
        let output = Command::new("stty")
            .arg("-g")
            .stdin(Stdio::inherit())
            .stderr(Stdio::null())
            .output()
            .context("failed to read terminal mode")?;
        if !output.status.success() {
            bail!("failed to read terminal mode");
        }
        let saved_mode = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if saved_mode.is_empty() {
            bail!("terminal mode was empty");
        }
        let status = Command::new("stty")
            .args(["raw", "-echo", "min", "0", "time", "1"])
            .stdin(Stdio::inherit())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("failed to enable raw terminal mode")?;
        if !status.success() {
            bail!("failed to enable raw terminal mode");
        }
        Ok(Self { saved_mode })
    }
}

impl Drop for LoginRawMode {
    fn drop(&mut self) {
        let _ = Command::new("stty")
            .arg(&self.saved_mode)
            .stdin(Stdio::inherit())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

struct LoginMenuScreen;

impl LoginMenuScreen {
    fn enter() -> Result<Self> {
        let mut stderr = io::stderr();
        write!(stderr, "\x1b[?1049h\x1b[?25l")?;
        stderr.flush()?;
        Ok(Self)
    }
}

impl Drop for LoginMenuScreen {
    fn drop(&mut self) {
        let mut stderr = io::stderr();
        let _ = write!(stderr, "\x1b[?25h\x1b[?1049l");
        let _ = stderr.flush();
    }
}

fn prompt_api_key() -> Result<String> {
    let api_key = rpassword::prompt_password("OpenAI/OpenAI-compatible API key: ")
        .context("failed to read API key")?
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
        "OpenAI-compatible base URL, e.g. http://localhost:11434/v1 [default https://api.openai.com/v1]: "
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
    if codex_args
        .iter()
        .any(|arg| arg == "--with-google" || arg == "--google")
    {
        return LoginMethod::Google;
    }
    if codex_args
        .iter()
        .any(|arg| arg == "--with-claude" || arg == "--claude")
    {
        return LoginMethod::Claude;
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

#[cfg(test)]
mod login_menu_tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn login_menu_entries_explain_runtime_only_api_key_providers() {
        let entries = login_menu_entries();
        assert!(entries.len() > 5);

        let deepseek = entries
            .iter()
            .find(|entry| entry.title == "DeepSeek API key")
            .expect("deepseek guidance should be listed");
        assert_eq!(
            deepseek.action,
            LoginMenuAction::Guidance(LoginGuidanceKind::DeepSeekApiKey)
        );
        assert_eq!(deepseek.auth, "Runtime API key only");
        assert!(deepseek.usage.contains("no OAuth login"));

        let gemini_api_key = entries
            .iter()
            .find(|entry| entry.title == "Google Gemini API key")
            .expect("gemini API key guidance should be listed");
        assert_eq!(
            gemini_api_key.action,
            LoginMenuAction::Guidance(LoginGuidanceKind::GeminiApiKey)
        );
        assert!(gemini_api_key.command.contains("GEMINI_API_KEY"));

        let google_oauth = entries
            .iter()
            .find(|entry| entry.title == "Google Gemini OAuth")
            .expect("gemini OAuth login should be listed");
        assert_eq!(
            google_oauth.action,
            LoginMenuAction::Method(LoginMethod::Google)
        );
        assert!(google_oauth.auth.contains("OAuth"));
    }

    #[test]
    fn login_menu_layout_and_render_fit_short_terminals() {
        let entries = login_menu_entries();
        let layout = login_menu_layout_for_rows(8, entries.len());
        assert!(layout.compact);
        assert_eq!(layout.visible_items, 3);

        let selected = entries
            .iter()
            .position(|entry| entry.title == "DeepSeek API key")
            .expect("deepseek guidance should be listed");
        let offset = login_menu_window_offset(selected, 0, layout.visible_items, entries.len());
        let lines = render_login_menu(entries, selected, offset, layout, 50);

        assert_eq!(lines.len(), 8);
        assert!(lines.iter().any(|line| line.contains("DeepSeek API key")));
        assert!(lines.iter().any(|line| line.contains("Use/Cmd:")));
        assert!(lines.iter().all(|line| line.chars().count() <= 50));
    }

    #[test]
    fn login_menu_window_keeps_selected_item_visible() {
        assert_eq!(login_menu_window_offset(0, 0, 3, 9), 0);
        assert_eq!(login_menu_window_offset(2, 0, 3, 9), 0);
        assert_eq!(login_menu_window_offset(3, 0, 3, 9), 1);
        assert_eq!(login_menu_window_offset(8, 1, 3, 9), 6);
        assert_eq!(login_menu_window_offset(2, 6, 3, 9), 2);
        assert_eq!(login_menu_window_offset(8, 0, 20, 9), 0);
    }

    #[test]
    fn login_menu_reads_common_arrow_keys() {
        let mut up = Cursor::new(b"\x1b[A".to_vec());
        assert_eq!(read_login_menu_key(&mut up).unwrap(), LoginMenuKey::Up);

        let mut down = Cursor::new(b"\x1b[B".to_vec());
        assert_eq!(read_login_menu_key(&mut down).unwrap(), LoginMenuKey::Down);

        let mut page_down = Cursor::new(b"\x1b[6~".to_vec());
        assert_eq!(
            read_login_menu_key(&mut page_down).unwrap(),
            LoginMenuKey::PageDown
        );

        let mut digit = Cursor::new(b"8".to_vec());
        assert_eq!(
            read_login_menu_key(&mut digit).unwrap(),
            LoginMenuKey::Digit(8)
        );
    }
}
