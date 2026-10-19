//! Prompt-cache profile affinity state.

use super::*;
use prodex_mojo_core::runtime_lineage::{
    PromptCacheBindingFacts, PromptCacheEvent, PromptCacheKey, PromptCacheOwnerUpdate,
    plan_prompt_cache_update, prepare_prompt_cache_key,
};

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

fn prune_runtime_prompt_cache_profile_bindings(
    bindings: &mut BTreeMap<String, RuntimePromptCacheProfileBinding>,
    now: i64,
) {
    bindings.retain(|_, binding| {
        prodex_mojo_core::state_policy::last_run_selection_keep(
            true,
            binding.bound_at,
            now,
            RUNTIME_PROMPT_CACHE_AFFINITY_RETENTION_SECONDS,
        )
        .expect("Mojo prompt-cache retention failed")
    });
    let ranks = bindings.values().map(|binding| [0, binding.bound_at]);
    let indices = prodex_mojo_core::state_policy::binding_prune_indices(
        ranks,
        RUNTIME_PROMPT_CACHE_AFFINITY_LIMIT,
    )
    .expect("Mojo prompt-cache pruning failed");
    if indices.is_empty() {
        return;
    }
    let keys = bindings.keys().collect::<Vec<_>>();
    let removed = indices
        .into_iter()
        .map(|index| keys[index].clone())
        .collect::<Vec<_>>();
    for key in removed {
        bindings.remove(&key);
    }
}

pub(super) fn remember_runtime_prompt_cache_profile_at(
    profile_name: &str,
    prompt_cache_key: Option<&str>,
    now: i64,
) -> bool {
    update_runtime_prompt_cache_profile_at(
        profile_name,
        prompt_cache_key,
        PromptCacheEvent::Remember,
        now,
    )
    .changed_owner()
}

pub(super) fn observe_runtime_prompt_cache_profile_hit_at(
    profile_name: &str,
    prompt_cache_key: Option<&str>,
    cached_input_tokens: u64,
    now: i64,
) -> RuntimePromptCacheProfileObservation {
    update_runtime_prompt_cache_profile_at(
        profile_name,
        prompt_cache_key,
        PromptCacheEvent::Hit(cached_input_tokens),
        now,
    )
}

fn update_runtime_prompt_cache_profile_at(
    profile_name: &str,
    prompt_cache_key: Option<&str>,
    event: PromptCacheEvent,
    now: i64,
) -> RuntimePromptCacheProfileObservation {
    let key = match prepare_prompt_cache_key(prompt_cache_key, event)
        .expect("Mojo prompt-cache key plan failed")
    {
        PromptCacheKey::NoKey => return RuntimePromptCacheProfileObservation::NoKey,
        PromptCacheKey::NoCachedTokens => {
            return RuntimePromptCacheProfileObservation::NoCachedTokens;
        }
        PromptCacheKey::Value(key) => key,
    };
    let mut bindings = runtime_prompt_cache_profile_bindings()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    prune_runtime_prompt_cache_profile_bindings(&mut bindings, now);
    let existing = bindings.get(key).map(|binding| PromptCacheBindingFacts {
        same_owner: binding.profile_name == profile_name,
        bound_at: binding.bound_at,
        cached_input_tokens: binding.cached_input_tokens,
    });
    let plan = plan_prompt_cache_update(event, existing, now)
        .expect("Mojo prompt-cache owner plan failed");
    match plan.owner {
        PromptCacheOwnerUpdate::Inserted | PromptCacheOwnerUpdate::Changed => {
            bindings.insert(
                key.to_string(),
                RuntimePromptCacheProfileBinding {
                    profile_name: profile_name.to_string(),
                    bound_at: plan.bound_at,
                    cached_input_tokens: plan.cached_input_tokens,
                },
            );
        }
        PromptCacheOwnerUpdate::Unchanged | PromptCacheOwnerUpdate::Preserved => {
            let binding = bindings
                .get_mut(key)
                .expect("validated existing prompt-cache owner");
            binding.bound_at = plan.bound_at;
            binding.cached_input_tokens = plan.cached_input_tokens;
        }
    }
    prune_runtime_prompt_cache_profile_bindings(&mut bindings, now);
    match plan.owner {
        PromptCacheOwnerUpdate::Inserted => RuntimePromptCacheProfileObservation::OwnerInserted,
        PromptCacheOwnerUpdate::Changed => RuntimePromptCacheProfileObservation::OwnerChanged,
        PromptCacheOwnerUpdate::Unchanged => RuntimePromptCacheProfileObservation::OwnerUnchanged,
        PromptCacheOwnerUpdate::Preserved => {
            RuntimePromptCacheProfileObservation::ExistingOwnerPreserved
        }
    }
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
    let PromptCacheKey::Value(prompt_cache_key) =
        prepare_prompt_cache_key(prompt_cache_key, PromptCacheEvent::Remember)
            .expect("Mojo prompt-cache key plan failed")
    else {
        return None;
    };
    let mut bindings = runtime_prompt_cache_profile_bindings()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    prune_runtime_prompt_cache_profile_bindings(&mut bindings, now);
    bindings
        .get(prompt_cache_key)
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

#[cfg(test)]
#[path = "../../../tests/src/runtime_proxy/prompt_cache.rs"]
mod tests;
