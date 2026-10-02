#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeWebsocketTcpConnectTaskKind {
    TcpConnect,
    DnsResolve,
}

impl RuntimeWebsocketTcpConnectTaskKind {
    pub fn as_str(self) -> &'static str {
        prodex_mojo_core::observability::runtime_websocket_task_label(self as i64)
            .expect("Mojo websocket task label returned invalid output")
    }

    pub fn worker_thread_prefix(self) -> &'static str {
        prodex_mojo_core::observability::runtime_websocket_worker_thread_prefix(self as i64)
            .expect("Mojo websocket worker-thread prefix returned invalid output")
    }

    pub fn dispatcher_thread_name(self) -> &'static str {
        prodex_mojo_core::observability::runtime_websocket_dispatcher_thread_name(self as i64)
            .expect("Mojo websocket dispatcher-thread name returned invalid output")
    }

    pub fn overflow_enqueue_event(self) -> &'static str {
        prodex_mojo_core::observability::runtime_websocket_overflow_enqueue_event(self as i64)
            .expect("Mojo websocket overflow-enqueue event returned invalid output")
    }

    pub fn overflow_dispatch_event(self) -> &'static str {
        prodex_mojo_core::observability::runtime_websocket_overflow_dispatch_event(self as i64)
            .expect("Mojo websocket overflow-dispatch event returned invalid output")
    }

    pub fn overflow_reject_event(self) -> &'static str {
        prodex_mojo_core::observability::runtime_websocket_overflow_reject_event(self as i64)
            .expect("Mojo websocket overflow-reject event returned invalid output")
    }
}
