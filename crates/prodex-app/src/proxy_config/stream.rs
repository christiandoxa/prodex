//! An upstream tunnel may itself use TLS before the target's TLS handshake.
//! Keep the original TCP handle available for bounded readiness and cancellation.

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::ops::Deref;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(crate) enum ProxyStream {
    Direct(TcpStream),
    Tls(Box<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>),
}

impl std::fmt::Debug for ProxyStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Direct(_) => "ProxyStream::Direct",
            Self::Tls(_) => "ProxyStream::Tls",
        })
    }
}
impl From<TcpStream> for ProxyStream {
    fn from(stream: TcpStream) -> Self {
        Self::Direct(stream)
    }
}
impl Deref for ProxyStream {
    type Target = TcpStream;
    fn deref(&self) -> &TcpStream {
        match self {
            Self::Direct(stream) => stream,
            Self::Tls(stream) => &stream.sock,
        }
    }
}
impl Read for ProxyStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Direct(stream) => stream.read(buffer),
            Self::Tls(stream) => stream.read(buffer),
        }
    }
}
impl Write for ProxyStream {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        match self {
            Self::Direct(stream) => stream.write(buffer),
            Self::Tls(stream) => stream.write(buffer),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Direct(stream) => stream.flush(),
            Self::Tls(stream) => stream.flush(),
        }
    }
}
impl ProxyStream {
    pub(crate) fn encrypt_proxy(
        self,
        host: &str,
        config: Arc<rustls::ClientConfig>,
        timeout: Duration,
    ) -> io::Result<Self> {
        let Self::Direct(mut stream) = self else {
            return Err(io::Error::other("proxy transport is already encrypted"));
        };
        let server_name = rustls::pki_types::ServerName::try_from(host.to_owned())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid HTTPS proxy host"))?;
        let mut connection = rustls::ClientConnection::new(config, server_name)
            .map_err(|_| io::Error::other("failed to initialize HTTPS proxy TLS"))?;
        let deadline = deadline(timeout)?;
        while connection.is_handshaking() {
            let left = remaining(deadline)?;
            stream.set_read_timeout(Some(left))?;
            stream.set_write_timeout(Some(left))?;
            connection.complete_io(&mut stream)?;
        }
        Ok(Self::Tls(Box::new(rustls::StreamOwned::new(
            connection, stream,
        ))))
    }
}

pub(crate) fn deadline(timeout: Duration) -> io::Result<Instant> {
    Instant::now().checked_add(timeout).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "proxy handshake timeout is too large",
        )
    })
}
fn remaining(deadline: Instant) -> io::Result<Duration> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "upstream proxy handshake timed out",
        ))
    } else {
        Ok(remaining)
    }
}

/// A slow or malformed proxy cannot reset the handshake budget with each byte.
pub(crate) struct ProxyHandshake<'a> {
    pub(crate) stream: &'a mut ProxyStream,
    pub(crate) deadline: Instant,
}
impl Read for ProxyHandshake<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.stream
            .set_read_timeout(Some(remaining(self.deadline)?))?;
        self.stream.read(bytes)
    }
}
impl Write for ProxyHandshake<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.stream
            .set_write_timeout(Some(remaining(self.deadline)?))?;
        self.stream.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stream
            .set_write_timeout(Some(remaining(self.deadline)?))?;
        self.stream.flush()
    }
}
