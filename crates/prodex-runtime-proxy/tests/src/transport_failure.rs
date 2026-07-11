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
