use super::*;

pub(super) fn handle_third_account(
    websocket: &mut tungstenite::WebSocket<TcpStream>,
    previous_response_id: Option<&str>,
) -> BackendWebsocketAction {
    if runtime_proxy_backend_is_owned_continuation("third-account", previous_response_id) {
        let next_response_id = runtime_proxy_backend_next_response_id(previous_response_id)
            .expect("next response id should exist");
        events::send_response_created(websocket, &next_response_id);
        events::send_response_completed(websocket, &next_response_id);
        return BackendWebsocketAction::Continue;
    }

    if previous_response_id.is_some() {
        events::send_previous_response_not_found(websocket, previous_response_id);
        return BackendWebsocketAction::Continue;
    }

    let response_id = runtime_proxy_backend_initial_response_id_for_account("third-account")
        .expect("third-account response id should exist");
    events::send_response_created(websocket, response_id);
    events::send_response_completed(websocket, response_id);
    BackendWebsocketAction::Continue
}
