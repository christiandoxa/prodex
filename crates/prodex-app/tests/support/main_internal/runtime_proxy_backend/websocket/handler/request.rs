use super::*;

pub(super) fn read_backend_websocket_text_request(
    websocket: &mut tungstenite::WebSocket<TcpStream>,
) -> Option<String> {
    loop {
        match websocket.read() {
            Ok(WsMessage::Text(text)) => return Some(text.to_string()),
            Ok(WsMessage::Ping(payload)) => {
                websocket
                    .send(WsMessage::Pong(payload))
                    .expect("backend websocket pong should be sent");
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
            Ok(WsMessage::Close(_))
            | Err(WsError::ConnectionClosed)
            | Err(WsError::AlreadyClosed) => return None,
            Ok(other) => panic!("backend websocket expects text requests, got {other:?}"),
            Err(err) => panic!("backend websocket failed to read request: {err}"),
        }
    }
}
