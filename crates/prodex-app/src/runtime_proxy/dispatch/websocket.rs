//! HTTP upgrade handling for runtime websocket proxy requests.

use super::super::*;

pub(crate) fn is_tiny_http_websocket_upgrade(request: &tiny_http::Request) -> bool {
    request.headers().iter().any(|header| {
        header.field.equiv("Upgrade") && header.value.as_str().eq_ignore_ascii_case("websocket")
    })
}

pub(super) fn capture_runtime_proxy_websocket_request(
    request: &tiny_http::Request,
) -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: request.method().as_str().to_string(),
        path_and_query: request.url().to_string(),
        headers: runtime_proxy_request_headers(request),
        body: Vec::new(),
    }
}

fn runtime_proxy_websocket_response_inspection_policy(
    rollout: prodex_config::GovernanceRolloutMode,
) -> Option<(&'static str, bool)> {
    match rollout {
        prodex_config::GovernanceRolloutMode::Off => None,
        prodex_config::GovernanceRolloutMode::Observe => Some(("observe", false)),
        prodex_config::GovernanceRolloutMode::Enforce => Some(("enforce", true)),
    }
}

pub(crate) fn proxy_runtime_responses_websocket_request(
    request_id: u64,
    request: tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
) {
    if !is_runtime_responses_path(request.url())
        && !is_runtime_realtime_websocket_path(request.url())
    {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "unsupported_path",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "websocket"),
                    runtime_proxy_log_field(
                        "unsupported_path",
                        runtime_proxy_log_url(request.url()),
                    ),
                ],
            ),
        );
        let _ = request.respond(build_runtime_proxy_text_response(
            404,
            "Runtime websocket proxy only supports Codex responses and realtime endpoints.",
        ));
        return;
    }

    if let Some((rollout, requires_https)) = runtime_proxy_websocket_response_inspection_policy(
        shared.runtime_config.governance.inspection,
    ) {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "native_websocket_response_inspection",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "websocket"),
                    runtime_proxy_log_field("coverage", "unsupported"),
                    runtime_proxy_log_field("rollout", rollout),
                    runtime_proxy_log_field(
                        "action",
                        if requires_https {
                            "https_fallback"
                        } else {
                            "observe"
                        },
                    ),
                ],
            ),
        );
        if requires_https {
            let _ = request.respond(build_runtime_proxy_text_response(
                501,
                "governance enforcement requires the HTTPS Responses transport",
            ));
            return;
        }
    }

    let handshake_request = capture_runtime_proxy_websocket_request(&request);
    let Some(websocket_key) = runtime_proxy_websocket_key(&handshake_request) else {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "missing_sec_websocket_key",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "websocket"),
                    runtime_proxy_log_field(
                        "path",
                        runtime_proxy_log_url(&handshake_request.path_and_query),
                    ),
                ],
            ),
        );
        let _ = request.respond(build_runtime_proxy_text_response(
            400,
            "Missing Sec-WebSocket-Key header for runtime auto-rotate websocket proxy.",
        ));
        return;
    };

    let response = match build_runtime_proxy_websocket_upgrade_response(&websocket_key) {
        Ok(response) => response,
        Err(err) => {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "websocket_upgrade_response_failed",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "websocket"),
                        runtime_proxy_log_field("error", format!("{err:#}")),
                    ],
                ),
            );
            let _ = request.respond(build_runtime_proxy_text_response(
                500,
                "Failed to build websocket upgrade response.",
            ));
            return;
        }
    };
    let upgraded = request.upgrade("websocket", response);
    let mut local_socket = WsSocket::from_raw_socket(upgraded, WsRole::Server, None);
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "upgraded",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "websocket"),
                runtime_proxy_log_field(
                    "path",
                    runtime_proxy_log_url(&handshake_request.path_and_query),
                ),
                runtime_proxy_log_field(
                    "previous_response_id",
                    if runtime_request_previous_response_id(&handshake_request).is_some() {
                        "present"
                    } else {
                        "none"
                    },
                ),
                runtime_proxy_log_field(
                    "turn_state",
                    if runtime_request_turn_state(&handshake_request).is_some() {
                        "present"
                    } else {
                        "none"
                    },
                ),
            ],
        ),
    );
    let compat_surface =
        runtime_detect_request_compatibility_surface(&handshake_request, "handshake", "websocket");
    runtime_proxy_log_request_compatibility(shared, request_id, &compat_surface);
    if let Err(err) = run_runtime_proxy_websocket_session(
        request_id,
        &mut local_socket,
        &handshake_request,
        shared,
        false,
    ) {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "session_error",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "websocket"),
                    runtime_proxy_log_field("session_error", format!("{err:#}")),
                ],
            ),
        );
        if !is_runtime_proxy_transport_failure(&err) {
            let _ = local_socket.close(None);
        }
    }
}

fn runtime_proxy_websocket_key(request: &RuntimeProxyRequest) -> Option<String> {
    request.headers.iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("Sec-WebSocket-Key")
            .then(|| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn build_runtime_proxy_websocket_upgrade_response(
    key: &str,
) -> Result<TinyResponse<std::io::Empty>> {
    let accept = derive_accept_key(key.as_bytes());
    let upgrade = TinyHeader::from_bytes("Upgrade", "websocket")
        .map_err(|err| anyhow::anyhow!("invalid websocket Upgrade header: {err:?}"))?;
    let connection = TinyHeader::from_bytes("Connection", "Upgrade")
        .map_err(|err| anyhow::anyhow!("invalid websocket Connection header: {err:?}"))?;
    let accept = TinyHeader::from_bytes("Sec-WebSocket-Accept", accept.as_bytes())
        .map_err(|err| anyhow::anyhow!("invalid websocket accept header: {err:?}"))?;
    Ok(TinyResponse::new_empty(TinyStatusCode(101))
        .with_header(upgrade)
        .with_header(connection)
        .with_header(accept))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_websocket_response_inspection_fails_closed_only_in_enforce_rollout() {
        use prodex_config::GovernanceRolloutMode::{Enforce, Observe, Off};

        assert_eq!(
            runtime_proxy_websocket_response_inspection_policy(Off),
            None
        );
        assert_eq!(
            runtime_proxy_websocket_response_inspection_policy(Observe),
            Some(("observe", false))
        );
        assert_eq!(
            runtime_proxy_websocket_response_inspection_policy(Enforce),
            Some(("enforce", true))
        );
    }
}
