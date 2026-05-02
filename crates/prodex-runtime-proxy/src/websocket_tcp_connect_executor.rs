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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeWebsocketTcpConnectTaskKind {
    TcpConnect,
    DnsResolve,
}

impl RuntimeWebsocketTcpConnectTaskKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TcpConnect => "tcp_connect",
            Self::DnsResolve => "dns_resolve",
        }
    }

    pub fn worker_thread_prefix(self) -> &'static str {
        match self {
            Self::TcpConnect => "prodex-ws-connect",
            Self::DnsResolve => "prodex-ws-dns",
        }
    }

    pub fn dispatcher_thread_name(self) -> &'static str {
        match self {
            Self::TcpConnect => "prodex-ws-connect-dispatch",
            Self::DnsResolve => "prodex-ws-dns-dispatch",
        }
    }

    pub fn overflow_enqueue_event(self) -> &'static str {
        match self {
            Self::TcpConnect => "websocket_connect_overflow_enqueue",
            Self::DnsResolve => "websocket_dns_overflow_enqueue",
        }
    }

    pub fn overflow_dispatch_event(self) -> &'static str {
        match self {
            Self::TcpConnect => "websocket_connect_overflow_dispatch",
            Self::DnsResolve => "websocket_dns_overflow_dispatch",
        }
    }

    pub fn overflow_reject_event(self) -> &'static str {
        match self {
            Self::TcpConnect => "websocket_connect_overflow_reject",
            Self::DnsResolve => "websocket_dns_overflow_reject",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuntimeWebsocketTcpConnectOverflowSnapshot {
    pub pending_jobs: usize,
    pub max_pending_jobs: usize,
    pub total_enqueued: usize,
    pub total_dispatched: usize,
    pub total_rejected: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RuntimeWebsocketTcpConnectOverflowState {
    pending_jobs: usize,
    max_pending_jobs: usize,
    total_enqueued: usize,
    total_dispatched: usize,
    total_rejected: usize,
}

impl RuntimeWebsocketTcpConnectOverflowState {
    pub fn snapshot(&self) -> RuntimeWebsocketTcpConnectOverflowSnapshot {
        RuntimeWebsocketTcpConnectOverflowSnapshot {
            pending_jobs: self.pending_jobs,
            max_pending_jobs: self.max_pending_jobs,
            total_enqueued: self.total_enqueued,
            total_dispatched: self.total_dispatched,
            total_rejected: self.total_rejected,
        }
    }

    pub fn try_enqueue(
        &mut self,
        overflow_capacity: usize,
    ) -> Result<
        RuntimeWebsocketTcpConnectOverflowSnapshot,
        RuntimeWebsocketTcpConnectOverflowSnapshot,
    > {
        if self.pending_jobs >= overflow_capacity {
            self.total_rejected = self.total_rejected.saturating_add(1);
            return Err(self.snapshot());
        }

        self.pending_jobs = self.pending_jobs.saturating_add(1);
        self.total_enqueued = self.total_enqueued.saturating_add(1);
        self.max_pending_jobs = self.max_pending_jobs.max(self.pending_jobs);
        Ok(self.snapshot())
    }

    pub fn dispatch(&mut self) -> RuntimeWebsocketTcpConnectOverflowSnapshot {
        self.pending_jobs = self.pending_jobs.saturating_sub(1);
        self.total_dispatched = self.total_dispatched.saturating_add(1);
        self.snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_pressure_error_round_trips_tagged_kind() {
        let err = runtime_websocket_local_pressure_io_error(
            RuntimeWebsocketLocalPressureKind::TcpConnectExecutorOverflow,
            "websocket TCP connect executor overflow for 127.0.0.1:443",
        );

        assert_eq!(err.kind(), io::ErrorKind::WouldBlock);
        assert_eq!(
            runtime_websocket_local_pressure_kind_from_io_error(&err),
            Some(RuntimeWebsocketLocalPressureKind::TcpConnectExecutorOverflow)
        );

        let plain = io::Error::new(io::ErrorKind::WouldBlock, "plain local backpressure");
        assert_eq!(
            runtime_websocket_local_pressure_kind_from_io_error(&plain),
            None
        );
    }

    #[test]
    fn task_kind_names_stay_stable_for_observability() {
        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::TcpConnect.as_str(),
            "tcp_connect"
        );
        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::TcpConnect.worker_thread_prefix(),
            "prodex-ws-connect"
        );
        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::TcpConnect.dispatcher_thread_name(),
            "prodex-ws-connect-dispatch"
        );
        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::TcpConnect.overflow_enqueue_event(),
            "websocket_connect_overflow_enqueue"
        );
        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::TcpConnect.overflow_dispatch_event(),
            "websocket_connect_overflow_dispatch"
        );
        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::TcpConnect.overflow_reject_event(),
            "websocket_connect_overflow_reject"
        );

        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::DnsResolve.as_str(),
            "dns_resolve"
        );
        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::DnsResolve.worker_thread_prefix(),
            "prodex-ws-dns"
        );
        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::DnsResolve.dispatcher_thread_name(),
            "prodex-ws-dns-dispatch"
        );
        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::DnsResolve.overflow_enqueue_event(),
            "websocket_dns_overflow_enqueue"
        );
        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::DnsResolve.overflow_dispatch_event(),
            "websocket_dns_overflow_dispatch"
        );
        assert_eq!(
            RuntimeWebsocketTcpConnectTaskKind::DnsResolve.overflow_reject_event(),
            "websocket_dns_overflow_reject"
        );
    }

    #[test]
    fn overflow_state_tracks_bounded_queue_transitions() {
        let mut state = RuntimeWebsocketTcpConnectOverflowState::default();

        let first = state.try_enqueue(2).expect("first enqueue should fit");
        assert_eq!(
            first,
            RuntimeWebsocketTcpConnectOverflowSnapshot {
                pending_jobs: 1,
                max_pending_jobs: 1,
                total_enqueued: 1,
                total_dispatched: 0,
                total_rejected: 0,
            }
        );

        let second = state.try_enqueue(2).expect("second enqueue should fit");
        assert_eq!(second.pending_jobs, 2);
        assert_eq!(second.max_pending_jobs, 2);
        assert_eq!(second.total_enqueued, 2);

        let rejected = state
            .try_enqueue(2)
            .expect_err("full overflow state should reject");
        assert_eq!(rejected.pending_jobs, 2);
        assert_eq!(rejected.total_rejected, 1);

        let dispatched = state.dispatch();
        assert_eq!(dispatched.pending_jobs, 1);
        assert_eq!(dispatched.max_pending_jobs, 2);
        assert_eq!(dispatched.total_dispatched, 1);
    }
}
