use super::*;

struct RuntimeProxyUpstreamRequestEvents {
    route_kind: RuntimeRouteKind,
    start: &'static str,
    response: &'static str,
}

pub(super) async fn send_runtime_proxy_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
    auth: UsageAuth,
) -> Result<reqwest::Response> {
    send_runtime_proxy_upstream_request_with_events(
        request_id,
        request,
        shared,
        profile_name,
        turn_state_override,
        auth,
        RuntimeProxyUpstreamRequestEvents {
            route_kind: runtime_proxy_request_lane(&request.path_and_query, false),
            start: "upstream_start",
            response: "upstream_response",
        },
    )
    .await
}

pub(crate) async fn send_runtime_proxy_upstream_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
    auth: UsageAuth,
) -> Result<reqwest::Response> {
    send_runtime_proxy_upstream_request_with_events(
        request_id,
        request,
        shared,
        profile_name,
        turn_state_override,
        auth,
        RuntimeProxyUpstreamRequestEvents {
            route_kind: RuntimeRouteKind::Responses,
            start: "upstream_async_start",
            response: "upstream_async_response",
        },
    )
    .await
}

async fn send_runtime_proxy_upstream_request_with_events(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
    auth: UsageAuth,
    events: RuntimeProxyUpstreamRequestEvents,
) -> Result<reqwest::Response> {
    let started_at = Instant::now();
    let upstream_base_url = shared
        .lock_runtime_state()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .upstream_base_url
        .clone();
    let upstream_url = runtime_proxy_upstream_url(&upstream_base_url, &request.path_and_query);
    let log_url = runtime_proxy_log_url(&upstream_url);
    let method = reqwest::Method::from_bytes(request.method.as_bytes()).with_context(|| {
        format!(
            "failed to proxy unsupported HTTP method '{}' for runtime auto-rotate",
            request.method
        )
    })?;
    let upstream_request =
        build_runtime_proxy_upstream_request(RuntimeProxyUpstreamRequestContext {
            request_id,
            request,
            shared,
            profile_name,
            turn_state_override,
            auth,
            route_kind: events.route_kind,
            method,
            upstream_url: &upstream_url,
        })?;

    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            events.start,
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("profile", profile_name),
                runtime_proxy_log_field("method", request.method.as_str()),
                runtime_proxy_log_field("url", log_url.as_str()),
                runtime_proxy_log_field(
                    "turn_state_override",
                    if turn_state_override.is_some() {
                        "present"
                    } else {
                        "none"
                    },
                ),
                runtime_proxy_log_field(
                    "previous_response_id",
                    if runtime_request_previous_response_id(request).is_some() {
                        "present"
                    } else {
                        "none"
                    },
                ),
            ],
        ),
    );
    if runtime_take_fault_injection_budget(
        "PRODEX_RUNTIME_FAULT_UPSTREAM_CONNECT_ERROR_ONCE",
        shared.runtime_config.fault_upstream_connect_error_once,
    ) {
        bail!("injected runtime upstream connect failure");
    }
    let response = send_runtime_proxy_upstream_request_once(
        upstream_request,
        shared,
        request_id,
        profile_name,
        &log_url,
    )
    .await?;
    runtime_proxy_capture_reqwest_cookies(shared, profile_name, &response);
    log_runtime_proxy_upstream_response(
        shared,
        request_id,
        profile_name,
        events.response,
        &response,
    );
    note_runtime_profile_latency_observation(
        shared,
        profile_name,
        events.route_kind,
        "connect",
        started_at.elapsed().as_millis() as u64,
    );
    Ok(response)
}

struct RuntimeProxyUpstreamRequestContext<'a> {
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &'a RuntimeRotationProxyShared,
    profile_name: &'a str,
    turn_state_override: Option<&'a str>,
    auth: UsageAuth,
    route_kind: RuntimeRouteKind,
    method: reqwest::Method,
    upstream_url: &'a str,
}

fn build_runtime_proxy_upstream_request(
    context: RuntimeProxyUpstreamRequestContext<'_>,
) -> Result<reqwest::RequestBuilder> {
    let RuntimeProxyUpstreamRequestContext {
        request_id,
        request,
        shared,
        profile_name,
        turn_state_override,
        auth,
        route_kind,
        method,
        upstream_url,
    } = context;
    let upstream_client = if route_kind == RuntimeRouteKind::Compact {
        shared.compact_client.clone()
    } else {
        shared.async_client.clone()
    };
    let mut upstream_request = upstream_client.request(method, upstream_url);
    for (name, value) in runtime_forward_request_headers(
        request
            .headers
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str())),
    ) {
        if turn_state_override.is_some() && name.eq_ignore_ascii_case("x-codex-turn-state") {
            continue;
        }
        if name.eq_ignore_ascii_case("cookie") {
            continue;
        }
        upstream_request = upstream_request.header(name, value);
    }
    if let Some(turn_state) = turn_state_override {
        upstream_request = upstream_request.header("x-codex-turn-state", turn_state);
    }
    let upstream_body = prepare_runtime_smart_context_http_body_for_profile(
        request_id,
        request,
        shared,
        route_kind,
        Some(profile_name),
    )?;
    log_runtime_upstream_payload_snapshot(
        shared,
        request_id,
        "http",
        route_kind,
        profile_name,
        upstream_body.as_ref(),
    );
    upstream_request = upstream_request
        .header("Authorization", format!("Bearer {}", auth.access_token))
        .body(upstream_body.into_owned());
    if let Some(cookie_header) = runtime_proxy_cookie_header_for_reqwest(
        shared,
        profile_name,
        upstream_url,
        &request.headers,
    ) && let Ok(cookie_header) = reqwest::header::HeaderValue::from_str(&cookie_header)
    {
        upstream_request = upstream_request.header(reqwest::header::COOKIE, cookie_header);
    }
    if let Some(account_id) = auth.account_id.as_deref() {
        upstream_request = upstream_request.header("ChatGPT-Account-Id", account_id);
    }
    Ok(upstream_request)
}

async fn send_runtime_proxy_upstream_request_once(
    upstream_request: reqwest::RequestBuilder,
    shared: &RuntimeRotationProxyShared,
    request_id: u64,
    profile_name: &str,
    log_url: &str,
) -> Result<reqwest::Response> {
    match upstream_request.send().await {
        Ok(response) => Ok(response),
        Err(err) => {
            log_runtime_upstream_connect_failure(
                shared,
                request_id,
                "http",
                profile_name,
                runtime_transport_failure_kind_from_reqwest(&err),
                &err,
            );
            Err(anyhow::Error::new(err).context(format!(
                "failed to proxy runtime request for profile '{}' to {}",
                profile_name, log_url
            )))
        }
    }
}

fn log_runtime_proxy_upstream_response(
    shared: &RuntimeRotationProxyShared,
    request_id: u64,
    profile_name: &str,
    event: &str,
    response: &reqwest::Response,
) {
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            event,
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("profile", profile_name),
                runtime_proxy_log_field("status", response.status().as_u16().to_string()),
                runtime_proxy_log_field(
                    "content_type",
                    format!(
                        "{:?}",
                        response
                            .headers()
                            .get(reqwest::header::CONTENT_TYPE)
                            .and_then(|value| value.to_str().ok())
                    ),
                ),
                runtime_proxy_log_field(
                    "turn_state",
                    if runtime_proxy_header_value(response.headers(), "x-codex-turn-state")
                        .is_some()
                    {
                        "present"
                    } else {
                        "none"
                    },
                ),
            ],
        ),
    );
}

pub(crate) use runtime_proxy_crate::{runtime_forward_request_headers, runtime_proxy_upstream_url};

pub(super) fn runtime_proxy_upstream_websocket_url(
    base_url: &str,
    path_and_query: &str,
) -> Result<String> {
    let upstream_url = runtime_proxy_upstream_url(base_url, path_and_query);
    let log_url = runtime_proxy_log_url(&upstream_url);
    let mut url = reqwest::Url::parse(&upstream_url)
        .with_context(|| format!("failed to parse upstream websocket URL {}", log_url))?;
    match url.scheme() {
        "http" => {
            url.set_scheme("ws")
                .map_err(|_| anyhow::anyhow!("failed to set websocket scheme for {log_url}"))?;
        }
        "https" => {
            url.set_scheme("wss")
                .map_err(|_| anyhow::anyhow!("failed to set websocket scheme for {log_url}"))?;
        }
        "ws" | "wss" => {}
        scheme => bail!(
            "unsupported upstream websocket scheme '{scheme}' in {}",
            log_url
        ),
    }
    Ok(url.to_string())
}

pub(super) fn runtime_proxy_log_url(value: &str) -> String {
    if let Ok(mut url) = reqwest::Url::parse(value) {
        let _ = url.set_username("");
        let _ = url.set_password(None);
        url.set_query(None);
        url.set_fragment(None);
        return url.to_string();
    }
    value
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(value)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::runtime_proxy_log_url;

    #[test]
    fn runtime_proxy_log_url_strips_userinfo_query_and_fragment() {
        assert_eq!(
            runtime_proxy_log_url(
                "https://user:secret@example.test/backend-api/codex/responses?access_token=secret#frag"
            ),
            "https://example.test/backend-api/codex/responses"
        );
        assert_eq!(
            runtime_proxy_log_url("/backend-api/codex/responses?key=secret"),
            "/backend-api/codex/responses"
        );
    }
}
