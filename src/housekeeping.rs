use super::*;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProdexCleanupSummary {
    pub(crate) duplicate_profiles_removed: usize,
    pub(crate) duplicate_managed_profile_homes_removed: usize,
    pub(crate) runtime_logs_removed: usize,
    pub(crate) stale_runtime_log_pointer_removed: usize,
    pub(crate) stale_login_dirs_removed: usize,
    pub(crate) orphan_managed_profile_dirs_removed: usize,
    pub(crate) transient_root_files_removed: usize,
    pub(crate) stale_root_temp_files_removed: usize,
    pub(crate) dead_runtime_broker_leases_removed: usize,
    pub(crate) dead_runtime_broker_registries_removed: usize,
}

impl ProdexCleanupSummary {
    pub(crate) fn total_removed(self) -> usize {
        self.duplicate_profiles_removed
            + self.duplicate_managed_profile_homes_removed
            + self.runtime_logs_removed
            + self.stale_runtime_log_pointer_removed
            + self.stale_login_dirs_removed
            + self.orphan_managed_profile_dirs_removed
            + self.transient_root_files_removed
            + self.stale_root_temp_files_removed
            + self.dead_runtime_broker_leases_removed
            + self.dead_runtime_broker_registries_removed
    }

    fn merge(mut self, other: Self) -> Self {
        self.duplicate_profiles_removed += other.duplicate_profiles_removed;
        self.duplicate_managed_profile_homes_removed +=
            other.duplicate_managed_profile_homes_removed;
        self.runtime_logs_removed += other.runtime_logs_removed;
        self.stale_runtime_log_pointer_removed += other.stale_runtime_log_pointer_removed;
        self.stale_login_dirs_removed += other.stale_login_dirs_removed;
        self.orphan_managed_profile_dirs_removed += other.orphan_managed_profile_dirs_removed;
        self.transient_root_files_removed += other.transient_root_files_removed;
        self.stale_root_temp_files_removed += other.stale_root_temp_files_removed;
        self.dead_runtime_broker_leases_removed += other.dead_runtime_broker_leases_removed;
        self.dead_runtime_broker_registries_removed += other.dead_runtime_broker_registries_removed;
        self
    }
}

fn remove_file_if_exists(path: &Path) -> bool {
    match fs::remove_file(path) {
        Ok(()) => true,
        Err(err) if err.kind() == io::ErrorKind::NotFound => false,
        Err(_) => false,
    }
}

fn prodex_cleanup_transient_root_file_paths(paths: &AppPaths) -> Vec<PathBuf> {
    vec![
        runtime_scores_file_path(paths),
        runtime_scores_last_good_file_path(paths),
        runtime_usage_snapshots_file_path(paths),
        runtime_usage_snapshots_last_good_file_path(paths),
        runtime_backoffs_file_path(paths),
        runtime_backoffs_last_good_file_path(paths),
        update_check_cache_file_path(paths),
    ]
}

pub(crate) fn cleanup_prodex_transient_root_files(paths: &AppPaths) -> usize {
    prodex_cleanup_transient_root_file_paths(paths)
        .into_iter()
        .filter(|path| remove_file_if_exists(path))
        .count()
}

fn prodex_root_temp_file_name_is_owned(name: &str) -> bool {
    name.starts_with("state.json.")
        || name.starts_with("runtime-")
        || name.starts_with("update-check.json.")
}

fn prodex_root_temp_file_pid(name: &str) -> Option<u32> {
    let stem = name.strip_suffix(".tmp")?;
    let mut parts = stem.rsplitn(4, '.');
    let _sequence = parts.next()?;
    let _nanos = parts.next()?;
    let pid = parts.next()?;
    let _base_name = parts.next()?;
    pid.parse::<u32>().ok()
}

pub(crate) fn cleanup_prodex_stale_root_temp_files_at(paths: &AppPaths, now: SystemTime) -> usize {
    let Ok(entries) = fs::read_dir(&paths.root) else {
        return 0;
    };
    let oldest_allowed = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
        - PROD_EX_TMP_LOGIN_RETENTION_SECONDS;
    let mut removed = 0usize;

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.ends_with(".tmp") || !prodex_root_temp_file_name_is_owned(name) {
            continue;
        }

        let modified = entry
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(i64::MIN);
        let pid_alive = prodex_root_temp_file_pid(name).is_some_and(runtime_process_pid_alive);
        if !pid_alive
            && (modified < oldest_allowed || prodex_root_temp_file_pid(name).is_some())
            && remove_file_if_exists(&path)
        {
            removed += 1;
        }
    }

    removed
}

fn runtime_managed_profile_dir_looks_safe_to_audit(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    path.join("auth.json").exists()
        || path.join("config.toml").exists()
        || path.join("state.json").exists()
        || path.join(".codex").exists()
}

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

fn cleanup_duplicate_profiles(
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

pub(crate) fn collect_orphan_managed_profile_dirs_at(
    paths: &AppPaths,
    state: &AppState,
    now: SystemTime,
) -> Vec<String> {
    let oldest_allowed = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
        - ORPHAN_MANAGED_PROFILE_AUDIT_RETENTION_SECONDS;
    let Ok(entries) = fs::read_dir(&paths.managed_profiles_root) else {
        return Vec::new();
    };
    let mut names = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?.to_string();
            if state.profiles.contains_key(&name) {
                return None;
            }
            let metadata = fs::symlink_metadata(&path).ok()?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return None;
            }
            let modified = metadata
                .modified()
                .ok()
                .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs() as i64)
                .unwrap_or(i64::MIN);
            if modified >= oldest_allowed || !runtime_managed_profile_dir_looks_safe_to_audit(&path)
            {
                return None;
            }
            Some(name)
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}

pub(crate) fn collect_orphan_managed_profile_dirs(
    paths: &AppPaths,
    state: &AppState,
) -> Vec<String> {
    collect_orphan_managed_profile_dirs_at(paths, state, SystemTime::now())
}

pub(crate) fn cleanup_orphan_managed_profile_dirs_at(
    paths: &AppPaths,
    state: &AppState,
    now: SystemTime,
) -> usize {
    let mut removed = 0usize;
    for name in collect_orphan_managed_profile_dirs_at(paths, state, now) {
        if remove_dir_if_exists(&paths.managed_profiles_root.join(name)).is_ok() {
            removed += 1;
        }
    }
    removed
}

pub(crate) fn prodex_runtime_log_paths_in_dir(dir: &Path) -> Vec<PathBuf> {
    let mut paths = fs::read_dir(dir)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(|entry| entry.ok().map(|item| item.path())))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with(RUNTIME_PROXY_LOG_FILE_PREFIX) && name.ends_with(".log")
                })
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

pub(crate) fn cleanup_runtime_proxy_logs_in_dir(dir: &Path, now: SystemTime) -> usize {
    let now_epoch = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    let oldest_allowed = now_epoch.saturating_sub(RUNTIME_PROXY_LOG_RETENTION_SECONDS);
    let mut paths = prodex_runtime_log_paths_in_dir(dir)
        .into_iter()
        .map(|path| {
            let modified = path
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs() as i64)
                .unwrap_or(i64::MIN);
            (path, modified)
        })
        .collect::<Vec<_>>();
    paths.sort_by_key(|(path, modified)| (*modified, path.clone()));
    let excess = paths
        .len()
        .saturating_sub(RUNTIME_PROXY_LOG_RETENTION_COUNT);
    let mut removed = 0usize;
    for (index, (path, modified)) in paths.into_iter().enumerate() {
        if (modified < oldest_allowed || index < excess) && fs::remove_file(path).is_ok() {
            removed += 1;
        }
    }
    removed
}

pub(crate) fn newest_runtime_proxy_log_in_dir(dir: &Path) -> Option<PathBuf> {
    prodex_runtime_log_paths_in_dir(dir)
        .into_iter()
        .filter_map(|path| {
            let modified = path
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis());
            modified.map(|modified| (modified, path))
        })
        .max_by(|(left_modified, left_path), (right_modified, right_path)| {
            left_modified
                .cmp(right_modified)
                .then_with(|| left_path.cmp(right_path))
        })
        .map(|(_, path)| path)
}

pub(crate) fn cleanup_runtime_proxy_latest_pointer(pointer_path: &Path) -> bool {
    let should_remove_pointer = fs::read_to_string(pointer_path)
        .ok()
        .map(|content| PathBuf::from(content.trim()))
        .is_some_and(|path| !path.exists());
    if should_remove_pointer {
        return fs::remove_file(pointer_path).is_ok();
    }
    false
}

pub(crate) fn cleanup_runtime_proxy_log_housekeeping() {
    let temp_dir = runtime_proxy_log_dir();
    cleanup_runtime_proxy_logs_in_dir(&temp_dir, SystemTime::now());
    cleanup_runtime_proxy_latest_pointer(&runtime_proxy_latest_log_pointer_path());
}

pub(crate) fn cleanup_stale_login_dirs_at(paths: &AppPaths, now: SystemTime) -> usize {
    let Ok(entries) = fs::read_dir(&paths.root) else {
        return 0;
    };
    let oldest_allowed = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
        - PROD_EX_TMP_LOGIN_RETENTION_SECONDS;
    let mut removed = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with(".login-") {
            continue;
        }
        let modified = entry
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(i64::MIN);
        if modified < oldest_allowed && remove_dir_if_exists(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

pub(crate) fn cleanup_stale_login_dirs(paths: &AppPaths) -> usize {
    cleanup_stale_login_dirs_at(paths, SystemTime::now())
}

fn runtime_broker_artifact_keys(paths: &AppPaths) -> Vec<String> {
    let Ok(entries) = fs::read_dir(&paths.root) else {
        return Vec::new();
    };
    let mut keys = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if let Some(key) = name
            .strip_prefix("runtime-broker-")
            .and_then(|suffix| suffix.strip_suffix(".json"))
        {
            keys.push(key.to_string());
            continue;
        }
        if let Some(key) = name
            .strip_prefix("runtime-broker-")
            .and_then(|suffix| suffix.strip_suffix(".json.last-good"))
        {
            keys.push(key.to_string());
            continue;
        }
        if path.is_dir()
            && let Some(key) = name
                .strip_prefix("runtime-broker-")
                .and_then(|suffix| suffix.strip_suffix("-leases"))
        {
            keys.push(key.to_string());
        }
    }
    keys.sort();
    keys.dedup();
    keys
}

pub(crate) fn cleanup_runtime_broker_stale_registries(paths: &AppPaths) -> Result<usize> {
    let mut removed = 0usize;
    for broker_key in runtime_broker_artifact_keys(paths) {
        let Some(registry) = load_runtime_broker_registry(paths, &broker_key)? else {
            continue;
        };
        if runtime_process_pid_alive(registry.pid) {
            continue;
        }
        remove_runtime_broker_registry_if_token_matches(
            paths,
            &broker_key,
            &registry.instance_token,
        );
        removed += 1;
    }
    Ok(removed)
}

pub(crate) fn cleanup_runtime_broker_stale_leases_for_all(paths: &AppPaths) -> usize {
    let mut removed = 0usize;
    for broker_key in runtime_broker_artifact_keys(paths) {
        let lease_dir = runtime_broker_lease_dir(paths, &broker_key);
        let Ok(entries) = fs::read_dir(&lease_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let pid = file_name
                .split('-')
                .next()
                .and_then(|value| value.parse::<u32>().ok());
            if pid.is_some_and(runtime_process_pid_alive) {
                continue;
            }
            if fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
        let should_remove_dir = fs::read_dir(&lease_dir)
            .ok()
            .is_some_and(|mut remaining| remaining.next().is_none());
        if should_remove_dir {
            let _ = fs::remove_dir(&lease_dir);
        }
    }
    removed
}

pub(crate) fn perform_prodex_cleanup_at(
    paths: &AppPaths,
    state: &AppState,
    runtime_log_dir: &Path,
    runtime_log_pointer_path: &Path,
    now: SystemTime,
) -> Result<ProdexCleanupSummary> {
    Ok(ProdexCleanupSummary {
        duplicate_profiles_removed: 0,
        duplicate_managed_profile_homes_removed: 0,
        runtime_logs_removed: cleanup_runtime_proxy_logs_in_dir(runtime_log_dir, now),
        stale_runtime_log_pointer_removed: usize::from(cleanup_runtime_proxy_latest_pointer(
            runtime_log_pointer_path,
        )),
        stale_login_dirs_removed: cleanup_stale_login_dirs_at(paths, now),
        orphan_managed_profile_dirs_removed: cleanup_orphan_managed_profile_dirs_at(
            paths, state, now,
        ),
        transient_root_files_removed: cleanup_prodex_transient_root_files(paths),
        stale_root_temp_files_removed: cleanup_prodex_stale_root_temp_files_at(paths, now),
        dead_runtime_broker_leases_removed: cleanup_runtime_broker_stale_leases_for_all(paths),
        dead_runtime_broker_registries_removed: cleanup_runtime_broker_stale_registries(paths)?,
    })
}

pub(crate) fn perform_prodex_cleanup(
    paths: &AppPaths,
    state: &mut AppState,
) -> Result<ProdexCleanupSummary> {
    let duplicate_summary = cleanup_duplicate_profiles(paths, state)?;
    let artifact_summary = perform_prodex_cleanup_at(
        paths,
        state,
        &runtime_proxy_log_dir(),
        &runtime_proxy_latest_log_pointer_path(),
        SystemTime::now(),
    )?;
    Ok(duplicate_summary.merge(artifact_summary))
}
