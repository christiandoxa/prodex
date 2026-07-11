use crate::{
    RuntimeRotationProxyShared, RuntimeWebsocketTcpAttemptResult, runtime_proxy_log_to_path,
};
use std::io;
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::sync::OnceLock;
use std::sync::mpsc;
use std::time::Duration;

pub(super) use runtime_proxy_crate::{
    RuntimeWebsocketDnsResolveExecutor, RuntimeWebsocketDnsResolveRequest,
    RuntimeWebsocketLocalPressureKind, RuntimeWebsocketTcpConnectExecutor,
    runtime_websocket_local_pressure_io_error, runtime_websocket_local_pressure_kind_from_io_error,
};

fn runtime_websocket_tcp_connect_executor(
    shared: &RuntimeRotationProxyShared,
) -> &'static RuntimeWebsocketTcpConnectExecutor {
    static EXECUTOR: OnceLock<RuntimeWebsocketTcpConnectExecutor> = OnceLock::new();
    EXECUTOR.get_or_init(|| {
        let worker_count = shared.runtime_config.tuning.websocket_connect_worker_count;
        let queue_capacity = shared
            .runtime_config
            .tuning
            .websocket_connect_queue_capacity;
        let overflow_capacity = shared
            .runtime_config
            .tuning
            .websocket_connect_overflow_capacity;
        RuntimeWebsocketTcpConnectExecutor::new_with_overflow_capacity(
            worker_count,
            queue_capacity,
            overflow_capacity,
        )
    })
}

pub(super) fn runtime_websocket_dns_resolve_executor(
    shared: &RuntimeRotationProxyShared,
) -> &'static RuntimeWebsocketDnsResolveExecutor {
    static EXECUTOR: OnceLock<RuntimeWebsocketDnsResolveExecutor> = OnceLock::new();
    EXECUTOR.get_or_init(|| {
        let worker_count = shared.runtime_config.tuning.websocket_dns_worker_count;
        let queue_capacity = shared.runtime_config.tuning.websocket_dns_queue_capacity;
        let overflow_capacity = shared.runtime_config.tuning.websocket_dns_overflow_capacity;
        RuntimeWebsocketDnsResolveExecutor::new_with_overflow_capacity(
            worker_count,
            queue_capacity,
            overflow_capacity,
        )
    })
}

pub(super) fn runtime_resolve_websocket_tcp_addrs_with_executor<F>(
    executor: &RuntimeWebsocketDnsResolveExecutor,
    log_path: Option<&Path>,
    request_id: Option<u64>,
    host: String,
    port: u16,
    timeout: Duration,
    resolver: F,
) -> io::Result<Vec<SocketAddr>>
where
    F: FnOnce(String, u16) -> io::Result<Vec<SocketAddr>> + Send + 'static,
{
    runtime_proxy_crate::runtime_resolve_websocket_tcp_addrs_with_executor(
        RuntimeWebsocketDnsResolveRequest {
            executor,
            log_path,
            request_id,
            port,
            timeout,
            log_to_path: Some(runtime_proxy_log_to_path),
        },
        host,
        resolver,
    )
}

pub(super) fn runtime_launch_websocket_tcp_connect_attempt(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    sender: mpsc::Sender<RuntimeWebsocketTcpAttemptResult>,
    addr: SocketAddr,
    connect_timeout: Duration,
) {
    let result_sender = sender.clone();
    let accepted = runtime_websocket_tcp_connect_executor(shared).spawn_observed(
        Some(shared.log_path.as_path()),
        Some(request_id),
        Some(addr),
        Some(runtime_proxy_log_to_path),
        move || {
            let result = TcpStream::connect_timeout(&addr, connect_timeout);
            let _ = result_sender.send(RuntimeWebsocketTcpAttemptResult { addr, result });
        },
    );
    if !accepted {
        let _ = sender.send(RuntimeWebsocketTcpAttemptResult {
            addr,
            result: Err(runtime_websocket_local_pressure_io_error(
                RuntimeWebsocketLocalPressureKind::TcpConnectExecutorOverflow,
                format!("websocket TCP connect executor overflow for {addr}"),
            )),
        });
    }
}
