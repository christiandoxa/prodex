use super::*;

fn remap_profile_binding_targets(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
    from_profile: &str,
    to_profile: &str,
) {
    if from_profile == to_profile {
        return;
    }
    for binding in bindings.values_mut() {
        if binding.profile_name == from_profile {
            binding.profile_name = to_profile.to_string();
        }
    }
}

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

fn duplicate_profile_cleanup_priority(state: &AppState, profile_name: &str) -> (bool, i64, bool) {
    let active = state.active_profile.as_deref() == Some(profile_name);
    let last_selected_at = state
        .last_run_selected_at
        .get(profile_name)
        .copied()
        .unwrap_or(i64::MIN);
    let prefer_external = state
        .profiles
        .get(profile_name)
        .map(|profile| !profile.managed)
        .unwrap_or(false);
    (active, last_selected_at, prefer_external)
}

fn select_canonical_duplicate_profile(
    state: &AppState,
    profile_names: &[String],
) -> Option<String> {
    profile_names.iter().cloned().max_by(|left, right| {
        duplicate_profile_cleanup_priority(state, left)
            .cmp(&duplicate_profile_cleanup_priority(state, right))
            .then_with(|| right.cmp(left))
    })
}

fn resolve_cleanup_profile_emails(state: &mut AppState) -> Vec<(String, String)> {
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
        (
            name,
            cached_email.or_else(|| fetch_profile_email(&codex_home).ok()),
        )
    });

    let mut discovered = Vec::new();
    for (name, email) in resolved {
        let Some(email) = email else {
            continue;
        };
        if let Some(profile) = state.profiles.get_mut(&name) {
            profile.email = Some(email.clone());
        }
        discovered.push((name, email));
    }
    discovered
}

pub(super) fn cleanup_duplicate_profiles(
    paths: &AppPaths,
    state: &mut AppState,
) -> Result<ProdexCleanupSummary> {
    let mut duplicates_by_email = BTreeMap::<String, Vec<String>>::new();
    for (profile_name, email) in resolve_cleanup_profile_emails(state) {
        duplicates_by_email
            .entry(normalize_email(&email))
            .or_default()
            .push(profile_name);
    }

    let duplicate_groups = duplicates_by_email
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
            let duplicate_last_selected_at = state.last_run_selected_at.remove(&duplicate_name);
            if let Some(last_selected_at) = duplicate_last_selected_at {
                let target = state
                    .last_run_selected_at
                    .entry(canonical_name.clone())
                    .or_insert(last_selected_at);
                *target = (*target).max(last_selected_at);
            }

            remap_profile_binding_targets(
                &mut state.response_profile_bindings,
                &duplicate_name,
                &canonical_name,
            );
            remap_profile_binding_targets(
                &mut state.session_profile_bindings,
                &duplicate_name,
                &canonical_name,
            );
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

            if state.active_profile.as_deref() == Some(duplicate_name.as_str()) {
                state.active_profile = Some(canonical_name.clone());
            }

            let Some(removed_profile) = state.profiles.remove(&duplicate_name) else {
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

    if state.active_profile.is_none() {
        state.active_profile = state.profiles.keys().next().cloned();
    }

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
