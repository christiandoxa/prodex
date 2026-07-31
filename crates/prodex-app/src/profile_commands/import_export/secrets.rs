use anyhow::{Context, Result, bail};
use chrono::Local;
use prodex_profile_export::{
    ImportedExistingProfileAuthUpdateJournal, ImportedExistingProfileFileUpdate,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use super::super::kiro::{
    KIRO_CREDENTIALS_FILE, KIRO_MODEL_CATALOG_FILE, parse_kiro_auth_secret_text,
    parse_kiro_model_catalog_text,
};
use crate::runtime_claude_auth::{CLAUDE_CREDENTIALS_FILE, parse_claude_oauth_secret_text};
use crate::runtime_gemini_auth::{GEMINI_OAUTH_SECRET_FILE, GeminiOAuthSecret};
use crate::{
    AppPaths, ExportedProfile, ImportedExistingProfileAuthUpdate, ProfileExportPayload,
    ProfileProvider, runtime_random_token,
};

pub(crate) fn write_secret_text_file(path: &Path, content: &str) -> Result<()> {
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(&secret_store::SecretLocation::file(path), content)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("failed to write {}", path.display()))
}

pub(super) fn profile_import_exported_profile(
    payload: &ProfileExportPayload,
    source_index: usize,
) -> Result<&ExportedProfile> {
    payload
        .profiles
        .get(source_index)
        .with_context(|| format!("import plan source index {} is missing", source_index))
}

pub(super) fn validate_exported_secret_files(exported: &ExportedProfile) -> Result<()> {
    let required_files = required_exported_secret_file_names(&exported.provider);
    let allowed_files = allowed_exported_secret_file_names(&exported.provider);
    let mut seen_paths = BTreeSet::new();

    for secret_file in &exported.secret_files {
        validate_exported_secret_file_path(&secret_file.path, &exported.name)?;
        if !seen_paths.insert(secret_file.path.clone()) {
            bail!(
                "profile export bundle contains duplicate secret file '{}' for profile '{}'",
                secret_file.path,
                exported.name
            );
        }
        if !allowed_files.contains(&secret_file.path.as_str()) {
            bail!(
                "profile export bundle contains unexpected secret file '{}' for profile '{}'",
                secret_file.path,
                exported.name
            );
        }
        validate_exported_secret_file_content(exported, &secret_file.path, &secret_file.text)?;
    }

    for required_file in required_files {
        if !seen_paths.contains(*required_file) {
            bail!(
                "profile export bundle is missing secret file '{}' for profile '{}'",
                required_file,
                exported.name
            );
        }
    }

    Ok(())
}

pub(super) fn validate_exported_secret_file_path(path: &str, profile_name: &str) -> Result<()> {
    if path.trim().is_empty()
        || Path::new(path).is_absolute()
        || path.contains('/')
        || path.contains('\\')
        || matches!(path, "." | "..")
    {
        bail!(
            "profile export bundle contains unsafe secret file path '{}' for profile '{}'",
            path,
            profile_name
        );
    }
    Ok(())
}

fn validate_exported_secret_file_content(
    exported: &ExportedProfile,
    path: &str,
    text: &str,
) -> Result<()> {
    match &exported.provider {
        ProfileProvider::Gemini { .. } => {
            let _: GeminiOAuthSecret = serde_json::from_str(text).with_context(|| {
                format!(
                    "failed to parse exported secret file '{}' for profile '{}'",
                    path, exported.name
                )
            })?;
        }
        ProfileProvider::Anthropic { .. } => {
            parse_claude_oauth_secret_text(text).with_context(|| {
                format!(
                    "failed to parse exported secret file '{}' for profile '{}'",
                    path, exported.name
                )
            })?;
        }
        ProfileProvider::Kiro { .. } => {
            if path == KIRO_CREDENTIALS_FILE {
                parse_kiro_auth_secret_text(text).with_context(|| {
                    format!(
                        "failed to parse exported secret file '{}' for profile '{}'",
                        path, exported.name
                    )
                })?;
            } else if path == KIRO_MODEL_CATALOG_FILE {
                parse_kiro_model_catalog_text(text).with_context(|| {
                    format!(
                        "failed to parse exported secret file '{}' for profile '{}'",
                        path, exported.name
                    )
                })?;
            }
        }
        ProfileProvider::Openai | ProfileProvider::Copilot { .. } | ProfileProvider::Agy { .. } => {
        }
    }
    Ok(())
}

fn required_exported_secret_file_names(provider: &ProfileProvider) -> &'static [&'static str] {
    match provider {
        ProfileProvider::Gemini { .. } => &[GEMINI_OAUTH_SECRET_FILE],
        ProfileProvider::Anthropic { .. } => &[CLAUDE_CREDENTIALS_FILE],
        ProfileProvider::Kiro { .. } => &[KIRO_CREDENTIALS_FILE],
        ProfileProvider::Openai | ProfileProvider::Copilot { .. } | ProfileProvider::Agy { .. } => {
            &[]
        }
    }
}

fn allowed_exported_secret_file_names(provider: &ProfileProvider) -> &'static [&'static str] {
    match provider {
        ProfileProvider::Gemini { .. } => &[GEMINI_OAUTH_SECRET_FILE],
        ProfileProvider::Anthropic { .. } => &[CLAUDE_CREDENTIALS_FILE],
        ProfileProvider::Kiro { .. } => &[KIRO_CREDENTIALS_FILE, KIRO_MODEL_CATALOG_FILE],
        ProfileProvider::Openai | ProfileProvider::Copilot { .. } | ProfileProvider::Agy { .. } => {
            &[]
        }
    }
}

pub(super) fn write_exported_secret_files(
    codex_home: &Path,
    exported: &ExportedProfile,
) -> Result<()> {
    for secret_file in &exported.secret_files {
        validate_exported_secret_file_path(&secret_file.path, &exported.name)?;
        write_secret_text_file(&codex_home.join(&secret_file.path), &secret_file.text)?;
    }
    Ok(())
}

pub(crate) fn read_optional_secret_text_file(path: &Path) -> Result<Option<String>> {
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .read_text(&secret_store::SecretLocation::file(path))
        .map_err(anyhow::Error::new)
        .with_context(|| format!("failed to read {}", path.display()))
}

pub(super) fn restore_optional_secret_text_file(
    path: &Path,
    previous_text: Option<&str>,
) -> Result<()> {
    if let Some(previous_text) = previous_text {
        return write_secret_text_file(path, previous_text);
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("failed to remove {}", path.display())),
    }
}

pub(crate) fn write_imported_auth_update_journal(
    paths: &AppPaths,
    rollback: &ImportedExistingProfileAuthUpdate,
    next_email: Option<String>,
    next_auth_json: Option<String>,
    next_provider_json: Option<String>,
    next_secret_files: Vec<ImportedExistingProfileFileUpdate>,
    temporary_home: Option<&Path>,
) -> Result<PathBuf> {
    for secret_file in &rollback.previous_secret_files {
        validate_exported_secret_file_path(&secret_file.path, &rollback.profile_name)?;
    }
    for secret_file in &next_secret_files {
        validate_exported_secret_file_path(&secret_file.path, &rollback.profile_name)?;
    }
    if let Some(temporary_home) = temporary_home {
        super::lifecycle::validate_temporary_home_path(
            paths,
            temporary_home,
            "auth journal temporary home",
        )?;
    }
    let journal_path = prodex_profile_export::unique_profile_import_auth_update_journal_path(
        &paths.root,
        &rollback.profile_name,
        &runtime_random_token("auth")?,
    )?;
    let mut journal = ImportedExistingProfileAuthUpdateJournal::new(
        rollback.profile_name.clone(),
        rollback.codex_home.display().to_string(),
        rollback.previous_email.clone(),
        rollback.previous_auth_json.clone(),
        Local::now().to_rfc3339(),
    );
    journal.restore_auth_json = rollback.restore_auth_json;
    journal.previous_provider_json = rollback.previous_provider_json.clone();
    journal.previous_secret_files = rollback.previous_secret_files.clone();
    journal.state_after_known = true;
    journal.next_email = next_email;
    journal.next_auth_json = next_auth_json;
    journal.next_provider_json = next_provider_json;
    journal.next_secret_files = next_secret_files;
    journal.temporary_home = temporary_home.map(|path| path.display().to_string());
    prodex_profile_export::write_profile_import_auth_update_journal(&journal_path, &journal)?;
    Ok(journal_path)
}
