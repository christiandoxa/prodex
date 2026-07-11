use super::*;

mod prompt_cache;

#[cfg(test)]
pub(crate) use prompt_cache::RuntimePromptCacheProfileObservation;
#[cfg(test)]
pub(crate) use prompt_cache::clear_runtime_prompt_cache_profile_bindings;
pub(crate) use prompt_cache::{
    observe_runtime_prompt_cache_profile_hit, remember_runtime_prompt_cache_profile,
    runtime_prompt_cache_bound_profile,
};
#[cfg(test)]
use prompt_cache::{
    observe_runtime_prompt_cache_profile_hit_at, remember_runtime_prompt_cache_profile_at,
    runtime_prompt_cache_bound_profile_at, runtime_prompt_cache_profile_bindings,
};

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

#[cfg(test)]
pub(crate) use runtime_proxy_crate::runtime_previous_response_fresh_fallback_shape_label;
#[cfg(test)]
pub(crate) use runtime_proxy_crate::{
    RuntimePreviousResponseFreshFallbackPolicy, RuntimePreviousResponseFreshFallbackPolicyInput,
    runtime_previous_response_fresh_fallback_policy,
};
#[cfg(test)]
pub(crate) use runtime_proxy_crate::{
    RuntimePreviousResponseFreshFallbackPolicyShape,
    runtime_previous_response_fresh_fallback_shape_allows_recovery,
};
pub(crate) use runtime_proxy_crate::{
    RuntimePreviousResponseFreshFallbackShape,
    runtime_previous_response_fresh_fallback_shape_with_session,
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

impl<'a> RuntimeResponseRouteAffinityLogContext<'a> {
    fn to_proxy(self) -> runtime_proxy_crate::RuntimeResponseRouteAffinityLogContext<'a> {
        runtime_proxy_crate::RuntimeResponseRouteAffinityLogContext {
            request_id: self.request_id,
            websocket_session_id: self.websocket_session_id,
            reason: self.reason,
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

pub(crate) fn log_runtime_response_route_affinity_for_context(
    context: RuntimeResponseRouteAffinityLogContext<'_>,
    presence: RuntimeResponseRouteAffinityPresence,
    affinity: &RuntimeResponseRouteAffinity,
) {
    runtime_proxy_log(
        context.shared,
        runtime_proxy_crate::runtime_response_route_affinity_recompute_result_log_message(
            context.to_proxy(),
            runtime_response_route_affinity_presence_to_proxy(presence),
            runtime_response_route_affinity_log_state(affinity),
        ),
    );
    for message in
        runtime_proxy_crate::runtime_response_route_affinity_compact_followup_owner_log_messages(
            context.to_proxy(),
            runtime_response_route_affinity_log_state(affinity),
        )
    {
        runtime_proxy_log(context.shared, message);
    }
}

pub(crate) fn log_runtime_response_route_affinity_recompute_for_context(
    context: RuntimeResponseRouteAffinityLogContext<'_>,
    presence: RuntimeResponseRouteAffinityPresence,
) {
    runtime_proxy_log(
        context.shared,
        runtime_proxy_crate::runtime_response_route_affinity_recompute_log_message(
            context.to_proxy(),
            runtime_response_route_affinity_presence_to_proxy(presence),
        ),
    );
}

fn runtime_response_route_affinity_presence_to_proxy(
    presence: RuntimeResponseRouteAffinityPresence,
) -> runtime_proxy_crate::RuntimeResponseRouteAffinityPresence {
    runtime_proxy_crate::RuntimeResponseRouteAffinityPresence {
        previous_response_id_present: presence.previous_response_id_present,
        request_turn_state_present: presence.request_turn_state_present,
        request_session_id_present: presence.request_session_id_present,
        explicit_request_session_id_present: presence.explicit_request_session_id_present,
    }
}

fn runtime_response_route_affinity_log_state(
    affinity: &RuntimeResponseRouteAffinity,
) -> runtime_proxy_crate::RuntimeResponseRouteAffinityLogState<'_> {
    runtime_proxy_crate::RuntimeResponseRouteAffinityLogState {
        bound_session_profile: affinity.bound_session_profile.as_deref(),
        compact_followup_profile: affinity
            .compact_followup_profile
            .as_ref()
            .map(|(profile_name, source)| (profile_name.as_str(), *source)),
        compact_session_profile: affinity.compact_session_profile.as_deref(),
        session_profile: affinity.session_profile.as_deref(),
        pinned_profile: affinity.pinned_profile.as_deref(),
    }
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

#[cfg(test)]
#[path = "../../tests/src/runtime_proxy/affinity.rs"]
mod tests;
