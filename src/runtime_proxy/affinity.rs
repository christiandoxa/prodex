use super::*;

pub(crate) fn runtime_request_previous_response_id(
    request: &RuntimeProxyRequest,
) -> Option<String> {
    runtime_request_previous_response_id_from_bytes(&request.body)
}

#[derive(Clone, Default)]
pub(crate) struct RuntimeWebsocketRequestMetadata {
    pub(crate) previous_response_id: Option<String>,
    pub(crate) session_id: Option<String>,
    pub(crate) requires_previous_response_affinity: bool,
    pub(crate) previous_response_fresh_fallback_shape:
        Option<RuntimePreviousResponseFreshFallbackShape>,
}

pub(crate) fn parse_runtime_websocket_request_metadata(
    request_text: &str,
) -> RuntimeWebsocketRequestMetadata {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(request_text) else {
        return RuntimeWebsocketRequestMetadata::default();
    };
    RuntimeWebsocketRequestMetadata {
        previous_response_id: runtime_request_previous_response_id_from_value(&value),
        session_id: runtime_request_session_id_from_value(&value),
        requires_previous_response_affinity:
            runtime_request_value_requires_previous_response_affinity(&value),
        previous_response_fresh_fallback_shape:
            runtime_request_value_previous_response_fresh_fallback_shape(&value),
    }
}

pub(crate) fn runtime_request_previous_response_id_from_bytes(body: &[u8]) -> Option<String> {
    if body.is_empty() {
        return None;
    }

    let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    runtime_request_previous_response_id_from_value(&value)
}

#[cfg(test)]
pub(crate) fn runtime_request_previous_response_id_from_text(request_text: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(request_text).ok()?;
    runtime_request_previous_response_id_from_value(&value)
}

pub(crate) fn runtime_request_previous_response_id_from_value(
    value: &serde_json::Value,
) -> Option<String> {
    value
        .get("previous_response_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn runtime_request_without_previous_response_id(
    request: &RuntimeProxyRequest,
) -> Option<RuntimeProxyRequest> {
    let _ = request;
    // Hard fuse: runtime must never synthesize a fresh request from a chained
    // previous_response continuation. Tests still call this helper directly.
    None
}

pub(crate) fn runtime_request_value_requires_previous_response_affinity(
    value: &serde_json::Value,
) -> bool {
    if runtime_request_previous_response_id_from_value(value).is_none() {
        return false;
    }

    value
        .get("input")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                let Some(object) = item.as_object() else {
                    return false;
                };
                let item_type = object
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                let has_call_id = object
                    .get("call_id")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|call_id| !call_id.trim().is_empty());
                has_call_id && item_type.ends_with("_call_output")
            })
        })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimePreviousResponseFreshFallbackShape {
    ToolOutputOnly,
    EmptyInputOnly,
    SessionScopedFreshReplay,
    ContextDependentContinuation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimePreviousResponseFreshFallbackPolicy {
    NotApplicable,
    FailClosed {
        request_shape: RuntimePreviousResponseFreshFallbackPolicyShape,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimePreviousResponseFreshFallbackPolicyShape {
    Unknown,
    ToolOutputOnly,
    EmptyInputOnly,
    SessionScopedFreshReplay,
    ContextDependentContinuation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RuntimePreviousResponseFreshFallbackPolicyInput {
    pub(crate) has_previous_response_context: bool,
    pub(crate) request_requires_locked_previous_response_affinity: bool,
    pub(crate) fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
}

impl RuntimePreviousResponseFreshFallbackPolicy {
    pub(crate) fn allows_fresh_fallback(self) -> bool {
        false
    }

    pub(crate) fn blocks_without_affinity(
        self,
        has_turn_state_retry: bool,
        request_requires_locked_previous_response_affinity: bool,
    ) -> bool {
        matches!(self, Self::FailClosed { .. })
            && !has_turn_state_retry
            && !request_requires_locked_previous_response_affinity
    }

    pub(crate) fn is_fail_closed(self) -> bool {
        matches!(self, Self::FailClosed { .. })
    }
}

pub(crate) fn runtime_previous_response_fresh_fallback_policy(
    input: RuntimePreviousResponseFreshFallbackPolicyInput,
) -> RuntimePreviousResponseFreshFallbackPolicy {
    if !input.has_previous_response_context
        && !input.request_requires_locked_previous_response_affinity
        && input.fresh_fallback_shape.is_none()
    {
        return RuntimePreviousResponseFreshFallbackPolicy::NotApplicable;
    }

    RuntimePreviousResponseFreshFallbackPolicy::FailClosed {
        request_shape: match input.fresh_fallback_shape {
            Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly) => {
                RuntimePreviousResponseFreshFallbackPolicyShape::ToolOutputOnly
            }
            Some(RuntimePreviousResponseFreshFallbackShape::EmptyInputOnly) => {
                RuntimePreviousResponseFreshFallbackPolicyShape::EmptyInputOnly
            }
            Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay) => {
                RuntimePreviousResponseFreshFallbackPolicyShape::SessionScopedFreshReplay
            }
            Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation) => {
                RuntimePreviousResponseFreshFallbackPolicyShape::ContextDependentContinuation
            }
            None => RuntimePreviousResponseFreshFallbackPolicyShape::Unknown,
        },
    }
}

pub(crate) fn runtime_previous_response_fresh_fallback_shape_label(
    shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> &'static str {
    match shape {
        Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly) => "tool_output_only",
        Some(RuntimePreviousResponseFreshFallbackShape::EmptyInputOnly) => "empty_input",
        Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay) => {
            "session_replayable"
        }
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation) => {
            "continuation_only"
        }
        None => "none",
    }
}

#[allow(dead_code)]
pub(crate) fn runtime_previous_response_fresh_fallback_shape_allows_recovery(
    shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> bool {
    runtime_previous_response_fresh_fallback_policy(
        RuntimePreviousResponseFreshFallbackPolicyInput {
            has_previous_response_context: shape.is_some(),
            request_requires_locked_previous_response_affinity: false,
            fresh_fallback_shape: shape,
        },
    )
    .allows_fresh_fallback()
}

pub(crate) fn runtime_previous_response_fresh_fallback_shape_with_session(
    shape: Option<RuntimePreviousResponseFreshFallbackShape>,
    has_session_affinity: bool,
) -> Option<RuntimePreviousResponseFreshFallbackShape> {
    if !has_session_affinity {
        return shape;
    }

    match shape {
        Some(RuntimePreviousResponseFreshFallbackShape::EmptyInputOnly) => {
            Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay)
        }
        other => other,
    }
}

fn runtime_request_value_previous_response_input_item_is_tool_output(
    item: &serde_json::Value,
) -> bool {
    let Some(object) = item.as_object() else {
        return false;
    };
    let item_type = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let has_call_id = object
        .get("call_id")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|call_id| !call_id.trim().is_empty());
    has_call_id && item_type.ends_with("_call_output")
}

pub(crate) fn runtime_request_value_previous_response_fresh_fallback_shape(
    value: &serde_json::Value,
) -> Option<RuntimePreviousResponseFreshFallbackShape> {
    runtime_request_previous_response_id_from_value(value)?;

    let has_session_affinity = runtime_request_session_id_from_value(value).is_some();
    let input = value
        .get("input")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let has_context_dependent_input = input
        .iter()
        .any(|item| !runtime_request_value_previous_response_input_item_is_tool_output(item));
    let tool_output_only = !input.is_empty()
        && input
            .iter()
            .all(runtime_request_value_previous_response_input_item_is_tool_output);

    Some(if tool_output_only {
        // A bare function_call_output still depends on the previous response's tool-call item.
        // Session metadata alone is not enough; fresh replay would surface "No tool call found".
        RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly
    } else if has_context_dependent_input {
        // Ordinary previous_response follow-ups carry only the incremental user/tool delta for the
        // next turn. Dropping previous_response_id would silently erase the earlier conversation
        // context, so these requests must stay chained even when session metadata is present.
        RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation
    } else if has_session_affinity {
        // Keep the label for diagnostics only. Session metadata is affinity, not replay state.
        RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay
    } else {
        RuntimePreviousResponseFreshFallbackShape::EmptyInputOnly
    })
}

pub(crate) fn runtime_request_previous_response_fresh_fallback_shape(
    request: &RuntimeProxyRequest,
) -> Option<RuntimePreviousResponseFreshFallbackShape> {
    let body_shape = serde_json::from_slice::<serde_json::Value>(&request.body)
        .ok()
        .and_then(|value| runtime_request_value_previous_response_fresh_fallback_shape(&value));
    runtime_previous_response_fresh_fallback_shape_with_session(
        body_shape,
        runtime_request_explicit_session_id(request).is_some()
            || runtime_request_session_id_from_turn_metadata(request).is_some(),
    )
}

pub(crate) fn runtime_request_requires_previous_response_affinity(
    request: &RuntimeProxyRequest,
) -> bool {
    serde_json::from_slice::<serde_json::Value>(&request.body)
        .map(|value| runtime_request_value_requires_previous_response_affinity(&value))
        .unwrap_or(false)
}

pub(crate) fn runtime_websocket_previous_response_requires_previous_response_affinity(
    trusted_previous_response_affinity: bool,
    previous_response_id: Option<&str>,
    request_turn_state: Option<&str>,
) -> bool {
    // A websocket continuation without replayable turn state cannot be retried on another
    // profile once the owning previous_response binding is trusted.
    trusted_previous_response_affinity
        && previous_response_id.is_some()
        && request_turn_state.is_none()
}

pub(crate) fn runtime_websocket_request_requires_locked_previous_response_affinity(
    request_requires_previous_response_affinity: bool,
    trusted_previous_response_affinity: bool,
    previous_response_id: Option<&str>,
    request_turn_state: Option<&str>,
) -> bool {
    request_requires_previous_response_affinity
        || runtime_websocket_previous_response_requires_previous_response_affinity(
            trusted_previous_response_affinity,
            previous_response_id,
            request_turn_state,
        )
}

#[cfg(test)]
pub(crate) fn runtime_request_text_without_previous_response_id(
    request_text: &str,
) -> Option<String> {
    let _ = request_text;
    // Hard fuse: websocket continuations must also stay chained.
    None
}

pub(crate) fn runtime_request_turn_state(request: &RuntimeProxyRequest) -> Option<String> {
    request.headers.iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("x-codex-turn-state")
            .then(|| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

pub(crate) fn runtime_request_session_id_from_value(value: &serde_json::Value) -> Option<String> {
    value
        .get("session_id")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            value
                .get("client_metadata")
                .and_then(|metadata| metadata.get("session_id"))
                .and_then(serde_json::Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn runtime_request_session_id_from_turn_metadata(
    request: &RuntimeProxyRequest,
) -> Option<String> {
    request
        .headers
        .iter()
        .find_map(|(name, value)| {
            name.eq_ignore_ascii_case("x-codex-turn-metadata")
                .then_some(value.as_str())
        })
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .and_then(|value| runtime_request_session_id_from_value(&value))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeExplicitSessionId(String);

impl RuntimeExplicitSessionId {
    pub(crate) fn from_header_value(value: &str) -> Option<Self> {
        let value = value.trim();
        (!value.is_empty()).then(|| Self(value.to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn into_string(self) -> String {
        self.0
    }
}

impl std::ops::Deref for RuntimeExplicitSessionId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

pub(crate) fn runtime_request_explicit_session_id(
    request: &RuntimeProxyRequest,
) -> Option<RuntimeExplicitSessionId> {
    runtime_proxy_request_header_value(&request.headers, "session_id")
        .or_else(|| runtime_proxy_request_header_value(&request.headers, "x-session-id"))
        .and_then(RuntimeExplicitSessionId::from_header_value)
}

pub(crate) fn runtime_request_session_id(request: &RuntimeProxyRequest) -> Option<String> {
    runtime_request_explicit_session_id(request)
        .map(RuntimeExplicitSessionId::into_string)
        .or_else(|| runtime_request_session_id_from_turn_metadata(request))
        .or_else(|| {
            serde_json::from_slice::<serde_json::Value>(&request.body)
                .ok()
                .and_then(|value| runtime_request_session_id_from_value(&value))
        })
}

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

#[allow(clippy::too_many_arguments)]
pub(crate) fn clear_runtime_response_profile_affinity(
    profile_name: &str,
    bound_profile: &mut Option<String>,
    session_profile: &mut Option<String>,
    candidate_turn_state_retry_profile: &mut Option<String>,
    candidate_turn_state_retry_value: &mut Option<String>,
    pinned_profile: &mut Option<String>,
    previous_response_retry_index: &mut usize,
    reset_previous_response_retry_index: bool,
    turn_state_profile: &mut Option<String>,
    compact_followup_profile: Option<&mut Option<(String, &'static str)>>,
) {
    if bound_profile.as_deref() == Some(profile_name) {
        *bound_profile = None;
    }
    if session_profile.as_deref() == Some(profile_name) {
        *session_profile = None;
    }
    if candidate_turn_state_retry_profile.as_deref() == Some(profile_name) {
        *candidate_turn_state_retry_profile = None;
        *candidate_turn_state_retry_value = None;
    }
    if pinned_profile.as_deref() == Some(profile_name) {
        *pinned_profile = None;
        if reset_previous_response_retry_index {
            *previous_response_retry_index = 0;
        }
    }
    if turn_state_profile.as_deref() == Some(profile_name) {
        *turn_state_profile = None;
    }
    if let Some(compact_followup_profile) = compact_followup_profile
        && compact_followup_profile
            .as_ref()
            .is_some_and(|(owner, _)| owner == profile_name)
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
mod tests {
    use super::*;

    fn runtime_proxy_affinity_test_shared(name: &str) -> RuntimeRotationProxyShared {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let root = env::temp_dir().join(format!(
            "prodex-affinity-test-{name}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("runtime affinity test root should exist");
        let paths = AppPaths {
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("shared-codex"),
            legacy_shared_codex_root: root.join("shared"),
            root,
        };

        RuntimeRotationProxyShared {
            async_client: reqwest::Client::new(),
            async_runtime: Arc::new(
                TokioRuntimeBuilder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("tokio runtime"),
            ),
            runtime: Arc::new(Mutex::new(RuntimeRotationState {
                paths,
                state: AppState::default(),
                upstream_base_url: "http://127.0.0.1".to_string(),
                include_code_review: false,
                current_profile: "main".to_string(),
                profile_usage_auth: BTreeMap::new(),
                turn_state_bindings: BTreeMap::new(),
                session_id_bindings: BTreeMap::new(),
                continuation_statuses: RuntimeContinuationStatuses::default(),
                profile_probe_cache: BTreeMap::new(),
                profile_usage_snapshots: BTreeMap::new(),
                profile_retry_backoff_until: BTreeMap::new(),
                profile_transport_backoff_until: BTreeMap::new(),
                profile_route_circuit_open_until: BTreeMap::new(),
                profile_inflight: BTreeMap::new(),
                profile_health: BTreeMap::new(),
            })),
            log_path: env::temp_dir().join(format!(
                "prodex-affinity-test-{name}-{}-{unique}.log",
                std::process::id()
            )),
            request_sequence: Arc::new(AtomicU64::new(1)),
            state_save_revision: Arc::new(AtomicU64::new(0)),
            local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
            active_request_count: Arc::new(AtomicUsize::new(0)),
            active_request_limit: 8,
            runtime_state_lock_wait_counters:
                RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
            lane_admission: RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
                responses: 8,
                compact: 8,
                websocket: 8,
                standard: 8,
            }),
        }
    }

    fn runtime_proxy_affinity_add_profile(shared: &RuntimeRotationProxyShared, profile_name: &str) {
        let mut runtime = shared
            .runtime
            .lock()
            .expect("runtime test state should lock");
        let root = runtime.paths.root.clone();
        runtime.state.profiles.insert(
            profile_name.to_string(),
            ProfileEntry {
                codex_home: root.join(profile_name),
                managed: false,
                email: None,
                provider: ProfileProvider::Openai,
            },
        );
    }

    fn runtime_proxy_affinity_bind_session(
        shared: &RuntimeRotationProxyShared,
        session_id: &str,
        profile_name: &str,
    ) {
        shared
            .runtime
            .lock()
            .expect("runtime test state should lock")
            .session_id_bindings
            .insert(
                session_id.to_string(),
                ResponseProfileBinding {
                    profile_name: profile_name.to_string(),
                    bound_at: Local::now().timestamp(),
                },
            );
    }

    fn runtime_proxy_affinity_bind_compact_turn_state(
        shared: &RuntimeRotationProxyShared,
        turn_state: &str,
        profile_name: &str,
    ) {
        shared
            .runtime
            .lock()
            .expect("runtime test state should lock")
            .turn_state_bindings
            .insert(
                runtime_compact_turn_state_lineage_key(turn_state),
                ResponseProfileBinding {
                    profile_name: profile_name.to_string(),
                    bound_at: Local::now().timestamp(),
                },
            );
    }

    #[test]
    fn runtime_proxy_affinity_request_context_prefers_compact_turn_state_over_session() {
        let shared = runtime_proxy_affinity_test_shared("compact-turn-state");
        runtime_proxy_affinity_add_profile(&shared, "session-owner");
        runtime_proxy_affinity_add_profile(&shared, "compact-owner");
        runtime_proxy_affinity_bind_session(&shared, "session-1", "session-owner");
        runtime_proxy_affinity_bind_compact_turn_state(&shared, "turn-state-1", "compact-owner");

        let affinity = derive_runtime_response_route_affinity_for_request(
            &shared,
            RuntimeResponseRouteAffinityRequest {
                request_turn_state: Some("turn-state-1"),
                request_session_id: Some("session-1"),
                ..RuntimeResponseRouteAffinityRequest::default()
            },
        )
        .expect("route affinity should derive");

        assert_eq!(
            affinity.bound_session_profile.as_deref(),
            Some("session-owner")
        );
        assert_eq!(
            affinity.compact_followup_profile,
            Some(("compact-owner".to_string(), "turn_state"))
        );
        assert_eq!(affinity.session_profile, None);
        assert_eq!(affinity.pinned_profile.as_deref(), Some("compact-owner"));
    }

    #[test]
    fn runtime_proxy_affinity_refresh_context_updates_slots_and_logs_presence() {
        let shared = runtime_proxy_affinity_test_shared("refresh");
        let mut bound_session_profile = Some("old-bound-session".to_string());
        let mut compact_followup_profile = Some(("old-compact".to_string(), "turn_state"));
        let mut compact_session_profile = Some("old-compact-session".to_string());
        let mut session_profile = Some("old-session".to_string());
        let mut pinned_profile = Some("old-pinned".to_string());

        refresh_and_log_runtime_response_route_affinity_for_request(
            RuntimeResponseRouteAffinityLogContext {
                shared: &shared,
                request_id: 42,
                websocket_session_id: Some(7),
                reason: "test_refresh",
            },
            RuntimeResponseRouteAffinityRequest {
                websocket_session_profile: Some("websocket-owner"),
                ..RuntimeResponseRouteAffinityRequest::default()
            },
            RuntimeResponseRouteAffinityRefreshSlots {
                bound_session_profile: &mut bound_session_profile,
                compact_followup_profile: &mut compact_followup_profile,
                compact_session_profile: &mut compact_session_profile,
                session_profile: &mut session_profile,
                pinned_profile: &mut pinned_profile,
            },
        )
        .expect("route affinity refresh should succeed");

        assert_eq!(bound_session_profile, None);
        assert_eq!(compact_followup_profile, None);
        assert_eq!(compact_session_profile, None);
        assert_eq!(session_profile.as_deref(), Some("websocket-owner"));
        assert_eq!(pinned_profile, None);

        let log = fs::read_to_string(&shared.log_path).expect("route affinity log should exist");
        assert!(log.contains(
            "request=42 websocket_session=7 route_affinity_recompute reason=test_refresh"
        ));
        assert!(log.contains("previous_response_id_present=false"));
        assert!(log.contains("request_turn_state_present=false"));
        assert!(log.contains("session_profile=Some(\"websocket-owner\")"));
    }
}
