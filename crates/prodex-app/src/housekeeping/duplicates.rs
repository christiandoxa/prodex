use super::{
    AppState, AppStateIoExt, ProdexCleanupSummary, ProfileIdentity, RuntimeContinuationStore,
    fetch_profile_identity, load_runtime_continuation_journal_with_recovery,
    load_runtime_continuations_with_recovery, map_parallel, read_profile_identity_from_auth,
    runtime_continuation_journal_file_path, runtime_continuation_journal_last_good_file_path,
    runtime_continuations_file_path, runtime_continuations_last_good_file_path,
    save_runtime_continuation_journal_for_profiles, save_runtime_continuations_for_profiles,
};
use anyhow::{Context, Result};
use prodex_core::{AppPaths, path_is_strictly_under_root, same_path};
use prodex_profile_identity::canonical_profile_identity_key;
use prodex_state::{
    ensure_active_profile_after_duplicate_cleanup, remap_profile_binding_targets,
    remove_duplicate_profile_from_state, select_canonical_duplicate_profile,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

fn remap_runtime_continuation_store_profiles(
    continuations: &mut RuntimeContinuationStore,
    from_profile: &str,
    to_profile: &str,
) {
    remap_profile_binding_targets(
        &mut continuations.response_profile_bindings,
        from_profile,
        to_profile,
    );
    remap_profile_binding_targets(
        &mut continuations.session_profile_bindings,
        from_profile,
        to_profile,
    );
    remap_profile_binding_targets(
        &mut continuations.turn_state_bindings,
        from_profile,
        to_profile,
    );
    remap_profile_binding_targets(
        &mut continuations.session_id_bindings,
        from_profile,
        to_profile,
    );
}

fn cleanup_duplicate_identity_key(identity: &ProfileIdentity) -> Option<String> {
    canonical_profile_identity_key(identity.account_id.as_deref(), identity.email.as_deref())
}

fn resolve_cleanup_profile_identities(state: &mut AppState) -> Vec<(String, ProfileIdentity)> {
    let jobs = state
        .profiles
        .iter()
        .map(|(name, profile)| {
            (
                name.clone(),
                profile.codex_home.clone(),
                profile
                    .email
                    .clone()
                    .filter(|email| !email.trim().is_empty()),
            )
        })
        .collect::<Vec<_>>();

    let resolved = map_parallel(jobs, |(name, codex_home, cached_email)| {
        let stored_identity = if secret_store::auth_json_path(&codex_home).exists() {
            read_profile_identity_from_auth(&codex_home)
        } else {
            Ok(ProfileIdentity::default())
        };
        let identity = stored_identity.map(|stored_identity| {
            let mut identity = fetch_profile_identity(&codex_home).unwrap_or(stored_identity);
            if identity.email.is_none() {
                identity.email = cached_email;
            }
            identity
        });
        (name, identity)
    });

    let mut discovered = Vec::new();
    for (name, identity) in resolved {
        let Ok(identity) = identity else {
            continue;
        };
        if let Some(email) = identity.email.as_ref()
            && let Some(profile) = state.profiles.get_mut(&name)
        {
            profile.email = Some(email.clone());
        }
        if cleanup_duplicate_identity_key(&identity).is_some() {
            discovered.push((name, identity));
        }
    }
    discovered
}

pub(super) fn cleanup_duplicate_profiles(
    paths: &AppPaths,
    state: &mut AppState,
) -> Result<ProdexCleanupSummary> {
    let mut duplicates_by_identity = BTreeMap::<String, Vec<String>>::new();
    for (profile_name, identity) in resolve_cleanup_profile_identities(state) {
        let Some(identity_key) = cleanup_duplicate_identity_key(&identity) else {
            continue;
        };
        duplicates_by_identity
            .entry(identity_key)
            .or_default()
            .push(profile_name);
    }

    let duplicate_groups = duplicates_by_identity
        .into_values()
        .filter(|profile_names| profile_names.len() > 1)
        .collect::<Vec<_>>();
    if duplicate_groups.is_empty() {
        return Ok(ProdexCleanupSummary::default());
    }

    let continuations_exist = runtime_continuations_file_path(paths).exists()
        || runtime_continuations_last_good_file_path(paths).exists();
    let journal_exists = runtime_continuation_journal_file_path(paths).exists()
        || runtime_continuation_journal_last_good_file_path(paths).exists();
    let mut continuations = if continuations_exist {
        Some(load_runtime_continuations_with_recovery(paths, &state.profiles)?.value)
    } else {
        None
    };
    let mut continuation_journal = if journal_exists {
        Some(load_runtime_continuation_journal_with_recovery(paths, &state.profiles)?.value)
    } else {
        None
    };

    let mut summary = ProdexCleanupSummary::default();
    let mut removed_names = BTreeSet::new();
    let mut homes_to_delete = Vec::new();
    for mut profile_names in duplicate_groups {
        profile_names.sort();
        let Some(canonical_name) = select_canonical_duplicate_profile(state, &profile_names) else {
            continue;
        };
        let canonical_home = state
            .profiles
            .get(&canonical_name)
            .map(|profile| profile.codex_home.clone())
            .with_context(|| format!("profile '{}' is missing during cleanup", canonical_name))?;

        for duplicate_name in profile_names
            .into_iter()
            .filter(|profile_name| profile_name != &canonical_name)
        {
            if let Some(continuations) = continuations.as_mut() {
                remap_runtime_continuation_store_profiles(
                    continuations,
                    &duplicate_name,
                    &canonical_name,
                );
            }
            if let Some(journal) = continuation_journal.as_mut() {
                remap_runtime_continuation_store_profiles(
                    &mut journal.continuations,
                    &duplicate_name,
                    &canonical_name,
                );
            }

            let Some(removed_profile) =
                remove_duplicate_profile_from_state(state, &duplicate_name, &canonical_name)
            else {
                continue;
            };
            summary.duplicate_profiles_removed += 1;
            removed_names.insert(duplicate_name);

            if removed_profile.managed
                && removed_profile.codex_home.exists()
                && !same_path(&removed_profile.codex_home, &canonical_home)
            {
                prodex_shared_codex_fs::ensure_managed_profiles_root(paths)?;
                if !path_is_strictly_under_root(
                    &paths.managed_profiles_root,
                    &removed_profile.codex_home,
                ) {
                    anyhow::bail!(
                        "refusing to delete duplicate managed profile home outside managed profiles root: {}",
                        removed_profile.codex_home.display()
                    );
                }
                homes_to_delete.push(removed_profile.codex_home);
            }
        }
    }

    ensure_active_profile_after_duplicate_cleanup(state);

    if let Some(continuations) = continuations.as_ref() {
        save_runtime_continuations_for_profiles(paths, continuations, &state.profiles)?;
    }
    if let Some(journal) = continuation_journal.as_ref() {
        save_runtime_continuation_journal_for_profiles(
            paths,
            &journal.continuations,
            &state.profiles,
            journal.saved_at,
        )?;
    }

    let removed_names = removed_names.into_iter().collect::<Vec<_>>();
    state.save_with_removed_profiles(paths, &removed_names)?;
    for home in homes_to_delete {
        fs::remove_dir_all(&home).with_context(|| {
            format!(
                "failed to delete duplicate managed profile home {}",
                home.display()
            )
        })?;
        summary.duplicate_managed_profile_homes_removed += 1;
    }
    Ok(summary)
}
