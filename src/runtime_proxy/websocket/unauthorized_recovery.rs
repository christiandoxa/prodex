use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::runtime_proxy) enum RuntimeProfileUnauthorizedRecoveryStep {
    Reload,
    Refresh,
}

pub(in crate::runtime_proxy) type RuntimeProfileUnauthorizedRecoverySteps =
    std::array::IntoIter<RuntimeProfileUnauthorizedRecoveryStep, 2>;

struct RuntimeProfileUnauthorizedRecoveryOutcome {
    source: &'static str,
    changed: bool,
}

impl RuntimeProfileUnauthorizedRecoveryStep {
    pub(in crate::runtime_proxy) fn ordered() -> RuntimeProfileUnauthorizedRecoverySteps {
        [Self::Reload, Self::Refresh].into_iter()
    }

    fn recover(
        self,
        shared: &RuntimeRotationProxyShared,
        profile_name: &str,
    ) -> Result<Option<RuntimeProfileUnauthorizedRecoveryOutcome>> {
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

        match self {
            RuntimeProfileUnauthorizedRecoveryStep::Reload => {
                let entry = load_runtime_profile_usage_auth_cache_entry(&codex_home)?;
                let auth_changed = cached_entry
                    .as_ref()
                    .is_some_and(|cached_entry| cached_entry.auth != entry.auth);
                if !auth_changed {
                    return Ok(None);
                }
                update_runtime_profile_usage_auth_cache_entry(
                    shared,
                    profile_name,
                    cached_entry.as_ref().map(|entry| &entry.auth),
                    entry,
                    "auth_reloaded",
                );
                Ok(Some(RuntimeProfileUnauthorizedRecoveryOutcome {
                    source: "reloaded",
                    changed: true,
                }))
            }
            RuntimeProfileUnauthorizedRecoveryStep::Refresh => {
                let outcome = sync_usage_auth_from_disk_or_refresh(
                    &codex_home,
                    cached_entry.as_ref().map(|entry| &entry.auth),
                )?;
                let entry = load_runtime_profile_usage_auth_cache_entry(&codex_home)?;
                update_runtime_profile_usage_auth_cache_entry(
                    shared,
                    profile_name,
                    cached_entry.as_ref().map(|entry| &entry.auth),
                    entry,
                    &format!("auth_{}", usage_auth_sync_source_label(outcome.source)),
                );
                Ok(Some(RuntimeProfileUnauthorizedRecoveryOutcome {
                    source: usage_auth_sync_source_label(outcome.source),
                    changed: outcome.auth_changed,
                }))
            }
        }
    }
}

fn runtime_try_recover_profile_auth_from_unauthorized(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    step: RuntimeProfileUnauthorizedRecoveryStep,
) -> bool {
    let attempt = (|| -> Result<bool> {
        let Some(outcome) = step.recover(shared, profile_name)? else {
            return Ok(false);
        };
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} profile_auth_recovered profile={profile_name} route={} source={} changed={}",
                runtime_route_kind_label(route_kind),
                outcome.source,
                outcome.changed,
            ),
        );
        Ok(true)
    })();

    match attempt {
        Ok(recovered) => recovered,
        Err(err) => {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} profile_auth_recovery_failed profile={profile_name} route={} error={err}",
                    runtime_route_kind_label(route_kind),
                ),
            );
            false
        }
    }
}

pub(in crate::runtime_proxy) fn runtime_try_recover_profile_auth_from_unauthorized_steps(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    recovery_steps: &mut RuntimeProfileUnauthorizedRecoverySteps,
) -> bool {
    for step in recovery_steps.by_ref() {
        if runtime_try_recover_profile_auth_from_unauthorized(
            request_id,
            shared,
            profile_name,
            route_kind,
            step,
        ) {
            return true;
        }
    }
    false
}
