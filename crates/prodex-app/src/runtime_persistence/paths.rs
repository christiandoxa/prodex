use super::*;

pub(crate) fn merge_app_state_for_save(existing: AppState, desired: &AppState) -> AppState {
    let active_profile = desired
        .active_profile
        .clone()
        .filter(|profile_name| desired.profiles.contains_key(profile_name));
    let merged = AppState {
        active_profile,
        profiles: desired.profiles.clone(),
        last_run_selected_at: merge_last_run_selection(
            &existing.last_run_selected_at,
            &desired.last_run_selected_at,
            &desired.profiles,
        ),
        response_profile_bindings: merge_profile_bindings(
            &existing.response_profile_bindings,
            &desired.response_profile_bindings,
            &desired.profiles,
        ),
        session_profile_bindings: merge_profile_bindings(
            &existing.session_profile_bindings,
            &desired.session_profile_bindings,
            &desired.profiles,
        ),
    };
    compact_app_state(merged, Local::now().timestamp())
}

pub(crate) fn runtime_continuations_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-continuations.json")
}

pub(crate) fn runtime_continuations_last_good_file_path(paths: &AppPaths) -> PathBuf {
    last_good_file_path(&runtime_continuations_file_path(paths))
}

pub(crate) fn runtime_continuation_journal_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-continuation-journal.json")
}

pub(crate) fn runtime_continuation_journal_last_good_file_path(paths: &AppPaths) -> PathBuf {
    last_good_file_path(&runtime_continuation_journal_file_path(paths))
}

pub(crate) fn runtime_broker_registry_file_path(paths: &AppPaths, broker_key: &str) -> PathBuf {
    paths.root.join(format!("runtime-broker-{broker_key}.json"))
}

pub(crate) fn runtime_broker_registry_last_good_file_path(
    paths: &AppPaths,
    broker_key: &str,
) -> PathBuf {
    last_good_file_path(&runtime_broker_registry_file_path(paths, broker_key))
}

pub(crate) fn runtime_broker_lease_dir(paths: &AppPaths, broker_key: &str) -> PathBuf {
    paths
        .root
        .join(format!("runtime-broker-{broker_key}-leases"))
}

pub(crate) fn runtime_broker_ensure_lock_path(paths: &AppPaths, broker_key: &str) -> PathBuf {
    paths
        .root
        .join(format!("runtime-broker-{broker_key}-ensure"))
}

pub(crate) fn runtime_broker_registry_keys(paths: &AppPaths) -> Vec<String> {
    let Ok(entries) = fs::read_dir(&paths.root) else {
        return Vec::new();
    };

    let mut keys = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            name.strip_prefix("runtime-broker-")
                .and_then(|suffix| suffix.strip_suffix(".json"))
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    keys.sort();
    keys.dedup();
    keys
}

pub(crate) fn update_check_cache_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("update-check.json")
}
