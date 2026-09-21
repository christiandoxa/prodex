use anyhow::{Context, Result, bail};
use std::io::ErrorKind;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;
use tungstenite::client::{IntoClientRequest, client_with_config};
use tungstenite::{Message, WebSocket};

pub(crate) type UnixAppServerSocket = WebSocket<UnixStream>;

const MAX_MESSAGES: usize = 64;
const MAX_MESSAGE_BYTES: usize = 512 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AppServerRequestOutcome {
    Accepted(serde_json::Value),
    Rejected,
    Ambiguous,
}

pub(crate) fn connect_unix_socket(path: &Path) -> Result<UnixAppServerSocket> {
    if !path.is_absolute() {
        bail!("app-server socket path must be absolute");
    }
    let stream = UnixStream::connect(path).context("connect to Codex app-server")?;
    stream
        .set_read_timeout(Some(REQUEST_TIMEOUT))
        .context("set app-server read timeout")?;
    stream
        .set_write_timeout(Some(REQUEST_TIMEOUT))
        .context("set app-server write timeout")?;
    let request = "ws://localhost/rpc"
        .into_client_request()
        .context("build app-server websocket request")?;
    let config = tungstenite::protocol::WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE_BYTES))
        .max_frame_size(Some(MAX_MESSAGE_BYTES));
    let (socket, _) = client_with_config(request, stream, Some(config))
        .context("handshake with Codex app-server")?;
    Ok(socket)
}

pub(crate) fn request_result(
    socket: &mut UnixAppServerSocket,
    request_id: u64,
    method: &str,
    params: serde_json::Value,
) -> AppServerRequestOutcome {
    if socket
        .send(Message::Text(
            serde_json::json!({
                "id": request_id,
                "method": method,
                "params": params,
            })
            .to_string()
            .into(),
        ))
        .is_err()
    {
        return AppServerRequestOutcome::Ambiguous;
    }
    for _ in 0..MAX_MESSAGES {
        let message = match socket.read() {
            Ok(message) => message,
            Err(tungstenite::Error::Io(error))
                if matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) =>
            {
                return AppServerRequestOutcome::Ambiguous;
            }
            Err(_) => return AppServerRequestOutcome::Ambiguous,
        };
        match message {
            Message::Text(text) => {
                let Ok(value) = serde_json::from_str::<serde_json::Value>(text.as_ref()) else {
                    return AppServerRequestOutcome::Ambiguous;
                };
                if value.get("id").and_then(serde_json::Value::as_u64) != Some(request_id) {
                    continue;
                }
                if value.get("error").is_some() {
                    return AppServerRequestOutcome::Rejected;
                }
                return AppServerRequestOutcome::Accepted(
                    value
                        .get("result")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                );
            }
            Message::Ping(payload) => {
                if socket.send(Message::Pong(payload)).is_err() {
                    return AppServerRequestOutcome::Ambiguous;
                }
            }
            Message::Close(_) => return AppServerRequestOutcome::Ambiguous,
            Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {}
        }
    }
    AppServerRequestOutcome::Ambiguous
}

#[cfg(test)]
mod tests {
    use super::{AppServerRequestOutcome, connect_unix_socket, request_result};
    use std::os::unix::net::UnixListener;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tungstenite::{Message, accept};

    fn with_server(response: Message) -> AppServerRequestOutcome {
        let root = std::env::temp_dir().join(format!(
            "prodex-app-server-control-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("control.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut socket = accept(stream).unwrap();
            let Message::Text(request) = socket.read().unwrap() else {
                panic!("expected app-server request")
            };
            let request: serde_json::Value = serde_json::from_str(request.as_ref()).unwrap();
            let id = request["id"].clone();
            let response = match response {
                Message::Text(text) => Message::Text(
                    text.to_string()
                        .replace("REQUEST_ID", id.as_u64().unwrap().to_string().as_str())
                        .into(),
                ),
                message => message,
            };
            socket.send(response).unwrap();
        });
        let mut client = connect_unix_socket(&path).unwrap();
        let outcome = request_result(
            &mut client,
            7,
            "thread/queue/add",
            serde_json::json!({"threadId": "thread"}),
        );
        server.join().unwrap();
        let _ = std::fs::remove_dir_all(root);
        outcome
    }

    #[test]
    fn explicit_json_rpc_rejection_is_not_accepted() {
        assert_eq!(
            with_server(Message::Text(
                r#"{"id":REQUEST_ID,"error":{"code":-32602,"message":"rejected"}}"#
                    .to_string()
                    .into(),
            )),
            AppServerRequestOutcome::Rejected
        );
    }

    #[test]
    fn close_after_request_is_ambiguous() {
        assert_eq!(
            with_server(Message::Close(None)),
            AppServerRequestOutcome::Ambiguous
        );
    }

    #[test]
    fn malformed_response_is_ambiguous() {
        assert_eq!(
            with_server(Message::Text("not-json".to_string().into())),
            AppServerRequestOutcome::Ambiguous
        );
    }
}
