use super::super::local_rewrite::{RUNTIME_LOCAL_REWRITE_PROFILE, RuntimeLocalRewriteProxyShared};
use super::super::local_rewrite_copilot::{
    RuntimeCopilotRequestContext, RuntimeCopilotResponsesSseBindingReader,
    runtime_copilot_remember_bindings_from_responses_body,
};
use super::super::local_rewrite_response_guardrails::runtime_gateway_guardrail_stream_body;
use super::super::local_rewrite_response_spend::{
    emit_runtime_gateway_response_spend_event_for_body, runtime_gateway_spend_stream_body,
};
use super::runtime_local_rewrite_append_call_id_header;
use super::runtime_local_rewrite_buffered_response_parts;
use super::runtime_local_rewrite_response_with_call_id;
use crate::{
    RuntimeStreamingResponse, build_runtime_proxy_text_response, write_runtime_streaming_response,
};
use std::io::Read;
use std::time::Instant;

#[allow(clippy::too_many_arguments)]
pub(super) fn respond_runtime_copilot_rewrite(
    request_id: u64,
    request: tiny_http::Request,
    response: reqwest::blocking::Response,
    status: u16,
    content_type: &str,
    text_headers: Vec<(String, String)>,
    headers: Vec<(String, Vec<u8>)>,
    shared: &RuntimeLocalRewriteProxyShared,
    captured: &crate::RuntimeProxyRequest,
    copilot_context: Option<RuntimeCopilotRequestContext>,
) {
    let profile_name = copilot_context
        .as_ref()
        .map(|context| context.profile_name.clone())
        .unwrap_or_else(|| RUNTIME_LOCAL_REWRITE_PROFILE.to_string());
    let binding_recorder = copilot_context
        .as_ref()
        .and_then(|context| context.binding_recorder.clone());
    if content_type.contains("text/event-stream") {
        let writer = request.into_writer();
        let mut headers = text_headers;
        runtime_local_rewrite_append_call_id_header(&mut headers, request_id, shared);
        let body: Box<dyn Read + Send> = Box::new(RuntimeCopilotResponsesSseBindingReader::new(
            response,
            binding_recorder,
        ));
        let body = runtime_gateway_spend_stream_body(
            runtime_gateway_guardrail_stream_body(body, request_id, shared),
            request_id,
            status,
            captured,
            shared,
        );
        let streaming = RuntimeStreamingResponse {
            status,
            headers,
            body,
            request_id,
            profile_name,
            log_path: shared.runtime_shared.log_path.clone(),
            shared: shared.runtime_shared.clone(),
            _inflight_guard: None,
        };
        let _ = write_runtime_streaming_response(writer, streaming);
        return;
    }

    let response_started_at = Instant::now();
    let response = runtime_local_rewrite_buffered_response_parts(status, headers, response)
        .map(|parts| {
            runtime_copilot_remember_bindings_from_responses_body(
                binding_recorder.as_ref(),
                &parts.body,
            );
            emit_runtime_gateway_response_spend_event_for_body(
                request_id,
                captured,
                shared,
                parts.status,
                response_started_at.elapsed().as_millis(),
                parts.body.as_slice(),
            );
            runtime_local_rewrite_response_with_call_id(parts, request_id, shared)
        })
        .unwrap_or_else(|err| build_runtime_proxy_text_response(502, &err.to_string()));
    let _ = request.respond(response);
}
