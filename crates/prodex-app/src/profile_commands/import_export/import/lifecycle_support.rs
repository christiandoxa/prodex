use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};

use super::super::lifecycle::{
    ProfileLifecycleHomeAction, ProfileLifecyclePlan, ProfileLifecyclePromoteRollback,
    acquire_profile_lifecycle_lock, lifecycle_profile_state,
};
use super::super::secrets::{read_optional_secret_text_file, validate_exported_secret_file_path};
use crate::{
    AppPaths, AppState, AppStateIoExt, ImportedExistingProfileAuthUpdate, PreparedImportedProfiles,
    ProfileEntry, ProfileExportPayload, read_auth_json_text,
};

pub(super) fn build_import_lifecycle_plan(
    state: &AppState,
    payload: &ProfileExportPayload,
    prepared: &PreparedImportedProfiles,
) -> Result<ProfileLifecyclePlan> {
    let mut desired = state.clone();
    for staged in &prepared.staged_profiles {
        desired.profiles.insert(
            staged.name.clone(),
            ProfileEntry {
                codex_home: staged.final_home.clone(),
                managed: true,
                email: staged.email.clone(),
                provider: staged.provider.clone(),
            },
        );
    }
    for update in &prepared.auth_updates {
        let profile = desired
            .profiles
            .get_mut(&update.target_profile_name)
            .with_context(|| format!("profile '{}' is missing", update.target_profile_name))?;
        profile.email = update.email.clone();
    }
    for update in &prepared.existing_profile_updates {
        let profile = desired
            .profiles
            .get_mut(&update.name)
            .with_context(|| format!("profile '{}' is missing", update.name))?;
        profile.email = update.email.clone();
        profile.provider = update.provider.clone();
    }
    desired.active_profile = prodex_profile_export::resolve_imported_active_profile(
        state.active_profile.as_deref(),
        payload.active_profile.as_deref(),
        &prepared.resolved_profile_names,
    );

    let mut names = prepared
        .staged_profiles
        .iter()
        .map(|profile| profile.name.clone())
        .chain(
            prepared
                .auth_updates
                .iter()
                .map(|update| update.target_profile_name.clone()),
        )
        .chain(
            prepared
                .existing_profile_updates
                .iter()
                .map(|update| update.name.clone()),
        )
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();

    Ok(ProfileLifecyclePlan {
        profile_states: names
            .iter()
            .map(|name| {
                lifecycle_profile_state(name, state.profiles.get(name), desired.profiles.get(name))
            })
            .collect::<Result<Vec<_>>>()?,
        previous_active_profile: state.active_profile.clone(),
        next_active_profile: desired.active_profile,
        home_actions: prepared
            .staged_profiles
            .iter()
            .map(|staged| ProfileLifecycleHomeAction::Promote {
                source: staged.staging_home.display().to_string(),
                destination: staged.final_home.display().to_string(),
                rollback: ProfileLifecyclePromoteRollback::Remove,
            })
            .collect(),
        auth_journal_paths: Vec::new(),
    })
}

fn cleanup_orphaned_import_staging_homes(paths: &AppPaths) -> Result<()> {
    let entries = match fs::read_dir(&paths.managed_profiles_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed to read import staging root {}",
                    paths.managed_profiles_root.display()
                )
            });
        }
    };
    for entry in entries {
        let entry = entry.with_context(|| {
            format!(
                "failed to read import staging entry in {}",
                paths.managed_profiles_root.display()
            )
        })?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with(".import-") {
            continue;
        }
        super::super::lifecycle::validate_temporary_home_path(
            paths,
            &path,
            "orphaned import staging home",
        )?;
        super::super::lifecycle::remove_home(&path)
            .with_context(|| format!("failed to clean import staging home {}", path.display()))?;
    }
    Ok(())
}

pub(crate) fn repair_profile_import_auth_journals(
    paths: &AppPaths,
    state: &mut AppState,
) -> Result<usize> {
    let _lock = acquire_profile_lifecycle_lock(paths)?;
    let mut current =
        if paths.state_file.exists() || crate::state_last_good_file_path(paths).exists() {
            AppState::load_and_repair(paths)?
        } else {
            state.clone()
        };
    let lifecycle = super::super::lifecycle::recover_profile_lifecycle_journals_locked(
        paths,
        &mut current,
        false,
    )?;
    let recovered_auth = recover_imported_auth_update_journals_locked(paths, &mut current)?;
    cleanup_orphaned_import_staging_homes(paths)?;
    if recovered_auth > 0 {
        current
            .save(paths)
            .context("failed to save recovered profile lifecycle state")?;
    }
    *state = current;
    Ok(lifecycle.recovered + recovered_auth)
}

#[cfg(test)]
pub(crate) fn load_profile_state_with_profile_recovery(
    paths: &AppPaths,
    recover_removals: bool,
) -> Result<(AppState, super::super::lifecycle::ProfileLifecycleRecovery)> {
    let _lock = acquire_profile_lifecycle_lock(paths)?;
    load_profile_state_with_profile_recovery_locked(paths, recover_removals)
}

pub(crate) fn load_profile_state_with_profile_recovery_locked(
    paths: &AppPaths,
    recover_removals: bool,
) -> Result<(AppState, super::super::lifecycle::ProfileLifecycleRecovery)> {
    let mut state = AppState::load_and_repair(paths)?;
    let lifecycle = super::super::lifecycle::recover_profile_lifecycle_journals_locked(
        paths,
        &mut state,
        recover_removals,
    )?;
    let recovered_auth = recover_imported_auth_update_journals_locked(paths, &mut state)?;
    cleanup_orphaned_import_staging_homes(paths)?;
    if recovered_auth > 0 {
        state
            .save(paths)
            .context("failed to save recovered profile lifecycle state")?;
    }
    Ok((state, lifecycle))
}

pub(crate) fn recover_pending_profile_lifecycle() -> Result<()> {
    let paths = AppPaths::discover()?;
    let _lock = acquire_profile_lifecycle_lock(&paths)?;
    let (state, lifecycle) = load_profile_state_with_profile_recovery_locked(&paths, true)?;
    super::super::super::remove::finalize_recovered_profile_removals(
        &paths,
        &state.profiles,
        &lifecycle.pending_removal_journals,
    )
}

pub(crate) fn recover_imported_auth_update_journals_locked(
    paths: &AppPaths,
    state: &mut AppState,
) -> Result<usize> {
    let journal_root = prodex_profile_export::profile_import_auth_update_journal_root(&paths.root);
    let journal_paths =
        prodex_profile_export::profile_import_auth_update_journal_paths(&paths.root)?;
    let lifecycle_auth_journals =
        super::super::lifecycle::lifecycle_referenced_auth_journals(paths)?;

    let mut journals = Vec::new();
    for journal_path in journal_paths {
        if lifecycle_auth_journals.contains(&journal_path) {
            continue;
        }
        let journal =
            prodex_profile_export::read_profile_import_auth_update_journal(&journal_path)?;
        imported_auth_update_from_journal(paths, state, &journal_path, &journal)?;
        journals.push((journal_path, journal));
    }
    journals.sort_by(|left, right| {
        right
            .1
            .created_at
            .cmp(&left.1.created_at)
            .then_with(|| right.0.cmp(&left.0))
    });

    let mut recovered = 0;
    for (journal_path, journal) in journals {
        if imported_auth_update_journal_is_committed(state, &journal)? {
            if let Some(temporary_home) = journal.temporary_home.as_deref() {
                let _ = fs::remove_dir_all(temporary_home);
            }
            let _ = fs::remove_file(&journal_path);
            recovered += 1;
            continue;
        }
        let update = imported_auth_update_from_journal(paths, state, &journal_path, &journal)?;
        super::rollback_imported_auth_updates(state, &[update])?;
        if let Some(temporary_home) = journal.temporary_home.as_deref() {
            let _ = fs::remove_dir_all(temporary_home);
        }
        prodex_profile_export::cleanup_profile_import_auth_update_journal(&journal_path);
        recovered += 1;
    }

    let _ = fs::remove_dir(&journal_root);
    Ok(recovered)
}

pub(crate) fn cleanup_profile_import_auth_journal(
    paths: &AppPaths,
    state: &AppState,
    journal_path: &Path,
) -> Result<()> {
    if let Err(error) = fs::symlink_metadata(journal_path) {
        if error.kind() == std::io::ErrorKind::NotFound {
            return Ok(());
        }
        return Err(error).with_context(|| format!("failed to inspect {}", journal_path.display()));
    }
    let journal = prodex_profile_export::read_profile_import_auth_update_journal(journal_path)?;
    imported_auth_update_from_journal(paths, state, journal_path, &journal)?;
    if let Some(temporary_home) = journal.temporary_home.as_deref() {
        let _ = fs::remove_dir_all(temporary_home);
    }
    prodex_profile_export::cleanup_profile_import_auth_update_journal(journal_path);
    Ok(())
}

pub(crate) fn imported_auth_update_from_journal(
    paths: &AppPaths,
    state: &AppState,
    journal_path: &Path,
    journal: &prodex_profile_export::ImportedExistingProfileAuthUpdateJournal,
) -> Result<ImportedExistingProfileAuthUpdate> {
    if !prodex_core::path_is_strictly_under_root(
        &prodex_profile_export::profile_import_auth_update_journal_root(&paths.root),
        journal_path,
    ) || journal_path
        .strip_prefix(prodex_profile_export::profile_import_auth_update_journal_root(&paths.root))
        .map_or(true, |relative| relative.components().count() != 1)
    {
        bail!(
            "auth update journal {} is outside its journal root",
            journal_path.display()
        );
    }
    prodex_profile_export::validate_profile_import_auth_update_journal_path(journal_path)?;
    prodex_profile_identity::validate_profile_name(&journal.profile_name)?;
    let profile = state.profiles.get(&journal.profile_name).with_context(|| {
        format!(
            "auth update journal {} references missing profile '{}'",
            journal_path.display(),
            journal.profile_name
        )
    })?;
    let journal_codex_home = PathBuf::from(&journal.codex_home);
    if journal_codex_home != profile.codex_home {
        bail!(
            "auth update journal {} targets {} but profile '{}' uses {}",
            journal_path.display(),
            journal_codex_home.display(),
            journal.profile_name,
            profile.codex_home.display()
        );
    }
    if let Some(temporary_home) = journal.temporary_home.as_deref() {
        super::super::lifecycle::validate_temporary_home_path(
            paths,
            Path::new(temporary_home),
            "auth journal temporary home",
        )
        .with_context(|| format!("in {}", journal_path.display()))?;
    }
    for secret_file in journal
        .previous_secret_files
        .iter()
        .map(|file| file.path.as_str())
        .chain(
            journal
                .next_secret_files
                .iter()
                .map(|file| file.path.as_str()),
        )
    {
        validate_exported_secret_file_path(secret_file, &journal.profile_name)?;
    }
    Ok(ImportedExistingProfileAuthUpdate {
        profile_name: journal.profile_name.clone(),
        codex_home: journal_codex_home,
        previous_auth_json: journal.previous_auth_json.clone(),
        previous_email: journal.previous_email.clone(),
        journal_path: Some(journal_path.to_path_buf()),
        restore_auth_json: journal.restore_auth_json,
        previous_provider_json: journal.previous_provider_json.clone(),
        previous_secret_files: journal.previous_secret_files.clone(),
    })
}

pub(crate) fn imported_auth_update_journal_is_committed(
    state: &AppState,
    journal: &prodex_profile_export::ImportedExistingProfileAuthUpdateJournal,
) -> Result<bool> {
    prodex_profile_identity::validate_profile_name(&journal.profile_name)?;
    for secret_file in journal
        .previous_secret_files
        .iter()
        .map(|file| file.path.as_str())
        .chain(
            journal
                .next_secret_files
                .iter()
                .map(|file| file.path.as_str()),
        )
    {
        validate_exported_secret_file_path(secret_file, &journal.profile_name)?;
    }
    let Some(profile) = state.profiles.get(&journal.profile_name) else {
        return Ok(false);
    };
    if profile.codex_home.as_path() != Path::new(&journal.codex_home) {
        return Ok(false);
    }

    if !journal.state_after_known {
        return Ok(false);
    }
    if journal.next_email.is_none()
        && journal.next_auth_json.is_none()
        && journal.next_provider_json.is_none()
        && journal.next_secret_files.is_empty()
    {
        return Ok(false);
    }
    if profile.email != journal.next_email {
        return Ok(false);
    }
    if let Some(next_provider_json) = journal.next_provider_json.as_deref()
        && serde_json::to_value(&profile.provider)?
            != serde_json::from_str::<serde_json::Value>(next_provider_json)?
    {
        return Ok(false);
    }
    if let Some(next_auth_json) = journal.next_auth_json.as_deref()
        && read_auth_json_text(&profile.codex_home)? != Some(next_auth_json.to_string())
    {
        return Ok(false);
    }
    for secret_file in &journal.next_secret_files {
        if read_optional_secret_text_file(&profile.codex_home.join(&secret_file.path))?
            != secret_file.text
        {
            return Ok(false);
        }
    }
    Ok(true)
}
