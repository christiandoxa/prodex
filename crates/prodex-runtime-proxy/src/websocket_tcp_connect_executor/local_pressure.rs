use std::io;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeWebsocketLocalPressureKind {
    DnsResolveTimeout,
    DnsResolveExecutorOverflow,
    TcpConnectExecutorOverflow,
}

impl RuntimeWebsocketLocalPressureKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DnsResolveTimeout => "dns_resolve_timeout",
            Self::DnsResolveExecutorOverflow => "dns_resolve_executor_overflow",
            Self::TcpConnectExecutorOverflow => "tcp_connect_executor_overflow",
        }
    }
}

#[derive(Debug)]
struct RuntimeWebsocketLocalPressureError {
    kind: RuntimeWebsocketLocalPressureKind,
    message: String,
}

impl RuntimeWebsocketLocalPressureError {
    fn new(kind: RuntimeWebsocketLocalPressureKind, message: String) -> Self {
        Self { kind, message }
    }
}

impl std::fmt::Display for RuntimeWebsocketLocalPressureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RuntimeWebsocketLocalPressureError {}

pub fn runtime_websocket_local_pressure_io_error(
    kind: RuntimeWebsocketLocalPressureKind,
    message: impl Into<String>,
) -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        RuntimeWebsocketLocalPressureError::new(kind, message.into()),
    )
}

pub fn runtime_websocket_local_pressure_kind_from_io_error(
    err: &io::Error,
) -> Option<RuntimeWebsocketLocalPressureKind> {
    err.get_ref()
        .and_then(|source| source.downcast_ref::<RuntimeWebsocketLocalPressureError>())
        .map(|err| err.kind)
}
