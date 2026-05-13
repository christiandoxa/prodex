use super::*;

pub(super) fn handle_runtime_proxy_backend_websocket(
    stream: TcpStream,
    responses_accounts: &Arc<Mutex<Vec<String>>>,
    responses_headers: &Arc<Mutex<Vec<BTreeMap<String, String>>>>,
    websocket_requests: &Arc<Mutex<Vec<String>>>,
    mode: RuntimeProxyBackendMode,
) {
    let accepted = accepted::accept_runtime_proxy_backend_websocket(stream, mode);
    let mut websocket = accepted.websocket;
    let account_id = accepted.account_id;
    let mut effective_turn_state = accepted.request_turn_state;
    responses_accounts
        .lock()
        .expect("responses_accounts poisoned")
        .push(account_id.clone());
    responses_headers
        .lock()
        .expect("responses_headers poisoned")
        .push(accepted.captured_headers);
    let response_turn_state =
        accepted::runtime_proxy_backend_response_turn_state(mode, &account_id);
    let mut request_count = 0usize;

    loop {
        let Some(request) = request::read_backend_websocket_text_request(&mut websocket) else {
            break;
        };
        websocket_requests
            .lock()
            .expect("websocket_requests poisoned")
            .push(request.clone());
        request_count += 1;
        let previous_response_id = runtime_request_previous_response_id_from_text(&request);

        if realtime::maybe_handle_realtime_sideband(&mut websocket, mode, request_count) {
            continue;
        }

        let action = dispatch::handle_backend_websocket_request(
            &mut websocket,
            &account_id,
            mode,
            &request,
            previous_response_id.as_deref(),
            request_count,
            effective_turn_state.as_deref(),
        );
        if action == BackendWebsocketAction::Break {
            break;
        }

        if response_turn_state.is_some() {
            effective_turn_state = response_turn_state.clone();
        }
    }
}
