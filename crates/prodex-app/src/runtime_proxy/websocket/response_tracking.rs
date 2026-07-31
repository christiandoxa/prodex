use super::*;
use bytes::Bytes;
use std::time::Instant;
mod commit;
mod duplex;
mod failure;
mod frame;
mod precommit;
mod previous_response;
mod quota_gate;
mod session;
mod terminal;
mod upstream_send;
use commit::*;
pub(in crate::runtime_proxy) use duplex::*;
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

    let realtime_websocket = websocket_session.is_realtime_duplex();
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
        upstream_turn_state,
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

    if realtime_websocket {
        let mut buffered_precommit_text_frames = Vec::new();
        let mut previous_response_owner_recorded = false;
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
            log_event: "committed_realtime",
        })?;
        websocket_session.store(
            upstream_socket,
            profile_name,
            upstream_turn_state,
            inflight_guard.take(),
        );
        return Ok(RuntimeWebsocketAttempt::Delivered);
    }

    run_runtime_websocket_response_loop(RuntimeWebsocketResponseLoop {
        request_id,
        local_socket,
        shared,
        websocket_session,
        profile_name,
        request_previous_response_id,
        request_prompt_cache_key,
        request_session_id,
        request_turn_state,
        request_model_name: request_model_name.as_deref(),
        realtime_websocket,
        upstream_socket,
        upstream_turn_state,
        inflight_guard,
        reuse_existing_session,
        precommit_hold_promotion_allowed,
        precommit_transport_retry_allowed,
        reuse_started_at,
        precommit_started_at,
        committed: false,
        first_upstream_frame_seen: false,
        buffered_precommit_text_frames: Vec::new(),
        committed_response_ids: BTreeSet::new(),
        previous_response_owner_recorded: false,
        precommit_hold_count: 0,
        precommit_hold_bytes: 0,
        precommit_hold_promotion_event_seen: false,
        promote_committed_profile,
    })
}

struct RuntimeWebsocketResponseLoop<'a> {
    request_id: u64,
    local_socket: &'a mut RuntimeLocalWebSocket,
    shared: &'a RuntimeRotationProxyShared,
    websocket_session: &'a mut RuntimeWebsocketSessionState,
    profile_name: &'a str,
    request_previous_response_id: Option<&'a str>,
    request_prompt_cache_key: Option<&'a str>,
    request_session_id: Option<&'a str>,
    request_turn_state: Option<&'a str>,
    request_model_name: Option<&'a str>,
    realtime_websocket: bool,
    upstream_socket: RuntimeUpstreamWebSocket,
    upstream_turn_state: Option<String>,
    inflight_guard: Option<RuntimeProfileInFlightGuard>,
    reuse_existing_session: bool,
    precommit_hold_promotion_allowed: bool,
    precommit_transport_retry_allowed: bool,
    reuse_started_at: Option<Instant>,
    precommit_started_at: Instant,
    promote_committed_profile: bool,
    committed: bool,
    first_upstream_frame_seen: bool,
    buffered_precommit_text_frames: Vec<RuntimeBufferedWebsocketTextFrame>,
    committed_response_ids: BTreeSet<String>,
    previous_response_owner_recorded: bool,
    precommit_hold_count: usize,
    precommit_hold_bytes: usize,
    precommit_hold_promotion_event_seen: bool,
}

struct RuntimeWebsocketTextTerminal {
    event_type: Option<String>,
    reset_upstream_socket: bool,
    committed_previous_response_not_found: bool,
}

enum RuntimeWebsocketTextResult {
    Continue,
    Terminal(RuntimeWebsocketTextTerminal),
    Attempt(RuntimeWebsocketAttempt),
}

fn run_runtime_websocket_response_loop(
    mut flow: RuntimeWebsocketResponseLoop<'_>,
) -> Result<RuntimeWebsocketAttempt> {
    loop {
        match flow.upstream_socket.read() {
            Ok(WsMessage::Text(text)) => match flow.handle_text(text.to_string())? {
                RuntimeWebsocketTextResult::Continue => {}
                RuntimeWebsocketTextResult::Terminal(terminal) => {
                    return Ok(flow.finish_terminal(terminal));
                }
                RuntimeWebsocketTextResult::Attempt(attempt) => return Ok(attempt),
            },
            Ok(WsMessage::Binary(payload)) => flow.handle_binary(payload)?,
            Ok(WsMessage::Ping(payload)) => flow.handle_ping(payload)?,
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => flow.mark_progress()?,
            Ok(WsMessage::Close(frame)) => {
                let _ = frame;
                return flow.handle_close();
            }
            Err(WsError::ConnectionClosed) | Err(WsError::AlreadyClosed) => {
                return flow.handle_connection_closed();
            }
            Err(err) => return flow.handle_read_error(err),
        }
    }
}

impl RuntimeWebsocketResponseLoop<'_> {
    fn handle_text(&mut self, text: String) -> Result<RuntimeWebsocketTextResult> {
        self.mark_text_progress()?;
        let inspected = self.inspect_text(&text)?;
        if let Some(attempt) = self.retry_attempt(&inspected, &text) {
            return Ok(RuntimeWebsocketTextResult::Attempt(attempt));
        }
        let promoted_precommit_hold = match self.buffer_precommit_hold(&inspected, &text) {
            Some(false) => return Ok(RuntimeWebsocketTextResult::Continue),
            Some(true) => true,
            None => false,
        };
        if !self.committed {
            self.commit("committed")?;
            if promoted_precommit_hold {
                return Ok(RuntimeWebsocketTextResult::Continue);
            }
        }
        let committed_previous_response_not_found = self.record_text(&inspected)?;
        self.forward_text(&text)?;
        if inspected.terminal_event {
            let reset_upstream_socket = !self.realtime_websocket
                && matches!(
                    inspected.event_type.as_deref(),
                    Some("error" | "response.failed" | "response.incomplete")
                );
            return Ok(RuntimeWebsocketTextResult::Terminal(
                RuntimeWebsocketTextTerminal {
                    event_type: inspected.event_type,
                    reset_upstream_socket,
                    committed_previous_response_not_found,
                },
            ));
        }
        Ok(RuntimeWebsocketTextResult::Continue)
    }

    fn inspect_text(
        &mut self,
        text: &str,
    ) -> Result<runtime_proxy_crate::RuntimeInspectedWebsocketTextFrame> {
        let mut inspected = inspect_runtime_websocket_text_frame_with_phase(
            text,
            if self.committed {
                runtime_proxy_crate::RuntimeHttpErrorPhase::Committed
            } else {
                runtime_proxy_crate::RuntimeHttpErrorPhase::PreCommit
            },
        );
        if self.realtime_websocket
            && inspected
                .event_type
                .as_deref()
                .is_some_and(runtime_realtime_websocket_terminal_event_kind)
        {
            inspected.terminal_event = true;
        }
        if let Some(turn_state) = inspected.turn_state.as_deref() {
            remember_runtime_turn_state(
                self.shared,
                self.profile_name,
                Some(turn_state),
                RuntimeRouteKind::Websocket,
            )?;
            self.upstream_turn_state = Some(turn_state.to_string());
        }
        Ok(inspected)
    }

    fn retry_attempt(
        &mut self,
        inspected: &runtime_proxy_crate::RuntimeInspectedWebsocketTextFrame,
        text: &str,
    ) -> Option<RuntimeWebsocketAttempt> {
        if self.committed {
            return None;
        }
        match inspected.retry_kind {
            Some(RuntimeWebsocketRetryInspectionKind::ConnectionLimitReached) => {
                runtime_proxy_log(
                    self.shared,
                    format!(
                        "request={} transport=websocket connection_limit_reached profile={}",
                        self.request_id, self.profile_name
                    ),
                );
                let _ = self.upstream_socket.close(None);
                self.websocket_session.reset();
                Some(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                    profile_name: self.profile_name.to_string(),
                    event: "connection_limit_reached",
                })
            }
            Some(RuntimeWebsocketRetryInspectionKind::QuotaBlocked) => {
                self.close_and_reset();
                Some(RuntimeWebsocketAttempt::QuotaBlocked {
                    profile_name: self.profile_name.to_string(),
                    payload: RuntimeWebsocketErrorPayload::Text(text.to_string()),
                })
            }
            Some(RuntimeWebsocketRetryInspectionKind::Overloaded) => {
                self.close_and_reset();
                Some(RuntimeWebsocketAttempt::Overloaded {
                    profile_name: self.profile_name.to_string(),
                    payload: RuntimeWebsocketErrorPayload::Text(text.to_string()),
                })
            }
            Some(RuntimeWebsocketRetryInspectionKind::PreviousResponseNotFound) => {
                self.close_and_reset();
                Some(RuntimeWebsocketAttempt::PreviousResponseNotFound {
                    profile_name: self.profile_name.to_string(),
                    payload: RuntimeWebsocketErrorPayload::Text(text.to_string()),
                    turn_state: self.upstream_turn_state.clone(),
                })
            }
            None => None,
        }
    }

    fn close_and_reset(&mut self) {
        let _ = self.upstream_socket.close(None);
        self.websocket_session.reset();
    }

    fn buffer_precommit_hold(
        &mut self,
        inspected: &runtime_proxy_crate::RuntimeInspectedWebsocketTextFrame,
        text: &str,
    ) -> Option<bool> {
        if self.committed || !inspected.precommit_hold {
            return None;
        }
        let promoted =
            runtime_websocket_buffer_precommit_hold(RuntimeWebsocketPrecommitHoldRequest {
                request_id: self.request_id,
                shared: self.shared,
                profile_name: self.profile_name,
                reuse_existing_session: self.reuse_existing_session,
                precommit_hold_promotion_allowed: self.precommit_hold_promotion_allowed,
                inspected,
                text,
                buffered_precommit_text_frames: &mut self.buffered_precommit_text_frames,
                precommit_hold_count: &mut self.precommit_hold_count,
                precommit_hold_bytes: &mut self.precommit_hold_bytes,
                precommit_hold_promotion_event_seen: &mut self.precommit_hold_promotion_event_seen,
            });
        Some(promoted)
    }

    fn commit(&mut self, log_event: &'static str) -> Result<()> {
        for frame in &self.buffered_precommit_text_frames {
            self.committed_response_ids
                .extend(frame.response_ids.iter().cloned());
        }
        commit_runtime_websocket_attempt(RuntimeWebsocketCommitRequest {
            request_id: self.request_id,
            local_socket: self.local_socket,
            upstream_socket: &mut self.upstream_socket,
            shared: self.shared,
            profile_name: self.profile_name,
            request_previous_response_id: self.request_previous_response_id,
            request_session_id: self.request_session_id,
            request_turn_state: self.request_turn_state,
            response_turn_state: self.upstream_turn_state.as_deref(),
            promote_committed_profile: self.promote_committed_profile,
            request_prompt_cache_key: self.request_prompt_cache_key,
            buffered_precommit_text_frames: &mut self.buffered_precommit_text_frames,
            previous_response_owner_recorded: &mut self.previous_response_owner_recorded,
            log_event,
        })?;
        self.committed = true;
        Ok(())
    }

    fn record_text(
        &mut self,
        inspected: &runtime_proxy_crate::RuntimeInspectedWebsocketTextFrame,
    ) -> Result<bool> {
        if !inspected.precommit_hold {
            self.committed_response_ids
                .extend(inspected.response_ids.iter().cloned());
            remember_runtime_websocket_response_ids(
                RuntimeWebsocketResponseBindingContext {
                    shared: self.shared,
                    profile_name: self.profile_name,
                    request_previous_response_id: self.request_previous_response_id,
                    request_session_id: self.request_session_id,
                    request_turn_state: self.request_turn_state,
                    response_turn_state: self.upstream_turn_state.as_deref(),
                },
                &inspected.response_ids,
                &mut self.previous_response_owner_recorded,
            )?;
        }
        if self.committed && runtime_token_usage_event_is_loggable(inspected.event_type.as_deref())
        {
            log_runtime_token_usage(RuntimeTokenUsageLog {
                shared: self.shared,
                request_id: self.request_id,
                transport: "websocket",
                profile_name: self.profile_name,
                source: "responses_websocket",
                prompt_cache_key: self.request_prompt_cache_key,
                model_name: self.request_model_name,
                usage: inspected.token_usage,
            });
        }
        let committed_previous_response_not_found = self.committed
            && matches!(
                inspected.retry_kind,
                Some(RuntimeWebsocketRetryInspectionKind::PreviousResponseNotFound)
            );
        if committed_previous_response_not_found {
            record_runtime_websocket_committed_previous_response_not_found(
                RuntimeWebsocketCommittedPreviousResponseNotFoundRequest {
                    request_id: self.request_id,
                    shared: self.shared,
                    profile_name: self.profile_name,
                    request_previous_response_id: self.request_previous_response_id,
                    committed_response_ids: &self.committed_response_ids,
                },
            );
        }
        Ok(committed_previous_response_not_found)
    }

    fn forward_text(&mut self, text: &str) -> Result<()> {
        let text = runtime_translate_previous_response_websocket_text_frame(text);
        self.local_socket
            .send(WsMessage::Text(text.into()))
            .with_context(|| {
                self.websocket_session.reset();
                "failed to forward runtime websocket text frame"
            })
    }

    fn handle_binary(&mut self, payload: Bytes) -> Result<()> {
        self.mark_progress()?;
        if !self.committed {
            self.commit("committed_binary")?;
        }
        self.local_socket
            .send(WsMessage::Binary(payload))
            .with_context(|| {
                self.websocket_session.reset();
                "failed to forward runtime websocket binary frame"
            })
    }

    fn handle_ping(&mut self, payload: Bytes) -> Result<()> {
        self.mark_progress()?;
        self.upstream_socket
            .send(WsMessage::Pong(payload))
            .context("failed to respond to upstream websocket ping")
    }

    fn mark_progress(&mut self) -> Result<()> {
        mark_runtime_websocket_upstream_frame_seen(
            &mut self.upstream_socket,
            &mut self.first_upstream_frame_seen,
            self.shared
                .runtime_config
                .tuning
                .websocket_precommit_progress_timeout_ms,
        )
    }

    fn mark_text_progress(&mut self) -> Result<()> {
        mark_runtime_websocket_upstream_frame_seen(
            &mut self.upstream_socket,
            &mut self.first_upstream_frame_seen,
            self.shared.runtime_config.tuning.stream_idle_timeout_ms,
        )
    }

    fn finish_terminal(
        mut self,
        terminal: RuntimeWebsocketTextTerminal,
    ) -> RuntimeWebsocketAttempt {
        finish_runtime_websocket_terminal_event(RuntimeWebsocketTerminalEventRequest {
            request_id: self.request_id,
            shared: self.shared,
            websocket_session: self.websocket_session,
            profile_name: self.profile_name,
            event_type: terminal.event_type.as_deref(),
            reset_upstream_socket: terminal.reset_upstream_socket,
            precommit_hold_count: self.precommit_hold_count,
            committed_previous_response_not_found: terminal.committed_previous_response_not_found,
            upstream_socket: self.upstream_socket,
            upstream_turn_state: self.upstream_turn_state,
            inflight_guard: self.inflight_guard.take(),
        })
    }

    fn handle_close(&mut self) -> Result<RuntimeWebsocketAttempt> {
        handle_runtime_websocket_upstream_close(RuntimeWebsocketUpstreamFailureRequest {
            request_id: self.request_id,
            shared: self.shared,
            websocket_session: self.websocket_session,
            profile_name: self.profile_name,
            reuse_started_at: self.reuse_started_at,
            reuse_existing_session: self.reuse_existing_session,
            committed: self.committed,
            precommit_transport_retry_allowed: self.precommit_transport_retry_allowed,
        })
    }

    fn handle_connection_closed(&mut self) -> Result<RuntimeWebsocketAttempt> {
        handle_runtime_websocket_connection_closed(RuntimeWebsocketUpstreamFailureRequest {
            request_id: self.request_id,
            shared: self.shared,
            websocket_session: self.websocket_session,
            profile_name: self.profile_name,
            reuse_started_at: self.reuse_started_at,
            reuse_existing_session: self.reuse_existing_session,
            committed: self.committed,
            precommit_transport_retry_allowed: self.precommit_transport_retry_allowed,
        })
    }

    fn handle_read_error(&mut self, err: WsError) -> Result<RuntimeWebsocketAttempt> {
        handle_runtime_websocket_read_error(RuntimeWebsocketReadErrorRequest {
            request_id: self.request_id,
            shared: self.shared,
            websocket_session: self.websocket_session,
            profile_name: self.profile_name,
            reuse_started_at: self.reuse_started_at,
            reuse_existing_session: self.reuse_existing_session,
            committed: self.committed,
            precommit_transport_retry_allowed: self.precommit_transport_retry_allowed,
            precommit_started_at: self.precommit_started_at,
            first_upstream_frame_seen: self.first_upstream_frame_seen,
            precommit_hold_count: self.precommit_hold_count,
            precommit_hold_promotion_allowed: self.precommit_hold_promotion_allowed,
            precommit_hold_promotion_event_seen: self.precommit_hold_promotion_event_seen,
            err,
        })
    }
}

#[cfg(test)]
#[path = "../../../tests/src/runtime_proxy/websocket.rs"]
mod tests;
