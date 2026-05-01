use super::*;

#[cfg(test)]
pub(crate) use runtime_proxy_crate::RUNTIME_PROFILE_CONNECT_FAILURE_HEALTH_PENALTY;
pub(crate) use runtime_proxy_crate::{
    RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY, RuntimeTransportFailureKind,
    runtime_profile_transport_health_penalty, runtime_transport_failure_kind_from_io_error,
    runtime_transport_failure_kind_from_message, runtime_transport_failure_kind_label,
    runtime_upstream_connect_failure_marker,
};

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
        runtime_proxy_structured_log_message(
            runtime_upstream_connect_failure_marker(failure_kind),
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", transport),
                runtime_proxy_log_field("profile", profile_name),
                runtime_proxy_log_field(
                    "class",
                    failure_kind
                        .map(runtime_transport_failure_kind_label)
                        .unwrap_or("unknown"),
                ),
                runtime_proxy_log_field("error", error.to_string()),
            ],
        ),
    );
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
