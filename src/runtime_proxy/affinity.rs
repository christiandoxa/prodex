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

pub(crate) fn runtime_request_without_previous_response_id(
    request: &RuntimeProxyRequest,
) -> Option<RuntimeProxyRequest> {
    let _ = request;
    // Hard fuse: runtime must never synthesize a fresh request from a chained
    // previous_response continuation. Keep stale call sites harmless.
    None
}

pub(crate) fn runtime_request_without_turn_state_header(
    request: &RuntimeProxyRequest,
) -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: request.method.clone(),
        path_and_query: request.path_and_query.clone(),
        headers: request
            .headers
            .iter()
            .filter(|(name, _)| !name.eq_ignore_ascii_case("x-codex-turn-state"))
            .cloned()
            .collect(),
        body: request.body.clone(),
    }
}

pub(crate) fn runtime_request_without_previous_response_affinity(
    request: &RuntimeProxyRequest,
) -> Option<RuntimeProxyRequest> {
    let request = runtime_request_without_previous_response_id(request)?;
    Some(runtime_request_without_turn_state_header(&request))
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
    _shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> bool {
    // Fail closed: previous_response continuations stay chained; session metadata only steers
    // affinity, it never authorizes dropping previous_response_id.
    false
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

pub(crate) fn runtime_request_text_without_previous_response_id(
    request_text: &str,
) -> Option<String> {
    let _ = request_text;
    // Hard fuse: websocket continuations must also stay chained.
    None
}

pub(crate) struct RuntimePreviousResponseFreshFallbackState<'a> {
    pub(crate) previous_response_id: &'a mut Option<String>,
    pub(crate) request_turn_state: &'a mut Option<String>,
    pub(crate) previous_response_fresh_fallback_used: &'a mut bool,
    pub(crate) saw_previous_response_not_found: &'a mut bool,
    pub(crate) previous_response_retry_candidate: &'a mut Option<String>,
    pub(crate) previous_response_retry_index: &'a mut usize,
    pub(crate) candidate_turn_state_retry_profile: &'a mut Option<String>,
    pub(crate) candidate_turn_state_retry_value: &'a mut Option<String>,
    pub(crate) trusted_previous_response_affinity: &'a mut bool,
    pub(crate) bound_profile: &'a mut Option<String>,
    pub(crate) pinned_profile: &'a mut Option<String>,
    pub(crate) turn_state_profile: &'a mut Option<String>,
    pub(crate) session_profile: &'a mut Option<String>,
    pub(crate) excluded_profiles: &'a mut BTreeSet<String>,
    pub(crate) last_failure: &'a mut Option<(RuntimeUpstreamFailureResponse, bool)>,
    pub(crate) selection_started_at: &'a mut Instant,
    pub(crate) selection_attempts: &'a mut usize,
}

impl RuntimePreviousResponseFreshFallbackState<'_> {
    fn reset(&mut self) {
        *self.previous_response_id = None;
        *self.request_turn_state = None;
        *self.previous_response_fresh_fallback_used = true;
        *self.saw_previous_response_not_found = false;
        *self.previous_response_retry_candidate = None;
        *self.previous_response_retry_index = 0;
        *self.candidate_turn_state_retry_profile = None;
        *self.candidate_turn_state_retry_value = None;
        *self.trusted_previous_response_affinity = false;
        *self.bound_profile = None;
        *self.pinned_profile = None;
        *self.turn_state_profile = None;
        *self.session_profile = None;
        self.excluded_profiles.clear();
        *self.last_failure = None;
        *self.selection_started_at = Instant::now();
        *self.selection_attempts = 0;
    }
}

pub(crate) fn reset_runtime_previous_response_fresh_fallback_state(
    mut state: RuntimePreviousResponseFreshFallbackState<'_>,
) {
    state.reset();
}

pub(crate) fn apply_runtime_responses_previous_response_fresh_fallback(
    request: &mut RuntimeProxyRequest,
    fresh_request: RuntimeProxyRequest,
    fallback_state: RuntimePreviousResponseFreshFallbackState<'_>,
) {
    *request = fresh_request;
    reset_runtime_previous_response_fresh_fallback_state(fallback_state);
}

pub(crate) struct RuntimeWebsocketFreshFallbackTarget<'a> {
    pub(crate) request_text: &'a mut String,
    pub(crate) handshake_request: &'a mut RuntimeProxyRequest,
    pub(crate) websocket_reuse_fresh_retry_profiles: &'a mut BTreeSet<String>,
}

pub(crate) fn apply_runtime_websocket_previous_response_fresh_fallback(
    fresh_request_text: String,
    target: RuntimeWebsocketFreshFallbackTarget<'_>,
    fallback_state: RuntimePreviousResponseFreshFallbackState<'_>,
) {
    *target.request_text = fresh_request_text;
    *target.handshake_request = runtime_request_without_turn_state_header(target.handshake_request);
    target.websocket_reuse_fresh_retry_profiles.clear();
    reset_runtime_previous_response_fresh_fallback_state(fallback_state);
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

#[allow(clippy::too_many_arguments)]
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
    let allows_session_affinity =
        previous_response_id.is_none() && bound_profile.is_none() && turn_state_profile.is_none();
    let bound_session_profile = if allows_session_affinity {
        request_session_id
            .map(|session_id| runtime_session_bound_profile(shared, session_id))
            .transpose()?
            .flatten()
    } else {
        None
    };
    let compact_followup_profile = if allows_session_affinity {
        runtime_compact_turn_state_followup_bound_profile(shared, request_turn_state)?
            .map(|profile_name| (profile_name, "turn_state"))
    } else {
        None
    };
    let compact_session_profile = if allows_session_affinity
        && compact_followup_profile.is_none()
        && bound_session_profile.is_none()
        && websocket_session_profile.is_none()
    {
        runtime_compact_session_followup_bound_profile(shared, explicit_request_session_id)?
    } else {
        None
    };
    let session_profile = if allows_session_affinity && compact_followup_profile.is_none() {
        websocket_session_profile
            .map(str::to_string)
            .or(bound_session_profile.clone())
            .or(compact_session_profile.clone())
    } else {
        None
    };
    let pinned_profile = bound_profile
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

#[allow(clippy::too_many_arguments)]
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
    runtime_proxy_log(
        shared,
        match websocket_session_id {
            Some(session_id) => format!(
                "request={request_id} websocket_session={session_id} route_affinity_recompute_result reason={reason} previous_response_id_present={previous_response_id_present} request_turn_state_present={request_turn_state_present} request_session_id_present={request_session_id_present} explicit_session_id_present={explicit_request_session_id_present} bound_session_profile={:?} compact_followup_profile={:?} compact_session_profile={:?} session_profile={:?} pinned_profile={:?}",
                affinity.bound_session_profile,
                affinity.compact_followup_profile,
                affinity.compact_session_profile,
                affinity.session_profile,
                affinity.pinned_profile,
            ),
            None => format!(
                "request={request_id} transport=http route_affinity_recompute_result reason={reason} previous_response_id_present={previous_response_id_present} request_turn_state_present={request_turn_state_present} request_session_id_present={request_session_id_present} explicit_session_id_present={explicit_request_session_id_present} bound_session_profile={:?} compact_followup_profile={:?} compact_session_profile={:?} session_profile={:?} pinned_profile={:?}",
                affinity.bound_session_profile,
                affinity.compact_followup_profile,
                affinity.compact_session_profile,
                affinity.session_profile,
                affinity.pinned_profile,
            ),
        },
    );
    if let Some((profile_name, source)) = affinity.compact_followup_profile.as_ref() {
        runtime_proxy_log(
            shared,
            match websocket_session_id {
                Some(session_id) => format!(
                    "request={request_id} websocket_session={session_id} compact_followup_owner profile={profile_name} source={source}"
                ),
                None => format!(
                    "request={request_id} transport=http compact_followup_owner profile={profile_name} source={source}"
                ),
            },
        );
    }
    if let Some(profile_name) = affinity.compact_session_profile.as_deref() {
        runtime_proxy_log(
            shared,
            match websocket_session_id {
                Some(session_id) => format!(
                    "request={request_id} websocket_session={session_id} compact_followup_owner profile={profile_name} source=session_id"
                ),
                None => format!(
                    "request={request_id} transport=http compact_followup_owner profile={profile_name} source=session_id"
                ),
            },
        );
    }
}

#[allow(clippy::too_many_arguments)]
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
    runtime_proxy_log(
        shared,
        match websocket_session_id {
            Some(session_id) => format!(
                "request={request_id} websocket_session={session_id} route_affinity_recompute reason={reason} previous_response_id_present={previous_response_id_present} request_turn_state_present={request_turn_state_present} request_session_id_present={request_session_id_present} explicit_session_id_present={explicit_request_session_id_present}"
            ),
            None => format!(
                "request={request_id} transport=http route_affinity_recompute reason={reason} previous_response_id_present={previous_response_id_present} request_turn_state_present={request_turn_state_present} request_session_id_present={request_session_id_present} explicit_session_id_present={explicit_request_session_id_present}"
            ),
        },
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
    log_runtime_response_route_affinity_recompute(
        shared,
        request_id,
        websocket_session_id,
        reason,
        previous_response_id.is_some(),
        request_turn_state.is_some(),
        request_session_id.is_some(),
        explicit_request_session_id.is_some(),
    );
    let derived = derive_runtime_response_route_affinity(
        shared,
        previous_response_id,
        bound_profile,
        turn_state_profile,
        request_turn_state,
        request_session_id,
        explicit_request_session_id,
        websocket_session_profile,
    )?;
    *bound_session_profile = derived.bound_session_profile.clone();
    *compact_followup_profile = derived.compact_followup_profile.clone();
    *compact_session_profile = derived.compact_session_profile.clone();
    *session_profile = derived.session_profile.clone();
    *pinned_profile = derived.pinned_profile.clone();
    log_runtime_response_route_affinity(
        shared,
        request_id,
        websocket_session_id,
        reason,
        previous_response_id.is_some(),
        request_turn_state.is_some(),
        request_session_id.is_some(),
        explicit_request_session_id.is_some(),
        &derived,
    );
    Ok(())
}
