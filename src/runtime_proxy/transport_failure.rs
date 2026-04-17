use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeTransportFailureKind {
    Dns,
    ConnectTimeout,
    ConnectRefused,
    ConnectReset,
    TlsHandshake,
    ConnectionAborted,
    BrokenPipe,
    UnexpectedEof,
    ReadTimeout,
    UpstreamClosedBeforeCommit,
    Other,
}

pub(crate) fn runtime_transport_failure_kind_label(
    kind: RuntimeTransportFailureKind,
) -> &'static str {
    match kind {
        RuntimeTransportFailureKind::Dns => "dns",
        RuntimeTransportFailureKind::ConnectTimeout => "connect_timeout",
        RuntimeTransportFailureKind::ConnectRefused => "connection_refused",
        RuntimeTransportFailureKind::ConnectReset => "connection_reset",
        RuntimeTransportFailureKind::TlsHandshake => "tls_handshake",
        RuntimeTransportFailureKind::ConnectionAborted => "connection_aborted",
        RuntimeTransportFailureKind::BrokenPipe => "broken_pipe",
        RuntimeTransportFailureKind::UnexpectedEof => "unexpected_eof",
        RuntimeTransportFailureKind::ReadTimeout => "read_timeout",
        RuntimeTransportFailureKind::UpstreamClosedBeforeCommit => "upstream_closed_before_commit",
        RuntimeTransportFailureKind::Other => "other",
    }
}

pub(crate) fn runtime_upstream_connect_failure_marker(
    failure_kind: Option<RuntimeTransportFailureKind>,
) -> &'static str {
    match failure_kind {
        Some(RuntimeTransportFailureKind::ConnectTimeout)
        | Some(RuntimeTransportFailureKind::ReadTimeout) => "upstream_connect_timeout",
        Some(RuntimeTransportFailureKind::Dns) => "upstream_connect_dns_error",
        Some(RuntimeTransportFailureKind::TlsHandshake) => "upstream_tls_handshake_error",
        _ => "upstream_connect_error",
    }
}

pub(crate) fn log_runtime_upstream_connect_failure(
    shared: &RuntimeRotationProxyShared,
    request_id: u64,
    transport: &str,
    profile_name: &str,
    failure_kind: Option<RuntimeTransportFailureKind>,
    error: &impl std::fmt::Display,
) {
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport={transport} {} profile={profile_name} class={} error={error}",
            runtime_upstream_connect_failure_marker(failure_kind),
            failure_kind
                .map(runtime_transport_failure_kind_label)
                .unwrap_or("unknown"),
        ),
    );
}

pub(crate) fn runtime_transport_failure_kind_from_message(
    message: &str,
) -> Option<RuntimeTransportFailureKind> {
    let message = message.to_ascii_lowercase();
    if message.contains("dns")
        || message.contains("failed to lookup address information")
        || message.contains("no such host")
        || message.contains("name or service not known")
    {
        Some(RuntimeTransportFailureKind::Dns)
    } else if message.contains("tls")
        || message.contains("handshake")
        || message.contains("certificate")
    {
        Some(RuntimeTransportFailureKind::TlsHandshake)
    } else if message.contains("connection refused") {
        Some(RuntimeTransportFailureKind::ConnectRefused)
    } else if message.contains("timed out") || message.contains("timeout") {
        Some(RuntimeTransportFailureKind::ConnectTimeout)
    } else if message.contains("connection reset") {
        Some(RuntimeTransportFailureKind::ConnectReset)
    } else if message.contains("broken pipe") {
        Some(RuntimeTransportFailureKind::BrokenPipe)
    } else if message.contains("unexpected eof") {
        Some(RuntimeTransportFailureKind::UnexpectedEof)
    } else if message.contains("connection aborted") {
        Some(RuntimeTransportFailureKind::ConnectionAborted)
    } else if message.contains("stream closed before response.completed")
        || message.contains("closed before response.completed")
    {
        Some(RuntimeTransportFailureKind::UpstreamClosedBeforeCommit)
    } else if message.contains("unable to connect") {
        Some(RuntimeTransportFailureKind::Other)
    } else {
        None
    }
}

pub(crate) fn runtime_transport_failure_kind_from_io_error(
    err: &io::Error,
) -> Option<RuntimeTransportFailureKind> {
    match err.kind() {
        io::ErrorKind::TimedOut => Some(RuntimeTransportFailureKind::ConnectTimeout),
        io::ErrorKind::ConnectionRefused => Some(RuntimeTransportFailureKind::ConnectRefused),
        io::ErrorKind::ConnectionReset => Some(RuntimeTransportFailureKind::ConnectReset),
        io::ErrorKind::ConnectionAborted => Some(RuntimeTransportFailureKind::ConnectionAborted),
        io::ErrorKind::BrokenPipe => Some(RuntimeTransportFailureKind::BrokenPipe),
        io::ErrorKind::UnexpectedEof => Some(RuntimeTransportFailureKind::UnexpectedEof),
        _ => runtime_transport_failure_kind_from_message(&err.to_string()),
    }
}

pub(crate) fn runtime_transport_failure_kind_from_reqwest(
    err: &reqwest::Error,
) -> Option<RuntimeTransportFailureKind> {
    if err.is_timeout() {
        return Some(RuntimeTransportFailureKind::ReadTimeout);
    }
    std::error::Error::source(err)
        .and_then(|source| source.downcast_ref::<io::Error>())
        .and_then(runtime_transport_failure_kind_from_io_error)
        .or_else(|| runtime_transport_failure_kind_from_message(&err.to_string()))
}

pub(crate) fn runtime_transport_failure_kind_from_ws(
    err: &WsError,
) -> Option<RuntimeTransportFailureKind> {
    match err {
        WsError::Io(io) => runtime_transport_failure_kind_from_io_error(io),
        WsError::Tls(_) => Some(RuntimeTransportFailureKind::TlsHandshake),
        WsError::ConnectionClosed | WsError::AlreadyClosed => {
            Some(RuntimeTransportFailureKind::UpstreamClosedBeforeCommit)
        }
        _ => runtime_transport_failure_kind_from_message(&err.to_string()),
    }
}

pub(crate) fn runtime_proxy_transport_failure_kind(
    err: &anyhow::Error,
) -> Option<RuntimeTransportFailureKind> {
    for cause in err.chain() {
        if let Some(reqwest_error) = cause.downcast_ref::<reqwest::Error>()
            && let Some(kind) = runtime_transport_failure_kind_from_reqwest(reqwest_error)
        {
            return Some(kind);
        }
        if let Some(ws_error) = cause.downcast_ref::<WsError>()
            && let Some(kind) = runtime_transport_failure_kind_from_ws(ws_error)
        {
            return Some(kind);
        }
        if let Some(io_error) = cause.downcast_ref::<io::Error>()
            && let Some(kind) = runtime_transport_failure_kind_from_io_error(io_error)
        {
            return Some(kind);
        }
        if let Some(kind) = runtime_transport_failure_kind_from_message(&cause.to_string()) {
            return Some(kind);
        }
    }
    None
}

pub(crate) fn runtime_profile_transport_health_penalty(kind: RuntimeTransportFailureKind) -> u32 {
    match kind {
        RuntimeTransportFailureKind::Dns
        | RuntimeTransportFailureKind::ConnectTimeout
        | RuntimeTransportFailureKind::ConnectRefused
        | RuntimeTransportFailureKind::ConnectReset
        | RuntimeTransportFailureKind::TlsHandshake => {
            RUNTIME_PROFILE_CONNECT_FAILURE_HEALTH_PENALTY
        }
        RuntimeTransportFailureKind::BrokenPipe
        | RuntimeTransportFailureKind::ConnectionAborted
        | RuntimeTransportFailureKind::UnexpectedEof
        | RuntimeTransportFailureKind::ReadTimeout
        | RuntimeTransportFailureKind::UpstreamClosedBeforeCommit
        | RuntimeTransportFailureKind::Other => RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY,
    }
}

pub(crate) fn is_runtime_proxy_transport_failure(err: &anyhow::Error) -> bool {
    runtime_proxy_transport_failure_kind(err).is_some()
}

pub(crate) fn runtime_reqwest_error_kind(err: &reqwest::Error) -> io::ErrorKind {
    match runtime_transport_failure_kind_from_reqwest(err) {
        Some(
            RuntimeTransportFailureKind::ConnectTimeout | RuntimeTransportFailureKind::ReadTimeout,
        ) => io::ErrorKind::TimedOut,
        Some(RuntimeTransportFailureKind::ConnectRefused) => io::ErrorKind::ConnectionRefused,
        Some(RuntimeTransportFailureKind::ConnectReset) => io::ErrorKind::ConnectionReset,
        Some(RuntimeTransportFailureKind::ConnectionAborted) => io::ErrorKind::ConnectionAborted,
        Some(RuntimeTransportFailureKind::BrokenPipe) => io::ErrorKind::BrokenPipe,
        Some(RuntimeTransportFailureKind::UnexpectedEof) => io::ErrorKind::UnexpectedEof,
        _ => io::ErrorKind::Other,
    }
}
