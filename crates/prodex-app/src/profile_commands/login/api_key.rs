use super::super::manage::print_profile_panel;
use super::super::write_secret_text_file;
use super::{default_api_key_profile_name, unique_profile_name_for_slug};
use crate::{
    AppPaths, AppState, AppStateIoExt, ProfileEntry, ProfileProvider, create_codex_home_if_missing,
    managed_profile_home_path, persist_login_home, prepare_managed_codex_home,
    remove_dir_if_exists, update_existing_profile_auth, write_profile_openai_compatible_base_url,
};
use anyhow::{Context, Result, bail};
use serde_json::json;
use std::path::Path;

pub(super) fn finish_auto_login_for_api_key_profile(
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
    print_profile_panel("Login", &fields)?;
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
    print_profile_panel("Login", &fields)?;
    Ok(())
}

pub(super) fn write_api_key_auth_json(codex_home: &Path, api_key: &str) -> Result<()> {
    create_codex_home_if_missing(codex_home)?;
    let auth_json = serde_json::to_string_pretty(&json!({
        "auth_mode": "apikey",
        "OPENAI_API_KEY": api_key,
    }))
    .context("failed to serialize API key auth JSON")?;
    write_secret_text_file(&secret_store::auth_json_path(codex_home), &auth_json)
}
