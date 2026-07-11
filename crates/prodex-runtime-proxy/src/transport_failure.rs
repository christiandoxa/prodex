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

struct RuntimeTransportFailureMessageRule {
    kind: RuntimeTransportFailureKind,
    needles: &'static [&'static str],
}

const RUNTIME_TRANSPORT_FAILURE_MESSAGE_RULES: &[RuntimeTransportFailureMessageRule] = &[
    RuntimeTransportFailureMessageRule {
        kind: RuntimeTransportFailureKind::Dns,
        needles: &[
            "dns",
            "failed to lookup address information",
            "no such host",
            "name or service not known",
        ],
    },
    RuntimeTransportFailureMessageRule {
        kind: RuntimeTransportFailureKind::TlsHandshake,
        needles: &["tls", "handshake", "certificate"],
    },
    RuntimeTransportFailureMessageRule {
        kind: RuntimeTransportFailureKind::ConnectRefused,
        needles: &["connection refused"],
    },
    RuntimeTransportFailureMessageRule {
        kind: RuntimeTransportFailureKind::ConnectTimeout,
        needles: &["timed out", "timeout"],
    },
    RuntimeTransportFailureMessageRule {
        kind: RuntimeTransportFailureKind::ConnectReset,
        needles: &["connection reset"],
    },
    RuntimeTransportFailureMessageRule {
        kind: RuntimeTransportFailureKind::BrokenPipe,
        needles: &["broken pipe"],
    },
    RuntimeTransportFailureMessageRule {
        kind: RuntimeTransportFailureKind::UnexpectedEof,
        needles: &["unexpected eof"],
    },
    RuntimeTransportFailureMessageRule {
        kind: RuntimeTransportFailureKind::ConnectionAborted,
        needles: &["connection aborted"],
    },
    RuntimeTransportFailureMessageRule {
        kind: RuntimeTransportFailureKind::UpstreamClosedBeforeCommit,
        needles: &[
            "stream closed before response.completed",
            "closed before response.completed",
        ],
    },
    RuntimeTransportFailureMessageRule {
        kind: RuntimeTransportFailureKind::Other,
        needles: &["unable to connect"],
    },
];

pub fn runtime_transport_failure_kind_from_message(
    message: &str,
) -> Option<RuntimeTransportFailureKind> {
    let message = message.to_ascii_lowercase();
    RUNTIME_TRANSPORT_FAILURE_MESSAGE_RULES
        .iter()
        .find(|rule| rule.needles.iter().any(|needle| message.contains(needle)))
        .map(|rule| rule.kind)
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
#[path = "../tests/src/transport_failure.rs"]
mod tests;
