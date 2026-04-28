use super::*;

struct RuntimeProxyUpstreamRequestEvents {
    route_kind: RuntimeRouteKind,
    start: &'static str,
    response: &'static str,
}

pub(super) fn send_runtime_proxy_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
) -> Result<reqwest::Response> {
    send_runtime_proxy_upstream_request_with_events(
        request_id,
        request,
        shared,
        profile_name,
        turn_state_override,
        RuntimeProxyUpstreamRequestEvents {
            route_kind: runtime_proxy_request_lane(&request.path_and_query, false),
            start: "upstream_start",
            response: "upstream_response",
        },
    )
}

pub(crate) fn send_runtime_proxy_upstream_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
) -> Result<reqwest::Response> {
    send_runtime_proxy_upstream_request_with_events(
        request_id,
        request,
        shared,
        profile_name,
        turn_state_override,
        RuntimeProxyUpstreamRequestEvents {
            route_kind: RuntimeRouteKind::Responses,
            start: "upstream_async_start",
            response: "upstream_async_response",
        },
    )
}

fn send_runtime_proxy_upstream_request_with_events(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
    events: RuntimeProxyUpstreamRequestEvents,
) -> Result<reqwest::Response> {
    let started_at = Instant::now();
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .clone();
    let auth = runtime_profile_usage_auth(shared, profile_name)?;
    let upstream_url =
        runtime_proxy_upstream_url(&runtime.upstream_base_url, &request.path_and_query);
    let method = reqwest::Method::from_bytes(request.method.as_bytes()).with_context(|| {
        format!(
            "failed to proxy unsupported HTTP method '{}' for runtime auto-rotate",
            request.method
        )
    })?;

    let mut upstream_request = shared.async_client.request(method, &upstream_url);
    for (name, value) in &request.headers {
        if turn_state_override.is_some() && name.eq_ignore_ascii_case("x-codex-turn-state") {
            continue;
        }
        if name.eq_ignore_ascii_case("cookie") {
            continue;
        }
        if should_skip_runtime_request_header(name) {
            continue;
        }
        upstream_request = upstream_request.header(name.as_str(), value.as_str());
    }
    if let Some(turn_state) = turn_state_override {
        upstream_request = upstream_request.header("x-codex-turn-state", turn_state);
    }

    upstream_request = upstream_request
        .header("Authorization", format!("Bearer {}", auth.access_token))
        .body(request.body.clone());

    if let Some(cookie_header) = runtime_proxy_cookie_header_for_reqwest(
        shared,
        profile_name,
        &upstream_url,
        &request.headers,
    ) && let Ok(cookie_header) = reqwest::header::HeaderValue::from_str(&cookie_header)
    {
        upstream_request = upstream_request.header(reqwest::header::COOKIE, cookie_header);
    }

    if let Some(user_agent) = runtime_proxy_effective_user_agent(&request.headers) {
        upstream_request = upstream_request.header("User-Agent", user_agent);
    } else {
        upstream_request = upstream_request.header("User-Agent", "codex-cli");
    }

    if let Some(account_id) = auth.account_id.as_deref() {
        upstream_request = upstream_request.header("ChatGPT-Account-Id", account_id);
    }

    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            events.start,
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("profile", profile_name),
                runtime_proxy_log_field("method", request.method.as_str()),
                runtime_proxy_log_field("url", upstream_url.as_str()),
                runtime_proxy_log_field("turn_state_override", format!("{turn_state_override:?}")),
                runtime_proxy_log_field(
                    "previous_response_id",
                    format!("{:?}", runtime_request_previous_response_id(request)),
                ),
            ],
        ),
    );
    if runtime_take_fault_injection("PRODEX_RUNTIME_FAULT_UPSTREAM_CONNECT_ERROR_ONCE") {
        bail!("injected runtime upstream connect failure");
    }
    let response = match shared
        .async_runtime
        .block_on(async move { upstream_request.send().await })
    {
        Ok(response) => response,
        Err(err) => {
            log_runtime_upstream_connect_failure(
                shared,
                request_id,
                "http",
                profile_name,
                runtime_transport_failure_kind_from_reqwest(&err),
                &err,
            );
            return Err(anyhow::Error::new(err).context(format!(
                "failed to proxy runtime request for profile '{}' to {}",
                profile_name, upstream_url
            )));
        }
    };
    runtime_proxy_capture_reqwest_cookies(shared, profile_name, &response);
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            events.response,
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
                    format!(
                        "{:?}",
                        runtime_proxy_header_value(response.headers(), "x-codex-turn-state")
                    ),
                ),
            ],
        ),
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

pub(crate) fn runtime_proxy_upstream_url(base_url: &str, path_and_query: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    let normalized_path_and_query = runtime_proxy_normalize_openai_path(path_and_query);
    if base_url.contains("/backend-api")
        && let Some(suffix) = normalized_path_and_query
            .as_ref()
            .strip_prefix("/backend-api")
    {
        return format!("{base_url}{suffix}");
    }
    if normalized_path_and_query.starts_with('/') {
        return format!("{base_url}{normalized_path_and_query}");
    }
    format!("{base_url}/{normalized_path_and_query}")
}

pub(super) fn runtime_proxy_upstream_websocket_url(
    base_url: &str,
    path_and_query: &str,
) -> Result<String> {
    let upstream_url = runtime_proxy_upstream_url(base_url, path_and_query);
    let mut url = reqwest::Url::parse(&upstream_url)
        .with_context(|| format!("failed to parse upstream websocket URL {}", upstream_url))?;
    match url.scheme() {
        "http" => {
            url.set_scheme("ws").map_err(|_| {
                anyhow::anyhow!("failed to set websocket scheme for {upstream_url}")
            })?;
        }
        "https" => {
            url.set_scheme("wss").map_err(|_| {
                anyhow::anyhow!("failed to set websocket scheme for {upstream_url}")
            })?;
        }
        "ws" | "wss" => {}
        scheme => bail!(
            "unsupported upstream websocket scheme '{scheme}' in {}",
            upstream_url
        ),
    }
    Ok(url.to_string())
}

pub(super) fn should_skip_runtime_request_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "authorization"
            | "chatgpt-account-id"
            | "connection"
            | "content-length"
            | "host"
            | "transfer-encoding"
            | "upgrade"
    ) || lower.starts_with("sec-websocket-")
        || lower.starts_with("x-prodex-internal-")
}

pub(super) fn runtime_proxy_effective_user_agent(headers: &[(String, String)]) -> Option<&str> {
    headers.iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("user-agent")
            .then_some(value.as_str())
            .filter(|value| !value.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_proxy_header_skip_list_preserves_codex_metadata_headers() {
        for header in [
            "session_id",
            "x-openai-subagent",
            "x-openai-memgen-request",
            "x-codex-installation-id",
            "x-codex-turn-state",
            "x-codex-turn-metadata",
            "x-codex-parent-thread-id",
            "x-codex-window-id",
            "x-client-request-id",
            "x-codex-beta-features",
            "x-responsesapi-include-timing-metrics",
            "OpenAI-Beta",
            "User-Agent",
        ] {
            assert!(
                !should_skip_runtime_request_header(header),
                "runtime proxy should preserve upstream Codex metadata header {header}"
            );
        }
    }

    #[test]
    fn runtime_proxy_header_skip_list_replaces_auth_and_transport_headers() {
        for header in [
            "Authorization",
            "ChatGPT-Account-Id",
            "Connection",
            "Content-Length",
            "Host",
            "Transfer-Encoding",
            "Upgrade",
            "sec-websocket-key",
            "x-prodex-internal-request-origin",
        ] {
            assert!(
                should_skip_runtime_request_header(header),
                "runtime proxy should not forward local/auth header {header}"
            );
        }
    }
}
