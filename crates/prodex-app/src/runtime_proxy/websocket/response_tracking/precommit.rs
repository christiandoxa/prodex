use super::{
    RUNTIME_PROXY_SSE_LOOKAHEAD_BYTES, RuntimeBufferedWebsocketTextFrame, RuntimeLocalWebSocket,
    RuntimeProxyRequest, RuntimeRotationProxyShared, RuntimeWebsocketResponseBindingContext,
    RuntimeWebsocketSessionState, release_runtime_compact_lineage,
    remember_runtime_response_ids_with_turn_state,
    remember_runtime_successful_previous_response_owner, runtime_proxy_log,
    runtime_translate_precommit_previous_response_websocket_text_frame,
    runtime_websocket_precommit_hold_promotion_event_seen,
};
use anyhow::{Context, Result};
use prodex_runtime_state::RuntimeRouteKind;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use tungstenite::Message as WsMessage;

pub(crate) fn remember_runtime_websocket_response_ids(
    context: RuntimeWebsocketResponseBindingContext<'_>,
    response_ids: &[String],
    previous_response_owner_recorded: &mut bool,
) -> Result<()> {
    let RuntimeWebsocketResponseBindingContext {
        shared,
        profile_name,
        request_previous_response_id,
        request_session_id,
        request_turn_state,
        response_turn_state,
    } = context;

    if !*previous_response_owner_recorded {
        remember_runtime_successful_previous_response_owner(
            shared,
            profile_name,
            request_previous_response_id,
            RuntimeRouteKind::Websocket,
        )?;
        *previous_response_owner_recorded = true;
    }
    remember_runtime_response_ids_with_turn_state(
        shared,
        profile_name,
        response_ids,
        response_turn_state,
        RuntimeRouteKind::Websocket,
    )?;
    if !response_ids.is_empty() && response_turn_state.is_some() {
        let _ = release_runtime_compact_lineage(
            shared,
            profile_name,
            request_session_id,
            request_turn_state,
            "response_committed",
        );
    }
    Ok(())
}

pub(crate) fn forward_runtime_proxy_buffered_websocket_text_frames(
    local_socket: &mut RuntimeLocalWebSocket,
    buffered_frames: &mut Vec<RuntimeBufferedWebsocketTextFrame>,
    context: RuntimeWebsocketResponseBindingContext<'_>,
    previous_response_owner_recorded: &mut bool,
) -> Result<()> {
    for frame in buffered_frames.drain(..) {
        remember_runtime_websocket_response_ids(
            context,
            &frame.response_ids,
            previous_response_owner_recorded,
        )?;
        let text = runtime_translate_precommit_previous_response_websocket_text_frame(&frame.text);
        local_socket
            .send(WsMessage::Text(text.into()))
            .context("failed to forward buffered runtime websocket text frame")?;
    }
    Ok(())
}

pub(super) struct RuntimeWebsocketPrecommitHoldRequest<'a> {
    pub(super) request_id: u64,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) profile_name: &'a str,
    pub(super) reuse_existing_session: bool,
    pub(super) precommit_hold_promotion_allowed: bool,
    pub(super) inspected: &'a runtime_proxy_crate::RuntimeInspectedWebsocketTextFrame,
    pub(super) text: &'a str,
    pub(super) buffered_precommit_text_frames: &'a mut Vec<RuntimeBufferedWebsocketTextFrame>,
    pub(super) precommit_hold_count: &'a mut usize,
    pub(super) precommit_hold_bytes: &'a mut usize,
    pub(super) precommit_hold_promotion_event_seen: &'a mut bool,
}

pub(super) fn runtime_websocket_buffer_precommit_hold(
    request: RuntimeWebsocketPrecommitHoldRequest<'_>,
) -> bool {
    let RuntimeWebsocketPrecommitHoldRequest {
        request_id,
        shared,
        profile_name,
        reuse_existing_session,
        precommit_hold_promotion_allowed,
        inspected,
        text,
        buffered_precommit_text_frames,
        precommit_hold_count,
        precommit_hold_bytes,
        precommit_hold_promotion_event_seen,
    } = request;

    if *precommit_hold_count == 0 {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "precommit_hold",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "websocket"),
                    runtime_proxy_log_field("profile", profile_name),
                    runtime_proxy_log_field(
                        "event_type",
                        inspected.event_type.as_deref().unwrap_or("-"),
                    ),
                ],
            ),
        );
    }
    *precommit_hold_count = (*precommit_hold_count).saturating_add(1);
    *precommit_hold_bytes = (*precommit_hold_bytes).saturating_add(text.len());
    *precommit_hold_promotion_event_seen |=
        runtime_websocket_precommit_hold_promotion_event_seen(inspected);
    buffered_precommit_text_frames.push(RuntimeBufferedWebsocketTextFrame {
        text: text.to_string(),
        response_ids: inspected.response_ids.clone(),
    });
    let lookahead_budget_exhausted = *precommit_hold_bytes >= RUNTIME_PROXY_SSE_LOOKAHEAD_BYTES;
    if *precommit_hold_promotion_event_seen || lookahead_budget_exhausted {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "websocket_precommit_hold_promoted",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("profile", profile_name),
                    runtime_proxy_log_field(
                        "event",
                        if lookahead_budget_exhausted {
                            "lookahead_budget_exhausted"
                        } else {
                            "response_created"
                        },
                    ),
                    runtime_proxy_log_field("reuse", reuse_existing_session.to_string()),
                    runtime_proxy_log_field("hold_count", (*precommit_hold_count).to_string()),
                    runtime_proxy_log_field("hold_bytes", (*precommit_hold_bytes).to_string()),
                    runtime_proxy_log_field(
                        "profile_promotion_allowed",
                        precommit_hold_promotion_allowed.to_string(),
                    ),
                ],
            ),
        );
        true
    } else {
        false
    }
}

pub(crate) struct RuntimeWebsocketAttemptRequest<'a> {
    pub(in crate::runtime_proxy) request_id: u64,
    pub(in crate::runtime_proxy) local_socket: &'a mut RuntimeLocalWebSocket,
    pub(in crate::runtime_proxy) handshake_request: &'a RuntimeProxyRequest,
    pub(in crate::runtime_proxy) request_text: &'a str,
    pub(in crate::runtime_proxy) request_previous_response_id: Option<&'a str>,
    pub(in crate::runtime_proxy) request_prompt_cache_key: Option<&'a str>,
    pub(in crate::runtime_proxy) request_session_id: Option<&'a str>,
    pub(in crate::runtime_proxy) request_turn_state: Option<&'a str>,
    pub(in crate::runtime_proxy) shared: &'a RuntimeRotationProxyShared,
    pub(in crate::runtime_proxy) websocket_session: &'a mut RuntimeWebsocketSessionState,
    pub(in crate::runtime_proxy) profile_name: &'a str,
    pub(in crate::runtime_proxy) turn_state_override: Option<&'a str>,
    pub(in crate::runtime_proxy) promote_committed_profile: bool,
}
