use super::profile_identity::{
    fetch_profile_email, find_profile_by_email, normalize_email, parse_email_from_auth_json,
    persist_login_home, remove_dir_if_exists, unique_profile_name_for_email,
};
use super::shared_codex_fs::{
    copy_codex_home, create_codex_home_if_missing, prepare_managed_codex_home,
};
use super::*;

mod copilot;
mod import_export;
mod login;
mod manage;
mod remove;

use self::copilot::handle_import_copilot_profile;
#[cfg(test)]
use self::import_export::{
    PROFILE_EXPORT_CIPHER, PROFILE_EXPORT_KDF, build_profile_export_payload,
    decode_profile_export_envelope, derive_profile_export_key, import_profile_export_payload,
    serialize_profile_export_payload, stage_imported_profiles, validate_profile_export_header,
};
pub(crate) use self::import_export::{
    handle_export_profiles, handle_import_current_profile, handle_import_profiles,
};
use self::import_export::{
    remove_committed_import_homes, rollback_imported_auth_updates, write_secret_text_file,
};
pub(crate) use self::login::{handle_codex_login, handle_codex_logout};
pub(crate) use self::manage::{
    handle_add_profile, handle_current_profile, handle_list_profiles, handle_set_active_profile,
};
pub(crate) use self::remove::handle_remove_profile;
#[cfg(test)]
use aes_gcm_siv::aead::Aead;
#[cfg(test)]
use aes_gcm_siv::aead::KeyInit;
#[cfg(test)]
use aes_gcm_siv::{Aes256GcmSiv, Nonce};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ProfileExportPayload {
    exported_at: String,
    source_prodex_version: String,
    active_profile: Option<String>,
    profiles: Vec<ExportedProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ExportedProfile {
    name: String,
    #[serde(default)]
    email: Option<String>,
    source_managed: bool,
    #[serde(default)]
    provider: ProfileProvider,
    auth_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "payload_kind", rename_all = "snake_case")]
pub(super) enum ProfileExportEnvelope {
    Plain {
        format: String,
        version: u32,
        payload: ProfileExportPayload,
    },
    Encrypted {
        format: String,
        version: u32,
        cipher: String,
        kdf: String,
        iterations: u32,
        salt_base64: String,
        nonce_base64: String,
        ciphertext_base64: String,
    },
}

#[derive(Debug)]
pub(super) struct StagedImportedProfile {
    name: String,
    email: Option<String>,
    staging_home: PathBuf,
    final_home: PathBuf,
    provider: ProfileProvider,
}

#[derive(Debug)]
pub(super) struct PreparedImportedProfiles {
    staged_profiles: Vec<StagedImportedProfile>,
    auth_updates: Vec<PreparedImportedProfileAuthUpdate>,
    resolved_profile_names: BTreeMap<String, String>,
}

#[derive(Debug)]
pub(super) struct PreparedImportedProfileAuthUpdate {
    target_profile_name: String,
    email: Option<String>,
    auth_json: String,
}

#[derive(Debug)]
pub(super) struct ImportedExistingProfileAuthUpdate {
    profile_name: String,
    codex_home: PathBuf,
    previous_auth_json: Option<String>,
    previous_email: Option<String>,
}

#[derive(Debug)]
pub(super) struct ImportedProfilesCommit {
    imported_names: Vec<String>,
    updated_existing_names: Vec<String>,
    committed_homes: Vec<PathBuf>,
    auth_updates: Vec<ImportedExistingProfileAuthUpdate>,
    previous_active_profile: Option<String>,
}

#[derive(Debug)]
pub(super) struct ImportedProfilesTransaction {
    imported_names: Vec<String>,
    updated_existing_names: Vec<String>,
    committed_homes: Vec<PathBuf>,
    auth_updates: Vec<ImportedExistingProfileAuthUpdate>,
    previous_active_profile: Option<String>,
}

impl ImportedProfilesTransaction {
    fn new(
        previous_active_profile: Option<String>,
        staged_profile_count: usize,
        auth_update_count: usize,
    ) -> Self {
        Self {
            imported_names: Vec::with_capacity(staged_profile_count),
            updated_existing_names: Vec::with_capacity(auth_update_count),
            committed_homes: Vec::with_capacity(staged_profile_count),
            auth_updates: Vec::with_capacity(auth_update_count),
            previous_active_profile,
        }
    }

    fn record_existing_auth_update(&mut self, update: ImportedExistingProfileAuthUpdate) {
        self.updated_existing_names
            .push(update.profile_name.clone());
        self.auth_updates.push(update);
    }

    fn record_imported_profile(&mut self, name: String, final_home: PathBuf) {
        self.committed_homes.push(final_home);
        self.imported_names.push(name);
    }

    fn rollback_partial(&self, state: &mut AppState) {
        for name in &self.imported_names {
            state.profiles.remove(name);
        }
        rollback_imported_auth_updates(state, &self.auth_updates);
        state.active_profile = self.previous_active_profile.clone();
        remove_committed_import_homes(&self.committed_homes);
    }

    fn into_commit(self) -> ImportedProfilesCommit {
        ImportedProfilesCommit {
            imported_names: self.imported_names,
            updated_existing_names: self.updated_existing_names,
            committed_homes: self.committed_homes,
            auth_updates: self.auth_updates,
            previous_active_profile: self.previous_active_profile,
        }
    }
}

#[derive(Debug)]
pub(super) enum ImportEmailTarget {
    Existing(String),
    PendingNew(usize),
}

#[derive(Debug)]
pub(super) struct ExistingProfileAuthUpdate {
    profile_name: String,
    codex_home: PathBuf,
}

pub(super) fn required_auth_json_text(codex_home: &Path) -> Result<String> {
    let auth_path = secret_store::auth_json_path(codex_home);
    read_auth_json_text(codex_home)
        .with_context(|| format!("failed to read {}", auth_path.display()))?
        .with_context(|| format!("failed to read {}", auth_path.display()))
}

pub(super) fn update_existing_profile_auth(
    paths: &AppPaths,
    state: &mut AppState,
    profile_name: &str,
    email: Option<&str>,
    auth_json: &str,
    activate: bool,
) -> Result<ExistingProfileAuthUpdate> {
    let profile = state
        .profiles
        .get(profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?
        .clone();

    if profile.managed {
        prepare_managed_codex_home(paths, &profile.codex_home)?;
    } else {
        create_codex_home_if_missing(&profile.codex_home)?;
    }
    write_secret_text_file(
        &secret_store::auth_json_path(&profile.codex_home),
        auth_json,
    )?;

    if let Some(email) = email
        && let Some(profile_entry) = state.profiles.get_mut(profile_name)
    {
        profile_entry.email = Some(email.to_string());
    }
    if activate {
        state.active_profile = Some(profile_name.to_string());
    }

    Ok(ExistingProfileAuthUpdate {
        profile_name: profile_name.to_string(),
        codex_home: profile.codex_home,
    })
}

fn resolved_exported_profile_email(exported: &ExportedProfile) -> Option<String> {
    parse_email_from_auth_json(&exported.auth_json)
        .ok()
        .flatten()
        .or_else(|| {
            exported
                .email
                .as_deref()
                .map(str::trim)
                .filter(|email| !email.is_empty())
                .map(ToOwned::to_owned)
        })
}

fn queue_existing_profile_auth_update(
    auth_updates: &mut Vec<PreparedImportedProfileAuthUpdate>,
    target_profile_name: &str,
    email: Option<String>,
    auth_json: String,
) {
    if let Some(existing) = auth_updates
        .iter_mut()
        .find(|update| update.target_profile_name == target_profile_name)
    {
        existing.auth_json = auth_json;
        if email.is_some() {
            existing.email = email;
        }
        return;
    }

    auth_updates.push(PreparedImportedProfileAuthUpdate {
        target_profile_name: target_profile_name.to_string(),
        email,
        auth_json,
    });
}

#[cfg(test)]
#[path = "../tests/support/profile_commands_internal_harness.rs"]
mod profile_commands_internal_tests;
