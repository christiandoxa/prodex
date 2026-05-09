use super::*;

pub(crate) use runtime_proxy_crate::{
    RuntimeExplicitSessionId, RuntimeWebsocketRequestMetadata,
    parse_runtime_websocket_request_metadata, runtime_request_explicit_session_id,
    runtime_request_previous_response_fresh_fallback_shape, runtime_request_previous_response_id,
    runtime_request_prompt_cache_key, runtime_request_requires_previous_response_affinity,
    runtime_request_session_id, runtime_request_turn_state,
    runtime_websocket_request_requires_locked_previous_response_affinity,
};
#[cfg(test)]
pub(crate) use runtime_proxy_crate::{
    runtime_request_previous_response_id_from_text,
    runtime_request_text_without_previous_response_id,
    runtime_request_without_previous_response_id,
};

#[derive(Clone, Debug)]
struct RuntimePromptCacheProfileBinding {
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

fn runtime_prompt_cache_profile_bindings()
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

fn remember_runtime_prompt_cache_profile_at(
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

fn observe_runtime_prompt_cache_profile_hit_at(
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

fn runtime_prompt_cache_bound_profile_at(
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

#[cfg(test)]
pub(crate) use runtime_proxy_crate::runtime_previous_response_fresh_fallback_shape_label;
pub(crate) use runtime_proxy_crate::{
    RuntimePreviousResponseFreshFallbackPolicy, RuntimePreviousResponseFreshFallbackPolicyInput,
    RuntimePreviousResponseFreshFallbackShape, runtime_previous_response_fresh_fallback_policy,
    runtime_previous_response_fresh_fallback_shape_with_session,
};
#[cfg(test)]
pub(crate) use runtime_proxy_crate::{
    RuntimePreviousResponseFreshFallbackPolicyShape,
    runtime_previous_response_fresh_fallback_shape_allows_recovery,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RuntimeResponseRouteAffinity {
    pub(crate) bound_session_profile: Option<String>,
    pub(crate) compact_followup_profile: Option<(String, &'static str)>,
    pub(crate) compact_session_profile: Option<String>,
    pub(crate) session_profile: Option<String>,
    pub(crate) pinned_profile: Option<String>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RuntimeResponseRouteAffinityRequest<'a> {
    pub(crate) previous_response_id: Option<&'a str>,
    pub(crate) bound_profile: Option<&'a str>,
    pub(crate) turn_state_profile: Option<&'a str>,
    pub(crate) request_turn_state: Option<&'a str>,
    pub(crate) request_session_id: Option<&'a str>,
    pub(crate) explicit_request_session_id: Option<&'a RuntimeExplicitSessionId>,
    pub(crate) websocket_session_profile: Option<&'a str>,
}

impl RuntimeResponseRouteAffinityRequest<'_> {
    fn allows_session_affinity(self) -> bool {
        self.previous_response_id.is_none()
            && self.bound_profile.is_none()
            && self.turn_state_profile.is_none()
    }

    fn presence(self) -> RuntimeResponseRouteAffinityPresence {
        RuntimeResponseRouteAffinityPresence {
            previous_response_id_present: self.previous_response_id.is_some(),
            request_turn_state_present: self.request_turn_state.is_some(),
            request_session_id_present: self.request_session_id.is_some(),
            explicit_request_session_id_present: self.explicit_request_session_id.is_some(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct RuntimeResponseRouteAffinityPresence {
    pub(crate) previous_response_id_present: bool,
    pub(crate) request_turn_state_present: bool,
    pub(crate) request_session_id_present: bool,
    pub(crate) explicit_request_session_id_present: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct RuntimeResponseRouteAffinityLogContext<'a> {
    pub(crate) shared: &'a RuntimeRotationProxyShared,
    pub(crate) request_id: u64,
    pub(crate) websocket_session_id: Option<u64>,
    pub(crate) reason: &'a str,
}

impl RuntimeResponseRouteAffinityLogContext<'_> {
    fn route_prefix(self) -> String {
        match self.websocket_session_id {
            Some(session_id) => {
                format!("request={} websocket_session={session_id}", self.request_id)
            }
            None => format!("request={} transport=http", self.request_id),
        }
    }
}

pub(crate) struct RuntimeResponseRouteAffinityRefreshSlots<'a> {
    pub(crate) bound_session_profile: &'a mut Option<String>,
    pub(crate) compact_followup_profile: &'a mut Option<(String, &'static str)>,
    pub(crate) compact_session_profile: &'a mut Option<String>,
    pub(crate) session_profile: &'a mut Option<String>,
    pub(crate) pinned_profile: &'a mut Option<String>,
}

impl RuntimeResponseRouteAffinityRefreshSlots<'_> {
    fn replace_with(&mut self, affinity: &RuntimeResponseRouteAffinity) {
        *self.bound_session_profile = affinity.bound_session_profile.clone();
        *self.compact_followup_profile = affinity.compact_followup_profile.clone();
        *self.compact_session_profile = affinity.compact_session_profile.clone();
        *self.session_profile = affinity.session_profile.clone();
        *self.pinned_profile = affinity.pinned_profile.clone();
    }
}

pub(crate) struct RuntimeResponseProfileAffinityClear<'a> {
    pub(crate) profile_name: &'a str,
    pub(crate) bound_profile: &'a mut Option<String>,
    pub(crate) session_profile: &'a mut Option<String>,
    pub(crate) candidate_turn_state_retry_profile: &'a mut Option<String>,
    pub(crate) candidate_turn_state_retry_value: &'a mut Option<String>,
    pub(crate) pinned_profile: &'a mut Option<String>,
    pub(crate) previous_response_retry_index: &'a mut usize,
    pub(crate) reset_previous_response_retry_index: bool,
    pub(crate) turn_state_profile: &'a mut Option<String>,
    pub(crate) compact_followup_profile: Option<&'a mut Option<(String, &'static str)>>,
}

pub(crate) fn clear_runtime_response_profile_affinity(
    clear: RuntimeResponseProfileAffinityClear<'_>,
) {
    if clear.bound_profile.as_deref() == Some(clear.profile_name) {
        *clear.bound_profile = None;
    }
    if clear.session_profile.as_deref() == Some(clear.profile_name) {
        *clear.session_profile = None;
    }
    if clear.candidate_turn_state_retry_profile.as_deref() == Some(clear.profile_name) {
        *clear.candidate_turn_state_retry_profile = None;
        *clear.candidate_turn_state_retry_value = None;
    }
    if clear.pinned_profile.as_deref() == Some(clear.profile_name) {
        *clear.pinned_profile = None;
        if clear.reset_previous_response_retry_index {
            *clear.previous_response_retry_index = 0;
        }
    }
    if clear.turn_state_profile.as_deref() == Some(clear.profile_name) {
        *clear.turn_state_profile = None;
    }
    if let Some(compact_followup_profile) = clear.compact_followup_profile
        && compact_followup_profile
            .as_ref()
            .is_some_and(|(owner, _)| owner == clear.profile_name)
    {
        *compact_followup_profile = None;
    }
}

pub(crate) fn derive_runtime_response_route_affinity_for_request(
    shared: &RuntimeRotationProxyShared,
    request: RuntimeResponseRouteAffinityRequest<'_>,
) -> Result<RuntimeResponseRouteAffinity> {
    let allows_session_affinity = request.allows_session_affinity();
    let bound_session_profile = if allows_session_affinity {
        request
            .request_session_id
            .map(|session_id| runtime_session_bound_profile(shared, session_id))
            .transpose()?
            .flatten()
    } else {
        None
    };
    let compact_followup_profile = if allows_session_affinity {
        runtime_compact_turn_state_followup_bound_profile(shared, request.request_turn_state)?
            .map(|profile_name| (profile_name, "turn_state"))
    } else {
        None
    };
    let compact_session_profile = if allows_session_affinity
        && compact_followup_profile.is_none()
        && bound_session_profile.is_none()
        && request.websocket_session_profile.is_none()
    {
        runtime_compact_session_followup_bound_profile(shared, request.explicit_request_session_id)?
    } else {
        None
    };
    let session_profile = if allows_session_affinity && compact_followup_profile.is_none() {
        request
            .websocket_session_profile
            .map(str::to_string)
            .or(bound_session_profile.clone())
            .or(compact_session_profile.clone())
    } else {
        None
    };
    let pinned_profile = request
        .bound_profile
        .map(str::to_string)
        .or(compact_followup_profile
            .as_ref()
            .map(|(profile_name, _)| profile_name.clone()));

    Ok(RuntimeResponseRouteAffinity {
        bound_session_profile,
        compact_followup_profile,
        compact_session_profile,
        session_profile,
        pinned_profile,
    })
}

#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn derive_runtime_response_route_affinity(
    shared: &RuntimeRotationProxyShared,
    previous_response_id: Option<&str>,
    bound_profile: Option<&str>,
    turn_state_profile: Option<&str>,
    request_turn_state: Option<&str>,
    request_session_id: Option<&str>,
    explicit_request_session_id: Option<&RuntimeExplicitSessionId>,
    websocket_session_profile: Option<&str>,
) -> Result<RuntimeResponseRouteAffinity> {
    derive_runtime_response_route_affinity_for_request(
        shared,
        RuntimeResponseRouteAffinityRequest {
            previous_response_id,
            bound_profile,
            turn_state_profile,
            request_turn_state,
            request_session_id,
            explicit_request_session_id,
            websocket_session_profile,
        },
    )
}

pub(crate) fn log_runtime_response_route_affinity_for_context(
    context: RuntimeResponseRouteAffinityLogContext<'_>,
    presence: RuntimeResponseRouteAffinityPresence,
    affinity: &RuntimeResponseRouteAffinity,
) {
    let prefix = context.route_prefix();
    runtime_proxy_log(
        context.shared,
        format!(
            "{prefix} route_affinity_recompute_result reason={} previous_response_id_present={} request_turn_state_present={} request_session_id_present={} explicit_session_id_present={} bound_session_profile={:?} compact_followup_profile={:?} compact_session_profile={:?} session_profile={:?} pinned_profile={:?}",
            context.reason,
            presence.previous_response_id_present,
            presence.request_turn_state_present,
            presence.request_session_id_present,
            presence.explicit_request_session_id_present,
            affinity.bound_session_profile,
            affinity.compact_followup_profile,
            affinity.compact_session_profile,
            affinity.session_profile,
            affinity.pinned_profile,
        ),
    );
    if let Some((profile_name, source)) = affinity.compact_followup_profile.as_ref() {
        runtime_proxy_log(
            context.shared,
            format!(
                "{} compact_followup_owner profile={profile_name} source={source}",
                context.route_prefix()
            ),
        );
    }
    if let Some(profile_name) = affinity.compact_session_profile.as_deref() {
        runtime_proxy_log(
            context.shared,
            format!(
                "{} compact_followup_owner profile={profile_name} source=session_id",
                context.route_prefix()
            ),
        );
    }
}

pub(crate) fn log_runtime_response_route_affinity_recompute_for_context(
    context: RuntimeResponseRouteAffinityLogContext<'_>,
    presence: RuntimeResponseRouteAffinityPresence,
) {
    runtime_proxy_log(
        context.shared,
        format!(
            "{} route_affinity_recompute reason={} previous_response_id_present={} request_turn_state_present={} request_session_id_present={} explicit_session_id_present={}",
            context.route_prefix(),
            context.reason,
            presence.previous_response_id_present,
            presence.request_turn_state_present,
            presence.request_session_id_present,
            presence.explicit_request_session_id_present,
        ),
    );
}

pub(crate) fn refresh_and_log_runtime_response_route_affinity_for_request(
    context: RuntimeResponseRouteAffinityLogContext<'_>,
    request: RuntimeResponseRouteAffinityRequest<'_>,
    mut slots: RuntimeResponseRouteAffinityRefreshSlots<'_>,
) -> Result<()> {
    let presence = request.presence();
    log_runtime_response_route_affinity_recompute_for_context(context, presence);
    let derived = derive_runtime_response_route_affinity_for_request(context.shared, request)?;
    slots.replace_with(&derived);
    log_runtime_response_route_affinity_for_context(context, presence, &derived);
    Ok(())
}

#[allow(dead_code)]
fn runtime_response_route_affinity_presence_from_flags(
    previous_response_id_present: bool,
    request_turn_state_present: bool,
    request_session_id_present: bool,
    explicit_request_session_id_present: bool,
) -> RuntimeResponseRouteAffinityPresence {
    RuntimeResponseRouteAffinityPresence {
        previous_response_id_present,
        request_turn_state_present,
        request_session_id_present,
        explicit_request_session_id_present,
    }
}

#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn log_runtime_response_route_affinity(
    shared: &RuntimeRotationProxyShared,
    request_id: u64,
    websocket_session_id: Option<u64>,
    reason: &str,
    previous_response_id_present: bool,
    request_turn_state_present: bool,
    request_session_id_present: bool,
    explicit_request_session_id_present: bool,
    affinity: &RuntimeResponseRouteAffinity,
) {
    log_runtime_response_route_affinity_for_context(
        RuntimeResponseRouteAffinityLogContext {
            shared,
            request_id,
            websocket_session_id,
            reason,
        },
        runtime_response_route_affinity_presence_from_flags(
            previous_response_id_present,
            request_turn_state_present,
            request_session_id_present,
            explicit_request_session_id_present,
        ),
        affinity,
    );
}

#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn log_runtime_response_route_affinity_recompute(
    shared: &RuntimeRotationProxyShared,
    request_id: u64,
    websocket_session_id: Option<u64>,
    reason: &str,
    previous_response_id_present: bool,
    request_turn_state_present: bool,
    request_session_id_present: bool,
    explicit_request_session_id_present: bool,
) {
    log_runtime_response_route_affinity_recompute_for_context(
        RuntimeResponseRouteAffinityLogContext {
            shared,
            request_id,
            websocket_session_id,
            reason,
        },
        runtime_response_route_affinity_presence_from_flags(
            previous_response_id_present,
            request_turn_state_present,
            request_session_id_present,
            explicit_request_session_id_present,
        ),
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn refresh_and_log_runtime_response_route_affinity(
    shared: &RuntimeRotationProxyShared,
    request_id: u64,
    websocket_session_id: Option<u64>,
    reason: &str,
    previous_response_id: Option<&str>,
    bound_profile: Option<&str>,
    turn_state_profile: Option<&str>,
    request_turn_state: Option<&str>,
    request_session_id: Option<&str>,
    explicit_request_session_id: Option<&RuntimeExplicitSessionId>,
    websocket_session_profile: Option<&str>,
    bound_session_profile: &mut Option<String>,
    compact_followup_profile: &mut Option<(String, &'static str)>,
    compact_session_profile: &mut Option<String>,
    session_profile: &mut Option<String>,
    pinned_profile: &mut Option<String>,
) -> Result<()> {
    refresh_and_log_runtime_response_route_affinity_for_request(
        RuntimeResponseRouteAffinityLogContext {
            shared,
            request_id,
            websocket_session_id,
            reason,
        },
        RuntimeResponseRouteAffinityRequest {
            previous_response_id,
            bound_profile,
            turn_state_profile,
            request_turn_state,
            request_session_id,
            explicit_request_session_id,
            websocket_session_profile,
        },
        RuntimeResponseRouteAffinityRefreshSlots {
            bound_session_profile,
            compact_followup_profile,
            compact_session_profile,
            session_profile,
            pinned_profile,
        },
    )
}

#[cfg(test)]
#[path = "../../tests/src/runtime_proxy/affinity.rs"]
mod tests;
