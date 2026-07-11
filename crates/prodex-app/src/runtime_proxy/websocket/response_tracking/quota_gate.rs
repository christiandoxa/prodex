use anyhow::Result;

use super::{
    RuntimePrecommitQuotaGateDecision, RuntimePrecommitQuotaGateRequest,
    RuntimeRotationProxyShared, RuntimeRouteKind, RuntimeWebsocketAttempt,
    RuntimeWebsocketSessionState, runtime_precommit_quota_gate, runtime_proxy_log,
    runtime_proxy_log_field, runtime_proxy_structured_log_message,
    runtime_quota_pressure_band_reason, runtime_quota_source_label,
    runtime_quota_window_status_reason,
};

pub(super) struct RuntimeWebsocketPreSendQuotaGateRequest<'a> {
    pub(super) request_id: u64,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) websocket_session: &'a mut RuntimeWebsocketSessionState,
    pub(super) profile_name: &'a str,
    pub(super) request_previous_response_id: Option<&'a str>,
    pub(super) request_session_id: Option<&'a str>,
    pub(super) request_turn_state: Option<&'a str>,
}

pub(super) fn runtime_websocket_pre_send_quota_gate(
    request: RuntimeWebsocketPreSendQuotaGateRequest<'_>,
) -> Result<Option<RuntimeWebsocketAttempt>> {
    let RuntimeWebsocketPreSendQuotaGateRequest {
        request_id,
        shared,
        websocket_session,
        profile_name,
        request_previous_response_id,
        request_session_id,
        request_turn_state,
    } = request;

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
        return Ok(Some(RuntimeWebsocketAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason: reason_label,
        }));
    }

    Ok(None)
}
