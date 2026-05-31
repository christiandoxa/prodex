use super::unique_profile_name_for_slug;
use crate::{
    AppPaths, AppState, AppStateIoExt, ProfileEntry, ProfileProvider,
    claude_oauth_profile_identity, copy_claude_oauth_credentials, create_codex_home_if_missing,
    managed_profile_home_path, prepare_managed_codex_home, print_panel, remove_dir_if_exists,
};
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

pub(super) fn prepare_anthropic_profile_login_home(
    paths: &AppPaths,
    state: &AppState,
    profile_name: &str,
) -> Result<PathBuf> {
    let profile = state
        .profiles
        .get(profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
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
    let codex_home = profile.codex_home.clone();
    if profile.managed {
        prepare_managed_codex_home(paths, &codex_home)?;
    } else {
        create_codex_home_if_missing(&codex_home)?;
    }
    Ok(codex_home)
}

pub(super) fn finish_named_anthropic_profile_login(
    paths: &AppPaths,
    state: &mut AppState,
    profile_name: &str,
    codex_home: &Path,
) -> Result<()> {
    let (account, auth_method) = claude_oauth_profile_identity(codex_home)?;
    if let Some(profile) = state.profiles.get_mut(profile_name) {
        profile.email = account.clone();
        profile.provider = ProfileProvider::Anthropic {
            account: account.clone(),
            auth_method: auth_method.clone(),
        };
    }
    state.active_profile = Some(profile_name.to_string());
    state.save(paths)?;

    let fields = vec![
        (
            "Result".to_string(),
            format!("Signed in with Claude for profile '{profile_name}'."),
        ),
        (
            "Account".to_string(),
            account.clone().unwrap_or_else(|| "-".to_string()),
        ),
        ("Profile".to_string(), profile_name.to_string()),
        ("CODEX_HOME".to_string(), codex_home.display().to_string()),
    ];
    print_panel("Login", &fields);
    Ok(())
}

pub(super) fn finish_auto_login_for_anthropic_profile(
    paths: &AppPaths,
    state: &mut AppState,
    login_home: &Path,
) -> Result<()> {
    let (account, auth_method) = claude_oauth_profile_identity(login_home)?;
    if let Some(profile_name) = state.profiles.iter().find_map(|(name, profile)| {
        profile
            .provider
            .anthropic_matches(account.as_deref(), auth_method.as_deref())
            .then(|| name.clone())
    }) {
        finish_anthropic_login_for_existing_profile(
            paths,
            state,
            login_home,
            &profile_name,
            account,
            auth_method,
        )?;
        return Ok(());
    }
    finish_anthropic_login_for_new_profile(paths, state, login_home, account, auth_method)
}

fn finish_anthropic_login_for_existing_profile(
    paths: &AppPaths,
    state: &mut AppState,
    login_home: &Path,
    profile_name: &str,
    account: Option<String>,
    auth_method: Option<String>,
) -> Result<()> {
    let codex_home = prepare_anthropic_profile_login_home(paths, state, profile_name)?;
    copy_claude_oauth_credentials(login_home, &codex_home)?;
    remove_dir_if_exists(login_home)?;
    if let Some(profile) = state.profiles.get_mut(profile_name) {
        profile.email = account.clone();
        profile.provider = ProfileProvider::Anthropic {
            account: account.clone(),
            auth_method: auth_method.clone(),
        };
    }
    state.active_profile = Some(profile_name.to_string());
    state.save(paths)?;

    let fields = vec![
        (
            "Result".to_string(),
            format!("Signed in with Claude. Updated Anthropic profile '{profile_name}'."),
        ),
        (
            "Account".to_string(),
            account.unwrap_or_else(|| "-".to_string()),
        ),
        ("Profile".to_string(), profile_name.to_string()),
        ("CODEX_HOME".to_string(), codex_home.display().to_string()),
    ];
    print_panel("Login", &fields);
    Ok(())
}

fn finish_anthropic_login_for_new_profile(
    paths: &AppPaths,
    state: &mut AppState,
    login_home: &Path,
    account: Option<String>,
    auth_method: Option<String>,
) -> Result<()> {
    let slug = account
        .as_deref()
        .map(|account| format!("claude_{account}"))
        .unwrap_or_else(|| "claude".to_string());
    let profile_name = unique_profile_name_for_slug(paths, state, &slug);
    let codex_home = managed_profile_home_path(paths, &profile_name)?;
    prepare_managed_codex_home(paths, &codex_home)?;
    copy_claude_oauth_credentials(login_home, &codex_home)?;
    remove_dir_if_exists(login_home)?;

    state.profiles.insert(
        profile_name.clone(),
        ProfileEntry {
            codex_home: codex_home.clone(),
            managed: true,
            email: account.clone(),
            provider: ProfileProvider::Anthropic {
                account: account.clone(),
                auth_method,
            },
        },
    );
    state.active_profile = Some(profile_name.clone());
    state.save(paths)?;

    let fields = vec![
        (
            "Result".to_string(),
            format!("Signed in with Claude. Created Anthropic profile '{profile_name}'."),
        ),
        (
            "Account".to_string(),
            account.unwrap_or_else(|| "-".to_string()),
        ),
        ("Profile".to_string(), profile_name),
        ("CODEX_HOME".to_string(), codex_home.display().to_string()),
    ];
    print_panel("Login", &fields);
    Ok(())
}
