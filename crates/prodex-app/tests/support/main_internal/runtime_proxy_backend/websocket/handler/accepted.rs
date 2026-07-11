use super::*;

pub(super) struct AcceptedWebsocket {
    pub(super) websocket: tungstenite::WebSocket<TcpStream>,
    pub(super) account_id: String,
    pub(super) request_turn_state: Option<String>,
    pub(super) captured_headers: BTreeMap<String, String>,
}

#[allow(clippy::result_large_err)]
pub(super) fn accept_runtime_proxy_backend_websocket(
    stream: TcpStream,
    mode: RuntimeProxyBackendMode,
) -> AcceptedWebsocket {
    let account_id = Arc::new(Mutex::new(String::new()));
    let request_turn_state = Arc::new(Mutex::new(None::<String>));
    let captured_headers = Arc::new(Mutex::new(BTreeMap::new()));
    let captured_account_id = Arc::clone(&account_id);
    let captured_turn_state = Arc::clone(&request_turn_state);
    let captured_headers_for_callback = Arc::clone(&captured_headers);
    let callback = move |req: &tungstenite::handshake::server::Request,
                         response: tungstenite::handshake::server::Response| {
        if let Some(value) = req
            .headers()
            .get("ChatGPT-Account-Id")
            .and_then(|value| value.to_str().ok())
        {
            *captured_account_id
                .lock()
                .expect("captured_account_id poisoned") = value.to_string();
        }
        if let Some(value) = req
            .headers()
            .get("x-codex-turn-state")
            .and_then(|value| value.to_str().ok())
        {
            *captured_turn_state
                .lock()
                .expect("captured_turn_state poisoned") = Some(value.to_string());
        }
        let mut headers = captured_headers_for_callback
            .lock()
            .expect("captured_headers poisoned");
        headers.clear();
        for (name, value) in req.headers() {
            if let Ok(value) = value.to_str() {
                headers.insert(name.as_str().to_ascii_lowercase(), value.to_string());
            }
        }
        let mut response = response;
        if matches!(
            mode,
            RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState
                | RuntimeProxyBackendMode::WebsocketReusePreviousResponseNeedsTurnState
                | RuntimeProxyBackendMode::WebsocketStaleReuseNeedsTurnState
                | RuntimeProxyBackendMode::WebsocketPreviousResponseNotFoundAfterCommit
        ) && req
            .headers()
            .get("ChatGPT-Account-Id")
            .and_then(|value| value.to_str().ok())
            == Some("second-account")
        {
            response.headers_mut().insert(
                tungstenite::http::header::HeaderName::from_static("x-codex-turn-state"),
                tungstenite::http::HeaderValue::from_static("turn-second"),
            );
        }
        Ok(response)
    };
    let websocket = tungstenite::accept_hdr(stream, callback)
        .expect("backend websocket handshake should succeed");
    let account_id = account_id.lock().expect("account_id poisoned").clone();
    let request_turn_state = request_turn_state
        .lock()
        .expect("request_turn_state poisoned")
        .clone();
    let captured_headers = captured_headers
        .lock()
        .expect("captured_headers poisoned")
        .clone();

    AcceptedWebsocket {
        websocket,
        account_id,
        request_turn_state,
        captured_headers,
    }
}

pub(super) fn runtime_proxy_backend_response_turn_state(
    mode: RuntimeProxyBackendMode,
    account_id: &str,
) -> Option<String> {
    (matches!(
        mode,
        RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState
            | RuntimeProxyBackendMode::WebsocketReusePreviousResponseNeedsTurnState
            | RuntimeProxyBackendMode::WebsocketStaleReuseNeedsTurnState
    ) && account_id == "second-account")
        .then(|| "turn-second".to_string())
}
