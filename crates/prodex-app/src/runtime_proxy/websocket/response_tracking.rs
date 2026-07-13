use super::*;
mod commit;
mod failure;
mod frame;
mod precommit;
mod previous_response;
mod quota_gate;
mod session;
mod terminal;
mod upstream_send;
use commit::*;
use failure::*;
use frame::*;
pub(crate) use precommit::*;
use previous_response::*;
use quota_gate::*;
use session::*;
use terminal::*;
use upstream_send::*;

pub(crate) fn attempt_runtime_websocket_request(
    attempt: RuntimeWebsocketAttemptRequest<'_>,
) -> Result<RuntimeWebsocketAttempt> {
    let RuntimeWebsocketAttemptRequest {
        request_id,
        local_socket,
        handshake_request,
        request_text,
        request_previous_response_id,
        request_prompt_cache_key,
        request_session_id,
        request_turn_state,
        shared,
        websocket_session,
        profile_name,
        turn_state_override,
        promote_committed_profile,
    } = attempt;
    let request_model_name = runtime_smart_context_model_name_from_body(request_text.as_bytes());

    let realtime_websocket = is_runtime_realtime_websocket_path(&handshake_request.path_and_query);
    if let Some(attempt) =
        runtime_websocket_pre_send_quota_gate(RuntimeWebsocketPreSendQuotaGateRequest {
            request_id,
            shared,
            websocket_session,
            profile_name,
            request_previous_response_id,
            request_session_id,
            request_turn_state,
        })?
    {
        return Ok(attempt);
    }

    let session_start =
        start_runtime_websocket_upstream_session(RuntimeWebsocketSessionStartRequest {
            request_id,
            handshake_request,
            request_previous_response_id,
            request_session_id,
            request_turn_state,
            shared,
            websocket_session,
            profile_name,
            turn_state_override,
            promote_committed_profile,
        })?;
    let RuntimeWebsocketSessionStart {
        mut upstream_socket,
        mut upstream_turn_state,
        mut inflight_guard,
        reuse_existing_session,
        precommit_hold_promotion_allowed,
        precommit_transport_retry_allowed,
        reuse_started_at,
        precommit_started_at,
    } = match session_start {
        RuntimeWebsocketSessionStartDecision::Started(start) => *start,
        RuntimeWebsocketSessionStartDecision::Attempt(attempt) => return Ok(attempt),
    };

    if let Some(attempt) =
        send_runtime_websocket_upstream_request(RuntimeWebsocketUpstreamSendRequest {
            request_id,
            request_text,
            handshake_request,
            shared,
            websocket_session,
            profile_name,
            reuse_existing_session,
            precommit_transport_retry_allowed,
            upstream_socket: &mut upstream_socket,
        })?
    {
        return Ok(attempt);
    }

    let mut committed = false;
    let mut first_upstream_frame_seen = false;
    let mut buffered_precommit_text_frames = Vec::new();
    let mut committed_response_ids = BTreeSet::new();
    let mut previous_response_owner_recorded = false;
    let mut precommit_hold_count = 0usize;
    let mut precommit_hold_bytes = 0usize;
    let mut precommit_hold_promotion_event_seen = false;
    loop {
        match upstream_socket.read() {
            Ok(WsMessage::Text(text)) => {
                let text = text.to_string();
                mark_runtime_websocket_upstream_frame_seen(
                    &mut upstream_socket,
                    &mut first_upstream_frame_seen,
                    shared.runtime_config.tuning.stream_idle_timeout_ms,
                )?;

                let mut inspected = inspect_runtime_websocket_text_frame(text.as_str());
                if realtime_websocket
                    && inspected
                        .event_type
                        .as_deref()
                        .is_some_and(runtime_realtime_websocket_terminal_event_kind)
                {
                    inspected.terminal_event = true;
                }
                if let Some(turn_state) = inspected.turn_state.as_deref() {
                    remember_runtime_turn_state(
                        shared,
                        profile_name,
                        Some(turn_state),
                        RuntimeRouteKind::Websocket,
                    )?;
                    upstream_turn_state = Some(turn_state.to_string());
                }
                let mut promoted_precommit_hold = false;

                if !committed {
                    match inspected.retry_kind {
                        Some(RuntimeWebsocketRetryInspectionKind::ConnectionLimitReached) => {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=websocket connection_limit_reached profile={profile_name}"
                                ),
                            );
                            let _ = upstream_socket.close(None);
                            websocket_session.reset();
                            return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                                profile_name: profile_name.to_string(),
                                event: "connection_limit_reached",
                            });
                        }
                        Some(RuntimeWebsocketRetryInspectionKind::QuotaBlocked) => {
                            let _ = upstream_socket.close(None);
                            websocket_session.reset();
                            return Ok(RuntimeWebsocketAttempt::QuotaBlocked {
                                profile_name: profile_name.to_string(),
                                payload: RuntimeWebsocketErrorPayload::Text(text),
                            });
                        }
                        Some(RuntimeWebsocketRetryInspectionKind::Overloaded) => {
                            let _ = upstream_socket.close(None);
                            websocket_session.reset();
                            return Ok(RuntimeWebsocketAttempt::Overloaded {
                                profile_name: profile_name.to_string(),
                                payload: RuntimeWebsocketErrorPayload::Text(text),
                            });
                        }
                        Some(RuntimeWebsocketRetryInspectionKind::PreviousResponseNotFound) => {
                            let _ = upstream_socket.close(None);
                            websocket_session.reset();
                            return Ok(RuntimeWebsocketAttempt::PreviousResponseNotFound {
                                profile_name: profile_name.to_string(),
                                payload: RuntimeWebsocketErrorPayload::Text(text),
                                turn_state: upstream_turn_state.clone(),
                            });
                        }
                        None => {}
                    }
                }

                if !committed && inspected.precommit_hold {
                    promoted_precommit_hold = runtime_websocket_buffer_precommit_hold(
                        RuntimeWebsocketPrecommitHoldRequest {
                            request_id,
                            shared,
                            profile_name,
                            reuse_existing_session,
                            precommit_hold_promotion_allowed,
                            inspected: &inspected,
                            text: &text,
                            buffered_precommit_text_frames: &mut buffered_precommit_text_frames,
                            precommit_hold_count: &mut precommit_hold_count,
                            precommit_hold_bytes: &mut precommit_hold_bytes,
                            precommit_hold_promotion_event_seen:
                                &mut precommit_hold_promotion_event_seen,
                        },
                    );
                    if !promoted_precommit_hold {
                        continue;
                    }
                }

                if !committed {
                    for frame in &buffered_precommit_text_frames {
                        committed_response_ids.extend(frame.response_ids.iter().cloned());
                    }
                    commit_runtime_websocket_attempt(RuntimeWebsocketCommitRequest {
                        request_id,
                        local_socket,
                        upstream_socket: &mut upstream_socket,
                        shared,
                        profile_name,
                        request_previous_response_id,
                        request_session_id,
                        request_turn_state,
                        response_turn_state: upstream_turn_state.as_deref(),
                        promote_committed_profile,
                        request_prompt_cache_key,
                        buffered_precommit_text_frames: &mut buffered_precommit_text_frames,
                        previous_response_owner_recorded: &mut previous_response_owner_recorded,
                        log_event: "committed",
                    })?;
                    committed = true;
                    if promoted_precommit_hold {
                        continue;
                    }
                }

                if !inspected.precommit_hold {
                    committed_response_ids.extend(inspected.response_ids.iter().cloned());
                    remember_runtime_websocket_response_ids(
                        RuntimeWebsocketResponseBindingContext {
                            shared,
                            profile_name,
                            request_previous_response_id,
                            request_session_id,
                            request_turn_state,
                            response_turn_state: upstream_turn_state.as_deref(),
                        },
                        &inspected.response_ids,
                        &mut previous_response_owner_recorded,
                    )?;
                }
                if committed
                    && runtime_token_usage_event_is_loggable(inspected.event_type.as_deref())
                {
                    log_runtime_token_usage(RuntimeTokenUsageLog {
                        shared,
                        request_id,
                        transport: "websocket",
                        profile_name,
                        source: "responses_websocket",
                        prompt_cache_key: request_prompt_cache_key,
                        model_name: request_model_name.as_deref(),
                        usage: inspected.token_usage,
                    });
                }
                let committed_previous_response_not_found = committed
                    && matches!(
                        inspected.retry_kind,
                        Some(RuntimeWebsocketRetryInspectionKind::PreviousResponseNotFound)
                    );
                if committed_previous_response_not_found {
                    record_runtime_websocket_committed_previous_response_not_found(
                        RuntimeWebsocketCommittedPreviousResponseNotFoundRequest {
                            request_id,
                            shared,
                            profile_name,
                            request_previous_response_id,
                            committed_response_ids: &committed_response_ids,
                        },
                    );
                }
                let text = runtime_translate_previous_response_websocket_text_frame(&text);
                local_socket
                    .send(WsMessage::Text(text.into()))
                    .with_context(|| {
                        websocket_session.reset();
                        "failed to forward runtime websocket text frame"
                    })?;
                if inspected.terminal_event {
                    return Ok(finish_runtime_websocket_terminal_event(
                        RuntimeWebsocketTerminalEventRequest {
                            request_id,
                            shared,
                            websocket_session,
                            profile_name,
                            event_type: inspected.event_type.as_deref(),
                            reset_upstream_socket: !realtime_websocket
                                && matches!(
                                    inspected.event_type.as_deref(),
                                    Some("error" | "response.failed" | "response.incomplete")
                                ),
                            precommit_hold_count,
                            committed_previous_response_not_found,
                            upstream_socket,
                            upstream_turn_state,
                            inflight_guard: inflight_guard.take(),
                        },
                    ));
                }
            }
            Ok(WsMessage::Binary(payload)) => {
                mark_runtime_websocket_upstream_frame_seen(
                    &mut upstream_socket,
                    &mut first_upstream_frame_seen,
                    shared
                        .runtime_config
                        .tuning
                        .websocket_precommit_progress_timeout_ms,
                )?;
                if !committed {
                    commit_runtime_websocket_attempt(RuntimeWebsocketCommitRequest {
                        request_id,
                        local_socket,
                        upstream_socket: &mut upstream_socket,
                        shared,
                        profile_name,
                        request_previous_response_id,
                        request_session_id,
                        request_turn_state,
                        response_turn_state: upstream_turn_state.as_deref(),
                        promote_committed_profile,
                        request_prompt_cache_key,
                        buffered_precommit_text_frames: &mut buffered_precommit_text_frames,
                        previous_response_owner_recorded: &mut previous_response_owner_recorded,
                        log_event: "committed_binary",
                    })?;
                    committed = true;
                }
                local_socket
                    .send(WsMessage::Binary(payload))
                    .with_context(|| {
                        websocket_session.reset();
                        "failed to forward runtime websocket binary frame"
                    })?;
            }
            Ok(WsMessage::Ping(payload)) => {
                mark_runtime_websocket_upstream_frame_seen(
                    &mut upstream_socket,
                    &mut first_upstream_frame_seen,
                    shared
                        .runtime_config
                        .tuning
                        .websocket_precommit_progress_timeout_ms,
                )?;
                upstream_socket
                    .send(WsMessage::Pong(payload))
                    .context("failed to respond to upstream websocket ping")?;
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {
                mark_runtime_websocket_upstream_frame_seen(
                    &mut upstream_socket,
                    &mut first_upstream_frame_seen,
                    shared
                        .runtime_config
                        .tuning
                        .websocket_precommit_progress_timeout_ms,
                )?;
            }
            Ok(WsMessage::Close(frame)) => {
                let _ = frame;
                return handle_runtime_websocket_upstream_close(
                    RuntimeWebsocketUpstreamFailureRequest {
                        request_id,
                        shared,
                        websocket_session,
                        profile_name,
                        reuse_started_at,
                        reuse_existing_session,
                        committed,
                        precommit_transport_retry_allowed,
                    },
                );
            }
            Err(WsError::ConnectionClosed) | Err(WsError::AlreadyClosed) => {
                return handle_runtime_websocket_connection_closed(
                    RuntimeWebsocketUpstreamFailureRequest {
                        request_id,
                        shared,
                        websocket_session,
                        profile_name,
                        reuse_started_at,
                        reuse_existing_session,
                        committed,
                        precommit_transport_retry_allowed,
                    },
                );
            }
            Err(err) => {
                return handle_runtime_websocket_read_error(RuntimeWebsocketReadErrorRequest {
                    request_id,
                    shared,
                    websocket_session,
                    profile_name,
                    reuse_started_at,
                    reuse_existing_session,
                    committed,
                    precommit_transport_retry_allowed,
                    precommit_started_at,
                    first_upstream_frame_seen,
                    precommit_hold_count,
                    precommit_hold_promotion_allowed,
                    precommit_hold_promotion_event_seen,
                    err,
                });
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/src/runtime_proxy/websocket.rs"]
mod tests;
