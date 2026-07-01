use super::{
    RuntimeRotationProxyShared, RuntimeWebsocketAttempt, RuntimeWebsocketSessionState,
    note_runtime_profile_transport_failure, runtime_proxy_log,
    runtime_proxy_websocket_precommit_progress_timeout_ms, runtime_websocket_error_log_value,
    runtime_websocket_timeout_error,
};
use anyhow::Result;
use prodex_runtime_state::RuntimeRouteKind;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::time::Instant;
use tungstenite::Error as WsError;

pub(super) struct RuntimeWebsocketUpstreamFailureRequest<'a> {
    pub(super) request_id: u64,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) websocket_session: &'a mut RuntimeWebsocketSessionState,
    pub(super) profile_name: &'a str,
    pub(super) reuse_started_at: Option<Instant>,
    pub(super) reuse_existing_session: bool,
    pub(super) committed: bool,
    pub(super) precommit_transport_retry_allowed: bool,
}

pub(super) struct RuntimeWebsocketReadErrorRequest<'a> {
    pub(super) request_id: u64,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) websocket_session: &'a mut RuntimeWebsocketSessionState,
    pub(super) profile_name: &'a str,
    pub(super) reuse_started_at: Option<Instant>,
    pub(super) reuse_existing_session: bool,
    pub(super) committed: bool,
    pub(super) precommit_transport_retry_allowed: bool,
    pub(super) precommit_started_at: Instant,
    pub(super) first_upstream_frame_seen: bool,
    pub(super) precommit_hold_count: usize,
    pub(super) precommit_hold_promotion_allowed: bool,
    pub(super) precommit_hold_promotion_event_seen: bool,
    pub(super) err: WsError,
}

pub(super) fn handle_runtime_websocket_upstream_close(
    request: RuntimeWebsocketUpstreamFailureRequest<'_>,
) -> Result<RuntimeWebsocketAttempt> {
    let RuntimeWebsocketUpstreamFailureRequest {
        request_id,
        shared,
        websocket_session,
        profile_name,
        reuse_started_at,
        reuse_existing_session,
        committed,
        precommit_transport_retry_allowed,
    } = request;

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
    Err(transport_error)
}

pub(super) fn handle_runtime_websocket_connection_closed(
    request: RuntimeWebsocketUpstreamFailureRequest<'_>,
) -> Result<RuntimeWebsocketAttempt> {
    let RuntimeWebsocketUpstreamFailureRequest {
        request_id,
        shared,
        websocket_session,
        profile_name,
        reuse_started_at,
        reuse_existing_session,
        committed,
        precommit_transport_retry_allowed,
    } = request;

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
    Err(transport_error)
}

pub(super) fn handle_runtime_websocket_read_error(
    request: RuntimeWebsocketReadErrorRequest<'_>,
) -> Result<RuntimeWebsocketAttempt> {
    let RuntimeWebsocketReadErrorRequest {
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
    } = request;

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
                    runtime_proxy_log_field("reuse", reuse_existing_session.to_string()),
                    runtime_proxy_log_field("hold_count", precommit_hold_count.to_string()),
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
    if !committed && !first_upstream_frame_seen && runtime_websocket_timeout_error(&err) {
        let elapsed_ms = precommit_started_at.elapsed().as_millis();
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "websocket_precommit_frame_timeout",
                [
                    runtime_proxy_log_field("profile", profile_name),
                    runtime_proxy_log_field("event", "no_first_upstream_frame_before_deadline"),
                    runtime_proxy_log_field("elapsed_ms", elapsed_ms.to_string()),
                    runtime_proxy_log_field("reuse", reuse_existing_session.to_string()),
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
                        runtime_proxy_log_field("event", "no_first_upstream_frame_before_deadline"),
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
                runtime_proxy_log_field(
                    "error",
                    runtime_websocket_error_log_value(&err.to_string()),
                ),
            ],
        ),
    );
    let transport_error =
        anyhow::anyhow!("runtime websocket upstream failed before response.completed: {err}");
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
    Err(transport_error)
}
