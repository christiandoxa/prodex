use super::profile_identity::{
    ProfileIdentity, find_profile_by_identity, parse_identity_from_auth_json,
};
use super::shared_codex_fs::{create_codex_home_if_missing, prepare_managed_codex_home};
use super::*;

mod anthropic;
mod copilot;
mod import_export;
mod kiro;
mod login;
mod logout;
mod manage;
mod remove;

pub(crate) use self::copilot::{
    CopilotUserInfo, fetch_copilot_user_info_for_account, fetch_copilot_user_info_json_for_account,
    handle_import_copilot_profile, resolve_copilot_runtime_api_auth,
};
use self::import_export::write_secret_text_file;
#[cfg(test)]
use self::import_export::{build_profile_export_payload, import_profile_export_payload};
pub(crate) use self::import_export::{
    count_profile_import_auth_journals, handle_export_profiles, handle_import_current_profile,
    handle_import_profiles, repair_profile_import_auth_journals,
};
pub(crate) use self::kiro::{
    KIRO_MODEL_CATALOG_FILE, create_private_kiro_temp_root, handle_import_kiro_profile,
    parse_kiro_model_catalog_text, read_kiro_auth_secret, write_kiro_cli_data_dir,
};
pub(crate) use self::login::handle_codex_login;
pub(crate) use self::logout::handle_codex_logout;
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
#[cfg(test)]
use prodex_profile_export::{
    PROFILE_EXPORT_CIPHER, PROFILE_EXPORT_KDF, derive_profile_export_key,
    serialize_profile_export_payload, validate_profile_export_header,
};

pub(super) type ProfileExportPayload = prodex_profile_export::ProfileExportPayload<ProfileProvider>;
pub(super) type ExportedProfile = prodex_profile_export::ExportedProfile<ProfileProvider>;
#[cfg(test)]
pub(super) type ProfileExportEnvelope =
    prodex_profile_export::ProfileExportEnvelope<ProfileExportPayload>;
pub(super) type StagedImportedProfile =
    prodex_profile_export::StagedImportedProfile<ProfileProvider>;
pub(super) type ImportedExistingProfileAuthUpdate =
    prodex_profile_export::ImportedExistingProfileAuthUpdate;
pub(super) type ImportedProfilesCommit = prodex_profile_export::ImportedProfilesCommit;
pub(super) type ImportedProfilesTransaction = prodex_profile_export::ImportedProfilesTransaction;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PreparedExistingProfileUpdate {
    pub name: String,
    pub email: Option<String>,
    pub provider: ProfileProvider,
    pub secret_files: Vec<prodex_profile_export::ExportedSecretFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PreparedImportedProfiles {
    pub staged_profiles: Vec<StagedImportedProfile>,
    pub auth_updates: Vec<prodex_profile_export::ProfileImportAuthUpdatePlan>,
    pub existing_profile_updates: Vec<PreparedExistingProfileUpdate>,
    pub resolved_profile_names: std::collections::BTreeMap<String, String>,
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

pub(super) fn ensure_managed_profiles_root(paths: &AppPaths) -> Result<()> {
    prodex_shared_codex_fs::ensure_managed_profiles_root(paths)
}

pub(super) fn managed_profile_home_path(paths: &AppPaths, profile_name: &str) -> Result<PathBuf> {
    ensure_managed_profiles_root(paths)?;
    absolutize(paths.managed_profiles_root.join(profile_name))
}

pub(super) fn prepare_profile_codex_home(paths: &AppPaths, profile: &ProfileEntry) -> Result<()> {
    if profile.managed {
        ensure_managed_profiles_root(paths)?;
        if !prodex_core::path_is_strictly_under_root(
            &paths.managed_profiles_root,
            &profile.codex_home,
        ) {
            bail!(
                "managed profile home {} is outside {}",
                profile.codex_home.display(),
                paths.managed_profiles_root.display()
            );
        }
        prepare_managed_codex_home(paths, &profile.codex_home)
    } else {
        create_codex_home_if_missing(&profile.codex_home)
    }
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

    prepare_profile_codex_home(paths, &profile)?;
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

#[cfg(test)]
#[path = "../tests/support/profile_commands_internal_harness.rs"]
mod profile_commands_internal_tests;
pub(crate) use self::anthropic::handle_import_claude_profile;
