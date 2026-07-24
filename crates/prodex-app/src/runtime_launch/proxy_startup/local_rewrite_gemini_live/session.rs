//! Gemini Live websocket session pumps and translated event forwarding.

use super::super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::super::local_rewrite_application_data_plane::runtime_gateway_application_websocket_governance;
use super::super::local_rewrite_gateway_admission::{
    RUNTIME_GATEWAY_REALTIME_SESSION_MAX_MILLIS, RuntimeGatewayRealtimeAccountingPlan,
    RuntimeGatewayRealtimeUsage,
};
use super::super::local_rewrite_response_guardrails::{
    RuntimeGatewayIncrementalInspector, runtime_gateway_guardrail_websocket_block,
};
use super::GEMINI_LIVE_IDLE_SLEEP;
use super::local_rewrite_gemini_live_translation::RuntimeGeminiLiveState;
use crate::{
    RuntimeUpstreamWebSocket, WsMessage, WsSocket, runtime_proxy_log, runtime_proxy_log_field,
    runtime_proxy_structured_log_message, runtime_set_upstream_websocket_read_timeout,
};
use anyhow::{Context, Result};
use prodex_application::ApplicationResponseObligationPlan;
use prodex_provider_core::{estimate_text_tokens, gemini_provider_core_live_binary_frame_error};
use std::io::{Read, Write};
use std::thread;
use std::time::{Duration, Instant};
use tungstenite::protocol::{CloseFrame, frame::coding::CloseCode};

pub(super) fn runtime_gemini_live_session<S>(
    request_id: u64,
    local_socket: &mut WsSocket<S>,
    upstream_socket: &mut RuntimeUpstreamWebSocket,
    shared: &RuntimeLocalRewriteProxyShared,
    network_zone: prodex_domain::NetworkZone,
    authorized: Option<&prodex_application::ApplicationAuthorizedRequestContext<'_>>,
    accounting_and_usage: (
        &RuntimeGatewayRealtimeAccountingPlan,
        &mut RuntimeGatewayRealtimeUsage,
    ),
) -> Result<()>
where
    S: Read + Write,
{
    let (accounting, usage) = accounting_and_usage;
    let mut state = RuntimeGeminiLiveState::new_with_model(
        request_id,
        shared
            .runtime_shared
            .runtime_config
            .gemini
            .live_model
            .clone(),
    );
    let mut output_inspector =
        RuntimeGatewayIncrementalInspector::new(&shared.gateway_guardrails.blocked_output_keywords);
    let started_at = Instant::now();
    loop {
        if runtime_gemini_live_session_expired(started_at, usage) {
            runtime_gateway_guardrail_websocket_block(
                request_id,
                shared,
                "realtime_session_duration_limit_exceeded",
            );
            let _ = local_socket.close(Some(CloseFrame {
                code: CloseCode::Policy,
                reason: "session duration limit exceeded".into(),
            }));
            return Ok(());
        }
        match local_socket.read() {
            Ok(WsMessage::Text(text)) => {
                let inspected = super::super::local_rewrite_classification_rules::apply_runtime_gateway_classification_to_websocket_text(
                    request_id,
                    text.as_ref(),
                    shared,
                    shared.gateway_guardrails.pii_redaction,
                    authorized
                        .and_then(|authorized| authorized.tenant_context())
                        .map(|tenant| tenant.tenant_id),
                )?;
                let response_obligations = match runtime_gateway_application_websocket_governance(
                    authorized,
                    inspected.text.as_ref(),
                    shared,
                    network_zone,
                    &inspected.inspection,
                ) {
                    Ok(obligations) => obligations,
                    Err(_) => {
                        usage.policy_interrupted = true;
                        let _ = local_socket.close(Some(CloseFrame {
                            code: CloseCode::Policy,
                            reason: "request denied by policy".into(),
                        }));
                        return Ok(());
                    }
                };
                if !runtime_gemini_live_accept_input(inspected.text.as_ref(), accounting, usage) {
                    runtime_gateway_guardrail_websocket_block(
                        request_id,
                        shared,
                        "realtime_session_token_limit_exceeded",
                    );
                    let _ = local_socket.close(Some(CloseFrame {
                        code: CloseCode::Policy,
                        reason: "session token limit exceeded".into(),
                    }));
                    return Ok(());
                }
                let translated = state.translate_client_message(inspected.text.as_ref())?;
                for event in translated.local_events {
                    runtime_gemini_live_send_json(local_socket, event)?;
                }
                for message in translated.upstream_messages {
                    upstream_socket
                        .send(WsMessage::Text(message.to_string().into()))
                        .context("failed to send Gemini Live upstream message")?;
                }
                let timeout = if translated.wait_for_setup {
                    Duration::from_secs(15)
                } else if translated.wait_for_turn {
                    Duration::from_secs(60)
                } else {
                    Duration::from_millis(10)
                };
                runtime_gemini_live_drain_upstream(
                    request_id,
                    upstream_socket,
                    local_socket,
                    &mut state,
                    &mut output_inspector,
                    response_obligations,
                    accounting,
                    usage,
                    shared,
                    timeout,
                    translated.wait_for_setup,
                    translated.wait_for_turn,
                )?;
            }
            Ok(WsMessage::Ping(payload)) => {
                local_socket
                    .send(WsMessage::Pong(payload))
                    .context("failed to respond to Gemini Live local ping")?;
            }
            Ok(WsMessage::Close(frame)) => {
                let _ = upstream_socket.close(frame.clone());
                let _ = local_socket.close(frame);
                return Ok(());
            }
            Ok(WsMessage::Binary(_)) => {
                runtime_gemini_live_send_json(
                    local_socket,
                    gemini_provider_core_live_binary_frame_error(),
                )?;
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                return Ok(());
            }
            Err(err) => return Err(anyhow::anyhow!("Gemini Live local websocket failed: {err}")),
        }
        runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "local_rewrite_gemini_live_frame",
                [runtime_proxy_log_field("request", request_id.to_string())],
            ),
        );
    }
}

pub(super) fn runtime_gemini_live_duplex_session<S>(
    request_id: u64,
    local_socket: &mut WsSocket<S>,
    upstream_socket: &mut RuntimeUpstreamWebSocket,
    shared: &RuntimeLocalRewriteProxyShared,
    network_zone: prodex_domain::NetworkZone,
    authorized: Option<&prodex_application::ApplicationAuthorizedRequestContext<'_>>,
    accounting_and_usage: (
        &RuntimeGatewayRealtimeAccountingPlan,
        &mut RuntimeGatewayRealtimeUsage,
    ),
) -> Result<()>
where
    S: Read + Write,
{
    let (accounting, usage) = accounting_and_usage;
    let mut state = RuntimeGeminiLiveState::new_with_model(
        request_id,
        shared
            .runtime_shared
            .runtime_config
            .gemini
            .live_model
            .clone(),
    );
    let mut output_inspector =
        RuntimeGatewayIncrementalInspector::new(&shared.gateway_guardrails.blocked_output_keywords);
    let mut response_obligations = None;
    let started_at = Instant::now();
    loop {
        if runtime_gemini_live_session_expired(started_at, usage) {
            runtime_gateway_guardrail_websocket_block(
                request_id,
                shared,
                "realtime_session_duration_limit_exceeded",
            );
            let _ = local_socket.close(Some(CloseFrame {
                code: CloseCode::Policy,
                reason: "session duration limit exceeded".into(),
            }));
            return Ok(());
        }
        let mut progressed = false;
        match local_socket.read() {
            Ok(WsMessage::Text(text)) => {
                progressed = true;
                let inspected = super::super::local_rewrite_classification_rules::apply_runtime_gateway_classification_to_websocket_text(
                    request_id,
                    text.as_ref(),
                    shared,
                    shared.gateway_guardrails.pii_redaction,
                    authorized
                        .and_then(|authorized| authorized.tenant_context())
                        .map(|tenant| tenant.tenant_id),
                )?;
                response_obligations = match runtime_gateway_application_websocket_governance(
                    authorized,
                    inspected.text.as_ref(),
                    shared,
                    network_zone,
                    &inspected.inspection,
                ) {
                    Ok(obligations) => obligations,
                    Err(_) => {
                        usage.policy_interrupted = true;
                        let _ = local_socket.close(Some(CloseFrame {
                            code: CloseCode::Policy,
                            reason: "request denied by policy".into(),
                        }));
                        return Ok(());
                    }
                };
                if !runtime_gemini_live_accept_input(inspected.text.as_ref(), accounting, usage) {
                    runtime_gateway_guardrail_websocket_block(
                        request_id,
                        shared,
                        "realtime_session_token_limit_exceeded",
                    );
                    let _ = local_socket.close(Some(CloseFrame {
                        code: CloseCode::Policy,
                        reason: "session token limit exceeded".into(),
                    }));
                    return Ok(());
                }
                let translated = state.translate_client_message(inspected.text.as_ref())?;
                for event in translated.local_events {
                    runtime_gemini_live_send_json(local_socket, event)?;
                }
                for message in translated.upstream_messages {
                    upstream_socket
                        .send(WsMessage::Text(message.to_string().into()))
                        .context("failed to send Gemini Live upstream message")?;
                }
            }
            Ok(WsMessage::Ping(payload)) => {
                progressed = true;
                local_socket
                    .send(WsMessage::Pong(payload))
                    .context("failed to respond to Gemini Live local ping")?;
            }
            Ok(WsMessage::Close(frame)) => {
                let _ = upstream_socket.close(frame.clone());
                let _ = local_socket.close(frame);
                return Ok(());
            }
            Ok(WsMessage::Binary(_)) => {
                progressed = true;
                runtime_gemini_live_send_json(
                    local_socket,
                    gemini_provider_core_live_binary_frame_error(),
                )?;
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {
                progressed = true;
            }
            Err(err) if crate::runtime_websocket_timeout_error(&err) => {}
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                return Ok(());
            }
            Err(err) => return Err(anyhow::anyhow!("Gemini Live local websocket failed: {err}")),
        }

        match upstream_socket.read() {
            Ok(WsMessage::Text(text)) => {
                progressed = true;
                let translated = state.translate_server_message(text.as_ref())?;
                for event in translated.events {
                    if !runtime_gemini_live_send_guarded_json(
                        request_id,
                        local_socket,
                        event,
                        &mut output_inspector,
                        response_obligations,
                        (accounting, &mut *usage),
                        shared,
                    )? {
                        return Ok(());
                    }
                }
            }
            Ok(WsMessage::Ping(payload)) => {
                progressed = true;
                upstream_socket
                    .send(WsMessage::Pong(payload))
                    .context("failed to respond to Gemini Live upstream ping")?;
            }
            Ok(WsMessage::Close(frame)) => {
                let _ = local_socket.close(frame);
                return Ok(());
            }
            Ok(WsMessage::Binary(_)) | Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {
                progressed = true;
            }
            Err(err) if crate::runtime_websocket_timeout_error(&err) => {}
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                return Ok(());
            }
            Err(err) => {
                return Err(anyhow::anyhow!(
                    "Gemini Live upstream websocket failed: {err}"
                ));
            }
        }

        if !progressed {
            thread::sleep(GEMINI_LIVE_IDLE_SLEEP);
        } else {
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "local_rewrite_gemini_live_duplex_pump",
                    [runtime_proxy_log_field("request", request_id.to_string())],
                ),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn runtime_gemini_live_drain_upstream<S>(
    request_id: u64,
    upstream_socket: &mut RuntimeUpstreamWebSocket,
    local_socket: &mut WsSocket<S>,
    state: &mut RuntimeGeminiLiveState,
    output_inspector: &mut RuntimeGatewayIncrementalInspector,
    response_obligations: Option<ApplicationResponseObligationPlan>,
    accounting: &RuntimeGatewayRealtimeAccountingPlan,
    usage: &mut RuntimeGatewayRealtimeUsage,
    shared: &RuntimeLocalRewriteProxyShared,
    timeout: Duration,
    stop_on_setup: bool,
    stop_on_turn: bool,
) -> Result<()>
where
    S: Read + Write,
{
    runtime_set_upstream_websocket_read_timeout(upstream_socket, Some(timeout))
        .context("failed to set Gemini Live drain timeout")?;
    loop {
        match upstream_socket.read() {
            Ok(WsMessage::Text(text)) => {
                let translated = state.translate_server_message(text.as_ref())?;
                for event in translated.events {
                    if !runtime_gemini_live_send_guarded_json(
                        request_id,
                        local_socket,
                        event,
                        output_inspector,
                        response_obligations,
                        (accounting, &mut *usage),
                        shared,
                    )? {
                        return Ok(());
                    }
                }
                if (stop_on_setup && translated.setup_complete)
                    || (stop_on_turn && translated.turn_complete)
                {
                    return Ok(());
                }
            }
            Ok(WsMessage::Ping(payload)) => {
                upstream_socket
                    .send(WsMessage::Pong(payload))
                    .context("failed to respond to Gemini Live upstream ping")?;
            }
            Ok(WsMessage::Close(frame)) => {
                let _ = local_socket.close(frame);
                return Ok(());
            }
            Ok(WsMessage::Binary(_)) | Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
            Err(err) if crate::runtime_websocket_timeout_error(&err) => return Ok(()),
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                return Ok(());
            }
            Err(err) => {
                return Err(anyhow::anyhow!(
                    "Gemini Live upstream websocket failed: {err}"
                ));
            }
        }
    }
}

fn runtime_gemini_live_send_guarded_json<S>(
    request_id: u64,
    socket: &mut WsSocket<S>,
    value: serde_json::Value,
    inspector: &mut RuntimeGatewayIncrementalInspector,
    response_obligations: Option<ApplicationResponseObligationPlan>,
    accounting_and_usage: (
        &RuntimeGatewayRealtimeAccountingPlan,
        &mut RuntimeGatewayRealtimeUsage,
    ),
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<bool>
where
    S: Read + Write,
{
    let (accounting, usage) = accounting_and_usage;
    let text = value.to_string();
    let within_session_limit = runtime_gemini_live_observe_output(&text, accounting, usage);
    let reason = if !within_session_limit {
        Some("realtime_session_token_limit_exceeded")
    } else if inspector.inspect(text.as_bytes()) {
        Some("blocked_output_keyword")
    } else if response_obligations.is_some_and(|plan| {
        plan.enforce
            && plan
                .maximum_output_tokens
                .is_some_and(|limit| usage.output_tokens > u64::from(limit))
    }) {
        Some("output_token_limit_exceeded")
    } else {
        None
    };
    if let Some(reason) = reason {
        usage.policy_interrupted = true;
        runtime_gateway_guardrail_websocket_block(request_id, shared, reason);
        let _ = socket.close(Some(CloseFrame {
            code: CloseCode::Policy,
            reason: "response blocked by policy".into(),
        }));
        return Ok(false);
    }
    socket
        .send(WsMessage::Text(text.into()))
        .context("failed to send translated Gemini Live event")?;
    Ok(true)
}

fn runtime_gemini_live_accept_input(
    text: &str,
    accounting: &RuntimeGatewayRealtimeAccountingPlan,
    usage: &mut RuntimeGatewayRealtimeUsage,
) -> bool {
    let tokens = estimate_text_tokens(text);
    if usage
        .input_tokens
        .saturating_add(usage.output_tokens)
        .saturating_add(tokens)
        > accounting.token_limit
    {
        usage.policy_interrupted = true;
        return false;
    }
    usage.input_tokens = usage.input_tokens.saturating_add(tokens);
    usage.input_bytes = usage.input_bytes.saturating_add(text.len());
    true
}

fn runtime_gemini_live_observe_output(
    text: &str,
    accounting: &RuntimeGatewayRealtimeAccountingPlan,
    usage: &mut RuntimeGatewayRealtimeUsage,
) -> bool {
    usage.output_tokens = usage
        .output_tokens
        .saturating_add(estimate_text_tokens(text));
    usage.output_bytes = usage.output_bytes.saturating_add(text.len());
    let within_limit =
        usage.input_tokens.saturating_add(usage.output_tokens) <= accounting.token_limit;
    usage.policy_interrupted |= !within_limit;
    within_limit
}

fn runtime_gemini_live_session_expired(
    started_at: Instant,
    usage: &mut RuntimeGatewayRealtimeUsage,
) -> bool {
    let expired =
        started_at.elapsed().as_millis() >= u128::from(RUNTIME_GATEWAY_REALTIME_SESSION_MAX_MILLIS);
    usage.policy_interrupted |= expired;
    expired
}

fn runtime_gemini_live_send_json<S>(
    socket: &mut WsSocket<S>,
    value: serde_json::Value,
) -> Result<()>
where
    S: Read + Write,
{
    socket
        .send(WsMessage::Text(value.to_string().into()))
        .context("failed to send translated Gemini Live event")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accounting(token_limit: u64) -> RuntimeGatewayRealtimeAccountingPlan {
        RuntimeGatewayRealtimeAccountingPlan {
            token_limit,
            model: "test-live-model".to_string(),
            cost: prodex_provider_core::ProviderModelCost::default(),
        }
    }

    #[test]
    fn realtime_accounting_bounds_input_and_records_billable_output() {
        let mut input_usage = RuntimeGatewayRealtimeUsage::default();
        assert!(!runtime_gemini_live_accept_input(
            "abcdefgh",
            &accounting(1),
            &mut input_usage,
        ));
        assert_eq!(input_usage.input_tokens, 0);
        assert!(input_usage.policy_interrupted);

        let mut output_usage = RuntimeGatewayRealtimeUsage::default();
        assert!(!runtime_gemini_live_observe_output(
            "abcdefgh",
            &accounting(1),
            &mut output_usage,
        ));
        assert!(output_usage.output_tokens > 1);
        assert_eq!(output_usage.output_bytes, 8);
        assert!(output_usage.policy_interrupted);
    }
}
