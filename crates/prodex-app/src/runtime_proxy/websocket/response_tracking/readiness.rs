//! Host readiness waits must not turn a healthy socket into a timed-out one.
//! In particular, Winsock documents a connection as indeterminate after a
//! blocking SO_RCVTIMEO expiry. Read nonblocking, preserving Tungstenite/TLS
//! partial buffers, then poll readiness for the Mojo-selected wait slice.
use crate::RuntimeUpstreamWebSocket;
use std::io;
use std::net::TcpStream;
use std::time::Duration;
use tungstenite::{Error as WsError, Message as WsMessage, stream::MaybeTlsStream};

fn transport(socket: &RuntimeUpstreamWebSocket) -> io::Result<&TcpStream> {
    match socket.get_ref() {
        MaybeTlsStream::Plain(stream) => Ok(stream),
        MaybeTlsStream::Rustls(stream) => Ok(&stream.sock),
        _ => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "unsupported WebSocket readiness transport",
        )),
    }
}

pub(super) fn read_response_frame(
    socket: &mut RuntimeUpstreamWebSocket,
    wait: Duration,
) -> Result<Option<WsMessage>, WsError> {
    transport(socket)?.set_nonblocking(true)?;
    // Drain already-buffered TLS/WebSocket bytes before waiting on the OS.
    // Restore blocking mode even when a real read/protocol error occurs;
    // writes and cached socket reuse retain their bounded host I/O contract.
    let result = socket.read();
    transport(socket)?.set_nonblocking(false)?;
    match result {
        Err(WsError::Io(error)) if error.kind() == io::ErrorKind::WouldBlock => {
            wait_readable(transport(socket)?, wait)?;
            Ok(None)
        }
        Err(WsError::Io(error)) if error.kind() == io::ErrorKind::Interrupted => Ok(None),
        result => result.map(Some),
    }
}

pub(super) fn wait_response_readiness(
    socket: &RuntimeUpstreamWebSocket,
    wait: Duration,
) -> io::Result<()> {
    wait_readable(transport(socket)?, wait)
}

fn wait_readable(stream: &TcpStream, wait: Duration) -> io::Result<()> {
    let milliseconds = i32::try_from(wait.as_millis()).unwrap_or(i32::MAX);
    #[cfg(unix)]
    let result = {
        use std::os::fd::AsRawFd;
        let mut descriptor = libc::pollfd {
            fd: stream.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: the descriptor array is alive and contains exactly one
        // borrowed, open socket; poll never owns or closes that descriptor.
        let result = unsafe { libc::poll(&mut descriptor, 1, milliseconds) };
        if result < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    };
    #[cfg(windows)]
    let result = {
        use std::os::windows::io::AsRawSocket;
        use windows_sys::Win32::Networking::WinSock::{
            POLLRDNORM, WSAGetLastError, WSAPOLLFD, WSAPoll,
        };
        let mut descriptor = WSAPOLLFD {
            fd: stream.as_raw_socket() as usize,
            events: POLLRDNORM,
            revents: 0,
        };
        // SAFETY: valid borrowed SOCKET and a live one-element descriptor
        // array. Unlike SO_RCVTIMEO, a polling timeout does not abort recv.
        let status = unsafe { WSAPoll(&mut descriptor, 1, milliseconds) };
        if status < 0 {
            Err(io::Error::from_raw_os_error(unsafe { WSAGetLastError() }))
        } else {
            Ok(())
        }
    };
    #[cfg(not(any(unix, windows)))]
    let result = {
        let _ = (stream, milliseconds);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "unsupported readiness platform",
        ))
    };
    match result {
        Err(error) if error.kind() == io::ErrorKind::Interrupted => Ok(()),
        result => result,
    }
}

#[cfg(test)]
#[path = "../../../../tests/src/runtime_proxy/websocket/readiness.rs"]
mod tests;
