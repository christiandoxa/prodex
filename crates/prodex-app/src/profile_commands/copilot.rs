use anyhow::{Context, Result, bail};
use dirs::home_dir;
use reqwest::blocking::Client;
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use zeroize::Zeroizing;

use super::import_export::{
    ProfileAuthUpdate, ProfileLifecycleHomeAction, ProfileLifecyclePlan,
    acquire_profile_lifecycle_lock, cleanup_profile_lifecycle_and_auth_journal,
    lifecycle_profile_state, load_profile_state_with_profile_recovery_locked,
    prepare_existing_profile_lifecycle, write_profile_lifecycle_plan,
};
use super::manage::print_profile_panel;
use crate::{
    AppPaths, AppState, AppStateIoExt, ImportProfileArgs, ProfileEntry, ProfileProvider,
    QUOTA_HTTP_CONNECT_TIMEOUT_MS, QUOTA_HTTP_READ_TIMEOUT_MS,
    RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES, absolutize, activate_profile, audit_log_event,
    create_codex_home_if_missing, ensure_path_is_unique, format_response_body,
    managed_profile_home_path, prepare_managed_codex_home, read_blocking_response_body_with_limit,
};

pub(crate) use prodex_profile_export::CopilotUserInfo;
use prodex_profile_export::{
    CopilotConfigFile, CopilotProfileImportStatePlan, CopilotProfileImportSummary,
    copilot_account_key, copilot_profile_import_summary_fields, copilot_token_from_config,
    copilot_user_api_origin, parse_copilot_config_file, parse_copilot_user_info_json_response,
    parse_copilot_user_info_value, plan_copilot_profile_import, plan_copilot_profile_import_state,
    select_copilot_logged_in_user,
};

const COPILOT_KEYCHAIN_SERVICE: &str = "copilot-cli";

mod keychain;
mod runtime_auth;
pub(crate) use self::runtime_auth::resolve_copilot_runtime_api_auth;
use keychain::{read_copilot_keychain_token, read_copilot_libsecret_token, read_copilot_sdk_token};

struct CopilotImportContext {
    host: String,
    login: String,
    token: String,
}

impl fmt::Debug for CopilotImportContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CopilotImportContext")
            .field("host", &"<redacted>")
            .field("login", &"<redacted>")
            .field("token", &"<redacted>")
            .finish()
    }
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
    let _lock = acquire_profile_lifecycle_lock(&paths)?;
    let (mut state, _) = load_profile_state_with_profile_recovery_locked(&paths, true)?;
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
            let mut desired_profile = state
                .profiles
                .get(&existing_name)
                .with_context(|| format!("profile '{}' is missing", existing_name))?
                .clone();
            desired_profile.provider = provider.clone();
            desired_profile.email = Some(context.login.clone());
            let next_active_profile = if activate {
                Some(existing_name.clone())
            } else {
                state.active_profile.clone()
            };
            let (lifecycle_path, auth_journal_path) = prepare_existing_profile_lifecycle(
                &paths,
                "import",
                &state,
                &existing_name,
                &desired_profile,
                next_active_profile,
                ProfileAuthUpdate {
                    next_auth_json: None,
                    next_provider_json: Some(serde_json::to_string(&desired_profile.provider)?),
                    next_secret_files: Vec::new(),
                    previous_secret_file_paths: &[],
                    temporary_home: None,
                },
            )?;
            let profile = state
                .profiles
                .get_mut(&existing_name)
                .with_context(|| format!("profile '{}' is missing", existing_name))?;
            profile.provider = provider.clone();
            profile.email = Some(context.login.clone());
            if activate {
                activate_profile(&mut state, &existing_name);
            }
            state.save(&paths)?;
            cleanup_profile_lifecycle_and_auth_journal(&lifecycle_path, &auth_journal_path)?;

            audit_log_event(
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
            )?;

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
            print_profile_panel("Profile Updated", &fields)?;
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
    let desired_profile = ProfileEntry {
        codex_home: codex_home.clone(),
        managed: true,
        email: Some(context.login.clone()),
        provider: provider.clone(),
    };
    let next_active_profile = if activate {
        Some(profile_name.clone())
    } else {
        state.active_profile.clone()
    };
    let lifecycle_path = write_profile_lifecycle_plan(
        &paths,
        "import",
        &ProfileLifecyclePlan {
            profile_states: vec![lifecycle_profile_state(
                &profile_name,
                None,
                Some(&desired_profile),
            )?],
            previous_active_profile: state.active_profile.clone(),
            next_active_profile,
            home_actions: vec![ProfileLifecycleHomeAction::Create {
                path: codex_home.display().to_string(),
            }],
            auth_journal_paths: Vec::new(),
        },
    )?;
    create_codex_home_if_missing(&codex_home)?;
    prepare_managed_codex_home(&paths, &codex_home)?;

    state.profiles.insert(profile_name.clone(), desired_profile);
    if activate {
        activate_profile(&mut state, &profile_name);
    }
    state.save(&paths)?;

    audit_log_event(
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
    )?;

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
    print_profile_panel("Profile Added", &fields)?;
    prodex_profile_export::cleanup_profile_lifecycle_journal(&lifecycle_path);
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
        crate::profile_name_is_available(paths, state, candidate)
    })
}

fn resolve_copilot_import_context() -> Result<CopilotImportContext> {
    let config = read_copilot_config()?;
    let users = copilot_import_candidate_users(&config);
    if users.is_empty() {
        bail!("no logged-in Copilot user found in config.json");
    }

    for user in &users {
        if let Ok(token) =
            resolve_copilot_account_token_from_config(&config, &user.host, &user.login)
        {
            return Ok(CopilotImportContext {
                host: user.host.clone(),
                login: user.login.clone(),
                token,
            });
        }
    }

    bail!("failed to resolve a stored Copilot token for any logged-in user from config or keychain")
}

fn copilot_import_candidate_users(
    config: &CopilotConfigFile,
) -> Vec<prodex_profile_export::CopilotConfigUser> {
    let mut users = Vec::new();
    if let Some(user) = select_copilot_logged_in_user(config) {
        users.push(user);
    }
    for user in &config.logged_in_users {
        if !users
            .iter()
            .any(|existing| existing.host == user.host && existing.login == user.login)
        {
            users.push(user.clone());
        }
    }
    users
}

fn read_copilot_config() -> Result<CopilotConfigFile> {
    let config_root = discover_copilot_config_root()?;
    let config_path = config_root.join("config.json");
    let raw = Zeroizing::new(
        fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read {}", config_path.display()))?,
    );
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

fn resolve_copilot_account_token_from_config(
    config: &CopilotConfigFile,
    host: &str,
    login: &str,
) -> Result<String> {
    let account_key = copilot_account_key(host, login);
    if let Some(token) = copilot_token_from_config(config, host, login) {
        return Ok(token);
    }
    let mut failures = Vec::new();
    for (backend, result) in [
        ("keychain", read_copilot_keychain_token(&account_key)),
        ("libsecret", read_copilot_libsecret_token(&account_key)),
        ("SDK", read_copilot_sdk_token(host, login)),
    ] {
        match result {
            Ok(Some(token)) => return Ok(token),
            Ok(None) => {}
            Err(error) => failures.push(format!(
                "{backend}: {}",
                redaction::redaction_redact_secret_like_text(&format!("{error:#}"))
            )),
        }
    }
    let detail = if failures.is_empty() {
        String::new()
    } else {
        format!("; credential backend failures: {}", failures.join("; "))
    };
    bail!(
        "failed to resolve the stored Copilot token for {} from config or keychain{}",
        account_key,
        detail
    )
}

pub(crate) fn resolve_copilot_account_token(host: &str, login: &str) -> Result<String> {
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
    let body = read_blocking_response_body_with_limit(
        response,
        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
        &format!("failed to read {}", user_url),
    )?;
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

#[cfg(test)]
#[path = "copilot_tests.rs"]
mod tests;
