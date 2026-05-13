use anyhow::{Context, Result};
use std::time::Duration;

use crate::{runtime_proxy_stream_idle_timeout_ms, runtime_set_upstream_websocket_io_timeout};

use super::{
    RuntimeBufferedWebsocketTextFrame, RuntimeLocalWebSocket, RuntimeRotationProxyShared,
    RuntimeRouteKind, RuntimeUpstreamWebSocket, RuntimeWebsocketResponseBindingContext,
    commit_runtime_proxy_profile_selection_with_policy,
    forward_runtime_proxy_buffered_websocket_text_frames, remember_runtime_prompt_cache_profile,
    remember_runtime_session_id, remember_runtime_turn_state, runtime_proxy_log,
};

pub(crate) struct RuntimeWebsocketCommitRequest<'a, 'socket> {
    pub(in crate::runtime_proxy) request_id: u64,
    pub(in crate::runtime_proxy) local_socket: &'socket mut RuntimeLocalWebSocket,
    pub(in crate::runtime_proxy) upstream_socket: &'socket mut RuntimeUpstreamWebSocket,
    pub(in crate::runtime_proxy) shared: &'a RuntimeRotationProxyShared,
    pub(in crate::runtime_proxy) profile_name: &'a str,
    pub(in crate::runtime_proxy) request_previous_response_id: Option<&'a str>,
    pub(in crate::runtime_proxy) request_session_id: Option<&'a str>,
    pub(in crate::runtime_proxy) request_turn_state: Option<&'a str>,
    pub(in crate::runtime_proxy) response_turn_state: Option<&'a str>,
    pub(in crate::runtime_proxy) promote_committed_profile: bool,
    pub(in crate::runtime_proxy) request_prompt_cache_key: Option<&'a str>,
    pub(in crate::runtime_proxy) buffered_precommit_text_frames:
        &'socket mut Vec<RuntimeBufferedWebsocketTextFrame>,
    pub(in crate::runtime_proxy) previous_response_owner_recorded: &'socket mut bool,
    pub(in crate::runtime_proxy) log_event: &'static str,
}

pub(crate) fn commit_runtime_websocket_attempt(
    request: RuntimeWebsocketCommitRequest<'_, '_>,
) -> Result<()> {
    let RuntimeWebsocketCommitRequest {
        request_id,
        local_socket,
        upstream_socket,
        shared,
        profile_name,
        request_previous_response_id,
        request_session_id,
        request_turn_state,
        response_turn_state,
        promote_committed_profile,
        request_prompt_cache_key,
        buffered_precommit_text_frames,
        previous_response_owner_recorded,
        log_event,
    } = request;

    runtime_set_upstream_websocket_io_timeout(
        upstream_socket,
        Some(Duration::from_millis(runtime_proxy_stream_idle_timeout_ms())),
    )
    .context("failed to restore runtime websocket idle timeout")?;
    remember_runtime_session_id(
        shared,
        profile_name,
        request_session_id,
        RuntimeRouteKind::Websocket,
    )?;
    remember_runtime_turn_state(
        shared,
        profile_name,
        response_turn_state,
        RuntimeRouteKind::Websocket,
    )?;
    let _ = commit_runtime_proxy_profile_selection_with_policy(
        shared,
        profile_name,
        RuntimeRouteKind::Websocket,
        promote_committed_profile,
    )?;
    remember_runtime_prompt_cache_profile(
        shared,
        profile_name,
        request_prompt_cache_key,
        RuntimeRouteKind::Websocket,
    );
    runtime_proxy_log(
        shared,
        format!("request={request_id} transport=websocket {log_event} profile={profile_name}"),
    );
    forward_runtime_proxy_buffered_websocket_text_frames(
        local_socket,
        buffered_precommit_text_frames,
        RuntimeWebsocketResponseBindingContext {
            shared,
            profile_name,
            request_previous_response_id,
            request_session_id,
            request_turn_state,
            response_turn_state,
        },
        previous_response_owner_recorded,
    )
}
