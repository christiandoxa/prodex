use super::*;

pub(crate) fn read_auth_json_text(codex_home: &Path) -> Result<Option<String>> {
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .read_text(&secret_store::auth_json_location(codex_home))
        .map_err(anyhow::Error::new)
}

pub(crate) fn probe_auth_secret_revision(
    codex_home: &Path,
) -> Result<Option<secret_store::SecretRevision>> {
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .probe_revision(&secret_store::auth_json_location(codex_home))
        .map_err(anyhow::Error::new)
}

pub(crate) fn load_runtime_profile_usage_auth_cache_entry(
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeProfileUsageAuthCacheFreshness {
    Fresh,
    Stale,
    Unknown,
}

impl RuntimeProfileUsageAuthCacheFreshness {
    fn resolve_cached_entry(
        self,
        shared: &RuntimeRotationProxyShared,
        profile_name: &str,
        codex_home: &Path,
        previous_auth: Option<&UsageAuth>,
        entry: RuntimeProfileUsageAuthCacheEntry,
    ) -> Result<UsageAuth> {
        match self {
            Self::Fresh => runtime_profile_usage_auth_from_fresh_cache_entry(
                shared,
                profile_name,
                codex_home,
                entry,
            ),
            Self::Stale => {
                reload_runtime_profile_usage_auth(shared, profile_name, codex_home, previous_auth)
            }
            Self::Unknown => {
                reload_runtime_profile_usage_auth(shared, profile_name, codex_home, previous_auth)
                    .or(Ok(entry.auth))
            }
        }
    }
}

#[derive(Debug, Clone)]
struct RuntimeProfileUsageAuthLookup {
    cached_entry: Option<RuntimeProfileUsageAuthCacheEntry>,
    cached_previous_auth: Option<UsageAuth>,
    codex_home: PathBuf,
}

pub(crate) fn runtime_profile_usage_auth_cache_entry_freshness(
    entry: &RuntimeProfileUsageAuthCacheEntry,
) -> RuntimeProfileUsageAuthCacheFreshness {
    let revision = match &entry.location {
        secret_store::SecretLocation::File(path) => match std::fs::metadata(path) {
            Ok(metadata) => Some(secret_store::SecretRevision::from_metadata(&metadata)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => return RuntimeProfileUsageAuthCacheFreshness::Unknown,
        },
        secret_store::SecretLocation::Keyring { .. } => {
            return RuntimeProfileUsageAuthCacheFreshness::Unknown;
        }
    };
    if revision == entry.revision {
        RuntimeProfileUsageAuthCacheFreshness::Fresh
    } else {
        RuntimeProfileUsageAuthCacheFreshness::Stale
    }
}

pub(crate) fn load_runtime_profile_usage_auth_cache(
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

pub(crate) fn update_runtime_profile_usage_auth_cache_entry(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    previous_auth: Option<&UsageAuth>,
    entry: RuntimeProfileUsageAuthCacheEntry,
    reason: &str,
) -> UsageAuth {
    let auth = entry.auth.clone();
    let auth_changed = previous_auth.is_some_and(|previous_auth| previous_auth != &auth);
    if let Ok(mut runtime) = shared.runtime.lock() {
        runtime
            .profile_usage_auth
            .insert(profile_name.to_string(), entry);
    }
    if auth_changed {
        clear_runtime_profile_auth_failure(shared, profile_name, reason);
    }
    auth
}

pub(crate) fn runtime_profile_usage_auth(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<UsageAuth> {
    let RuntimeProfileUsageAuthLookup {
        cached_entry,
        cached_previous_auth,
        codex_home,
    } = load_runtime_profile_usage_auth_lookup(shared, profile_name)?;

    if let Some(entry) = cached_entry {
        return runtime_profile_usage_auth_from_cached_entry(
            shared,
            profile_name,
            &codex_home,
            cached_previous_auth.as_ref(),
            entry,
        );
    }

    reload_runtime_profile_usage_auth(
        shared,
        profile_name,
        &codex_home,
        cached_previous_auth.as_ref(),
    )
}

fn load_runtime_profile_usage_auth_lookup(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<RuntimeProfileUsageAuthLookup> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let profile = runtime
        .state
        .profiles
        .get(profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    let cached_entry = runtime.profile_usage_auth.get(profile_name).cloned();
    let cached_previous_auth = cached_entry.as_ref().map(|entry| entry.auth.clone());

    Ok(RuntimeProfileUsageAuthLookup {
        cached_entry,
        cached_previous_auth,
        codex_home: profile.codex_home.clone(),
    })
}

fn runtime_profile_usage_auth_from_cached_entry(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    codex_home: &Path,
    previous_auth: Option<&UsageAuth>,
    entry: RuntimeProfileUsageAuthCacheEntry,
) -> Result<UsageAuth> {
    runtime_profile_usage_auth_cache_entry_freshness(&entry).resolve_cached_entry(
        shared,
        profile_name,
        codex_home,
        previous_auth,
        entry,
    )
}

fn runtime_profile_usage_auth_from_fresh_cache_entry(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    codex_home: &Path,
    entry: RuntimeProfileUsageAuthCacheEntry,
) -> Result<UsageAuth> {
    let now = Local::now().timestamp();
    if !usage_auth_needs_proactive_refresh(&entry.auth, now) {
        return Ok(entry.auth);
    }

    match sync_usage_auth_from_disk_or_refresh(codex_home, Some(&entry.auth)) {
        Ok(outcome) => {
            log_runtime_profile_proactive_sync(shared, profile_name, &outcome);
            reload_runtime_profile_usage_auth_after_sync(
                shared,
                profile_name,
                codex_home,
                &entry.auth,
                outcome.source,
            )
        }
        Err(err) => {
            runtime_proxy_log(
                shared,
                format!("profile_auth_proactive_sync_failed profile={profile_name} error={err}"),
            );
            Ok(entry.auth)
        }
    }
}

fn reload_runtime_profile_usage_auth(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    codex_home: &Path,
    previous_auth: Option<&UsageAuth>,
) -> Result<UsageAuth> {
    let entry = load_runtime_profile_usage_auth_cache_entry(codex_home)?;
    Ok(update_runtime_profile_usage_auth_cache_entry(
        shared,
        profile_name,
        previous_auth,
        entry,
        "auth_changed",
    ))
}

fn reload_runtime_profile_usage_auth_after_sync(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    codex_home: &Path,
    previous_auth: &UsageAuth,
    source: UsageAuthSyncSource,
) -> Result<UsageAuth> {
    let refreshed_entry = load_runtime_profile_usage_auth_cache_entry(codex_home)?;
    Ok(update_runtime_profile_usage_auth_cache_entry(
        shared,
        profile_name,
        Some(previous_auth),
        refreshed_entry,
        &format!("auth_{}", usage_auth_sync_source_label(source)),
    ))
}

fn log_runtime_profile_proactive_sync(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    outcome: &UsageAuthSyncOutcome,
) {
    runtime_proxy_log(
        shared,
        format!(
            "profile_auth_proactive_sync profile={profile_name} source={} changed={}",
            usage_auth_sync_source_label(outcome.source),
            outcome.auth_changed,
        ),
    );
}

pub(crate) fn runtime_profile_auth_failure_key(profile_name: &str) -> String {
    format!("__auth_failure__:{profile_name}")
}

pub(crate) fn runtime_profile_auth_failure_active_from_map(
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

pub(crate) fn runtime_profile_auth_failure_active_with_auth_cache(
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
    matches!(
        runtime_profile_usage_auth_cache_entry_freshness(entry),
        RuntimeProfileUsageAuthCacheFreshness::Fresh
    )
}

pub(crate) fn runtime_profile_auth_failure_active(
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

pub(crate) fn runtime_profile_auth_failure_score(status: u16) -> u32 {
    match status {
        401 => RUNTIME_PROFILE_AUTH_FAILURE_401_SCORE,
        _ => RUNTIME_PROFILE_AUTH_FAILURE_403_SCORE,
    }
}

pub(crate) fn clear_runtime_profile_auth_failure(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    reason: &str,
) {
    let mut runtime = match shared.runtime.lock() {
        Ok(runtime) => runtime,
        Err(_) => return,
    };
    if runtime
        .profile_health
        .remove(&runtime_profile_auth_failure_key(profile_name))
        .is_none()
    {
        return;
    }
    runtime_proxy_log(
        shared,
        format!("profile_auth_backoff_cleared profile={profile_name} reason={reason}"),
    );
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        &format!("profile_auth_backoff_cleared:{profile_name}"),
    );
}

pub(crate) fn note_runtime_profile_auth_failure(
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
