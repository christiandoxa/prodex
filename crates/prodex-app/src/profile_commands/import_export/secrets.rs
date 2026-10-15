use anyhow::{Context, Result, bail};
use chrono::Local;
use prodex_profile_export::{
    ImportedExistingProfileAuthUpdateJournal, ImportedExistingProfileFileUpdate,
};
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
    let mut seen_paths = Vec::new();

    for secret_file in &exported.secret_files {
        validate_exported_secret_file_path(&secret_file.path, &exported.name)?;
        let previous_paths = seen_paths.iter().map(String::as_str).collect::<Vec<_>>();
        match prodex_mojo_core::profile_export::profile_import_secret_file_policy(
            exported.provider.label(),
            prodex_mojo_core::profile_export::ProfileImportSecretFilePolicyMode::Item,
            Some(&secret_file.path),
            &previous_paths,
        )
        .map_err(|error| {
            anyhow::anyhow!("Mojo profile-import secret-file policy failed: {error:?}")
        })? {
            prodex_mojo_core::profile_export::ProfileImportSecretFilePolicy::Accepted => {}
            prodex_mojo_core::profile_export::ProfileImportSecretFilePolicy::Duplicate => {
                bail!(
                    "profile export bundle contains duplicate secret file '{}' for profile '{}'",
                    secret_file.path,
                    exported.name
                );
            }
            prodex_mojo_core::profile_export::ProfileImportSecretFilePolicy::Unexpected => {
                bail!(
                    "profile export bundle contains unexpected secret file '{}' for profile '{}'",
                    secret_file.path,
                    exported.name
                );
            }
            _ => bail!("Mojo returned an invalid imported secret-file item result"),
        }
        validate_exported_secret_file_content(exported, &secret_file.path, &secret_file.text)?;
        seen_paths.push(secret_file.path.clone());
    }

    let previous_paths = seen_paths.iter().map(String::as_str).collect::<Vec<_>>();
    match prodex_mojo_core::profile_export::profile_import_secret_file_policy(
        exported.provider.label(),
        prodex_mojo_core::profile_export::ProfileImportSecretFilePolicyMode::Required,
        None,
        &previous_paths,
    )
    .map_err(|error| anyhow::anyhow!("Mojo profile-import secret-file policy failed: {error:?}"))?
    {
        prodex_mojo_core::profile_export::ProfileImportSecretFilePolicy::Accepted => Ok(()),
        prodex_mojo_core::profile_export::ProfileImportSecretFilePolicy::MissingGemini => {
            bail!(
                "profile export bundle is missing secret file '{}' for profile '{}'",
                GEMINI_OAUTH_SECRET_FILE,
                exported.name
            );
        }
        prodex_mojo_core::profile_export::ProfileImportSecretFilePolicy::MissingAnthropic => {
            bail!(
                "profile export bundle is missing secret file '{}' for profile '{}'",
                CLAUDE_CREDENTIALS_FILE,
                exported.name
            );
        }
        prodex_mojo_core::profile_export::ProfileImportSecretFilePolicy::MissingKiro => {
            bail!(
                "profile export bundle is missing secret file '{}' for profile '{}'",
                KIRO_CREDENTIALS_FILE,
                exported.name
            );
        }
        _ => bail!("Mojo returned an invalid imported secret-file requirement result"),
    }
}

pub(super) fn validate_exported_secret_file_path(path: &str, profile_name: &str) -> Result<()> {
    let is_absolute = Path::new(path).is_absolute();
    let is_safe =
        prodex_mojo_core::profile_export::profile_import_secret_path_is_safe(path, is_absolute)
            .map_err(|error| {
                anyhow::anyhow!("Mojo profile-import secret path failed: {error:?}")
            })?;
    if !is_safe {
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
