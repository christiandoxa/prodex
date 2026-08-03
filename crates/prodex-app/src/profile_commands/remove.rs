use super::import_export::{
    ProfileLifecycleHomeAction, ProfileLifecyclePlan, acquire_profile_lifecycle_lock,
    lifecycle_profile_state, load_profile_state_with_profile_recovery_locked, remove_home,
    write_profile_lifecycle_plan,
};
use super::manage::print_profile_panel;
use anyhow::{Context, Result, bail};
use prodex_core::path_is_strictly_under_root;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use crate::{
    AppPaths, AppState, AppStateIoExt, ProfileEntry, RemoveProfileArgs, audit_log_event,
    load_runtime_continuation_journal_with_recovery, load_runtime_continuations_with_recovery,
    runtime_continuation_journal_file_path, runtime_continuation_journal_last_good_file_path,
    runtime_continuations_file_path, runtime_continuations_last_good_file_path,
    runtime_random_token, save_runtime_continuation_journal_for_profiles,
    save_runtime_continuations_for_profiles,
};

#[derive(Debug)]
struct RemovedProfileRecord {
    name: String,
    managed: bool,
    deleted_home: bool,
    delete_home: bool,
    codex_home: PathBuf,
    quarantine_home: Option<PathBuf>,
}

pub(crate) fn persist_pruned_profile_runtime_sidecars(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<()> {
    let continuations_exist = runtime_continuations_file_path(paths).exists()
        || runtime_continuations_last_good_file_path(paths).exists();
    if continuations_exist {
        let continuations = load_runtime_continuations_with_recovery(paths, profiles)?.value;
        save_runtime_continuations_for_profiles(paths, &continuations, profiles)?;
    }

    let journal_exists = runtime_continuation_journal_file_path(paths).exists()
        || runtime_continuation_journal_last_good_file_path(paths).exists();
    if journal_exists {
        let journal = load_runtime_continuation_journal_with_recovery(paths, profiles)?.value;
        save_runtime_continuation_journal_for_profiles(
            paths,
            &journal.continuations,
            profiles,
            journal.saved_at,
        )?;
    }

    Ok(())
}

pub(crate) fn finalize_recovered_profile_removals(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
    journal_paths: &[PathBuf],
) -> Result<()> {
    if journal_paths.is_empty() {
        return Ok(());
    }
    persist_pruned_profile_runtime_sidecars(paths, profiles)?;
    for journal_path in journal_paths {
        prodex_profile_export::cleanup_profile_lifecycle_journal(journal_path);
    }
    Ok(())
}

pub(crate) fn handle_remove_profile(args: RemoveProfileArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let _lock = acquire_profile_lifecycle_lock(&paths)?;
    let (mut state, lifecycle_recovery) =
        load_profile_state_with_profile_recovery_locked(&paths, true)?;
    finalize_recovered_profile_removals(
        &paths,
        &state.profiles,
        &lifecycle_recovery.pending_removal_journals,
    )?;
    if !args.all
        && args.name.as_deref().is_some_and(|name| {
            lifecycle_recovery
                .completed_removal_profiles
                .iter()
                .any(|removed| removed == name)
        })
    {
        return Ok(());
    }

    let target_names = prodex_profile_identity::resolve_remove_profile_targets(
        state
            .profiles
            .iter()
            .map(|(name, profile)| (name.as_str(), profile.managed)),
        args.all,
        args.name.as_deref(),
        args.delete_home,
    )?;
    let previous_state = state.clone();
    let mut removed_profiles =
        remove_profiles_from_state(&paths, &mut state, &target_names, args.delete_home)?;
    prune_removed_profile_metadata(&mut state, &target_names);
    assign_quarantine_homes(&paths, &mut removed_profiles)?;
    let lifecycle_path = write_profile_lifecycle_plan(
        &paths,
        "remove",
        &build_remove_lifecycle_plan(&previous_state, &state, &removed_profiles)?,
    )?;
    quarantine_removed_profile_homes(&mut removed_profiles)?;
    state.save_with_removed_profiles(&paths, &target_names)?;
    persist_pruned_profile_runtime_sidecars(&paths, &state.profiles)?;
    delete_removed_profile_homes(&mut removed_profiles)?;

    if args.all {
        print_bulk_profile_removal_result(&state, &removed_profiles)?;
        prodex_profile_export::cleanup_profile_lifecycle_journal(&lifecycle_path);
        return Ok(());
    }

    let Some(removed_profile) = removed_profiles.into_iter().next() else {
        bail!("internal error: single-profile removal did not remove a profile");
    };
    print_single_profile_removal_result(&state, removed_profile)?;
    prodex_profile_export::cleanup_profile_lifecycle_journal(&lifecycle_path);

    Ok(())
}

fn remove_profiles_from_state(
    paths: &AppPaths,
    state: &mut AppState,
    target_names: &[String],
    delete_home: bool,
) -> Result<Vec<RemovedProfileRecord>> {
    let mut removed_profiles = Vec::with_capacity(target_names.len());
    for name in target_names {
        let profile = state
            .profiles
            .remove(name)
            .with_context(|| format!("profile '{}' disappeared from state", name))?;
        let delete_home = profile_home_deletion_requested(paths, &profile, delete_home)?;
        removed_profiles.push(RemovedProfileRecord {
            name: name.clone(),
            managed: profile.managed,
            deleted_home: false,
            delete_home,
            codex_home: profile.codex_home,
            quarantine_home: None,
        });
    }

    Ok(removed_profiles)
}

fn profile_home_deletion_requested(
    paths: &AppPaths,
    profile: &ProfileEntry,
    delete_home: bool,
) -> Result<bool> {
    let should_delete_home = prodex_profile_identity::should_delete_profile_home(
        profile.managed,
        delete_home,
        profile.codex_home.display(),
    )?;
    if !should_delete_home {
        return Ok(false);
    }

    if profile.managed {
        super::ensure_managed_profiles_root(paths)?;
        if !path_is_strictly_under_root(&paths.managed_profiles_root, &profile.codex_home) {
            bail!(
                "refusing to delete managed profile home outside managed profiles root: {}",
                profile.codex_home.display()
            );
        }
    }

    Ok(true)
}

fn delete_removed_profile_homes(removed_profiles: &mut [RemovedProfileRecord]) -> Result<()> {
    for profile in removed_profiles
        .iter_mut()
        .filter(|profile| profile.delete_home)
    {
        let home = profile
            .quarantine_home
            .as_deref()
            .unwrap_or(&profile.codex_home);
        remove_home(home)?;
        profile.deleted_home = true;
    }
    Ok(())
}

fn assign_quarantine_homes(
    paths: &AppPaths,
    removed_profiles: &mut [RemovedProfileRecord],
) -> Result<()> {
    for profile in removed_profiles
        .iter_mut()
        .filter(|profile| profile.delete_home)
    {
        let quarantine = paths.managed_profiles_root.join(format!(
            ".remove-{}-{}",
            profile.name,
            runtime_random_token("home")?
        ));
        profile.quarantine_home = Some(quarantine);
    }
    Ok(())
}

fn quarantine_removed_profile_homes(removed_profiles: &mut [RemovedProfileRecord]) -> Result<()> {
    for profile in removed_profiles
        .iter()
        .filter(|profile| profile.delete_home)
    {
        match fs::symlink_metadata(&profile.codex_home) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "failed to inspect profile home {}",
                        profile.codex_home.display()
                    )
                });
            }
        }
        let quarantine = profile
            .quarantine_home
            .as_ref()
            .context("missing profile home quarantine path")?;
        fs::rename(&profile.codex_home, quarantine).with_context(|| {
            format!(
                "failed to quarantine profile home {}",
                profile.codex_home.display()
            )
        })?;
    }
    Ok(())
}

fn build_remove_lifecycle_plan(
    previous_state: &AppState,
    state: &AppState,
    removed_profiles: &[RemovedProfileRecord],
) -> Result<ProfileLifecyclePlan> {
    Ok(ProfileLifecyclePlan {
        profile_states: removed_profiles
            .iter()
            .map(|profile| {
                lifecycle_profile_state(
                    &profile.name,
                    previous_state.profiles.get(&profile.name),
                    state.profiles.get(&profile.name),
                )
            })
            .collect::<Result<Vec<_>>>()?,
        previous_active_profile: previous_state.active_profile.clone(),
        next_active_profile: state.active_profile.clone(),
        home_actions: removed_profiles
            .iter()
            .filter_map(|profile| {
                profile.quarantine_home.as_ref().map(|quarantine| {
                    ProfileLifecycleHomeAction::Quarantine {
                        source: profile.codex_home.display().to_string(),
                        quarantine: quarantine.display().to_string(),
                    }
                })
            })
            .collect(),
        auth_journal_paths: Vec::new(),
    })
}

pub(crate) fn prune_removed_profile_metadata(state: &mut AppState, target_names: &[String]) {
    let plan = prodex_profile_identity::plan_removed_profile_state(
        state.profiles.keys().map(String::as_str),
        state.active_profile.as_deref(),
        target_names.iter().map(String::as_str),
    );
    state
        .last_run_selected_at
        .retain(|profile_name, _| !plan.removed_names.contains(profile_name));
    state
        .response_profile_bindings
        .retain(|_, binding| !plan.removed_names.contains(&binding.profile_name));
    state
        .session_profile_bindings
        .retain(|_, binding| !plan.removed_names.contains(&binding.profile_name));

    state.active_profile = plan.active_profile;
}

fn print_bulk_profile_removal_result(
    state: &AppState,
    removed_profiles: &[RemovedProfileRecord],
) -> Result<()> {
    audit_log_event(
        "profile",
        "remove",
        "success",
        serde_json::json!({
            "all": true,
            "removed_count": removed_profiles.len(),
            "profile_names": removed_profiles.iter().map(|profile| profile.name.clone()).collect::<Vec<_>>(),
            "deleted_home_count": removed_profiles.iter().filter(|profile| profile.deleted_home).count(),
            "active_profile": state.active_profile.clone(),
        }),
    )?;

    let mut fields = vec![
        (
            "Result".to_string(),
            format!("Removed {} profile(s).", removed_profiles.len()),
        ),
        (
            "Deleted homes".to_string(),
            removed_profiles
                .iter()
                .filter(|profile| profile.deleted_home)
                .count()
                .to_string(),
        ),
        (
            "Active".to_string(),
            state
                .active_profile
                .clone()
                .unwrap_or_else(|| "cleared".to_string()),
        ),
    ];
    if !removed_profiles.is_empty() {
        fields.push((
            "Profiles".to_string(),
            removed_profiles
                .iter()
                .map(|profile| profile.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        ));
    }
    print_profile_panel("Profiles Removed", &fields)
}

fn print_single_profile_removal_result(
    state: &AppState,
    removed_profile: RemovedProfileRecord,
) -> Result<()> {
    audit_log_event(
        "profile",
        "remove",
        "success",
        serde_json::json!({
            "profile_name": removed_profile.name.clone(),
            "managed": removed_profile.managed,
            "deleted_home": removed_profile.deleted_home,
            "codex_home": removed_profile.codex_home.display().to_string(),
            "active_profile": state.active_profile.clone(),
        }),
    )?;

    let fields = vec![
        (
            "Result".to_string(),
            format!("Removed profile '{}'.", removed_profile.name),
        ),
        (
            "Deleted home".to_string(),
            if removed_profile.deleted_home {
                "Yes".to_string()
            } else {
                "No".to_string()
            },
        ),
        (
            "Active".to_string(),
            state
                .active_profile
                .clone()
                .unwrap_or_else(|| "cleared".to_string()),
        ),
    ];
    print_profile_panel("Profile Removed", &fields)
}
