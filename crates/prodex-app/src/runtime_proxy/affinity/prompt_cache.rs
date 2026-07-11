//! Prompt-cache profile affinity state.

use super::*;

#[derive(Clone, Debug)]
pub(super) struct RuntimePromptCacheProfileBinding {
    profile_name: String,
    bound_at: i64,
    cached_input_tokens: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimePromptCacheProfileObservation {
    NoKey,
    NoCachedTokens,
    OwnerInserted,
    OwnerChanged,
    OwnerUnchanged,
    ExistingOwnerPreserved,
}

impl RuntimePromptCacheProfileObservation {
    pub(crate) fn log_label(self) -> &'static str {
        match self {
            Self::NoKey => "no_key",
            Self::NoCachedTokens => "no_cached_tokens",
            Self::OwnerInserted => "owner_inserted",
            Self::OwnerChanged => "owner_changed",
            Self::OwnerUnchanged => "owner_unchanged",
            Self::ExistingOwnerPreserved => "existing_owner_preserved",
        }
    }

    fn changed_owner(self) -> bool {
        matches!(self, Self::OwnerInserted | Self::OwnerChanged)
    }
}

static RUNTIME_PROMPT_CACHE_PROFILE_BINDINGS: OnceLock<
    Mutex<BTreeMap<String, RuntimePromptCacheProfileBinding>>,
> = OnceLock::new();

pub(super) fn runtime_prompt_cache_profile_bindings()
-> &'static Mutex<BTreeMap<String, RuntimePromptCacheProfileBinding>> {
    RUNTIME_PROMPT_CACHE_PROFILE_BINDINGS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn runtime_normalized_prompt_cache_key(prompt_cache_key: Option<&str>) -> Option<String> {
    prompt_cache_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn prune_runtime_prompt_cache_profile_bindings(
    bindings: &mut BTreeMap<String, RuntimePromptCacheProfileBinding>,
    now: i64,
) {
    let oldest_allowed = now.saturating_sub(RUNTIME_PROMPT_CACHE_AFFINITY_RETENTION_SECONDS);
    bindings.retain(|_, binding| binding.bound_at >= oldest_allowed);
    if bindings.len() <= RUNTIME_PROMPT_CACHE_AFFINITY_LIMIT {
        return;
    }

    let excess = bindings.len() - RUNTIME_PROMPT_CACHE_AFFINITY_LIMIT;
    let mut coldest = bindings
        .iter()
        .map(|(key, binding)| (key.clone(), binding.bound_at))
        .collect::<Vec<_>>();
    coldest.sort_by_key(|(_, bound_at)| *bound_at);
    for (key, _) in coldest.into_iter().take(excess) {
        bindings.remove(&key);
    }
}

pub(super) fn remember_runtime_prompt_cache_profile_at(
    profile_name: &str,
    prompt_cache_key: Option<&str>,
    now: i64,
) -> bool {
    let Some(prompt_cache_key) = runtime_normalized_prompt_cache_key(prompt_cache_key) else {
        return false;
    };
    let mut bindings = runtime_prompt_cache_profile_bindings()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    prune_runtime_prompt_cache_profile_bindings(&mut bindings, now);
    let changed = match bindings.get_mut(&prompt_cache_key) {
        Some(binding) if binding.profile_name == profile_name => {
            if binding.bound_at < now {
                binding.bound_at = now;
            }
            false
        }
        Some(binding) if binding.cached_input_tokens > 0 => {
            if binding.bound_at < now {
                binding.bound_at = now;
            }
            false
        }
        Some(binding) => {
            binding.profile_name = profile_name.to_string();
            binding.bound_at = now;
            true
        }
        None => {
            bindings.insert(
                prompt_cache_key,
                RuntimePromptCacheProfileBinding {
                    profile_name: profile_name.to_string(),
                    bound_at: now,
                    cached_input_tokens: 0,
                },
            );
            true
        }
    };
    prune_runtime_prompt_cache_profile_bindings(&mut bindings, now);
    changed
}

pub(super) fn observe_runtime_prompt_cache_profile_hit_at(
    profile_name: &str,
    prompt_cache_key: Option<&str>,
    cached_input_tokens: u64,
    now: i64,
) -> RuntimePromptCacheProfileObservation {
    if cached_input_tokens == 0 {
        return RuntimePromptCacheProfileObservation::NoCachedTokens;
    }
    let Some(prompt_cache_key) = runtime_normalized_prompt_cache_key(prompt_cache_key) else {
        return RuntimePromptCacheProfileObservation::NoKey;
    };
    let mut bindings = runtime_prompt_cache_profile_bindings()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    prune_runtime_prompt_cache_profile_bindings(&mut bindings, now);
    let observation = match bindings.get_mut(&prompt_cache_key) {
        Some(binding) if binding.profile_name == profile_name => {
            if binding.bound_at < now {
                binding.bound_at = now;
            }
            if binding.cached_input_tokens < cached_input_tokens {
                binding.cached_input_tokens = cached_input_tokens;
            }
            RuntimePromptCacheProfileObservation::OwnerUnchanged
        }
        Some(binding) if cached_input_tokens > binding.cached_input_tokens => {
            binding.profile_name = profile_name.to_string();
            binding.bound_at = now;
            binding.cached_input_tokens = cached_input_tokens;
            RuntimePromptCacheProfileObservation::OwnerChanged
        }
        Some(binding) => {
            if binding.bound_at < now {
                binding.bound_at = now;
            }
            RuntimePromptCacheProfileObservation::ExistingOwnerPreserved
        }
        None => {
            bindings.insert(
                prompt_cache_key,
                RuntimePromptCacheProfileBinding {
                    profile_name: profile_name.to_string(),
                    bound_at: now,
                    cached_input_tokens,
                },
            );
            RuntimePromptCacheProfileObservation::OwnerInserted
        }
    };
    prune_runtime_prompt_cache_profile_bindings(&mut bindings, now);
    observation
}

pub(crate) fn observe_runtime_prompt_cache_profile_hit(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    prompt_cache_key: Option<&str>,
    cached_input_tokens: u64,
    route_kind: RuntimeRouteKind,
) -> RuntimePromptCacheProfileObservation {
    let observation = observe_runtime_prompt_cache_profile_hit_at(
        profile_name,
        prompt_cache_key,
        cached_input_tokens,
        Local::now().timestamp(),
    );
    if observation.changed_owner() {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "binding_prompt_cache_hit",
                [
                    runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                    runtime_proxy_log_field("profile", profile_name),
                    runtime_proxy_log_field("cached_input_tokens", cached_input_tokens.to_string()),
                    runtime_proxy_log_field("owner_update", observation.log_label()),
                ],
            ),
        );
    }
    observation
}

pub(crate) fn remember_runtime_prompt_cache_profile(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    prompt_cache_key: Option<&str>,
    route_kind: RuntimeRouteKind,
) {
    if remember_runtime_prompt_cache_profile_at(
        profile_name,
        prompt_cache_key,
        Local::now().timestamp(),
    ) {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "binding_prompt_cache",
                [
                    runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                    runtime_proxy_log_field("profile", profile_name),
                ],
            ),
        );
    }
}

pub(super) fn runtime_prompt_cache_bound_profile_at(
    prompt_cache_key: Option<&str>,
    now: i64,
) -> Option<String> {
    let prompt_cache_key = runtime_normalized_prompt_cache_key(prompt_cache_key)?;
    let mut bindings = runtime_prompt_cache_profile_bindings()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    prune_runtime_prompt_cache_profile_bindings(&mut bindings, now);
    bindings
        .get(&prompt_cache_key)
        .map(|binding| binding.profile_name.clone())
}

pub(crate) fn runtime_prompt_cache_bound_profile(prompt_cache_key: Option<&str>) -> Option<String> {
    runtime_prompt_cache_bound_profile_at(prompt_cache_key, Local::now().timestamp())
}

#[cfg(test)]
pub(crate) fn clear_runtime_prompt_cache_profile_bindings() {
    runtime_prompt_cache_profile_bindings()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
}
