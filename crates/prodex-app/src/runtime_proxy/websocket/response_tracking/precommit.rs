use super::*;

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
