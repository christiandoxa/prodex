use std::io;

pub const RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY: u32 = 4;
pub const RUNTIME_PROFILE_CONNECT_FAILURE_HEALTH_PENALTY: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeTransportFailureKind {
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

pub fn runtime_transport_failure_kind_label(kind: RuntimeTransportFailureKind) -> &'static str {
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

pub fn runtime_upstream_connect_failure_marker(
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

pub fn runtime_transport_failure_kind_from_message(
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

pub fn runtime_transport_failure_kind_from_io_error(
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

pub fn runtime_profile_transport_health_penalty(kind: RuntimeTransportFailureKind) -> u32 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_transport_failure_messages() {
        assert_eq!(
            runtime_transport_failure_kind_from_message("failed to lookup address information"),
            Some(RuntimeTransportFailureKind::Dns)
        );
        assert_eq!(
            runtime_transport_failure_kind_from_message("TLS handshake failed"),
            Some(RuntimeTransportFailureKind::TlsHandshake)
        );
        assert_eq!(
            runtime_transport_failure_kind_from_message("stream closed before response.completed"),
            Some(RuntimeTransportFailureKind::UpstreamClosedBeforeCommit)
        );
    }

    #[test]
    fn maps_io_errors_to_transport_kinds() {
        let err = io::Error::new(io::ErrorKind::ConnectionRefused, "refused");
        assert_eq!(
            runtime_transport_failure_kind_from_io_error(&err),
            Some(RuntimeTransportFailureKind::ConnectRefused)
        );
    }

    #[test]
    fn maps_failure_kind_to_log_marker_and_penalty() {
        assert_eq!(
            runtime_upstream_connect_failure_marker(Some(RuntimeTransportFailureKind::Dns)),
            "upstream_connect_dns_error"
        );
        assert_eq!(
            runtime_profile_transport_health_penalty(RuntimeTransportFailureKind::Dns),
            RUNTIME_PROFILE_CONNECT_FAILURE_HEALTH_PENALTY
        );
        assert_eq!(
            runtime_profile_transport_health_penalty(RuntimeTransportFailureKind::BrokenPipe),
            RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY
        );
    }
}
