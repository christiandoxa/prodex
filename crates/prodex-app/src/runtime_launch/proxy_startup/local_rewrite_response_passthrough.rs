use super::super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::super::local_rewrite_request::RuntimeLocalRewriteRequest;
use super::super::local_rewrite_response_spend::{
    emit_runtime_gateway_response_spend_event_for_body, runtime_gateway_spend_stream_body,
};
use super::respond_runtime_local_rewrite_stream;
use super::runtime_local_rewrite_append_call_id_header;
use super::runtime_local_rewrite_buffered_response_parts;
use super::runtime_local_rewrite_governed_response_with_call_id;
use super::runtime_local_rewrite_invalid_response;
use crate::{RuntimeProxyRequest, RuntimeStreamingResponse};
use prodex_application::ApplicationResponseObligationPlan;
use std::time::Instant;

#[allow(clippy::too_many_arguments)]
pub(super) fn respond_runtime_passthrough_rewrite(
    request_id: u64,
    request: RuntimeLocalRewriteRequest,
    response: reqwest::blocking::Response,
    status: u16,
    text_headers: Vec<(String, String)>,
    headers: Vec<(String, Vec<u8>)>,
    shared: &RuntimeLocalRewriteProxyShared,
    captured: &RuntimeProxyRequest,
    profile_name: String,
    stream: bool,
    response_obligations: Option<ApplicationResponseObligationPlan>,
) {
    if stream {
        let mut headers = text_headers;
        runtime_local_rewrite_append_call_id_header(&mut headers, request_id, shared);
        let body = runtime_gateway_spend_stream_body(
            Box::new(response),
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
        respond_runtime_local_rewrite_stream(request, streaming, shared, response_obligations);
        return;
    }

    let response_started_at = Instant::now();
    let response = runtime_local_rewrite_buffered_response_parts(status, headers, response)
        .map(|parts| {
            emit_runtime_gateway_response_spend_event_for_body(
                request_id,
                captured,
                shared,
                parts.status,
                response_started_at.elapsed().as_millis(),
                parts.body.as_slice(),
            );
            runtime_local_rewrite_governed_response_with_call_id(
                parts,
                request_id,
                shared,
                response_obligations,
            )
        })
        .unwrap_or_else(|err| runtime_local_rewrite_invalid_response(request_id, shared, &err));
    let _ = request.respond(response);
}
