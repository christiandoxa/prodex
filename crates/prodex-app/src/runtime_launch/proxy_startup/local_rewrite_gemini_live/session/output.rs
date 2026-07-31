use super::{
    GEMINI_LIVE_BINARY_OUTPUT_UNSUPPORTED_MESSAGE, RuntimeGeminiLiveProcessContext,
    RuntimeLocalRewriteProxyShared, runtime_gateway_guardrail_websocket_block,
    runtime_gemini_live_observe_output, runtime_gemini_live_send_json, runtime_proxy_log,
    runtime_proxy_log_field, runtime_proxy_structured_log_message,
};
use crate::{WsMessage, WsSocket};
use anyhow::{Context, Result};
use prodex_observability::InspectionStage;
use prodex_provider_core::gemini_provider_core_live_provider_stream_error;
use std::io::{Read, Write};
use tungstenite::protocol::{CloseFrame, frame::coding::CloseCode};

pub(super) fn runtime_gemini_live_send_guarded_json<S>(
    context: &mut RuntimeGeminiLiveProcessContext<'_, '_, S>,
    value: serde_json::Value,
    response_obligations: Option<super::ApplicationResponseObligationPlan>,
) -> Result<bool>
where
    S: Read + Write,
{
    let text = value.to_string();
    let within_session_limit =
        runtime_gemini_live_observe_output(&text, context.accounting, context.usage);
    let reason = if !within_session_limit {
        Some("realtime_session_token_limit_exceeded")
    } else if context.output_inspector.inspect(text.as_bytes()) {
        Some("blocked_output_keyword")
    } else if response_obligations.is_some_and(|plan| {
        plan.enforce
            && plan
                .maximum_output_tokens
                .is_some_and(|limit| context.usage.output_tokens > u64::from(limit))
    }) {
        Some("output_token_limit_exceeded")
    } else {
        None
    };
    if let Some(reason) = reason {
        context.usage.policy_interrupted = true;
        runtime_gateway_guardrail_websocket_block(
            context.request_id,
            context.shared,
            context.authorized,
            reason,
        );
        let _ = context.local_socket.close(Some(CloseFrame {
            code: CloseCode::Policy,
            reason: "response blocked by policy".into(),
        }));
        return Ok(false);
    }
    context
        .local_socket
        .send(WsMessage::Text(text.into()))
        .context("failed to send translated Gemini Live event")?;
    Ok(true)
}

pub(super) fn runtime_gemini_live_reject_upstream_binary<S>(
    request_id: u64,
    socket: &mut WsSocket<S>,
    usage: &mut super::RuntimeGatewayRealtimeUsage,
    shared: &RuntimeLocalRewriteProxyShared,
    authorized: Option<&prodex_application::ApplicationAuthorizedRequestContext<'_>>,
) -> Result<()>
where
    S: Read + Write,
{
    let mode = shared.runtime_shared.runtime_config.governance.mode;
    let enforcing = mode.is_enforcing();
    usage.policy_interrupted = true;
    crate::runtime_proxy::presidio::runtime_emit_inspection_denied_metric(
        &shared.runtime_shared,
        InspectionStage::ResponseEnforcement,
    );
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "gateway_response_inspection",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "gemini_live_websocket"),
                runtime_proxy_log_field("coverage", "unsupported"),
                runtime_proxy_log_field("outcome", "denied"),
                runtime_proxy_log_field("mode", mode.as_str()),
                runtime_proxy_log_field(
                    "action",
                    if enforcing {
                        "policy_close"
                    } else {
                        "unsupported_close"
                    },
                ),
            ],
        ),
    );
    if enforcing {
        runtime_gateway_guardrail_websocket_block(
            request_id,
            shared,
            authorized,
            "response_inspection_unsupported",
        );
    }
    runtime_gemini_live_send_binary_output_error_and_close(
        socket,
        runtime_gemini_live_binary_output_close_code(mode),
    )
}

pub(super) fn runtime_gemini_live_binary_output_close_code(
    mode: prodex_config::GovernanceMode,
) -> CloseCode {
    if mode.is_enforcing() {
        CloseCode::Policy
    } else {
        CloseCode::Unsupported
    }
}

pub(super) fn runtime_gemini_live_send_binary_output_error_and_close<S>(
    socket: &mut WsSocket<S>,
    code: CloseCode,
) -> Result<()>
where
    S: Read + Write,
{
    let send_result = runtime_gemini_live_send_json(
        socket,
        gemini_provider_core_live_provider_stream_error(
            GEMINI_LIVE_BINARY_OUTPUT_UNSUPPORTED_MESSAGE,
        ),
    );
    let close_result = socket.close(Some(CloseFrame {
        code,
        reason: "binary provider output unsupported".into(),
    }));
    send_result?;
    close_result.context("failed to close Gemini Live after unsupported binary output")
}
