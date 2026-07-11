use super::super::manage::print_profile_panel;
use super::unique_profile_name_for_slug;
use crate::{
    AppPaths, AppState, AppStateIoExt, GeminiOAuthSecret, ProfileEntry, ProfileProvider,
    managed_profile_home_path, prepare_managed_codex_home, prepare_profile_codex_home,
    remove_dir_if_exists, write_gemini_oauth_secret,
};
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

pub(super) fn prepare_gemini_profile_login_home(
    paths: &AppPaths,
    state: &AppState,
    profile_name: &str,
) -> Result<PathBuf> {
    let profile = state
        .profiles
        .get(profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    if matches!(profile.provider, ProfileProvider::Copilot { .. }) {
        bail!(
            "profile '{}' uses {}. Google sign-in supports OpenAI/Codex placeholders or Google Gemini profiles.",
            profile_name,
            profile.provider.display_name()
        );
    }
    let codex_home = profile.codex_home.clone();
    prepare_profile_codex_home(paths, profile)?;
    Ok(codex_home)
}

pub(super) fn finish_named_gemini_profile_login(
    paths: &AppPaths,
    state: &mut AppState,
    profile_name: &str,
    codex_home: &Path,
    secret: &GeminiOAuthSecret,
) -> Result<()> {
    if let Some(profile) = state.profiles.get_mut(profile_name) {
        profile.email = Some(secret.email.clone());
        profile.provider = ProfileProvider::Gemini {
            email: secret.email.clone(),
            project_id: secret.project_id.clone(),
        };
    }
    state.active_profile = Some(profile_name.to_string());
    state.save(paths)?;

    let fields = vec![
        (
            "Result".to_string(),
            format!("Signed in with Google for profile '{profile_name}'."),
        ),
        ("Account".to_string(), secret.email.clone()),
        ("Profile".to_string(), profile_name.to_string()),
        ("CODEX_HOME".to_string(), codex_home.display().to_string()),
    ];
    print_profile_panel("Login", &fields)?;
    Ok(())
}

pub(super) fn finish_auto_login_for_gemini_profile(
    paths: &AppPaths,
    state: &mut AppState,
    login_home: &Path,
    secret: &GeminiOAuthSecret,
) -> Result<()> {
    if let Some(profile_name) = state.profiles.iter().find_map(|(name, profile)| {
        profile
            .provider
            .gemini_matches(&secret.email)
            .then(|| name.clone())
    }) {
        finish_gemini_login_for_existing_profile(paths, state, login_home, &profile_name, secret)?;
        return Ok(());
    }
    finish_gemini_login_for_new_profile(paths, state, login_home, secret)
}

fn finish_gemini_login_for_existing_profile(
    paths: &AppPaths,
    state: &mut AppState,
    login_home: &Path,
    profile_name: &str,
    secret: &GeminiOAuthSecret,
) -> Result<()> {
    let codex_home = prepare_gemini_profile_login_home(paths, state, profile_name)?;
    write_gemini_oauth_secret(&codex_home, secret)?;
    remove_dir_if_exists(login_home)?;
    if let Some(profile) = state.profiles.get_mut(profile_name) {
        profile.email = Some(secret.email.clone());
        profile.provider = ProfileProvider::Gemini {
            email: secret.email.clone(),
            project_id: secret.project_id.clone(),
        };
    }
    state.active_profile = Some(profile_name.to_string());
    state.save(paths)?;

    let fields = vec![
        (
            "Result".to_string(),
            format!(
                "Signed in with Google as {}. Updated Gemini profile '{}'.",
                secret.email, profile_name
            ),
        ),
        ("Account".to_string(), secret.email.clone()),
        ("Profile".to_string(), profile_name.to_string()),
        ("CODEX_HOME".to_string(), codex_home.display().to_string()),
    ];
    print_profile_panel("Login", &fields)?;
    Ok(())
}

fn finish_gemini_login_for_new_profile(
    paths: &AppPaths,
    state: &mut AppState,
    login_home: &Path,
    secret: &GeminiOAuthSecret,
) -> Result<()> {
    let profile_name =
        unique_profile_name_for_slug(paths, state, &format!("gemini_{}", secret.email));
    let codex_home = managed_profile_home_path(paths, &profile_name)?;
    prepare_managed_codex_home(paths, &codex_home)?;
    write_gemini_oauth_secret(&codex_home, secret)?;
    remove_dir_if_exists(login_home)?;

    state.profiles.insert(
        profile_name.clone(),
        ProfileEntry {
            codex_home: codex_home.clone(),
            managed: true,
            email: Some(secret.email.clone()),
            provider: ProfileProvider::Gemini {
                email: secret.email.clone(),
                project_id: secret.project_id.clone(),
            },
        },
    );
    state.active_profile = Some(profile_name.clone());
    state.save(paths)?;

    let fields = vec![
        (
            "Result".to_string(),
            format!(
                "Signed in with Google as {}. Created Gemini profile '{profile_name}'.",
                secret.email
            ),
        ),
        ("Account".to_string(), secret.email.clone()),
        ("Profile".to_string(), profile_name),
        ("CODEX_HOME".to_string(), codex_home.display().to_string()),
    ];
    print_profile_panel("Login", &fields)?;
    Ok(())
}
