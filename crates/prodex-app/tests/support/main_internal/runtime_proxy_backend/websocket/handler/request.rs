use super::*;

fn backend_websocket_connection_closed(error: &WsError) -> bool {
    match error {
        WsError::ConnectionClosed | WsError::AlreadyClosed => true,
        WsError::Io(error) => matches!(
            error.kind(),
            std::io::ErrorKind::BrokenPipe
                | std::io::ErrorKind::ConnectionAborted
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::NotConnected
                | std::io::ErrorKind::UnexpectedEof
        ),
        WsError::Protocol(tungstenite::error::ProtocolError::ResetWithoutClosingHandshake) => true,
        _ => false,
    }
}

pub(super) fn read_backend_websocket_text_request(
    websocket: &mut tungstenite::WebSocket<TcpStream>,
) -> Option<String> {
    loop {
        match websocket.read() {
            Ok(WsMessage::Text(text)) => return Some(text.to_string()),
            Ok(WsMessage::Ping(payload)) => {
                if let Err(error) = websocket.send(WsMessage::Pong(payload)) {
                    if backend_websocket_connection_closed(&error) {
                        return None;
                    }
                    panic!("backend websocket pong should be sent: {error}");
                }
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
            Ok(WsMessage::Close(_))
            | Err(WsError::ConnectionClosed)
            | Err(WsError::AlreadyClosed) => return None,
            Err(error) if backend_websocket_connection_closed(&error) => return None,
            Ok(other) => panic!("backend websocket expects text requests, got {other:?}"),
            Err(err) => panic!("backend websocket failed to read request: {err}"),
        }
    }
}
