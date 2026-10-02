use prodex_mojo_core::transport_failure_policy as mojo_transport_failure;
use std::io;

pub const RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY: u32 = 4;
pub const RUNTIME_PROFILE_CONNECT_FAILURE_HEALTH_PENALTY: u32 = 5;

#[repr(i64)]
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
    mojo_transport_failure::failure_kind_label(kind as i64)
        .expect("Mojo transport-failure label policy returned invalid output")
}

pub fn runtime_upstream_connect_failure_marker(
    failure_kind: Option<RuntimeTransportFailureKind>,
) -> &'static str {
    mojo_transport_failure::upstream_connect_failure_marker(failure_kind.map(|kind| kind as i64))
        .expect("Mojo upstream-connect marker policy returned invalid output")
}

fn runtime_transport_failure_kind_from_tag(tag: i64) -> RuntimeTransportFailureKind {
    match tag {
        0 => RuntimeTransportFailureKind::Dns,
        1 => RuntimeTransportFailureKind::ConnectTimeout,
        2 => RuntimeTransportFailureKind::ConnectRefused,
        3 => RuntimeTransportFailureKind::ConnectReset,
        4 => RuntimeTransportFailureKind::TlsHandshake,
        5 => RuntimeTransportFailureKind::ConnectionAborted,
        6 => RuntimeTransportFailureKind::BrokenPipe,
        7 => RuntimeTransportFailureKind::UnexpectedEof,
        8 => RuntimeTransportFailureKind::ReadTimeout,
        9 => RuntimeTransportFailureKind::UpstreamClosedBeforeCommit,
        10 => RuntimeTransportFailureKind::Other,
        _ => unreachable!("validated Mojo transport-failure tag"),
    }
}

pub fn runtime_transport_failure_kind_from_message(
    message: &str,
) -> Option<RuntimeTransportFailureKind> {
    mojo_transport_failure::classify_message(message)
        .expect("Mojo transport-failure message classifier returned invalid output")
        .map(runtime_transport_failure_kind_from_tag)
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
    mojo_transport_failure::health_penalty(kind as i64)
        .expect("Mojo transport-failure health penalty policy returned invalid output")
}

#[cfg(test)]
#[path = "../tests/src/transport_failure.rs"]
mod tests;
