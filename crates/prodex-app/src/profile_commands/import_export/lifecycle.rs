use anyhow::{Context, Result, bail};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::{
    AppPaths, AppState, AppStateIoExt, ProfileEntry, runtime_random_token,
    state_last_good_file_path,
};
use prodex_core::path_is_strictly_under_root;

mod home_action_validation;
use home_action_validation::validate_home_actions;

pub(crate) struct ProfileLifecycleLock {
    _lock: crate::JsonFileLock,
}

pub(crate) struct ProfileAuthUpdate<'a> {
    pub next_auth_json: Option<String>,
    pub next_provider_json: Option<String>,
    pub next_secret_files: Vec<prodex_profile_export::ImportedExistingProfileFileUpdate>,
    pub previous_secret_file_paths: &'a [&'a str],
    pub temporary_home: Option<&'a Path>,
}

#[cfg(test)]
pub(crate) fn profile_lifecycle_lock_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("profile-lifecycle.json.lock")
}

pub(crate) fn acquire_profile_lifecycle_lock(paths: &AppPaths) -> Result<ProfileLifecycleLock> {
    // ponytail: one global profile lock; use per-profile locks only if startup contention is measured.
    fs::create_dir_all(&paths.root)
        .with_context(|| format!("failed to create {}", paths.root.display()))?;
    Ok(ProfileLifecycleLock {
        _lock: crate::runtime_store::acquire_json_file_lock(
            &paths.root.join("profile-lifecycle.json"),
        )?,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProfileLifecyclePlan {
    pub profile_states: Vec<ProfileLifecycleProfileState>,
    pub previous_active_profile: Option<String>,
    pub next_active_profile: Option<String>,
    pub home_actions: Vec<ProfileLifecycleHomeAction>,
    #[serde(default)]
    pub auth_journal_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProfileLifecycleProfileState {
    pub name: String,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ProfileLifecycleHomeAction {
    Promote {
        source: String,
        destination: String,
        rollback: ProfileLifecyclePromoteRollback,
    },
    Create {
        path: String,
    },
    Cleanup {
        path: String,
    },
    Quarantine {
        source: String,
        quarantine: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProfileLifecyclePromoteRollback {
    RestoreSource,
    Remove,
}

#[derive(Debug, Default)]
pub(crate) struct ProfileLifecycleRecovery {
    pub recovered: usize,
    pub pending_removal_journals: Vec<PathBuf>,
    pub completed_removal_profiles: Vec<String>,
}

pub(crate) fn write_profile_lifecycle_plan(
    paths: &AppPaths,
    operation: &str,
    plan: &ProfileLifecyclePlan,
) -> Result<PathBuf> {
    let path = prodex_profile_export::unique_profile_lifecycle_journal_path(
        &paths.root,
        operation,
        &runtime_random_token("profile-lifecycle")?,
    )?;
    write_profile_lifecycle_plan_at(paths, &path, operation, plan)?;
    Ok(path)
}

pub(crate) fn update_profile_lifecycle_plan(
    paths: &AppPaths,
    path: &Path,
    operation: &str,
    plan: &ProfileLifecyclePlan,
) -> Result<()> {
    write_profile_lifecycle_plan_at(paths, path, operation, plan)
}

fn write_profile_lifecycle_plan_at(
    paths: &AppPaths,
    path: &Path,
    operation: &str,
    plan: &ProfileLifecyclePlan,
) -> Result<()> {
    prodex_profile_export::validate_profile_lifecycle_operation(operation)?;
    validate_plan(paths, operation, plan)?;
    validate_auth_journal_paths(paths, plan)?;
    let payload =
        serde_json::to_value(plan).context("failed to serialize profile lifecycle plan")?;
    let journal = prodex_profile_export::ProfileLifecycleJournal::new(
        operation.to_string(),
        payload,
        Local::now().to_rfc3339(),
    );
    prodex_profile_export::write_profile_lifecycle_journal(path, &journal)
}

pub(crate) fn cleanup_profile_lifecycle_journal(path: &Path) {
    prodex_profile_export::cleanup_profile_lifecycle_journal(path);
}

pub(crate) fn cleanup_profile_lifecycle_and_auth_journal(
    lifecycle_path: &Path,
    auth_path: &Path,
) -> Result<()> {
    remove_lifecycle_journal(lifecycle_path)?;
    prodex_profile_export::cleanup_profile_import_auth_update_journal(auth_path);
    Ok(())
}

pub(crate) fn prepare_profile_auth_update_journal(
    paths: &AppPaths,
    state: &AppState,
    profile_name: &str,
    next_email: Option<String>,
    update: ProfileAuthUpdate<'_>,
) -> Result<PathBuf> {
    let profile = state
        .profiles
        .get(profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    let previous_secret_files = update
        .previous_secret_file_paths
        .iter()
        .map(|path| {
            super::secrets::validate_exported_secret_file_path(path, profile_name)?;
            Ok(prodex_profile_export::ImportedExistingProfileFileRollback {
                path: (*path).to_string(),
                previous_text: super::read_optional_secret_text_file(
                    &profile.codex_home.join(path),
                )?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let rollback = super::super::ImportedExistingProfileAuthUpdate {
        profile_name: profile_name.to_string(),
        codex_home: profile.codex_home.clone(),
        previous_auth_json: crate::read_auth_json_text(&profile.codex_home)?,
        previous_email: profile.email.clone(),
        journal_path: None,
        restore_auth_json: update.next_auth_json.is_some(),
        previous_provider_json: Some(serde_json::to_string(&profile.provider)?),
        previous_secret_files,
    };
    super::write_imported_auth_update_journal(
        paths,
        &rollback,
        next_email,
        update.next_auth_json,
        update.next_provider_json,
        update.next_secret_files,
        update.temporary_home,
    )
}

pub(crate) fn prepare_existing_profile_lifecycle(
    paths: &AppPaths,
    operation: &str,
    state: &AppState,
    profile_name: &str,
    desired_profile: &ProfileEntry,
    next_active_profile: Option<String>,
    auth_update: ProfileAuthUpdate<'_>,
) -> Result<(PathBuf, PathBuf)> {
    let current_profile = state
        .profiles
        .get(profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    let mut plan = ProfileLifecyclePlan {
        profile_states: vec![lifecycle_profile_state(
            profile_name,
            Some(current_profile),
            Some(desired_profile),
        )?],
        previous_active_profile: state.active_profile.clone(),
        next_active_profile,
        home_actions: Vec::new(),
        auth_journal_paths: Vec::new(),
    };
    let lifecycle_path = write_profile_lifecycle_plan(paths, operation, &plan)?;
    let auth_path = prepare_profile_auth_update_journal(
        paths,
        state,
        profile_name,
        desired_profile.email.clone(),
        auth_update,
    )?;
    plan.auth_journal_paths
        .push(auth_path.display().to_string());
    update_profile_lifecycle_plan(paths, &lifecycle_path, operation, &plan)?;
    Ok((lifecycle_path, auth_path))
}

#[cfg(test)]
pub(crate) fn recover_profile_lifecycle_journals(
    paths: &AppPaths,
    state: &mut AppState,
    recover_removals: bool,
) -> Result<ProfileLifecycleRecovery> {
    let _lock = acquire_profile_lifecycle_lock(paths)?;
    recover_profile_lifecycle_journals_locked(paths, state, recover_removals)
}

pub(crate) fn recover_profile_lifecycle_journals_locked(
    paths: &AppPaths,
    state: &mut AppState,
    recover_removals: bool,
) -> Result<ProfileLifecycleRecovery> {
    let mut recovery = ProfileLifecycleRecovery::default();
    for path in prodex_profile_export::profile_lifecycle_journal_paths(&paths.root)? {
        let journal = prodex_profile_export::read_profile_lifecycle_journal(&path)?;
        if journal.operation == "remove" && !recover_removals {
            continue;
        }
        let plan: ProfileLifecyclePlan =
            serde_json::from_value(journal.payload).with_context(|| {
                format!("failed to parse profile lifecycle plan {}", path.display())
            })?;
        validate_plan(paths, &journal.operation, &plan)?;
        validate_auth_journal_paths(paths, &plan)?;
        let auth_journals = read_auth_journals(paths, &plan)?;
        let persisted_state =
            if paths.state_file.exists() || state_last_good_file_path(paths).exists() {
                Some(AppState::load(paths)?)
            } else {
                None
            };
        let committed = match persisted_state.as_ref() {
            Some(persisted_state) => {
                lifecycle_state_matches(
                    persisted_state,
                    &plan.profile_states,
                    &plan.next_active_profile,
                    true,
                )? && auth_journals_match_committed(persisted_state, &auth_journals)?
                    && lifecycle_home_actions_match_committed(&plan.home_actions)
            }
            None => false,
        };

        if committed {
            finish_committed_lifecycle(
                paths,
                path,
                &journal.operation,
                &plan,
                persisted_state.as_ref(),
                &auth_journals,
                &mut recovery,
            )?;
        } else {
            rollback_lifecycle(paths, state, &path, &plan, &auth_journals)?;
        }
        recovery.recovered += 1;
    }
    Ok(recovery)
}

fn finish_committed_lifecycle(
    paths: &AppPaths,
    path: PathBuf,
    operation: &str,
    plan: &ProfileLifecyclePlan,
    persisted_state: Option<&AppState>,
    auth_journals: &[(
        PathBuf,
        prodex_profile_export::ImportedExistingProfileAuthUpdateJournal,
    )],
    recovery: &mut ProfileLifecycleRecovery,
) -> Result<()> {
    finish_home_actions(&plan.home_actions, true)?;
    cleanup_auth_journal_temporary_homes(auth_journals)?;
    if operation == "remove" {
        recovery.completed_removal_profiles.extend(
            plan.profile_states
                .iter()
                .map(|profile| profile.name.clone()),
        );
        recovery.pending_removal_journals.push(path);
        return Ok(());
    }
    let auth_state =
        persisted_state.context("committed profile lifecycle has no persisted state")?;
    remove_lifecycle_journal(&path)?;
    for (journal_path, _) in auth_journals {
        super::import::lifecycle_support::cleanup_profile_import_auth_journal(
            paths,
            auth_state,
            journal_path,
        )?;
    }
    Ok(())
}

fn rollback_lifecycle(
    paths: &AppPaths,
    state: &mut AppState,
    path: &Path,
    plan: &ProfileLifecyclePlan,
    auth_journals: &[(
        PathBuf,
        prodex_profile_export::ImportedExistingProfileAuthUpdateJournal,
    )],
) -> Result<()> {
    let removed_profiles = restore_lifecycle_state(state, plan)?;
    let auth_updates = auth_journals
        .iter()
        .map(|(journal_path, journal)| {
            super::import::lifecycle_support::imported_auth_update_from_journal(
                paths,
                state,
                journal_path,
                journal,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    super::import::rollback_imported_auth_updates(state, &auth_updates)?;
    finish_home_actions(&plan.home_actions, false)?;
    cleanup_auth_journal_temporary_homes(auth_journals)?;
    save_rolled_back_lifecycle_state(paths, state, &removed_profiles)?;
    remove_lifecycle_journal(path)?;
    for (journal_path, _) in auth_journals {
        prodex_profile_export::cleanup_profile_import_auth_update_journal(journal_path);
    }
    Ok(())
}

pub(super) fn lifecycle_referenced_auth_journals(paths: &AppPaths) -> Result<BTreeSet<PathBuf>> {
    let mut referenced = BTreeSet::new();
    for path in prodex_profile_export::profile_lifecycle_journal_paths(&paths.root)? {
        let journal = prodex_profile_export::read_profile_lifecycle_journal(&path)?;
        let plan: ProfileLifecyclePlan =
            serde_json::from_value(journal.payload).with_context(|| {
                format!("failed to parse profile lifecycle plan {}", path.display())
            })?;
        validate_plan(paths, &journal.operation, &plan)?;
        validate_auth_journal_paths(paths, &plan)?;
        referenced.extend(plan.auth_journal_paths.into_iter().map(PathBuf::from));
    }
    Ok(referenced)
}

fn validate_auth_journal_paths(paths: &AppPaths, plan: &ProfileLifecyclePlan) -> Result<()> {
    let root = prodex_profile_export::profile_import_auth_update_journal_root(&paths.root);
    let mut seen = BTreeSet::new();
    for path in &plan.auth_journal_paths {
        let path = Path::new(path);
        if !path_is_strictly_under_root(&root, path)
            || path
                .strip_prefix(&root)
                .map_or(true, |relative| relative.components().count() != 1)
        {
            bail!(
                "profile lifecycle auth journal path {} is outside its journal root",
                path.display()
            );
        }
        prodex_profile_export::validate_profile_import_auth_update_journal_path(path)?;
        if !seen.insert(path.to_path_buf()) {
            bail!(
                "profile lifecycle auth journal path {} is duplicated",
                path.display()
            );
        }
    }
    Ok(())
}

fn read_auth_journals(
    paths: &AppPaths,
    plan: &ProfileLifecyclePlan,
) -> Result<
    Vec<(
        PathBuf,
        prodex_profile_export::ImportedExistingProfileAuthUpdateJournal,
    )>,
> {
    plan.auth_journal_paths
        .iter()
        .map(|path| {
            let path = PathBuf::from(path);
            let journal = prodex_profile_export::read_profile_import_auth_update_journal(&path)
                .with_context(|| {
                    format!("failed to read lifecycle auth journal {}", path.display())
                })?;
            if let Some(temporary_home) = journal.temporary_home.as_deref() {
                validate_temporary_home_path(
                    paths,
                    Path::new(temporary_home),
                    "auth journal temporary home",
                )?;
            }
            Ok((path, journal))
        })
        .collect()
}

fn auth_journals_match_committed(
    state: &AppState,
    journals: &[(
        PathBuf,
        prodex_profile_export::ImportedExistingProfileAuthUpdateJournal,
    )],
) -> Result<bool> {
    journals.iter().try_fold(true, |matches, (_, journal)| {
        Ok(matches
            && super::import::lifecycle_support::imported_auth_update_journal_is_committed(
                state, journal,
            )?)
    })
}

fn lifecycle_home_actions_match_committed(actions: &[ProfileLifecycleHomeAction]) -> bool {
    actions.iter().all(|action| match action {
        ProfileLifecycleHomeAction::Promote { destination, .. }
        | ProfileLifecycleHomeAction::Create { path: destination } => {
            matches!(
                fs::symlink_metadata(destination),
                Ok(metadata) if metadata.is_dir()
            )
        }
        ProfileLifecycleHomeAction::Cleanup { .. }
        | ProfileLifecycleHomeAction::Quarantine { .. } => true,
    })
}

fn cleanup_auth_journal_temporary_homes(
    journals: &[(
        PathBuf,
        prodex_profile_export::ImportedExistingProfileAuthUpdateJournal,
    )],
) -> Result<()> {
    for (_, journal) in journals {
        if let Some(temporary_home) = journal.temporary_home.as_deref() {
            remove_home(Path::new(temporary_home))?;
        }
    }
    Ok(())
}

fn save_rolled_back_lifecycle_state(
    paths: &AppPaths,
    state: &AppState,
    removed_profiles: &[String],
) -> Result<()> {
    let _lock = crate::acquire_state_file_lock(paths)?;
    let existing = AppState::load(paths)?;
    let mut merged = prodex_state::merge_app_state_for_save(existing, state);
    for profile_name in removed_profiles {
        merged.profiles.remove(profile_name);
    }
    merged.active_profile = state
        .active_profile
        .clone()
        .filter(|profile_name| merged.profiles.contains_key(profile_name));
    let merged = prodex_state::compact_app_state(merged, Local::now().timestamp());
    let json = serde_json::to_string_pretty(&merged)
        .context("failed to serialize rolled-back profile lifecycle state")?;
    crate::write_state_json_atomic(paths, &json)
}

fn lifecycle_state_matches(
    state: &AppState,
    profile_states: &[ProfileLifecycleProfileState],
    active_profile: &Option<String>,
    committed: bool,
) -> Result<bool> {
    if state.active_profile != *active_profile {
        return Ok(false);
    }
    for expected in profile_states {
        let actual = state
            .profiles
            .get(&expected.name)
            .map(serde_json::to_value)
            .transpose()?;
        let expected_value = if committed {
            &expected.after
        } else {
            &expected.before
        };
        if actual.as_ref() != expected_value.as_ref() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn validate_plan(paths: &AppPaths, operation: &str, plan: &ProfileLifecyclePlan) -> Result<()> {
    prodex_profile_export::validate_profile_lifecycle_operation(operation)?;
    let mut names = BTreeSet::new();
    for profile in &plan.profile_states {
        prodex_profile_identity::validate_profile_name(&profile.name)?;
        if !names.insert(profile.name.clone()) {
            bail!(
                "profile lifecycle journal repeats profile '{}'",
                profile.name
            );
        }
        for value in [&profile.before, &profile.after].into_iter().flatten() {
            let entry: ProfileEntry = serde_json::from_value(value.clone()).with_context(|| {
                format!(
                    "profile lifecycle journal profile '{}' is invalid",
                    profile.name
                )
            })?;
            if entry.managed
                && !path_is_strictly_under_root(&paths.managed_profiles_root, &entry.codex_home)
            {
                bail!(
                    "profile lifecycle journal profile '{}' home is outside managed profiles root",
                    profile.name
                );
            }
        }
    }
    for active_profile in [&plan.previous_active_profile, &plan.next_active_profile]
        .into_iter()
        .flatten()
    {
        prodex_profile_identity::validate_profile_name(active_profile)?;
    }
    validate_home_actions(paths, &plan.home_actions)?;
    Ok(())
}

fn validate_managed_path(paths: &AppPaths, path: &Path, label: &str) -> Result<()> {
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
        || !path_is_strictly_under_root(&paths.managed_profiles_root, path)
    {
        bail!(
            "profile lifecycle {label} {} is outside managed profiles root",
            path.display()
        );
    }
    Ok(())
}

pub(super) fn validate_temporary_home_path(
    paths: &AppPaths,
    path: &Path,
    label: &str,
) -> Result<()> {
    validate_managed_path(paths, path, label)?;
    if path
        .strip_prefix(&paths.managed_profiles_root)
        .map_or(true, |relative| relative.components().count() != 1)
    {
        bail!("profile lifecycle {label} {} is invalid", path.display());
    }
    let valid_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(".login-") || name.starts_with(".import-"));
    if !valid_name {
        bail!("profile lifecycle {label} {} is invalid", path.display());
    }
    Ok(())
}

fn restore_lifecycle_state(
    state: &mut AppState,
    plan: &ProfileLifecyclePlan,
) -> Result<Vec<String>> {
    let mut removed_profiles = Vec::new();
    for profile in &plan.profile_states {
        match profile.before.as_ref() {
            Some(before) => {
                let entry: ProfileEntry = serde_json::from_value(before.clone())
                    .with_context(|| format!("failed to restore profile '{}'", profile.name))?;
                state.profiles.insert(profile.name.clone(), entry);
            }
            None => {
                state.profiles.remove(&profile.name);
                removed_profiles.push(profile.name.clone());
                state.last_run_selected_at.remove(&profile.name);
                state
                    .response_profile_bindings
                    .retain(|_, binding| binding.profile_name != profile.name);
                state
                    .session_profile_bindings
                    .retain(|_, binding| binding.profile_name != profile.name);
            }
        }
    }
    state.active_profile = plan.previous_active_profile.clone();
    Ok(removed_profiles)
}

fn remove_lifecycle_journal(path: &Path) -> Result<()> {
    cleanup_profile_lifecycle_journal(path);
    match fs::symlink_metadata(path) {
        Ok(_) => bail!(
            "failed to remove profile lifecycle journal {}",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

fn finish_home_actions(actions: &[ProfileLifecycleHomeAction], committed: bool) -> Result<()> {
    for action in actions {
        match action {
            ProfileLifecycleHomeAction::Promote {
                source,
                destination,
                rollback,
            } => {
                if committed {
                    promote_home(Path::new(source), Path::new(destination))?;
                } else {
                    rollback_promoted_home(Path::new(source), Path::new(destination), rollback)?;
                }
            }
            ProfileLifecycleHomeAction::Create { path } => {
                if !committed {
                    remove_home(Path::new(path))?;
                }
            }
            ProfileLifecycleHomeAction::Cleanup { path } => {
                remove_home(Path::new(path))?;
            }
            ProfileLifecycleHomeAction::Quarantine { source, quarantine } => {
                finish_quarantine_home(source, quarantine, committed)?;
            }
        }
    }
    Ok(())
}

fn finish_quarantine_home(source: &str, quarantine: &str, committed: bool) -> Result<()> {
    let source = Path::new(source);
    let quarantine = Path::new(quarantine);
    if committed {
        remove_home(quarantine)?;
        remove_home(source)?;
    } else {
        let source_exists = lifecycle_path_exists(source)?;
        let quarantine_exists = lifecycle_path_exists(quarantine)?;
        if !source_exists && quarantine_exists {
            fs::rename(quarantine, source).with_context(|| {
                format!(
                    "failed to restore quarantined profile home {}",
                    source.display()
                )
            })?;
        } else if source_exists && quarantine_exists {
            remove_home(quarantine)?;
        }
    }
    Ok(())
}

fn lifecycle_path_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

fn promote_home(source: &Path, destination: &Path) -> Result<()> {
    if !destination.exists() && source.exists() {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        match fs::rename(source, destination) {
            Ok(()) => {}
            Err(_) => {
                crate::copy_codex_home(source, destination)?;
                remove_home(source)?;
            }
        }
    }
    if source.exists() && destination.exists() {
        remove_home(source)?;
    }
    Ok(())
}

fn rollback_promoted_home(
    source: &Path,
    destination: &Path,
    rollback: &ProfileLifecyclePromoteRollback,
) -> Result<()> {
    match rollback {
        ProfileLifecyclePromoteRollback::Remove => {
            remove_home(source)?;
            remove_home(destination)?;
        }
        ProfileLifecyclePromoteRollback::RestoreSource => {
            if !source.exists() && destination.exists() {
                fs::rename(destination, source).with_context(|| {
                    format!(
                        "failed to restore temporary profile home {}",
                        source.display()
                    )
                })?;
            } else if source.exists() && destination.exists() {
                remove_home(destination)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn remove_home(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))
        }
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(path).with_context(|| format!("failed to remove {}", path.display()))
        }
        Ok(_) => bail!(
            "profile lifecycle path {} is not a directory",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

pub(crate) fn lifecycle_profile_state(
    name: &str,
    before: Option<&ProfileEntry>,
    after: Option<&ProfileEntry>,
) -> Result<ProfileLifecycleProfileState> {
    Ok(ProfileLifecycleProfileState {
        name: name.to_string(),
        before: before.map(serde_json::to_value).transpose()?,
        after: after.map(serde_json::to_value).transpose()?,
    })
}
