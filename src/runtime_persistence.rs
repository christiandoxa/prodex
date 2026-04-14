use super::*;

pub(super) fn merge_last_run_selection(
    existing: &BTreeMap<String, i64>,
    incoming: &BTreeMap<String, i64>,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> BTreeMap<String, i64> {
    let mut merged = existing.clone();
    for (profile_name, timestamp) in incoming {
        merged
            .entry(profile_name.clone())
            .and_modify(|current| *current = (*current).max(*timestamp))
            .or_insert(*timestamp);
    }
    merged.retain(|profile_name, _| profiles.contains_key(profile_name));
    merged
}

pub(super) fn prune_last_run_selection(
    selections: &mut BTreeMap<String, i64>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) {
    let oldest_allowed = now.saturating_sub(APP_STATE_LAST_RUN_RETENTION_SECONDS);
    selections.retain(|profile_name, timestamp| {
        profiles.contains_key(profile_name) && *timestamp >= oldest_allowed
    });
}

pub(super) fn merge_profile_bindings(
    existing: &BTreeMap<String, ResponseProfileBinding>,
    incoming: &BTreeMap<String, ResponseProfileBinding>,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> BTreeMap<String, ResponseProfileBinding> {
    let mut merged = existing.clone();
    for (response_id, binding) in incoming {
        let should_replace = merged
            .get(response_id)
            .is_none_or(|current| current.bound_at <= binding.bound_at);
        if should_replace {
            merged.insert(response_id.clone(), binding.clone());
        }
    }
    merged.retain(|_, binding| profiles.contains_key(&binding.profile_name));
    merged
}

pub(super) fn runtime_continuation_binding_lifecycle_rank(
    state: RuntimeContinuationBindingLifecycle,
) -> u8 {
    match state {
        RuntimeContinuationBindingLifecycle::Dead => 0,
        RuntimeContinuationBindingLifecycle::Suspect => 1,
        RuntimeContinuationBindingLifecycle::Warm => 2,
        RuntimeContinuationBindingLifecycle::Verified => 3,
    }
}

pub(super) fn runtime_continuation_status_evidence_sort_key(
    status: &RuntimeContinuationBindingStatus,
) -> (u8, u32, u32, u32, u8, i64, i64, i64) {
    (
        runtime_continuation_binding_lifecycle_rank(status.state),
        status.confidence.min(RUNTIME_CONTINUATION_CONFIDENCE_MAX),
        status.success_count,
        u32::MAX.saturating_sub(status.not_found_streak),
        if status.last_verified_route.is_some() {
            1
        } else {
            0
        },
        status.last_verified_at.unwrap_or(i64::MIN),
        status.last_touched_at.unwrap_or(i64::MIN),
        status.last_not_found_at.unwrap_or(i64::MIN),
    )
}

pub(super) fn runtime_continuation_status_is_more_evidenced(
    candidate: &RuntimeContinuationBindingStatus,
    current: &RuntimeContinuationBindingStatus,
) -> bool {
    runtime_continuation_status_evidence_sort_key(candidate)
        > runtime_continuation_status_evidence_sort_key(current)
}

pub(super) fn runtime_continuation_status_should_replace(
    candidate: &RuntimeContinuationBindingStatus,
    current: &RuntimeContinuationBindingStatus,
) -> bool {
    match (
        runtime_continuation_status_last_event_at(candidate),
        runtime_continuation_status_last_event_at(current),
    ) {
        (Some(candidate_at), Some(current_at)) if candidate_at != current_at => {
            return candidate_at > current_at;
        }
        (Some(_), None) => return true,
        (None, Some(_)) => return false,
        _ => {}
    }

    match (
        runtime_continuation_status_is_terminal(candidate),
        runtime_continuation_status_is_terminal(current),
    ) {
        (true, false) => return true,
        (false, true) => return false,
        _ => {}
    }

    runtime_continuation_status_is_more_evidenced(candidate, current)
}

pub(super) fn runtime_continuation_status_last_event_at(
    status: &RuntimeContinuationBindingStatus,
) -> Option<i64> {
    [
        status.last_not_found_at,
        status.last_verified_at,
        status.last_touched_at,
    ]
    .into_iter()
    .flatten()
    .max()
}

pub(super) fn runtime_continuation_status_is_terminal(
    status: &RuntimeContinuationBindingStatus,
) -> bool {
    status.state == RuntimeContinuationBindingLifecycle::Dead
        || status.not_found_streak >= RUNTIME_CONTINUATION_SUSPECT_NOT_FOUND_STREAK_LIMIT
        || (status.state == RuntimeContinuationBindingLifecycle::Suspect
            && status.confidence == 0
            && status.failure_count > 0)
}

pub(super) fn runtime_continuation_status_is_stale_verified(
    status: &RuntimeContinuationBindingStatus,
    now: i64,
) -> bool {
    status.state == RuntimeContinuationBindingLifecycle::Verified
        && runtime_continuation_status_last_event_at(status).is_some_and(|last| {
            now.saturating_sub(last) >= RUNTIME_CONTINUATION_VERIFIED_STALE_SECONDS
        })
}

pub(super) fn runtime_age_stale_verified_continuation_status(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
) -> bool {
    let Some(status) = runtime_continuation_status_map_mut(statuses, kind).get_mut(key) else {
        return false;
    };
    if !runtime_continuation_status_is_stale_verified(status, now) {
        return false;
    }
    status.state = RuntimeContinuationBindingLifecycle::Warm;
    true
}

pub(super) fn runtime_continuation_status_should_retain_with_binding(
    status: &RuntimeContinuationBindingStatus,
    now: i64,
) -> bool {
    match status.state {
        RuntimeContinuationBindingLifecycle::Dead => false,
        RuntimeContinuationBindingLifecycle::Verified
        | RuntimeContinuationBindingLifecycle::Warm => {
            status.confidence > 0
                || status.success_count > 0
                || status.last_verified_at.is_some()
                || status.last_touched_at.is_some()
        }
        RuntimeContinuationBindingLifecycle::Suspect => {
            status.not_found_streak < RUNTIME_CONTINUATION_SUSPECT_NOT_FOUND_STREAK_LIMIT
                && status.confidence > 0
                && status.last_not_found_at.is_some_and(|last| {
                    now.saturating_sub(last) < RUNTIME_CONTINUATION_SUSPECT_GRACE_SECONDS
                })
        }
    }
}

pub(super) fn runtime_continuation_status_should_retain_without_binding(
    status: &RuntimeContinuationBindingStatus,
    now: i64,
) -> bool {
    match status.state {
        RuntimeContinuationBindingLifecycle::Dead => status
            .last_not_found_at
            .or(status.last_touched_at)
            .is_some_and(|last| now.saturating_sub(last) < RUNTIME_CONTINUATION_DEAD_GRACE_SECONDS),
        _ => runtime_continuation_status_should_retain_with_binding(status, now),
    }
}

pub(super) fn runtime_continuation_status_dead_at(
    status: &RuntimeContinuationBindingStatus,
) -> Option<i64> {
    (status.state == RuntimeContinuationBindingLifecycle::Dead)
        .then(|| status.last_not_found_at.or(status.last_touched_at))
        .flatten()
}

pub(super) fn runtime_continuation_dead_status_shadowed_by_binding(
    binding: &ResponseProfileBinding,
    status: &RuntimeContinuationBindingStatus,
) -> bool {
    runtime_continuation_status_dead_at(status).is_some_and(|dead_at| binding.bound_at > dead_at)
}

pub(super) fn merge_runtime_continuation_status_map(
    existing: &BTreeMap<String, RuntimeContinuationBindingStatus>,
    incoming: &BTreeMap<String, RuntimeContinuationBindingStatus>,
    live_bindings: &BTreeMap<String, ResponseProfileBinding>,
) -> BTreeMap<String, RuntimeContinuationBindingStatus> {
    let now = Local::now().timestamp();
    let mut merged = existing.clone();
    for (key, status) in incoming {
        let should_replace = merged
            .get(key)
            .is_none_or(|current| runtime_continuation_status_should_replace(status, current));
        if should_replace {
            merged.insert(key.clone(), status.clone());
        }
    }
    merged.retain(|key, status| {
        live_bindings.contains_key(key)
            || runtime_continuation_status_should_retain_without_binding(status, now)
    });
    merged
}

pub(super) fn merge_runtime_continuation_statuses(
    existing: &RuntimeContinuationStatuses,
    incoming: &RuntimeContinuationStatuses,
    response_bindings: &BTreeMap<String, ResponseProfileBinding>,
    turn_state_bindings: &BTreeMap<String, ResponseProfileBinding>,
    session_id_bindings: &BTreeMap<String, ResponseProfileBinding>,
) -> RuntimeContinuationStatuses {
    RuntimeContinuationStatuses {
        response: merge_runtime_continuation_status_map(
            &existing.response,
            &incoming.response,
            response_bindings,
        ),
        turn_state: merge_runtime_continuation_status_map(
            &existing.turn_state,
            &incoming.turn_state,
            turn_state_bindings,
        ),
        session_id: merge_runtime_continuation_status_map(
            &existing.session_id,
            &incoming.session_id,
            session_id_bindings,
        ),
    }
}

pub(super) fn compact_runtime_continuation_statuses(
    statuses: RuntimeContinuationStatuses,
    continuations: &RuntimeContinuationStore,
) -> RuntimeContinuationStatuses {
    let now = Local::now().timestamp();
    let mut merged = merge_runtime_continuation_statuses(
        &RuntimeContinuationStatuses::default(),
        &statuses,
        &continuations.response_profile_bindings,
        &continuations.turn_state_bindings,
        &continuations.session_id_bindings,
    );
    merged.response.retain(|key, status| {
        if let Some(binding) = continuations.response_profile_bindings.get(key) {
            !runtime_continuation_dead_status_shadowed_by_binding(binding, status)
                && runtime_continuation_status_should_retain_with_binding(status, now)
        } else {
            runtime_continuation_status_should_retain_without_binding(status, now)
        }
    });
    merged.turn_state.retain(|key, status| {
        if let Some(binding) = continuations.turn_state_bindings.get(key) {
            !runtime_continuation_dead_status_shadowed_by_binding(binding, status)
                && runtime_continuation_status_should_retain_with_binding(status, now)
        } else {
            runtime_continuation_status_should_retain_without_binding(status, now)
        }
    });
    merged.session_id.retain(|key, status| {
        if let Some(binding) = continuations.session_id_bindings.get(key) {
            !runtime_continuation_dead_status_shadowed_by_binding(binding, status)
                && runtime_continuation_status_should_retain_with_binding(status, now)
        } else {
            runtime_continuation_status_should_retain_without_binding(status, now)
        }
    });
    merged
}

pub(super) fn runtime_continuation_binding_should_retain(
    binding: &ResponseProfileBinding,
    status: Option<&RuntimeContinuationBindingStatus>,
    now: i64,
) -> bool {
    match status {
        Some(status) if runtime_continuation_dead_status_shadowed_by_binding(binding, status) => {
            true
        }
        Some(status) if runtime_continuation_status_is_terminal(status) => false,
        Some(status) => runtime_continuation_status_should_retain_with_binding(status, now),
        None => binding.bound_at <= now,
    }
}

pub(super) fn runtime_continuation_binding_retention_sort_key(
    binding: &ResponseProfileBinding,
    status: Option<&RuntimeContinuationBindingStatus>,
) -> (u8, u32, u32, u32, u8, i64, i64, i64, i64) {
    let evidence = status
        .map(runtime_continuation_status_evidence_sort_key)
        .unwrap_or((0, 0, 0, 0, 0, i64::MIN, i64::MIN, i64::MIN));
    (
        evidence.0,
        evidence.1,
        evidence.2,
        evidence.3,
        evidence.4,
        evidence.5,
        evidence.6,
        evidence.7,
        binding.bound_at,
    )
}

pub(super) fn prune_runtime_continuation_response_bindings(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
    statuses: &BTreeMap<String, RuntimeContinuationBindingStatus>,
    max_entries: usize,
) {
    if bindings.len() <= max_entries {
        return;
    }

    let excess = bindings.len() - max_entries;
    let mut coldest = bindings
        .iter()
        .map(|(response_id, binding)| {
            (
                response_id.clone(),
                runtime_continuation_binding_retention_sort_key(binding, statuses.get(response_id)),
            )
        })
        .collect::<Vec<_>>();
    coldest.sort_by_key(|(_, retention)| *retention);

    for (response_id, _) in coldest.into_iter().take(excess) {
        bindings.remove(&response_id);
    }
}

pub(super) fn prune_profile_bindings_for_housekeeping(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
    retention_seconds: i64,
    max_entries: usize,
) {
    let oldest_allowed = now.saturating_sub(retention_seconds);
    bindings.retain(|_, binding| {
        profiles.contains_key(&binding.profile_name) && binding.bound_at >= oldest_allowed
    });
    prune_profile_bindings(bindings, max_entries);
}

pub(super) fn prune_profile_bindings_for_housekeeping_without_retention(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
    profiles: &BTreeMap<String, ProfileEntry>,
) {
    bindings.retain(|_, binding| profiles.contains_key(&binding.profile_name));
}

pub(super) fn compact_app_state(mut state: AppState, now: i64) -> AppState {
    state.active_profile = state
        .active_profile
        .filter(|profile_name| state.profiles.contains_key(profile_name));
    prune_last_run_selection(&mut state.last_run_selected_at, &state.profiles, now);
    prune_profile_bindings_for_housekeeping_without_retention(
        &mut state.response_profile_bindings,
        &state.profiles,
    );
    prune_profile_bindings_for_housekeeping(
        &mut state.session_profile_bindings,
        &state.profiles,
        now,
        APP_STATE_SESSION_BINDING_RETENTION_SECONDS,
        SESSION_ID_PROFILE_BINDING_LIMIT,
    );
    state
}

pub(super) fn merge_runtime_state_snapshot(existing: AppState, snapshot: &AppState) -> AppState {
    let profiles = if existing.profiles.is_empty() {
        snapshot.profiles.clone()
    } else {
        existing.profiles.clone()
    };
    let active_profile = snapshot
        .active_profile
        .clone()
        .or(existing.active_profile.clone())
        .filter(|profile_name| profiles.contains_key(profile_name));

    let merged = AppState {
        active_profile,
        profiles: profiles.clone(),
        last_run_selected_at: merge_last_run_selection(
            &existing.last_run_selected_at,
            &snapshot.last_run_selected_at,
            &profiles,
        ),
        response_profile_bindings: merge_profile_bindings(
            &existing.response_profile_bindings,
            &snapshot.response_profile_bindings,
            &profiles,
        ),
        session_profile_bindings: merge_profile_bindings(
            &existing.session_profile_bindings,
            &snapshot.session_profile_bindings,
            &profiles,
        ),
    };
    compact_app_state(merged, Local::now().timestamp())
}

pub(super) fn read_auth_json_text(codex_home: &Path) -> Result<Option<String>> {
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .read_text(&secret_store::auth_json_location(codex_home))
        .map_err(anyhow::Error::new)
}

pub(super) fn probe_auth_secret_revision(
    codex_home: &Path,
) -> Result<Option<secret_store::SecretRevision>> {
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .probe_revision(&secret_store::auth_json_location(codex_home))
        .map_err(anyhow::Error::new)
}

pub(super) fn load_runtime_profile_usage_auth_cache_entry(
    codex_home: &Path,
) -> Result<RuntimeProfileUsageAuthCacheEntry> {
    let location = secret_store::auth_json_location(codex_home);
    let revision = probe_auth_secret_revision(codex_home)?;
    let auth = read_usage_auth(codex_home)?;
    Ok(RuntimeProfileUsageAuthCacheEntry {
        auth,
        location,
        revision,
    })
}

pub(super) fn runtime_profile_usage_auth_cache_entry_matches(
    entry: &RuntimeProfileUsageAuthCacheEntry,
) -> Result<bool> {
    let revision = match &entry.location {
        secret_store::SecretLocation::File(path) => match std::fs::metadata(path) {
            Ok(metadata) => Some(secret_store::SecretRevision::from_metadata(&metadata)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => {
                return Err(
                    anyhow::Error::new(err).context(format!("failed to stat {}", path.display()))
                );
            }
        },
        secret_store::SecretLocation::Keyring { .. } => {
            secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
                .probe_revision(&entry.location)
                .map_err(anyhow::Error::new)?
        }
    };
    Ok(revision == entry.revision)
}

pub(super) fn load_runtime_profile_usage_auth_cache(
    state: &AppState,
) -> BTreeMap<String, RuntimeProfileUsageAuthCacheEntry> {
    state
        .profiles
        .iter()
        .filter_map(|(name, profile)| {
            load_runtime_profile_usage_auth_cache_entry(&profile.codex_home)
                .ok()
                .map(|entry| (name.clone(), entry))
        })
        .collect()
}

pub(super) fn runtime_profile_usage_auth(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<UsageAuth> {
    let (cached_entry, codex_home) = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        let profile = runtime
            .state
            .profiles
            .get(profile_name)
            .with_context(|| format!("profile '{}' is missing", profile_name))?;
        (
            runtime.profile_usage_auth.get(profile_name).cloned(),
            profile.codex_home.clone(),
        )
    };

    if let Some(entry) = cached_entry {
        match runtime_profile_usage_auth_cache_entry_matches(&entry) {
            Ok(true) => return Ok(entry.auth),
            Ok(false) => {}
            Err(_) => return Ok(entry.auth),
        }
    }

    let entry = load_runtime_profile_usage_auth_cache_entry(&codex_home)?;
    let auth = entry.auth.clone();
    if let Ok(mut runtime) = shared.runtime.lock() {
        runtime
            .profile_usage_auth
            .insert(profile_name.to_string(), entry);
    }
    Ok(auth)
}

pub(super) fn runtime_profile_auth_failure_key(profile_name: &str) -> String {
    format!("__auth_failure__:{profile_name}")
}

pub(super) fn runtime_profile_auth_failure_active_from_map(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    profile_name: &str,
    now: i64,
) -> bool {
    runtime_profile_effective_score_from_map(
        profile_health,
        &runtime_profile_auth_failure_key(profile_name),
        now,
        RUNTIME_PROFILE_AUTH_FAILURE_DECAY_SECONDS,
    ) > 0
}

pub(super) fn runtime_profile_auth_failure_active_with_auth_cache(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    profile_usage_auth: &BTreeMap<String, RuntimeProfileUsageAuthCacheEntry>,
    profile_name: &str,
    now: i64,
) -> bool {
    if !runtime_profile_auth_failure_active_from_map(profile_health, profile_name, now) {
        return false;
    }
    let Some(entry) = profile_usage_auth.get(profile_name) else {
        return true;
    };
    runtime_profile_usage_auth_cache_entry_matches(entry).unwrap_or(true)
}

pub(super) fn runtime_profile_auth_failure_active(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    now: i64,
) -> bool {
    runtime_profile_auth_failure_active_with_auth_cache(
        &runtime.profile_health,
        &runtime.profile_usage_auth,
        profile_name,
        now,
    )
}

pub(super) fn runtime_profile_auth_failure_score(status: u16) -> u32 {
    match status {
        401 => RUNTIME_PROFILE_AUTH_FAILURE_401_SCORE,
        _ => RUNTIME_PROFILE_AUTH_FAILURE_403_SCORE,
    }
}

pub(super) fn note_runtime_profile_auth_failure(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    status: u16,
) {
    let mut runtime = match shared.runtime.lock() {
        Ok(runtime) => runtime,
        Err(_) => return,
    };
    let now = Local::now().timestamp();
    let next_score = runtime_profile_effective_score_from_map(
        &runtime.profile_health,
        &runtime_profile_auth_failure_key(profile_name),
        now,
        RUNTIME_PROFILE_AUTH_FAILURE_DECAY_SECONDS,
    )
    .max(runtime_profile_auth_failure_score(status));
    runtime.profile_health.insert(
        runtime_profile_auth_failure_key(profile_name),
        RuntimeProfileHealth {
            score: next_score,
            updated_at: now,
        },
    );
    runtime_proxy_log(
        shared,
        format!(
            "profile_auth_backoff profile={profile_name} route={} status={} score={} seconds={}",
            runtime_route_kind_label(route_kind),
            status,
            next_score,
            RUNTIME_PROFILE_AUTH_FAILURE_DECAY_SECONDS
        ),
    );
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        &format!("profile_auth_backoff:{profile_name}"),
    );
}

pub(super) fn merge_app_state_for_save(existing: AppState, desired: &AppState) -> AppState {
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

pub(super) fn runtime_continuations_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-continuations.json")
}

pub(super) fn runtime_continuations_last_good_file_path(paths: &AppPaths) -> PathBuf {
    last_good_file_path(&runtime_continuations_file_path(paths))
}

pub(super) fn runtime_continuation_journal_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("runtime-continuation-journal.json")
}

pub(super) fn runtime_continuation_journal_last_good_file_path(paths: &AppPaths) -> PathBuf {
    last_good_file_path(&runtime_continuation_journal_file_path(paths))
}

pub(super) fn runtime_broker_registry_file_path(paths: &AppPaths, broker_key: &str) -> PathBuf {
    paths.root.join(format!("runtime-broker-{broker_key}.json"))
}

pub(super) fn runtime_broker_registry_last_good_file_path(
    paths: &AppPaths,
    broker_key: &str,
) -> PathBuf {
    last_good_file_path(&runtime_broker_registry_file_path(paths, broker_key))
}

pub(super) fn runtime_broker_lease_dir(paths: &AppPaths, broker_key: &str) -> PathBuf {
    paths
        .root
        .join(format!("runtime-broker-{broker_key}-leases"))
}

pub(super) fn runtime_broker_ensure_lock_path(paths: &AppPaths, broker_key: &str) -> PathBuf {
    paths
        .root
        .join(format!("runtime-broker-{broker_key}-ensure"))
}

pub(super) fn runtime_broker_registry_keys(paths: &AppPaths) -> Vec<String> {
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

pub(super) fn update_check_cache_file_path(paths: &AppPaths) -> PathBuf {
    paths.root.join("update-check.json")
}

pub(super) fn runtime_continuation_store_from_app_state(
    state: &AppState,
) -> RuntimeContinuationStore {
    RuntimeContinuationStore {
        response_profile_bindings: runtime_external_response_profile_bindings(
            &state.response_profile_bindings,
        ),
        session_profile_bindings: state.session_profile_bindings.clone(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: runtime_external_session_id_bindings(&state.session_profile_bindings),
        statuses: RuntimeContinuationStatuses::default(),
    }
}

pub(super) fn compact_runtime_continuation_store(
    mut continuations: RuntimeContinuationStore,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> RuntimeContinuationStore {
    let now = Local::now().timestamp();
    let response_statuses = continuations.statuses.response.clone();
    let turn_state_statuses = continuations.statuses.turn_state.clone();
    let session_id_statuses = continuations.statuses.session_id.clone();
    prune_profile_bindings_for_housekeeping_without_retention(
        &mut continuations.response_profile_bindings,
        profiles,
    );
    prune_profile_bindings_for_housekeeping_without_retention(
        &mut continuations.session_profile_bindings,
        profiles,
    );
    prune_profile_bindings_for_housekeeping_without_retention(
        &mut continuations.turn_state_bindings,
        profiles,
    );
    prune_profile_bindings_for_housekeeping_without_retention(
        &mut continuations.session_id_bindings,
        profiles,
    );
    continuations
        .response_profile_bindings
        .retain(|key, binding| {
            runtime_continuation_binding_should_retain(binding, response_statuses.get(key), now)
        });
    let response_turn_state_keys = continuations
        .response_profile_bindings
        .keys()
        .filter(|key| runtime_is_response_turn_state_lineage_key(key))
        .cloned()
        .collect::<Vec<_>>();
    for key in response_turn_state_keys {
        let Some((response_id, _)) = runtime_response_turn_state_lineage_parts(&key) else {
            continuations.response_profile_bindings.remove(&key);
            continue;
        };
        if continuations
            .response_profile_bindings
            .get(response_id)
            .is_none_or(|binding| !profiles.contains_key(&binding.profile_name))
        {
            continuations.response_profile_bindings.remove(&key);
        }
    }
    continuations.turn_state_bindings.retain(|key, binding| {
        runtime_continuation_binding_should_retain(binding, turn_state_statuses.get(key), now)
    });
    continuations
        .session_profile_bindings
        .retain(|key, binding| {
            runtime_continuation_binding_should_retain(binding, session_id_statuses.get(key), now)
        });
    continuations.session_id_bindings.retain(|key, binding| {
        runtime_continuation_binding_should_retain(binding, session_id_statuses.get(key), now)
    });
    prune_runtime_continuation_response_bindings(
        &mut continuations.response_profile_bindings,
        &response_statuses,
        RESPONSE_PROFILE_BINDING_LIMIT,
    );
    prune_profile_bindings(
        &mut continuations.turn_state_bindings,
        TURN_STATE_PROFILE_BINDING_LIMIT,
    );
    prune_profile_bindings(
        &mut continuations.session_profile_bindings,
        SESSION_ID_PROFILE_BINDING_LIMIT,
    );
    prune_profile_bindings(
        &mut continuations.session_id_bindings,
        SESSION_ID_PROFILE_BINDING_LIMIT,
    );
    let statuses = continuations.statuses.clone();
    continuations.statuses = compact_runtime_continuation_statuses(statuses, &continuations);
    continuations
}

pub(super) fn merge_runtime_continuation_store(
    existing: &RuntimeContinuationStore,
    incoming: &RuntimeContinuationStore,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> RuntimeContinuationStore {
    compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings: merge_profile_bindings(
                &existing.response_profile_bindings,
                &incoming.response_profile_bindings,
                profiles,
            ),
            session_profile_bindings: merge_profile_bindings(
                &existing.session_profile_bindings,
                &incoming.session_profile_bindings,
                profiles,
            ),
            turn_state_bindings: merge_profile_bindings(
                &existing.turn_state_bindings,
                &incoming.turn_state_bindings,
                profiles,
            ),
            session_id_bindings: merge_profile_bindings(
                &existing.session_id_bindings,
                &incoming.session_id_bindings,
                profiles,
            ),
            statuses: merge_runtime_continuation_statuses(
                &existing.statuses,
                &incoming.statuses,
                &merge_profile_bindings(
                    &existing.response_profile_bindings,
                    &incoming.response_profile_bindings,
                    profiles,
                ),
                &merge_profile_bindings(
                    &existing.turn_state_bindings,
                    &incoming.turn_state_bindings,
                    profiles,
                ),
                &merge_profile_bindings(
                    &existing.session_id_bindings,
                    &incoming.session_id_bindings,
                    profiles,
                ),
            ),
        },
        profiles,
    )
}

pub(super) fn load_runtime_continuations_with_recovery(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<RecoveredLoad<RuntimeContinuationStore>> {
    let path = runtime_continuations_file_path(paths);
    if !path.exists() && !runtime_continuations_last_good_file_path(paths).exists() {
        return Ok(RecoveredLoad {
            value: RuntimeContinuationStore::default(),
            recovered_from_backup: false,
        });
    }
    let loaded = read_versioned_json_file_with_backup::<RuntimeContinuationStore>(
        &path,
        &runtime_continuations_last_good_file_path(paths),
    )?;
    remember_runtime_sidecar_generation(&path, loaded.generation);
    Ok(RecoveredLoad {
        value: compact_runtime_continuation_store(loaded.value, profiles),
        recovered_from_backup: loaded.recovered_from_backup,
    })
}

pub(super) fn save_runtime_continuations_for_profiles(
    paths: &AppPaths,
    continuations: &RuntimeContinuationStore,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<()> {
    if runtime_take_fault_injection("PRODEX_RUNTIME_FAULT_CONTINUATIONS_SAVE_ERROR_ONCE") {
        bail!("injected runtime continuations save failure");
    }
    let path = runtime_continuations_file_path(paths);
    let compacted = compact_runtime_continuation_store(continuations.clone(), profiles);
    save_versioned_json_file_with_fence(
        &path,
        &runtime_continuations_last_good_file_path(paths),
        &compacted,
    )?;
    Ok(())
}

pub(super) fn load_runtime_continuation_journal_with_recovery(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> Result<RecoveredLoad<RuntimeContinuationJournal>> {
    let path = runtime_continuation_journal_file_path(paths);
    if !path.exists() && !runtime_continuation_journal_last_good_file_path(paths).exists() {
        return Ok(RecoveredLoad {
            value: RuntimeContinuationJournal::default(),
            recovered_from_backup: false,
        });
    }
    let loaded = read_versioned_json_file_with_backup::<RuntimeContinuationJournal>(
        &path,
        &runtime_continuation_journal_last_good_file_path(paths),
    )?;
    remember_runtime_sidecar_generation(&path, loaded.generation);
    Ok(RecoveredLoad {
        value: RuntimeContinuationJournal {
            saved_at: loaded.value.saved_at,
            continuations: compact_runtime_continuation_store(loaded.value.continuations, profiles),
        },
        recovered_from_backup: loaded.recovered_from_backup,
    })
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn save_runtime_continuation_journal(
    paths: &AppPaths,
    continuations: &RuntimeContinuationStore,
    saved_at: i64,
) -> Result<()> {
    let profiles = AppState::load(paths)
        .map(|state| state.profiles)
        .unwrap_or_default();
    save_runtime_continuation_journal_for_profiles(paths, continuations, &profiles, saved_at)
}

pub(super) fn save_runtime_continuation_journal_for_profiles(
    paths: &AppPaths,
    continuations: &RuntimeContinuationStore,
    profiles: &BTreeMap<String, ProfileEntry>,
    saved_at: i64,
) -> Result<()> {
    let incoming = compact_runtime_continuation_store(continuations.clone(), profiles);
    for attempt in 0..=RUNTIME_SIDECAR_STALE_SAVE_RETRY_LIMIT {
        let existing = load_runtime_continuation_journal_with_recovery(paths, profiles)?;
        let journal = RuntimeContinuationJournal {
            saved_at: saved_at.max(existing.value.saved_at),
            continuations: merge_runtime_continuation_store(
                &existing.value.continuations,
                &incoming,
                profiles,
            ),
        };
        match save_versioned_json_file_with_fence(
            &runtime_continuation_journal_file_path(paths),
            &runtime_continuation_journal_last_good_file_path(paths),
            &journal,
        ) {
            Ok(()) => return Ok(()),
            Err(err)
                if runtime_sidecar_generation_error_is_stale(&err)
                    && attempt < RUNTIME_SIDECAR_STALE_SAVE_RETRY_LIMIT =>
            {
                continue;
            }
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

pub(super) fn runtime_profile_route_key_parts<'a>(
    key: &'a str,
    prefix: &str,
) -> Option<(&'a str, &'a str)> {
    let rest = key.strip_prefix(prefix)?;
    let (route, profile_name) = rest.split_once(':')?;
    Some((route, profile_name))
}

pub(super) fn runtime_profile_transport_backoff_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_transport_backoff__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub(super) fn runtime_profile_transport_backoff_key_parts(key: &str) -> Option<(&str, &str)> {
    runtime_profile_route_key_parts(key, "__route_transport_backoff__:")
}

pub(super) fn runtime_profile_transport_backoff_profile_name(key: &str) -> &str {
    runtime_profile_transport_backoff_key_parts(key)
        .map(|(_, profile_name)| profile_name)
        .unwrap_or(key)
}

pub(super) fn runtime_profile_transport_backoff_key_valid(
    key: &str,
    valid_profiles: &BTreeSet<String>,
) -> bool {
    runtime_profile_transport_backoff_key_parts(key)
        .map(|(route, profile_name)| {
            runtime_route_kind_from_label(route).is_some() && valid_profiles.contains(profile_name)
        })
        .unwrap_or_else(|| valid_profiles.contains(key))
}

pub(super) fn runtime_profile_transport_backoff_key_matches_profiles(
    key: &str,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> bool {
    runtime_profile_transport_backoff_key_parts(key)
        .map(|(route, profile_name)| {
            runtime_route_kind_from_label(route).is_some() && profiles.contains_key(profile_name)
        })
        .unwrap_or_else(|| profiles.contains_key(key))
}

pub(super) fn runtime_profile_transport_backoff_until_from_map(
    transport_backoff_until: &BTreeMap<String, i64>,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> Option<i64> {
    let route_key = runtime_profile_transport_backoff_key(profile_name, route_kind);
    [
        transport_backoff_until.get(&route_key).copied(),
        transport_backoff_until.get(profile_name).copied(),
    ]
    .into_iter()
    .flatten()
    .filter(|until| *until > now)
    .max()
}

pub(super) fn runtime_profile_transport_backoff_max_until(
    transport_backoff_until: &BTreeMap<String, i64>,
    profile_name: &str,
    now: i64,
) -> Option<i64> {
    transport_backoff_until
        .iter()
        .filter(|(key, until)| {
            runtime_profile_transport_backoff_profile_name(key) == profile_name && **until > now
        })
        .map(|(_, until)| *until)
        .max()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn save_runtime_state_snapshot_if_latest(
    paths: &AppPaths,
    snapshot: &AppState,
    continuations: &RuntimeContinuationStore,
    profile_scores: &BTreeMap<String, RuntimeProfileHealth>,
    usage_snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    backoffs: &RuntimeProfileBackoffs,
    revision: u64,
    latest_revision: &AtomicU64,
) -> Result<bool> {
    for attempt in 0..=RUNTIME_SIDECAR_STALE_SAVE_RETRY_LIMIT {
        if latest_revision.load(Ordering::SeqCst) != revision {
            return Ok(false);
        }

        let _lock = acquire_state_file_lock(paths)?;

        if latest_revision.load(Ordering::SeqCst) != revision {
            return Ok(false);
        }

        let existing = AppState::load(paths)?;
        let merged = merge_runtime_state_snapshot(existing, snapshot);
        let existing_continuations =
            load_runtime_continuations_with_recovery(paths, &merged.profiles)?;
        let merged_continuations = merge_runtime_continuation_store(
            &existing_continuations.value,
            continuations,
            &merged.profiles,
        );
        let mut merged = merged;
        merged.response_profile_bindings = runtime_external_response_profile_bindings(
            &merged_continuations.response_profile_bindings,
        );
        merged.session_profile_bindings = merged_continuations.session_profile_bindings.clone();
        let json =
            serde_json::to_string_pretty(&merged).context("failed to serialize prodex state")?;
        let existing_scores = load_runtime_profile_scores(paths, &merged.profiles)?;
        let merged_scores =
            merge_runtime_profile_scores(&existing_scores, profile_scores, &merged.profiles);
        let existing_usage_snapshots = load_runtime_usage_snapshots(paths, &merged.profiles)?;
        let merged_usage_snapshots = merge_runtime_usage_snapshots(
            &existing_usage_snapshots,
            usage_snapshots,
            &merged.profiles,
        );
        let existing_backoffs = load_runtime_profile_backoffs(paths, &merged.profiles)?;
        let merged_backoffs = merge_runtime_profile_backoffs(
            &existing_backoffs,
            backoffs,
            &merged.profiles,
            Local::now().timestamp(),
        );

        if latest_revision.load(Ordering::SeqCst) != revision {
            return Ok(false);
        }

        let save_result = (|| -> Result<()> {
            // Continuations are restored as the stronger source of truth on startup,
            // so persist them before the state snapshot to reduce crash windows where
            // a newer state file could be overwritten by an older continuation sidecar.
            save_runtime_continuations_for_profiles(
                paths,
                &merged_continuations,
                &merged.profiles,
            )?;
            write_state_json_atomic(paths, &json)?;
            save_runtime_profile_scores_for_profiles(paths, &merged_scores, &merged.profiles)?;
            save_runtime_usage_snapshots_for_profiles(
                paths,
                &merged_usage_snapshots,
                &merged.profiles,
            )?;
            save_runtime_profile_backoffs_for_profiles(paths, &merged_backoffs, &merged.profiles)?;
            Ok(())
        })();
        match save_result {
            Ok(()) => return Ok(true),
            Err(err)
                if runtime_sidecar_generation_error_is_stale(&err)
                    && attempt < RUNTIME_SIDECAR_STALE_SAVE_RETRY_LIMIT =>
            {
                continue;
            }
            Err(err) => return Err(err),
        }
    }
    Ok(false)
}
