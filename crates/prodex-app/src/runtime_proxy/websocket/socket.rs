use std::io;
use std::time::Duration;
use tiny_http::ReadWrite as TinyReadWrite;
use tungstenite::{Error as WsError, WebSocket as WsSocket, stream::MaybeTlsStream};

pub(crate) type RuntimeLocalWebSocket = WsSocket<Box<dyn TinyReadWrite + Send>>;
pub(crate) type RuntimeUpstreamWebSocket =
    WsSocket<MaybeTlsStream<crate::proxy_config::stream::ProxyStream>>;

pub(crate) fn runtime_set_upstream_websocket_io_timeout(
    socket: &mut RuntimeUpstreamWebSocket,
    timeout: Option<Duration>,
) -> io::Result<()> {
    runtime_set_upstream_websocket_read_timeout(socket, timeout)?;
    match socket.get_mut() {
        MaybeTlsStream::Plain(stream) => stream.set_write_timeout(timeout)?,
        MaybeTlsStream::Rustls(stream) => stream.sock.set_write_timeout(timeout)?,
        _ => {}
    }
    Ok(())
}

pub(crate) fn runtime_set_upstream_websocket_read_timeout(
    socket: &mut RuntimeUpstreamWebSocket,
    timeout: Option<Duration>,
) -> io::Result<()> {
    match socket.get_mut() {
        MaybeTlsStream::Plain(stream) => stream.set_read_timeout(timeout),
        MaybeTlsStream::Rustls(stream) => stream.sock.set_read_timeout(timeout),
        _ => Ok(()),
    }
}

pub(crate) fn runtime_websocket_timeout_error(err: &WsError) -> bool {
    matches!(
        err,
        WsError::Io(io_err)
            if matches!(
                io_err.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            )
    )
}
