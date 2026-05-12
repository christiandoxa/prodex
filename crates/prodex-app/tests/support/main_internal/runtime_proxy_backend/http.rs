use super::*;

#[path = "http/compact.rs"]
mod compact;
#[path = "http/responses.rs"]
mod responses;
#[path = "http/write.rs"]
mod write;

use compact::*;
use responses::*;
pub(crate) use write::*;

pub(super) fn handle_runtime_proxy_backend_request(
    mut stream: TcpStream,
    responses_accounts: &Arc<Mutex<Vec<String>>>,
    responses_headers: &Arc<Mutex<Vec<BTreeMap<String, String>>>>,
    responses_bodies: &Arc<Mutex<Vec<String>>>,
    usage_accounts: &Arc<Mutex<Vec<String>>>,
    fault_script: Option<&Arc<Mutex<RuntimeProxyBackendFaultScript>>>,
    mode: RuntimeProxyBackendMode,
) {
    let request = match read_http_request(&mut stream) {
        Some(request) => request,
        None => return,
    };

    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");
    let request_body = request
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or_default();
    let account_id = request_header(&request, "ChatGPT-Account-Id").unwrap_or_default();
    let turn_state = request_header(&request, "x-codex-turn-state");
    let captured_headers = request_headers_map(&request);

    if let Some(route) = runtime_proxy_backend_fault_route_for_path(path)
        && let Some(script) = fault_script
        && let Some(response) = script
            .lock()
            .expect("runtime proxy backend fault script poisoned")
            .next_response(route, &account_id)
    {
        if route == RuntimeProxyBackendFaultRoute::Usage {
            usage_accounts
                .lock()
                .expect("usage_accounts poisoned")
                .push(account_id.clone());
        } else {
            responses_accounts
                .lock()
                .expect("responses_accounts poisoned")
                .push(account_id.clone());
            responses_headers
                .lock()
                .expect("responses_headers poisoned")
                .push(captured_headers);
            responses_bodies
                .lock()
                .expect("responses_bodies poisoned")
                .push(request_body);
        }
        write_runtime_proxy_backend_http_response(stream, response, &account_id, mode);
        return;
    }

    if path.ends_with("/backend-api/codex/responses")
        && account_id == "main-account"
        && matches!(mode, RuntimeProxyBackendMode::HttpOnlyResetBeforeFirstByte)
    {
        responses_accounts
            .lock()
            .expect("responses_accounts poisoned")
            .push(account_id.clone());
        responses_headers
            .lock()
            .expect("responses_headers poisoned")
            .push(captured_headers);
        responses_bodies
            .lock()
            .expect("responses_bodies poisoned")
            .push(request_body);
        return;
    }

    let response = if path.ends_with("/backend-api/wham/usage") {
        usage_accounts
            .lock()
            .expect("usage_accounts poisoned")
            .push(account_id.clone());
        handle_runtime_proxy_backend_usage_route(&account_id, mode)
    } else if path.ends_with("/backend-api/status") {
        responses_accounts
            .lock()
            .expect("responses_accounts poisoned")
            .push(account_id.clone());
        responses_headers
            .lock()
            .expect("responses_headers poisoned")
            .push(captured_headers);
        handle_runtime_proxy_backend_status_route(&account_id, mode)
    } else if path.ends_with("/backend-api/codex/realtime/calls") {
        responses_accounts
            .lock()
            .expect("responses_accounts poisoned")
            .push(account_id.clone());
        responses_headers
            .lock()
            .expect("responses_headers poisoned")
            .push(captured_headers);
        responses_bodies
            .lock()
            .expect("responses_bodies poisoned")
            .push(request_body.clone());
        handle_runtime_proxy_backend_realtime_route(&account_id, mode)
    } else if path.ends_with("/backend-api/codex/responses") {
        responses_accounts
            .lock()
            .expect("responses_accounts poisoned")
            .push(account_id.clone());
        responses_headers
            .lock()
            .expect("responses_headers poisoned")
            .push(captured_headers);
        responses_bodies
            .lock()
            .expect("responses_bodies poisoned")
            .push(request_body.clone());
        handle_runtime_proxy_backend_responses_route(
            &account_id,
            &request,
            &request_body,
            turn_state.as_deref(),
            mode,
        )
    } else if path.ends_with("/backend-api/codex/responses/compact") {
        responses_accounts
            .lock()
            .expect("responses_accounts poisoned")
            .push(account_id.clone());
        responses_headers
            .lock()
            .expect("responses_headers poisoned")
            .push(captured_headers);
        responses_bodies
            .lock()
            .expect("responses_bodies poisoned")
            .push(request_body);
        handle_runtime_proxy_backend_compact_route(&account_id, &request, mode)
    } else {
        RuntimeProxyBackendHttpResponse::new(
            "HTTP/1.1 404 Not Found",
            "application/json",
            serde_json::json!({ "error": "not_found" }).to_string(),
            None,
            None,
            None,
        )
    };

    write_runtime_proxy_backend_http_response(stream, response, &account_id, mode);
}

fn handle_runtime_proxy_backend_usage_route(
    account_id: &str,
    mode: RuntimeProxyBackendMode,
) -> RuntimeProxyBackendHttpResponse {
    let body = match account_id {
        "main-account"
            if matches!(
                mode,
                RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth
            ) =>
        {
            runtime_proxy_usage_body_with_remaining("main@example.com", 0, 0)
        }
        "main-account" => runtime_proxy_usage_body("main@example.com"),
        "second-account"
            if matches!(
                mode,
                RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth
            ) =>
        {
            runtime_proxy_usage_body_with_remaining("second@example.com", 0, 0)
        }
        "second-account" => runtime_proxy_usage_body("second@example.com"),
        "third-account"
            if matches!(
                mode,
                RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth
            ) =>
        {
            runtime_proxy_usage_body_with_remaining("third@example.com", 0, 0)
        }
        "third-account" => runtime_proxy_usage_body("third@example.com"),
        "fourth-account"
            if matches!(
                mode,
                RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth
            ) =>
        {
            runtime_proxy_usage_body_with_remaining("fourth@example.com", 0, 0)
        }
        "fourth-account" => runtime_proxy_usage_body("fourth@example.com"),
        "fifth-account" => runtime_proxy_usage_body("fifth@example.com"),
        _ => serde_json::json!({ "error": "unauthorized" }).to_string(),
    };
    let status = if matches!(
        account_id,
        "main-account" | "second-account" | "third-account" | "fourth-account" | "fifth-account"
    ) {
        "HTTP/1.1 200 OK"
    } else {
        "HTTP/1.1 401 Unauthorized"
    };
    RuntimeProxyBackendHttpResponse::new(status, "application/json", body, None, None, None)
}

fn handle_runtime_proxy_backend_status_route(
    account_id: &str,
    mode: RuntimeProxyBackendMode,
) -> RuntimeProxyBackendHttpResponse {
    let (status_line, body) = match (account_id, mode) {
        (
            "main-account",
            RuntimeProxyBackendMode::HttpOnlyUsageLimitMessage
            | RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth,
        ) => (
            "HTTP/1.1 429 Too Many Requests",
            serde_json::json!({
                "error": {
                    "type": "usage_limit_reached",
                    "message": "The usage limit has been reached",
                },
                "status": 429
            })
            .to_string(),
        ),
        ("main-account", RuntimeProxyBackendMode::HttpOnlyPlain429) => {
            return RuntimeProxyBackendHttpResponse::new(
                "HTTP/1.1 429 Too Many Requests",
                "text/plain",
                "Too Many Requests".to_string(),
                None,
                None,
                None,
            );
        }
        ("main-account", RuntimeProxyBackendMode::HttpOnlyUnauthorizedMain) => (
            "HTTP/1.1 401 Unauthorized",
            serde_json::json!({
                "error": "unauthorized"
            })
            .to_string(),
        ),
        ("main-account", _) => (
            "HTTP/1.1 200 OK",
            serde_json::json!({
                "status": "ok",
                "account_id": "main-account"
            })
            .to_string(),
        ),
        ("second-account", _) => (
            "HTTP/1.1 200 OK",
            serde_json::json!({
                "status": "ok",
                "account_id": "second-account"
            })
            .to_string(),
        ),
        ("third-account", _) => (
            "HTTP/1.1 200 OK",
            serde_json::json!({
                "status": "ok",
                "account_id": "third-account"
            })
            .to_string(),
        ),
        _ => (
            "HTTP/1.1 401 Unauthorized",
            serde_json::json!({ "error": "unauthorized" }).to_string(),
        ),
    };
    RuntimeProxyBackendHttpResponse::new(
        status_line,
        "application/json",
        body,
        None,
        None,
        None,
    )
}

fn handle_runtime_proxy_backend_realtime_route(
    account_id: &str,
    mode: RuntimeProxyBackendMode,
) -> RuntimeProxyBackendHttpResponse {
    let (status_line, body) = match (account_id, mode) {
        (
            "main-account",
            RuntimeProxyBackendMode::HttpOnlyUsageLimitMessage
            | RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth,
        ) => (
            "HTTP/1.1 429 Too Many Requests",
            serde_json::json!({
                "error": {
                    "type": "usage_limit_reached",
                    "message": "The usage limit has been reached",
                },
                "status": 429
            })
            .to_string(),
        ),
        ("main-account", _) => (
            "HTTP/1.1 200 OK",
            serde_json::json!({
                "status": "ok",
                "account_id": "main-account"
            })
            .to_string(),
        ),
        ("second-account", _) => (
            "HTTP/1.1 200 OK",
            serde_json::json!({
                "status": "ok",
                "account_id": "second-account"
            })
            .to_string(),
        ),
        ("third-account", _) => (
            "HTTP/1.1 200 OK",
            serde_json::json!({
                "status": "ok",
                "account_id": "third-account"
            })
            .to_string(),
        ),
        _ => (
            "HTTP/1.1 401 Unauthorized",
            serde_json::json!({ "error": "unauthorized" }).to_string(),
        ),
    };
    RuntimeProxyBackendHttpResponse::new(
        status_line,
        "application/json",
        body,
        None,
        None,
        None,
    )
}
