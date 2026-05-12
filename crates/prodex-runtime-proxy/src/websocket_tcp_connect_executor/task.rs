use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use super::task_kind::RuntimeWebsocketTcpConnectTaskKind;

pub type RuntimeWebsocketLogToPath = fn(&Path, &str);

pub(super) type RuntimeWebsocketTcpConnectJob = Box<dyn FnOnce() + Send + 'static>;

#[derive(Clone)]
pub(super) struct RuntimeWebsocketTcpConnectTaskObservability {
    pub(super) log_path: Option<PathBuf>,
    pub(super) log_to_path: Option<RuntimeWebsocketLogToPath>,
    pub(super) request_id: Option<u64>,
    pub(super) kind: RuntimeWebsocketTcpConnectTaskKind,
    pub(super) addr: Option<SocketAddr>,
    pub(super) host: Option<String>,
    pub(super) port: Option<u16>,
}

pub(super) struct RuntimeWebsocketTcpConnectTask {
    pub(super) job: RuntimeWebsocketTcpConnectJob,
    pub(super) observability: RuntimeWebsocketTcpConnectTaskObservability,
}

impl RuntimeWebsocketTcpConnectTask {
    pub(super) fn new_tcp_connect(
        job: RuntimeWebsocketTcpConnectJob,
        log_path: Option<&Path>,
        request_id: Option<u64>,
        addr: Option<SocketAddr>,
        log_to_path: Option<RuntimeWebsocketLogToPath>,
    ) -> Self {
        Self {
            job,
            observability: RuntimeWebsocketTcpConnectTaskObservability {
                log_path: log_path.map(Path::to_path_buf),
                log_to_path,
                request_id,
                kind: RuntimeWebsocketTcpConnectTaskKind::TcpConnect,
                addr,
                host: None,
                port: None,
            },
        }
    }

    pub(super) fn new_dns_resolve(
        job: RuntimeWebsocketTcpConnectJob,
        log_path: Option<&Path>,
        request_id: Option<u64>,
        host: &str,
        port: u16,
        log_to_path: Option<RuntimeWebsocketLogToPath>,
    ) -> Self {
        Self {
            job,
            observability: RuntimeWebsocketTcpConnectTaskObservability {
                log_path: log_path.map(Path::to_path_buf),
                log_to_path,
                request_id,
                kind: RuntimeWebsocketTcpConnectTaskKind::DnsResolve,
                addr: None,
                host: Some(host.to_string()),
                port: Some(port),
            },
        }
    }

    pub(super) fn run(self) {
        (self.job)();
    }
}
