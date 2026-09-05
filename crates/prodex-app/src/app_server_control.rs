use anyhow::{Context, Result, bail};
use std::io::ErrorKind;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;
use tungstenite::client::{IntoClientRequest, client_with_config};
use tungstenite::{Message, WebSocket};

pub(crate) type UnixAppServerSocket = WebSocket<UnixStream>;

const MAX_MESSAGES: usize = 64;
const MAX_MESSAGE_BYTES: usize = 64 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

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
) -> Result<Option<serde_json::Value>> {
    socket
        .send(Message::Text(
            serde_json::json!({
                "id": request_id,
                "method": method,
                "params": params,
            })
            .to_string()
            .into(),
        ))
        .context("send app-server request")?;
    for _ in 0..MAX_MESSAGES {
        let message = match socket.read() {
            Ok(message) => message,
            Err(tungstenite::Error::Io(error))
                if matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) =>
            {
                bail!("app-server request timed out")
            }
            Err(error) => return Err(error).context("read app-server response"),
        };
        match message {
            Message::Text(text) => {
                let value: serde_json::Value =
                    serde_json::from_str(text.as_ref()).context("parse app-server response")?;
                if value.get("id").and_then(serde_json::Value::as_u64) != Some(request_id) {
                    continue;
                }
                if value.get("error").is_some() {
                    bail!("Codex app-server rejected {method}")
                }
                return Ok(Some(
                    value
                        .get("result")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                ));
            }
            Message::Ping(payload) => socket
                .send(Message::Pong(payload))
                .context("reply to app-server ping")?,
            Message::Close(_) => return Ok(None),
            Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {}
        }
    }
    bail!("app-server response limit exceeded")
}
