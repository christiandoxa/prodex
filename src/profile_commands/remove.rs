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

    let target_names = resolve_remove_profile_targets(&state, &args)?;
    validate_bulk_profile_home_deletion(&state, &target_names, &args)?;
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

fn resolve_remove_profile_targets(
    state: &AppState,
    args: &RemoveProfileArgs,
) -> Result<Vec<String>> {
    if args.all {
        return Ok(state.profiles.keys().cloned().collect::<Vec<_>>());
    }

    let Some(name) = args.name.as_deref() else {
        bail!("provide a profile name or pass --all");
    };
    if !state.profiles.contains_key(name) {
        bail!("profile '{}' does not exist", name);
    }
    Ok(vec![name.to_string()])
}

fn validate_bulk_profile_home_deletion(
    state: &AppState,
    target_names: &[String],
    args: &RemoveProfileArgs,
) -> Result<()> {
    if !args.all || !args.delete_home {
        return Ok(());
    }

    let external_profiles = target_names
        .iter()
        .filter(|name| {
            state
                .profiles
                .get(*name)
                .is_some_and(|profile| !profile.managed)
        })
        .cloned()
        .collect::<Vec<_>>();
    if !external_profiles.is_empty() {
        bail!(
            "--delete-home with --all refuses to delete external profiles: {}",
            external_profiles.join(", ")
        );
    }

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
    let should_delete_home = profile.managed || delete_home;
    if !should_delete_home {
        return Ok(false);
    }

    if !profile.managed && delete_home {
        bail!(
            "refusing to delete external path {}",
            profile.codex_home.display()
        );
    }
    if profile.codex_home.exists() {
        fs::remove_dir_all(&profile.codex_home)
            .with_context(|| format!("failed to delete {}", profile.codex_home.display()))?;
    }

    Ok(true)
}

fn prune_removed_profile_metadata(state: &mut AppState, target_names: &[String]) {
    let removed_names = target_names.iter().cloned().collect::<BTreeSet<_>>();
    state
        .last_run_selected_at
        .retain(|profile_name, _| !removed_names.contains(profile_name));
    state
        .response_profile_bindings
        .retain(|_, binding| !removed_names.contains(&binding.profile_name));
    state
        .session_profile_bindings
        .retain(|_, binding| !removed_names.contains(&binding.profile_name));

    if state
        .active_profile
        .as_deref()
        .is_some_and(|profile_name| removed_names.contains(profile_name))
    {
        state.active_profile = state.profiles.keys().next().cloned();
    }
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
