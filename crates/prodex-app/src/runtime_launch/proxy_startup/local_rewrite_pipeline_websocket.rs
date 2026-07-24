use super::super::local_rewrite_gemini_live::handle_runtime_gemini_live_websocket_request;
use super::{
    RuntimeLocalRewriteDispatchReadyRequest, RuntimeLocalRewritePipelineExit,
    RuntimeLocalRewritePipelineResult, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyShared, build_runtime_proxy_json_error_response,
    build_runtime_proxy_text_response, path_without_query,
    runtime_gateway_application_provider_dispatch, runtime_local_rewrite_request_timeout_response,
    runtime_proxy_log, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};
use runtime_proxy_crate::is_runtime_realtime_websocket_path;

pub(super) fn runtime_local_rewrite_dispatch_websocket<'target>(
    request: RuntimeLocalRewriteDispatchReadyRequest<'target>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewriteDispatchReadyRequest<'target>> {
    if request.state.deadline_expired() {
        return Err(request
            .state
            .reject(runtime_local_rewrite_request_timeout_response()));
    }
    if !request.state.request.is_websocket_upgrade() {
        return Ok(request);
    }
    let mut state = request.state;
    let selected_shared =
        match runtime_gateway_application_provider_dispatch(&request.application_admission, shared)
        {
            Ok(dispatch) => dispatch.selected_shared(shared),
            Err(_) => {
                return Err(state.reject(build_runtime_proxy_json_error_response(
                    503,
                    "governed_provider_unavailable",
                    "governed provider dispatch is unavailable",
                )));
            }
        };
    if matches!(
        &selected_shared.provider,
        RuntimeLocalRewriteProviderOptions::Gemini { .. }
    ) && is_runtime_realtime_websocket_path(&state.path)
    {
        handle_runtime_gemini_live_websocket_request(
            state.request_id,
            state.request,
            &selected_shared,
            state.application.as_ref(),
            request.realtime_accounting,
            state.guards.usage.take(),
        );
        return Err(RuntimeLocalRewritePipelineExit::Handled);
    }
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_websocket_https_fallback",
            [
                runtime_proxy_log_field("request", state.request_id.to_string()),
                runtime_proxy_log_field("transport", "websocket"),
                runtime_proxy_log_field("path", path_without_query(&state.path)),
            ],
        ),
    );
    Err(state.reject(build_runtime_proxy_text_response(
        501,
        "provider adapter requires the HTTPS Responses transport",
    )))
}
