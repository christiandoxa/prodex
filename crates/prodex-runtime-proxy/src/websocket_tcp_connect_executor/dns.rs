use std::io;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use super::executor::RuntimeWebsocketDnsResolveExecutor;
use super::local_pressure::{
    RuntimeWebsocketLocalPressureKind, runtime_websocket_local_pressure_io_error,
};
use super::task::RuntimeWebsocketLogToPath;

#[derive(Clone, Copy)]
pub struct RuntimeWebsocketDnsResolveRequest<'a> {
    pub executor: &'a RuntimeWebsocketDnsResolveExecutor,
    pub log_path: Option<&'a Path>,
    pub request_id: Option<u64>,
    pub port: u16,
    pub timeout: Duration,
    pub log_to_path: Option<RuntimeWebsocketLogToPath>,
}

pub fn runtime_resolve_websocket_tcp_addrs_with_executor<F>(
    request: RuntimeWebsocketDnsResolveRequest<'_>,
    host: String,
    resolver: F,
) -> io::Result<Vec<SocketAddr>>
where
    F: FnOnce(String, u16) -> io::Result<Vec<SocketAddr>> + Send + 'static,
{
    let RuntimeWebsocketDnsResolveRequest {
        executor,
        log_path,
        request_id,
        port,
        timeout,
        log_to_path,
    } = request;
    let (sender, receiver) = mpsc::channel::<io::Result<Vec<SocketAddr>>>();
    let observed_host = host.clone();
    let accepted = executor.spawn_observed(
        log_path,
        request_id,
        &observed_host,
        port,
        log_to_path,
        move || {
            let result = resolver(host, port);
            let _ = sender.send(result);
        },
    );
    if !accepted {
        return Err(runtime_websocket_local_pressure_io_error(
            RuntimeWebsocketLocalPressureKind::DnsResolveExecutorOverflow,
            format!(
                "websocket DNS resolution executor overflow for {}:{}",
                observed_host, port
            ),
        ));
    }

    match receiver.recv_timeout(timeout) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            runtime_websocket_tcp_connect_log_dns_resolve_timeout(
                log_path,
                request_id,
                &observed_host,
                port,
                timeout,
                log_to_path,
            );
            Err(runtime_websocket_local_pressure_io_error(
                RuntimeWebsocketLocalPressureKind::DnsResolveTimeout,
                format!(
                    "websocket DNS resolution timed out after {}ms for {}:{}",
                    timeout.as_millis(),
                    observed_host,
                    port
                ),
            ))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            format!(
                "websocket DNS resolution worker disconnected for {}:{}",
                observed_host, port
            ),
        )),
    }
}

fn runtime_websocket_tcp_connect_log_dns_resolve_timeout(
    log_path: Option<&Path>,
    request_id: Option<u64>,
    host: &str,
    port: u16,
    timeout: Duration,
    log_to_path: Option<RuntimeWebsocketLogToPath>,
) {
    let (Some(log_path), Some(log_to_path)) = (log_path, log_to_path) else {
        return;
    };
    let request = request_id
        .map(|request_id| format!("request={request_id} "))
        .unwrap_or_default();
    log_to_path(
        log_path,
        &format!(
            "{request}transport=websocket websocket_dns_resolve_timeout host={host} port={port} timeout_ms={}",
            timeout.as_millis()
        ),
    );
}
