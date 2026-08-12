use super::{
    AppState, RUNTIME_PROFILE_AUTH_FAILURE_401_SCORE, RUNTIME_PROFILE_AUTH_FAILURE_403_SCORE,
    RUNTIME_PROFILE_AUTH_FAILURE_DECAY_SECONDS, RUNTIME_PROFILE_USAGE_CACHE_FRESH_SECONDS,
    RuntimeProfileHealth, RuntimeProfileUsageAuthCacheEntry, RuntimeRotationProxyShared,
    RuntimeRotationState, read_usage_auth, runtime_profile_effective_score_from_map,
    runtime_proxy_log, runtime_route_kind_label, schedule_runtime_probe_refresh,
    schedule_runtime_state_save_from_runtime, usage_auth_needs_proactive_refresh,
};
use anyhow::{Context, Result};
use chrono::Local;
use prodex_quota::UsageAuth;
use prodex_runtime_state::{RuntimeRouteKind, RuntimeStateMutation};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static RUNTIME_PROFILE_USAGE_AUTH_CACHE_GENERATION: AtomicU64 = AtomicU64::new(1);

pub(crate) fn read_auth_json_text(codex_home: &Path) -> Result<Option<String>> {
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .read_text(&secret_store::auth_json_location(codex_home))
        .map_err(anyhow::Error::new)
}

pub(crate) fn load_runtime_profile_usage_auth_cache_entry(
    codex_home: &Path,
) -> Result<RuntimeProfileUsageAuthCacheEntry> {
    let generation = RUNTIME_PROFILE_USAGE_AUTH_CACHE_GENERATION.fetch_add(1, Ordering::Relaxed);
    let auth = read_usage_auth(codex_home)?;
    Ok(RuntimeProfileUsageAuthCacheEntry {
        auth,
        checked_at: Local::now().timestamp(),
        generation,
    })
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
    let mut auth_changed = previous_auth.is_some_and(|previous_auth| previous_auth != &auth);
    if let Ok(mut runtime) = shared.runtime.lock() {
        if let Some(current) = runtime.profile_usage_auth.get(profile_name) {
            if current.generation > entry.generation {
                return current.auth.clone();
            }
            auth_changed = current.auth != auth;
        }
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
    let now = Local::now().timestamp();
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
        let revalidation_due = now < entry.checked_at
            || now.saturating_sub(entry.checked_at) >= RUNTIME_PROFILE_USAGE_CACHE_FRESH_SECONDS
            || usage_auth_needs_proactive_refresh(&entry.auth, now);
        let auth = entry.auth;
        if revalidation_due {
            schedule_runtime_probe_refresh(shared, profile_name, &codex_home);
        }
        return Ok(auth);
    }

    reload_runtime_profile_usage_auth(shared, profile_name, &codex_home)
}

pub(crate) fn apply_runtime_profile_usage_auth_revalidation(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    result: Result<RuntimeProfileUsageAuthCacheEntry>,
) {
    match result {
        Ok(entry) => {
            let previous_auth = shared.runtime.lock().ok().and_then(|runtime| {
                runtime
                    .profile_usage_auth
                    .get(profile_name)
                    .map(|entry| entry.auth.clone())
            });
            update_runtime_profile_usage_auth_cache_entry(
                shared,
                profile_name,
                previous_auth.as_ref(),
                entry,
                "auth_background_refresh",
            );
        }
        Err(err) => {
            if let Ok(mut runtime) = shared.runtime.lock()
                && let Some(entry) = runtime.profile_usage_auth.get_mut(profile_name)
            {
                entry.checked_at = Local::now().timestamp();
            }
            runtime_proxy_log(
                shared,
                format!(
                    "profile_auth_background_refresh_failed profile={profile_name} error={}",
                    redaction::redaction_redact_secret_like_text(&format!("{err:#}"))
                        .replace('\n', " ")
                ),
            );
        }
    }
}

fn reload_runtime_profile_usage_auth(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    codex_home: &Path,
) -> Result<UsageAuth> {
    let entry = load_runtime_profile_usage_auth_cache_entry(codex_home)?;
    Ok(update_runtime_profile_usage_auth_cache_entry(
        shared,
        profile_name,
        None,
        entry,
        "auth_changed",
    ))
}

pub(crate) fn runtime_profile_auth_failure_key(profile_name: &str) -> String {
    format!("__auth_failure__:{profile_name}")
}

pub(crate) fn runtime_profile_auth_failure_active_from_map(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    profile_name: &str,
    now: i64,
) -> bool {
    if profile_health.is_empty() {
        return false;
    }
    runtime_profile_effective_score_from_map(
        profile_health,
        &runtime_profile_auth_failure_key(profile_name),
        now,
        RUNTIME_PROFILE_AUTH_FAILURE_DECAY_SECONDS,
    ) > 0
}

pub(crate) fn runtime_profile_auth_failure_active(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    now: i64,
) -> bool {
    runtime_profile_auth_failure_active_from_map(&runtime.profile_health, profile_name, now)
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
    if !prodex_runtime_store::clear_runtime_profile_score(
        &mut runtime.profile_health,
        &runtime_profile_auth_failure_key(profile_name),
        Local::now().timestamp(),
    ) {
        return;
    }
    runtime_proxy_log(
        shared,
        format!("profile_auth_backoff_cleared profile={profile_name} reason={reason}"),
    );
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        RuntimeStateMutation::ProfileAuthBackoffCleared(profile_name.to_string()),
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
    let codex_home = runtime
        .state
        .profiles
        .get(profile_name)
        .map(|profile| profile.codex_home.clone());
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
        RuntimeStateMutation::ProfileAuthBackoff(profile_name.to_string()),
    );
    drop(runtime);
    if let Some(codex_home) = codex_home {
        schedule_runtime_probe_refresh(shared, profile_name, &codex_home);
    }
}
