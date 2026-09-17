use super::{
    RuntimeLocalRewriteDispatchReadyRequest, RuntimeLocalRewritePipelineResult,
    RuntimeLocalRewriteProxyShared, build_runtime_proxy_text_response, path_without_query,
    runtime_local_rewrite_request_timeout_response, runtime_proxy_log, runtime_proxy_log_field,
    runtime_proxy_structured_log_message,
};

pub(super) fn runtime_local_rewrite_dispatch_websocket(
    request: RuntimeLocalRewriteDispatchReadyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewriteDispatchReadyRequest> {
    if request.state.deadline_expired() {
        return Err(request
            .state
            .reject(runtime_local_rewrite_request_timeout_response()));
    }
    if !request.state.request.is_websocket_upgrade() {
        return Ok(request);
    }
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_websocket_https_fallback",
            [
                runtime_proxy_log_field("request", request.state.request_id.to_string()),
                runtime_proxy_log_field("transport", "websocket"),
                runtime_proxy_log_field("path", path_without_query(&request.state.path)),
            ],
        ),
    );
    Err(request.state.reject(build_runtime_proxy_text_response(
        426,
        "provider adapter requires HTTPS Responses transport fallback",
    )))
}
