use super::*;
mod commit;
mod precommit;
pub(crate) use commit::*;
pub(crate) use precommit::*;

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
    let quota_gate = runtime_precommit_quota_gate(RuntimePrecommitQuotaGateRequest {
        shared,
        profile_name,
        route_kind: RuntimeRouteKind::Websocket,
        has_continuation_context: request_previous_response_id.is_some()
            || request_session_id.is_some()
            || request_turn_state.is_some(),
        reprobe_context: "websocket_precommit_reprobe",
    })?;
    if let RuntimePrecommitQuotaGateDecision::Block {
        reason,
        summary,
        source,
    } = quota_gate
    {
        websocket_session.close();
        let reason_label = reason.as_str();
        let mut log_fields = vec![
            runtime_proxy_log_field("request", request_id.to_string()),
            runtime_proxy_log_field("transport", "websocket"),
            runtime_proxy_log_field("profile", profile_name),
            runtime_proxy_log_field("reason", reason_label),
            runtime_proxy_log_field(
                "quota_source",
                source.map(runtime_quota_source_label).unwrap_or("unknown"),
            ),
        ];
        log_fields.extend([
            runtime_proxy_log_field(
                "quota_band",
                runtime_quota_pressure_band_reason(summary.route_band),
            ),
            runtime_proxy_log_field(
                "five_hour_status",
                runtime_quota_window_status_reason(summary.five_hour.status),
            ),
            runtime_proxy_log_field(
                "five_hour_remaining",
                summary.five_hour.remaining_percent.to_string(),
            ),
            runtime_proxy_log_field("five_hour_reset_at", summary.five_hour.reset_at.to_string()),
            runtime_proxy_log_field(
                "weekly_status",
                runtime_quota_window_status_reason(summary.weekly.status),
            ),
            runtime_proxy_log_field(
                "weekly_remaining",
                summary.weekly.remaining_percent.to_string(),
            ),
            runtime_proxy_log_field("weekly_reset_at", summary.weekly.reset_at.to_string()),
        ]);
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message("websocket_pre_send_skip", log_fields),
        );
        return Ok(RuntimeWebsocketAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason: reason_label,
        });
    }

    let reuse_existing_session = websocket_session.can_reuse(profile_name, turn_state_override);
    let precommit_hold_promotion_allowed = runtime_websocket_precommit_hold_promotion_allowed(
        reuse_existing_session,
        request_previous_response_id,
        request_session_id,
        request_turn_state,
        turn_state_override,
        promote_committed_profile,
    );
    let precommit_transport_retry_allowed = runtime_websocket_precommit_transport_retry_allowed(
        reuse_existing_session,
        request_previous_response_id,
        request_turn_state,
        turn_state_override,
        promote_committed_profile,
    );
    let reuse_started_at = reuse_existing_session.then(Instant::now);
    let precommit_started_at = Instant::now();
    let (mut upstream_socket, mut upstream_turn_state, mut inflight_guard) =
        if reuse_existing_session {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "websocket_reuse_start",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "websocket"),
                        runtime_proxy_log_field("profile", profile_name),
                        runtime_proxy_log_field(
                            "turn_state_override",
                            format!("{turn_state_override:?}"),
                        ),
                    ],
                ),
            );
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=websocket upstream_session=reuse profile={profile_name} turn_state_override={:?}",
                    turn_state_override
                ),
            );
            let Some(socket) = websocket_session.take_socket() else {
                websocket_session.reset();
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "websocket_reuse_missing_socket",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("transport", "websocket"),
                            runtime_proxy_log_field("profile", profile_name),
                        ],
                    ),
                );
                return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                    profile_name: profile_name.to_string(),
                    event: "reuse_missing_socket",
                });
            };
            (socket, websocket_session.turn_state.clone(), None)
        } else {
            websocket_session.close();
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=websocket upstream_session=connect profile={profile_name} turn_state_override={:?}",
                    turn_state_override
                ),
            );
            match connect_runtime_proxy_upstream_websocket(
                request_id,
                handshake_request,
                shared,
                profile_name,
                turn_state_override,
            ) {
                Ok(RuntimeWebsocketConnectResult::Connected { socket, turn_state }) => (
                    socket,
                    turn_state,
                    Some(acquire_runtime_profile_inflight_guard(
                        shared,
                        profile_name,
                        "websocket_session",
                    )?),
                ),
                Ok(RuntimeWebsocketConnectResult::QuotaBlocked(payload)) => {
                    return Ok(RuntimeWebsocketAttempt::QuotaBlocked {
                        profile_name: profile_name.to_string(),
                        payload,
                    });
                }
                Ok(RuntimeWebsocketConnectResult::Overloaded(payload)) => {
                    return Ok(RuntimeWebsocketAttempt::Overloaded {
                        profile_name: profile_name.to_string(),
                        payload,
                    });
                }
                Err(_err) if precommit_transport_retry_allowed => {
                    return Ok(RuntimeWebsocketAttempt::TransportFailed {
                        profile_name: profile_name.to_string(),
                        stage: "connect",
                    });
                }
                Err(err) => return Err(err),
            }
        };
    runtime_set_upstream_websocket_io_timeout(
        &mut upstream_socket,
        Some(Duration::from_millis(
            runtime_proxy_websocket_precommit_progress_timeout_ms(),
        )),
    )
    .context("failed to configure runtime websocket pre-commit timeout")?;

    let upstream_request_text = prepare_runtime_smart_context_websocket_text(
        request_id,
        request_text,
        handshake_request,
        shared,
        profile_name,
    );
    if let Err(err) =
        upstream_socket.send(WsMessage::Text(upstream_request_text.into_owned().into()))
    {
        let _ = upstream_socket.close(None);
        websocket_session.reset();
        let transport_error =
            anyhow::anyhow!("failed to send runtime websocket request upstream: {err}");
        note_runtime_profile_transport_failure(
            shared,
            profile_name,
            RuntimeRouteKind::Websocket,
            "websocket_upstream_send",
            &transport_error,
        );
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "upstream_send_error",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "websocket"),
                    runtime_proxy_log_field("profile", profile_name),
                    runtime_proxy_log_field("error", err.to_string()),
                ],
            ),
        );
        if reuse_existing_session {
            return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                profile_name: profile_name.to_string(),
                event: "upstream_send_error",
            });
        }
        if precommit_transport_retry_allowed {
            return Ok(RuntimeWebsocketAttempt::TransportFailed {
                profile_name: profile_name.to_string(),
                stage: "send",
            });
        }
        return Err(transport_error);
    }

    let mut committed = false;
    let mut first_upstream_frame_seen = false;
    let mut buffered_precommit_text_frames = Vec::new();
    let mut committed_response_ids = BTreeSet::new();
    let mut previous_response_owner_recorded = false;
    let mut precommit_hold_count = 0usize;
    let mut precommit_hold_promotion_event_seen = false;
    loop {
        match upstream_socket.read() {
            Ok(WsMessage::Text(text)) => {
                let text = text.to_string();
                mark_runtime_websocket_upstream_frame_seen(
                    &mut upstream_socket,
                    &mut first_upstream_frame_seen,
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
                    if precommit_hold_count == 0 {
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
                    precommit_hold_count = precommit_hold_count.saturating_add(1);
                    precommit_hold_promotion_event_seen |=
                        runtime_websocket_precommit_hold_promotion_event_seen(&inspected);
                    buffered_precommit_text_frames.push(RuntimeBufferedWebsocketTextFrame {
                        text: text.clone(),
                        response_ids: inspected.response_ids.clone(),
                    });
                    if precommit_hold_promotion_allowed && precommit_hold_promotion_event_seen {
                        runtime_proxy_log(
                            shared,
                            runtime_proxy_structured_log_message(
                                "websocket_precommit_hold_promoted",
                                [
                                    runtime_proxy_log_field("request", request_id.to_string()),
                                    runtime_proxy_log_field("profile", profile_name),
                                    runtime_proxy_log_field("event", "response_created"),
                                    runtime_proxy_log_field(
                                        "reuse",
                                        reuse_existing_session.to_string(),
                                    ),
                                    runtime_proxy_log_field(
                                        "hold_count",
                                        precommit_hold_count.to_string(),
                                    ),
                                ],
                            ),
                        );
                        promoted_precommit_hold = true;
                    } else {
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
                    let mut dead_response_ids =
                        committed_response_ids.iter().cloned().collect::<Vec<_>>();
                    if let Some(previous_response_id) = request_previous_response_id {
                        dead_response_ids.push(previous_response_id.to_string());
                    }
                    let _ = clear_runtime_dead_response_bindings(
                        shared,
                        profile_name,
                        &dead_response_ids,
                        "previous_response_not_found_after_commit",
                    );
                    runtime_proxy_log_previous_response_stale_continuation(
                        shared,
                        RuntimePreviousResponseLogContext {
                            request_id,
                            transport: "websocket",
                            route: "websocket",
                            websocket_session: None,
                            via: None,
                        },
                        profile_name,
                    );
                    runtime_proxy_log_chain_dead_upstream_confirmed(
                        shared,
                        RuntimeProxyChainLog {
                            request_id,
                            transport: "websocket",
                            route: "websocket",
                            websocket_session: None,
                            profile_name,
                            previous_response_id: request_previous_response_id,
                            reason: "previous_response_not_found_locked_affinity",
                            via: None,
                        },
                        Some("post_commit"),
                    );
                }
                let text = if committed_previous_response_not_found {
                    runtime_translate_precommit_previous_response_websocket_text_frame(&text)
                } else {
                    runtime_translate_previous_response_websocket_text_frame(&text)
                };
                local_socket
                    .send(WsMessage::Text(text.into()))
                    .with_context(|| {
                        websocket_session.reset();
                        "failed to forward runtime websocket text frame"
                    })?;
                if inspected.terminal_event {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=websocket terminal_event profile={profile_name} event_type={} precommit_hold_count={precommit_hold_count}",
                            inspected.event_type.as_deref().unwrap_or("-"),
                        ),
                    );
                    if committed_previous_response_not_found {
                        let _ = upstream_socket.close(None);
                        websocket_session.reset();
                    } else {
                        websocket_session.store(
                            upstream_socket,
                            profile_name,
                            upstream_turn_state,
                            inflight_guard.take(),
                        );
                    }
                    return Ok(RuntimeWebsocketAttempt::Delivered);
                }
            }
            Ok(WsMessage::Binary(payload)) => {
                mark_runtime_websocket_upstream_frame_seen(
                    &mut upstream_socket,
                    &mut first_upstream_frame_seen,
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
                )?;
                upstream_socket
                    .send(WsMessage::Pong(payload))
                    .context("failed to respond to upstream websocket ping")?;
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {
                mark_runtime_websocket_upstream_frame_seen(
                    &mut upstream_socket,
                    &mut first_upstream_frame_seen,
                )?;
            }
            Ok(WsMessage::Close(frame)) => {
                websocket_session.reset();
                if let Some(started_at) = reuse_started_at {
                    runtime_proxy_log(
                        shared,
                        runtime_proxy_structured_log_message(
                            "websocket_reuse_watchdog",
                            [
                                runtime_proxy_log_field("profile", profile_name),
                                runtime_proxy_log_field("event", "upstream_close_before_terminal"),
                                runtime_proxy_log_field(
                                    "elapsed_ms",
                                    started_at.elapsed().as_millis().to_string(),
                                ),
                                runtime_proxy_log_field("committed", committed.to_string()),
                            ],
                        ),
                    );
                }
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "upstream_close_before_completed",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("transport", "websocket"),
                            runtime_proxy_log_field("profile", profile_name),
                        ],
                    ),
                );
                let _ = frame;
                let transport_error =
                    anyhow::anyhow!("runtime websocket upstream closed before response.completed");
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Websocket,
                    "websocket_upstream_close",
                    &transport_error,
                );
                if reuse_existing_session && !committed {
                    return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                        profile_name: profile_name.to_string(),
                        event: "upstream_close_before_commit",
                    });
                }
                if !committed && precommit_transport_retry_allowed {
                    return Ok(RuntimeWebsocketAttempt::TransportFailed {
                        profile_name: profile_name.to_string(),
                        stage: "upstream_close_before_commit",
                    });
                }
                return Err(transport_error);
            }
            Err(WsError::ConnectionClosed) | Err(WsError::AlreadyClosed) => {
                websocket_session.reset();
                if let Some(started_at) = reuse_started_at {
                    runtime_proxy_log(
                        shared,
                        runtime_proxy_structured_log_message(
                            "websocket_reuse_watchdog",
                            [
                                runtime_proxy_log_field("profile", profile_name),
                                runtime_proxy_log_field("event", "connection_closed"),
                                runtime_proxy_log_field(
                                    "elapsed_ms",
                                    started_at.elapsed().as_millis().to_string(),
                                ),
                                runtime_proxy_log_field("committed", committed.to_string()),
                            ],
                        ),
                    );
                }
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "upstream_connection_closed",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("transport", "websocket"),
                            runtime_proxy_log_field("profile", profile_name),
                        ],
                    ),
                );
                let transport_error =
                    anyhow::anyhow!("runtime websocket upstream closed before response.completed");
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Websocket,
                    "websocket_upstream_connection_closed",
                    &transport_error,
                );
                if reuse_existing_session && !committed {
                    return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                        profile_name: profile_name.to_string(),
                        event: "connection_closed_before_commit",
                    });
                }
                if !committed && precommit_transport_retry_allowed {
                    return Ok(RuntimeWebsocketAttempt::TransportFailed {
                        profile_name: profile_name.to_string(),
                        stage: "connection_closed_before_commit",
                    });
                }
                return Err(transport_error);
            }
            Err(err) => {
                if !committed && precommit_hold_count > 0 && runtime_websocket_timeout_error(&err) {
                    let elapsed_ms = precommit_started_at.elapsed().as_millis();
                    let timeout_ms = runtime_proxy_websocket_precommit_progress_timeout_ms();
                    runtime_proxy_log(
                        shared,
                        runtime_proxy_structured_log_message(
                            "websocket_precommit_hold_timeout",
                            [
                                runtime_proxy_log_field("request", request_id.to_string()),
                                runtime_proxy_log_field("profile", profile_name),
                                runtime_proxy_log_field("elapsed_ms", elapsed_ms.to_string()),
                                runtime_proxy_log_field("threshold_ms", timeout_ms.to_string()),
                                runtime_proxy_log_field(
                                    "reuse",
                                    reuse_existing_session.to_string(),
                                ),
                                runtime_proxy_log_field(
                                    "hold_count",
                                    precommit_hold_count.to_string(),
                                ),
                                runtime_proxy_log_field(
                                    "promotion_allowed",
                                    precommit_hold_promotion_allowed.to_string(),
                                ),
                                runtime_proxy_log_field(
                                    "promotion_event_seen",
                                    precommit_hold_promotion_event_seen.to_string(),
                                ),
                            ],
                        ),
                    );
                }
                websocket_session.reset();
                if !committed && !first_upstream_frame_seen && runtime_websocket_timeout_error(&err)
                {
                    let elapsed_ms = precommit_started_at.elapsed().as_millis();
                    runtime_proxy_log(
                        shared,
                        runtime_proxy_structured_log_message(
                            "websocket_precommit_frame_timeout",
                            [
                                runtime_proxy_log_field("profile", profile_name),
                                runtime_proxy_log_field(
                                    "event",
                                    "no_first_upstream_frame_before_deadline",
                                ),
                                runtime_proxy_log_field("elapsed_ms", elapsed_ms.to_string()),
                                runtime_proxy_log_field(
                                    "reuse",
                                    reuse_existing_session.to_string(),
                                ),
                            ],
                        ),
                    );
                    let transport_error = anyhow::anyhow!(
                        "runtime websocket upstream produced no first frame before the pre-commit deadline: {err}"
                    );
                    note_runtime_profile_transport_failure(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Websocket,
                        "websocket_first_frame_timeout",
                        &transport_error,
                    );
                    if reuse_existing_session {
                        runtime_proxy_log(
                            shared,
                            runtime_proxy_structured_log_message(
                                "websocket_reuse_watchdog",
                                [
                                    runtime_proxy_log_field("profile", profile_name),
                                    runtime_proxy_log_field(
                                        "event",
                                        "no_first_upstream_frame_before_deadline",
                                    ),
                                    runtime_proxy_log_field("elapsed_ms", elapsed_ms.to_string()),
                                    runtime_proxy_log_field("committed", committed.to_string()),
                                ],
                            ),
                        );
                        return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                            profile_name: profile_name.to_string(),
                            event: "no_first_upstream_frame_before_deadline",
                        });
                    }
                    if precommit_transport_retry_allowed {
                        return Ok(RuntimeWebsocketAttempt::TransportFailed {
                            profile_name: profile_name.to_string(),
                            stage: "first_frame_timeout",
                        });
                    }
                    return Err(transport_error);
                }
                if let Some(started_at) = reuse_started_at {
                    runtime_proxy_log(
                        shared,
                        runtime_proxy_structured_log_message(
                            "websocket_reuse_watchdog",
                            [
                                runtime_proxy_log_field("profile", profile_name),
                                runtime_proxy_log_field("event", "read_error"),
                                runtime_proxy_log_field(
                                    "elapsed_ms",
                                    started_at.elapsed().as_millis().to_string(),
                                ),
                                runtime_proxy_log_field("committed", committed.to_string()),
                            ],
                        ),
                    );
                }
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "upstream_read_error",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("transport", "websocket"),
                            runtime_proxy_log_field("profile", profile_name),
                            runtime_proxy_log_field("error", err.to_string()),
                        ],
                    ),
                );
                let transport_error = anyhow::anyhow!(
                    "runtime websocket upstream failed before response.completed: {err}"
                );
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Websocket,
                    "websocket_upstream_read",
                    &transport_error,
                );
                if reuse_existing_session && !committed {
                    return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                        profile_name: profile_name.to_string(),
                        event: "upstream_read_error",
                    });
                }
                if !committed && precommit_transport_retry_allowed {
                    return Ok(RuntimeWebsocketAttempt::TransportFailed {
                        profile_name: profile_name.to_string(),
                        stage: "read_error",
                    });
                }
                return Err(transport_error);
            }
        }
    }
}

fn mark_runtime_websocket_upstream_frame_seen(
    upstream_socket: &mut RuntimeUpstreamWebSocket,
    first_upstream_frame_seen: &mut bool,
) -> Result<()> {
    if *first_upstream_frame_seen {
        return Ok(());
    }
    *first_upstream_frame_seen = true;
    runtime_set_upstream_websocket_io_timeout(
        upstream_socket,
        Some(Duration::from_millis(
            runtime_proxy_websocket_precommit_progress_timeout_ms(),
        )),
    )
    .context("failed to restore runtime websocket upstream timeout")
}

#[cfg(test)]
#[path = "../../../tests/src/runtime_proxy/websocket.rs"]
mod tests;
