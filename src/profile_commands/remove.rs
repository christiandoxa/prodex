use super::*;

#[derive(Debug)]
struct RemovedProfileRecord {
    name: String,
    managed: bool,
    deleted_home: bool,
    codex_home: PathBuf,
}

fn persist_pruned_profile_runtime_sidecars(
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

pub(crate) fn handle_remove_profile(args: RemoveProfileArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;

    let target_names = prodex_profile_identity::resolve_remove_profile_targets(
        state
            .profiles
            .iter()
            .map(|(name, profile)| (name.as_str(), profile.managed)),
        args.all,
        args.name.as_deref(),
        args.delete_home,
    )?;
    let removed_profiles = remove_profiles_from_state(&mut state, &target_names, args.delete_home)?;
    prune_removed_profile_metadata(&mut state, &target_names);
    state.save(&paths)?;
    persist_pruned_profile_runtime_sidecars(&paths, &state.profiles)?;

    if args.all {
        print_bulk_profile_removal_result(&state, &removed_profiles);
        return Ok(());
    }

    let removed_profile = removed_profiles
        .into_iter()
        .next()
        .expect("single-profile removal should record the removed profile");
    print_single_profile_removal_result(&state, removed_profile);

    Ok(())
}

fn remove_profiles_from_state(
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
        let deleted_home = remove_profile_home_if_requested(&profile, delete_home)?;
        removed_profiles.push(RemovedProfileRecord {
            name: name.clone(),
            managed: profile.managed,
            deleted_home,
            codex_home: profile.codex_home,
        });
    }

    Ok(removed_profiles)
}

fn remove_profile_home_if_requested(profile: &ProfileEntry, delete_home: bool) -> Result<bool> {
    let should_delete_home = prodex_profile_identity::should_delete_profile_home(
        profile.managed,
        delete_home,
        profile.codex_home.display(),
    )?;
    if !should_delete_home {
        return Ok(false);
    }

    if profile.codex_home.exists() {
        fs::remove_dir_all(&profile.codex_home)
            .with_context(|| format!("failed to delete {}", profile.codex_home.display()))?;
    }

    Ok(true)
}

fn prune_removed_profile_metadata(state: &mut AppState, target_names: &[String]) {
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

fn print_bulk_profile_removal_result(state: &AppState, removed_profiles: &[RemovedProfileRecord]) {
    audit_log_event_best_effort(
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
    );

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
    print_panel("Profiles Removed", &fields);
}

fn print_single_profile_removal_result(state: &AppState, removed_profile: RemovedProfileRecord) {
    audit_log_event_best_effort(
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
    );

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
    print_panel("Profile Removed", &fields);
}
