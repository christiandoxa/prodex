use super::super::local_rewrite_gemini_compact::respond_runtime_gemini_compact_request;
use super::super::local_rewrite_kiro::runtime_kiro_compact_response_parts;
use super::super::local_rewrite_upstream::send_runtime_local_rewrite_upstream_request;
use super::*;
use crate::runtime_launch::proxy_startup::provider_bridge::{
    RuntimeProviderRouteKind, runtime_provider_route_kind,
};
use runtime_proxy_crate::path_without_query;

pub(super) enum RuntimeLocalRewriteTailOutcome {
    Handled,
    Upstream(RuntimeLocalRewriteUpstreamResult, tiny_http::Request),
}

pub(super) fn handle_runtime_local_rewrite_request_tail(
    request_id: u64,
    request: tiny_http::Request,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewriteTailOutcome {
    if matches!(
        runtime_provider_route_kind(path_without_query(&captured.path_and_query)),
        Some(RuntimeProviderRouteKind::ResponsesCompact)
    ) {
        if let RuntimeLocalRewriteProviderOptions::Gemini { auth, .. } = &shared.provider {
            respond_runtime_gemini_compact_request(request_id, request, captured, shared, auth);
            return RuntimeLocalRewriteTailOutcome::Handled;
        }
        if let RuntimeLocalRewriteProviderOptions::Kiro { auth } = &shared.provider {
            let parts = runtime_kiro_compact_response_parts(
                request_id,
                &captured.body,
                &shared.runtime_shared.async_runtime,
                auth,
            );
            let _ = request.respond(runtime_local_rewrite_response_with_call_id(
                parts, request_id, shared,
            ));
            return RuntimeLocalRewriteTailOutcome::Handled;
        }
        if !matches!(
            shared.provider,
            RuntimeLocalRewriteProviderOptions::Copilot { .. }
        ) {
            let _ = request.respond(build_runtime_proxy_text_response(
                501,
                &runtime_local_rewrite_remote_compact_unsupported_message(&shared.provider),
            ));
            return RuntimeLocalRewriteTailOutcome::Handled;
        }
    }
    if let Some(response) =
        runtime_local_rewrite_builtin_models_response(request_id, captured, shared)
    {
        let _ = request.respond(response);
        return RuntimeLocalRewriteTailOutcome::Handled;
    }
    match send_runtime_local_rewrite_upstream_request(request_id, captured, shared) {
        Ok(response) => RuntimeLocalRewriteTailOutcome::Upstream(response, request),
        Err(err) => {
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "local_rewrite_upstream_error",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field("error", format!("{err:#}")),
                    ],
                ),
            );
            let _ = request.respond(build_runtime_proxy_text_response(502, &err.to_string()));
            RuntimeLocalRewriteTailOutcome::Handled
        }
    }
}
