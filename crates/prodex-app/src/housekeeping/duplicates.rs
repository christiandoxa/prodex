use super::*;
use prodex_core::same_path;
use prodex_state::{
    duplicate_profile_identity_key, ensure_active_profile_after_duplicate_cleanup,
    remap_profile_binding_targets, remove_duplicate_profile_from_state,
    select_canonical_duplicate_profile,
};

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
    duplicate_profile_identity_key(identity.account_id.as_deref(), identity.email.as_deref())
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
        let mut identity = fetch_profile_identity(&codex_home).unwrap_or_default();
        if identity.email.is_none() {
            identity.email = cached_email;
        }
        (name, identity)
    });

    let mut discovered = Vec::new();
    for (name, identity) in resolved {
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

            if removed_profile.managed
                && removed_profile.codex_home.exists()
                && !same_path(&removed_profile.codex_home, &canonical_home)
            {
                fs::remove_dir_all(&removed_profile.codex_home).with_context(|| {
                    format!(
                        "failed to delete duplicate managed profile home {}",
                        removed_profile.codex_home.display()
                    )
                })?;
                summary.duplicate_managed_profile_homes_removed += 1;
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

    state.save(paths)?;
    Ok(summary)
}
