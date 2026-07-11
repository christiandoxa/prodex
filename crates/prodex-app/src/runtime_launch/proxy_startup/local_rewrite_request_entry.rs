use super::*;

pub(super) enum RuntimeLocalRewriteEntryOutcome {
    Proceed {
        request: tiny_http::Request,
        guard: RuntimeProxyActiveRequestGuard,
    },
    Handled,
}

pub(super) fn enter_runtime_local_rewrite_request(
    request_id: u64,
    request: tiny_http::Request,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewriteEntryOutcome {
    let request_path = request.url().to_string();
    let runtime_shared = &shared.runtime_shared;
    if runtime_gateway_virtual_key_entries_is_empty(shared)
        && let Some(auth_token_hash) = shared.gateway_auth_token_hash.as_ref()
        && !runtime_gateway_request_path_is_admin(&request_path, shared)
        && !runtime_local_rewrite_request_is_authorized(&request, auth_token_hash)
    {
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_auth_rejected",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("path", request_path.as_str()),
                ],
            ),
        );
        let _ = request.respond(build_runtime_proxy_text_response(
            401,
            "missing or invalid gateway bearer token",
        ));
        return RuntimeLocalRewriteEntryOutcome::Handled;
    }
    let websocket = is_tiny_http_websocket_upgrade(&request);
    let request_transport = if websocket { "websocket" } else { "http" };
    let guard = match acquire_runtime_proxy_active_request_slot_with_wait(
        runtime_shared,
        request_transport,
        &request_path,
    ) {
        Ok(guard) => guard,
        Err(RuntimeProxyAdmissionRejection::GlobalLimit) => {
            mark_runtime_proxy_local_overload(runtime_shared, "active_request_limit");
            reject_runtime_proxy_overloaded_request(
                request,
                runtime_shared,
                "active_request_limit",
            );
            return RuntimeLocalRewriteEntryOutcome::Handled;
        }
        Err(RuntimeProxyAdmissionRejection::LaneLimit(lane)) => {
            let reason = format!("lane_limit:{}", runtime_route_kind_label(lane));
            reject_runtime_proxy_overloaded_request(request, runtime_shared, &reason);
            return RuntimeLocalRewriteEntryOutcome::Handled;
        }
    };

    if websocket {
        if matches!(
            &shared.provider,
            RuntimeLocalRewriteProviderOptions::Gemini { .. }
        ) && is_runtime_realtime_websocket_path(&request_path)
        {
            handle_runtime_gemini_live_websocket_request(request_id, request, shared);
            return RuntimeLocalRewriteEntryOutcome::Handled;
        }
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "local_rewrite_websocket_rejected",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "websocket"),
                    runtime_proxy_log_field("path", request_path),
                ],
            ),
        );
        let _ = request.respond(build_runtime_proxy_text_response(
            501,
            "runtime local rewrite proxy does not support websocket upstreams",
        ));
        return RuntimeLocalRewriteEntryOutcome::Handled;
    }

    RuntimeLocalRewriteEntryOutcome::Proceed { request, guard }
}
