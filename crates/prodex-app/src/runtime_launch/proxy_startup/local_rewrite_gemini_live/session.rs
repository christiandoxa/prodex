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

mod output;

use output::{runtime_gemini_live_reject_upstream_binary, runtime_gemini_live_send_guarded_json};

const GEMINI_LIVE_BINARY_OUTPUT_UNSUPPORTED_MESSAGE: &str =
    "Gemini Live provider returned unsupported binary websocket output.";

struct RuntimeGeminiLiveProcessContext<'a, 'auth, S>
where
    S: Read + Write,
{
    request_id: u64,
    local_socket: &'a mut WsSocket<S>,
    state: &'a mut RuntimeGeminiLiveState,
    output_inspector: &'a mut RuntimeGatewayIncrementalInspector,
    accounting: &'a RuntimeGatewayRealtimeAccountingPlan,
    usage: &'a mut RuntimeGatewayRealtimeUsage,
    shared: &'a RuntimeLocalRewriteProxyShared,
    authorized: Option<&'a prodex_application::ApplicationAuthorizedRequestContext<'auth>>,
}

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
                authorized,
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
                let mut context = RuntimeGeminiLiveProcessContext {
                    request_id,
                    local_socket,
                    state: &mut state,
                    output_inspector: &mut output_inspector,
                    accounting,
                    usage,
                    shared,
                    authorized,
                };
                if runtime_gemini_live_process_session_text(
                    &mut context,
                    text.as_ref(),
                    upstream_socket,
                    network_zone,
                )? {
                    return Ok(());
                }
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

fn runtime_gemini_live_process_session_text<S>(
    context: &mut RuntimeGeminiLiveProcessContext<'_, '_, S>,
    text: &str,
    upstream_socket: &mut RuntimeUpstreamWebSocket,
    network_zone: prodex_domain::NetworkZone,
) -> Result<bool>
where
    S: Read + Write,
{
    let inspected = super::super::local_rewrite_classification_rules::apply_runtime_gateway_classification_to_websocket_text(
        context.request_id,
        text,
        context.shared,
        context.shared.gateway_guardrails.pii_redaction,
        context.authorized
            .and_then(|authorized| authorized.tenant_context())
            .map(|tenant| tenant.tenant_id),
    )?;
    let response_obligations = match runtime_gateway_application_websocket_governance(
        context.authorized,
        inspected.text.as_ref(),
        context.shared,
        network_zone,
        &inspected.inspection,
    ) {
        Ok(obligations) => obligations,
        Err(_) => {
            context.usage.policy_interrupted = true;
            let _ = context.local_socket.close(Some(CloseFrame {
                code: CloseCode::Policy,
                reason: "request denied by policy".into(),
            }));
            return Ok(true);
        }
    };
    if !runtime_gemini_live_accept_input(inspected.text.as_ref(), context.accounting, context.usage)
    {
        runtime_gateway_guardrail_websocket_block(
            context.request_id,
            context.shared,
            context.authorized,
            "realtime_session_token_limit_exceeded",
        );
        let _ = context.local_socket.close(Some(CloseFrame {
            code: CloseCode::Policy,
            reason: "session token limit exceeded".into(),
        }));
        return Ok(true);
    }
    let translated = context
        .state
        .translate_client_message(inspected.text.as_ref())?;
    for event in translated.local_events {
        runtime_gemini_live_send_json(context.local_socket, event)?;
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
        context.request_id,
        upstream_socket,
        context.local_socket,
        context.state,
        context.output_inspector,
        response_obligations,
        context.accounting,
        context.usage,
        context.shared,
        context.authorized,
        timeout,
        translated.wait_for_setup,
        translated.wait_for_turn,
    )?;
    Ok(context.usage.policy_interrupted)
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
    let mut context = RuntimeGeminiLiveProcessContext {
        request_id,
        local_socket,
        state: &mut state,
        output_inspector: &mut output_inspector,
        accounting,
        usage,
        shared,
        authorized,
    };
    let mut response_obligations = None;
    let started_at = Instant::now();
    loop {
        if runtime_gemini_live_session_expired(started_at, context.usage) {
            runtime_gateway_guardrail_websocket_block(
                context.request_id,
                context.shared,
                context.authorized,
                "realtime_session_duration_limit_exceeded",
            );
            let _ = context.local_socket.close(Some(CloseFrame {
                code: CloseCode::Policy,
                reason: "session duration limit exceeded".into(),
            }));
            return Ok(());
        }
        let mut progressed = false;
        let Some(local_progressed) = runtime_gemini_live_process_duplex_local_message(
            &mut context,
            upstream_socket,
            network_zone,
            &mut response_obligations,
        )?
        else {
            return Ok(());
        };
        progressed |= local_progressed;
        let Some(upstream_progressed) = runtime_gemini_live_process_duplex_upstream_message(
            &mut context,
            upstream_socket,
            response_obligations,
        )?
        else {
            return Ok(());
        };
        progressed |= upstream_progressed;

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

fn runtime_gemini_live_process_duplex_local_message<S>(
    context: &mut RuntimeGeminiLiveProcessContext<'_, '_, S>,
    upstream_socket: &mut RuntimeUpstreamWebSocket,
    network_zone: prodex_domain::NetworkZone,
    response_obligations: &mut Option<ApplicationResponseObligationPlan>,
) -> Result<Option<bool>>
where
    S: Read + Write,
{
    match context.local_socket.read() {
        Ok(WsMessage::Text(text)) => {
            let (stop, obligations) = runtime_gemini_live_process_duplex_text(
                context,
                text.as_ref(),
                upstream_socket,
                network_zone,
            )?;
            *response_obligations = obligations;
            if stop {
                return Ok(None);
            }
            Ok(Some(true))
        }
        Ok(WsMessage::Ping(payload)) => {
            context
                .local_socket
                .send(WsMessage::Pong(payload))
                .context("failed to respond to Gemini Live local ping")?;
            Ok(Some(true))
        }
        Ok(WsMessage::Close(frame)) => {
            let _ = upstream_socket.close(frame.clone());
            let _ = context.local_socket.close(frame);
            Ok(None)
        }
        Ok(WsMessage::Binary(_)) => {
            runtime_gemini_live_send_json(
                context.local_socket,
                gemini_provider_core_live_binary_frame_error(),
            )?;
            Ok(Some(true))
        }
        Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => Ok(Some(true)),
        Err(err) if crate::runtime_websocket_timeout_error(&err) => Ok(Some(false)),
        Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => Ok(None),
        Err(err) => Err(anyhow::anyhow!("Gemini Live local websocket failed: {err}")),
    }
}

fn runtime_gemini_live_process_duplex_text<S>(
    context: &mut RuntimeGeminiLiveProcessContext<'_, '_, S>,
    text: &str,
    upstream_socket: &mut RuntimeUpstreamWebSocket,
    network_zone: prodex_domain::NetworkZone,
) -> Result<(bool, Option<ApplicationResponseObligationPlan>)>
where
    S: Read + Write,
{
    let inspected = super::super::local_rewrite_classification_rules::apply_runtime_gateway_classification_to_websocket_text(
        context.request_id,
        text,
        context.shared,
        context.shared.gateway_guardrails.pii_redaction,
        context.authorized
            .and_then(|authorized| authorized.tenant_context())
            .map(|tenant| tenant.tenant_id),
    )?;
    let obligations = match runtime_gateway_application_websocket_governance(
        context.authorized,
        inspected.text.as_ref(),
        context.shared,
        network_zone,
        &inspected.inspection,
    ) {
        Ok(obligations) => obligations,
        Err(_) => {
            context.usage.policy_interrupted = true;
            let _ = context.local_socket.close(Some(CloseFrame {
                code: CloseCode::Policy,
                reason: "request denied by policy".into(),
            }));
            return Ok((true, None));
        }
    };
    if !runtime_gemini_live_accept_input(inspected.text.as_ref(), context.accounting, context.usage)
    {
        runtime_gateway_guardrail_websocket_block(
            context.request_id,
            context.shared,
            context.authorized,
            "realtime_session_token_limit_exceeded",
        );
        let _ = context.local_socket.close(Some(CloseFrame {
            code: CloseCode::Policy,
            reason: "session token limit exceeded".into(),
        }));
        return Ok((true, None));
    }
    let translated = context
        .state
        .translate_client_message(inspected.text.as_ref())?;
    for event in translated.local_events {
        runtime_gemini_live_send_json(context.local_socket, event)?;
    }
    for message in translated.upstream_messages {
        upstream_socket
            .send(WsMessage::Text(message.to_string().into()))
            .context("failed to send Gemini Live upstream message")?;
    }
    Ok((false, obligations))
}

fn runtime_gemini_live_process_duplex_upstream_message<S>(
    context: &mut RuntimeGeminiLiveProcessContext<'_, '_, S>,
    upstream_socket: &mut RuntimeUpstreamWebSocket,
    response_obligations: Option<ApplicationResponseObligationPlan>,
) -> Result<Option<bool>>
where
    S: Read + Write,
{
    match upstream_socket.read() {
        Ok(WsMessage::Text(text)) => {
            let translated = context.state.translate_server_message(text.as_ref())?;
            for event in translated.events {
                if !runtime_gemini_live_send_guarded_json(context, event, response_obligations)? {
                    return Ok(None);
                }
            }
            Ok(Some(true))
        }
        Ok(WsMessage::Ping(payload)) => {
            upstream_socket
                .send(WsMessage::Pong(payload))
                .context("failed to respond to Gemini Live upstream ping")?;
            Ok(Some(true))
        }
        Ok(WsMessage::Close(frame)) => {
            let _ = context.local_socket.close(frame);
            Ok(None)
        }
        Ok(WsMessage::Binary(_)) => {
            runtime_gemini_live_reject_upstream_binary(
                context.request_id,
                context.local_socket,
                context.usage,
                context.shared,
                context.authorized,
            )?;
            Ok(None)
        }
        Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => Ok(Some(true)),
        Err(err) if crate::runtime_websocket_timeout_error(&err) => Ok(Some(false)),
        Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => Ok(None),
        Err(err) => Err(anyhow::anyhow!(
            "Gemini Live upstream websocket failed: {err}"
        )),
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
    authorized: Option<&prodex_application::ApplicationAuthorizedRequestContext<'_>>,
    timeout: Duration,
    stop_on_setup: bool,
    stop_on_turn: bool,
) -> Result<()>
where
    S: Read + Write,
{
    runtime_set_upstream_websocket_read_timeout(upstream_socket, Some(timeout))
        .context("failed to set Gemini Live drain timeout")?;
    let mut context = RuntimeGeminiLiveProcessContext {
        request_id,
        local_socket,
        state,
        output_inspector,
        accounting,
        usage,
        shared,
        authorized,
    };
    loop {
        match upstream_socket.read() {
            Ok(WsMessage::Text(text)) => {
                if runtime_gemini_live_drain_text(
                    &mut context,
                    text.as_ref(),
                    response_obligations,
                    stop_on_setup,
                    stop_on_turn,
                )? {
                    return Ok(());
                }
            }
            Ok(WsMessage::Ping(payload)) => {
                upstream_socket
                    .send(WsMessage::Pong(payload))
                    .context("failed to respond to Gemini Live upstream ping")?;
            }
            Ok(WsMessage::Close(frame)) => {
                let _ = context.local_socket.close(frame);
                return Ok(());
            }
            Ok(WsMessage::Binary(_)) => {
                return runtime_gemini_live_reject_upstream_binary(
                    context.request_id,
                    context.local_socket,
                    context.usage,
                    context.shared,
                    context.authorized,
                );
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
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

fn runtime_gemini_live_drain_text<S>(
    context: &mut RuntimeGeminiLiveProcessContext<'_, '_, S>,
    text: &str,
    response_obligations: Option<ApplicationResponseObligationPlan>,
    stop_on_setup: bool,
    stop_on_turn: bool,
) -> Result<bool>
where
    S: Read + Write,
{
    let translated = context.state.translate_server_message(text)?;
    for event in translated.events {
        if !runtime_gemini_live_send_guarded_json(context, event, response_obligations)? {
            return Ok(true);
        }
    }
    Ok((stop_on_setup && translated.setup_complete) || (stop_on_turn && translated.turn_complete))
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
    use super::output::{
        runtime_gemini_live_binary_output_close_code,
        runtime_gemini_live_send_binary_output_error_and_close,
    };
    use super::*;
    use std::net::{TcpListener, TcpStream};
    use tungstenite::{connect, stream::MaybeTlsStream};

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

    #[test]
    fn upstream_binary_output_is_explicit_and_terminal() {
        let (mut bridge, mut client) = test_websocket_pair();

        runtime_gemini_live_send_binary_output_error_and_close(&mut bridge, CloseCode::Unsupported)
            .expect("unsupported output should produce a terminal protocol response");

        let WsMessage::Text(error) = client.read().expect("client should receive error") else {
            panic!("expected provider stream error");
        };
        let error: serde_json::Value =
            serde_json::from_str(error.as_ref()).expect("error should be JSON");
        assert_eq!(error["error"]["type"], "provider_stream_error");
        assert_eq!(
            error["error"]["message"],
            GEMINI_LIVE_BINARY_OUTPUT_UNSUPPORTED_MESSAGE
        );
        let WsMessage::Close(Some(frame)) = client.read().expect("client should receive close")
        else {
            panic!("expected terminal close frame");
        };
        assert_eq!(frame.code, CloseCode::Unsupported);
    }

    #[test]
    fn enforcing_mode_uses_policy_close_for_binary_output() {
        assert_eq!(
            runtime_gemini_live_binary_output_close_code(
                prodex_config::GovernanceMode::EnterpriseEnforce
            ),
            CloseCode::Policy
        );
        assert_eq!(
            runtime_gemini_live_binary_output_close_code(
                prodex_config::GovernanceMode::BankEnforce
            ),
            CloseCode::Policy
        );
        assert_eq!(
            runtime_gemini_live_binary_output_close_code(prodex_config::GovernanceMode::Personal),
            CloseCode::Unsupported
        );
    }

    fn test_websocket_pair() -> (WsSocket<TcpStream>, WsSocket<MaybeTlsStream<TcpStream>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let address = listener
            .local_addr()
            .expect("listener should expose address");
        let client = thread::spawn(move || {
            connect(format!("ws://{address}"))
                .expect("client should connect")
                .0
        });
        let (stream, _) = listener.accept().expect("server should accept client");
        let bridge = tungstenite::accept(stream).expect("server handshake should succeed");
        (bridge, client.join().expect("client thread should join"))
    }
}
