use super::*;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

#[path = "websocket_tcp_connect_executor/dns.rs"]
mod dns;
#[path = "websocket_tcp_connect_executor/tcp.rs"]
mod tcp;

pub(super) fn record_max_active(max_active: &AtomicUsize, active_now: usize) {
    let mut observed = max_active.load(Ordering::SeqCst);
    while active_now > observed {
        match max_active.compare_exchange(observed, active_now, Ordering::SeqCst, Ordering::SeqCst)
        {
            Ok(_) => break,
            Err(next) => observed = next,
        }
    }
}

pub(super) fn websocket_test_log_path(name: &str) -> PathBuf {
    static NEXT_LOG_ID: AtomicU64 = AtomicU64::new(1);
    std::env::temp_dir().join(format!(
        "prodex-runtime-proxy-websocket-{name}-{}-{}.log",
        std::process::id(),
        NEXT_LOG_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

pub(super) fn test_log_to_path(path: &Path, message: &str) {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("test runtime log should open");
    writeln!(file, "{message}").expect("test runtime log should write");
}

pub(super) fn read_websocket_test_log_after_marker(log_path: &Path, marker: &str) -> String {
    let started_at = Instant::now();
    loop {
        let log = fs::read_to_string(log_path).unwrap_or_default();
        if log.contains(marker) || started_at.elapsed() >= Duration::from_secs(1) {
            return log;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

impl RuntimeWebsocketTcpConnectExecutor {
    pub(crate) fn new_with_spawn_outcome_for_test(
        worker_count: usize,
        queue_capacity: usize,
        overflow_capacity: usize,
        started_worker_count: usize,
        dispatcher_started: bool,
    ) -> Self {
        Self::new_for_kind_with_spawn_outcome_for_test(
            RuntimeWebsocketTcpConnectTaskKind::TcpConnect,
            worker_count,
            queue_capacity,
            overflow_capacity,
            started_worker_count,
            dispatcher_started,
        )
    }

    fn new_for_kind_with_spawn_outcome_for_test(
        task_kind: RuntimeWebsocketTcpConnectTaskKind,
        worker_count: usize,
        queue_capacity: usize,
        overflow_capacity: usize,
        started_worker_count: usize,
        dispatcher_started: bool,
    ) -> Self {
        let worker_prefix = task_kind.worker_thread_prefix();
        let dispatcher_name = task_kind.dispatcher_thread_name();
        Self::new_for_kind_with_spawner(
            task_kind,
            worker_count,
            queue_capacity,
            overflow_capacity,
            |name, job| {
                let accepted = if name == dispatcher_name {
                    dispatcher_started
                } else {
                    name.strip_prefix(worker_prefix)
                        .and_then(|suffix| suffix.strip_prefix('-'))
                        .and_then(|index| index.parse::<usize>().ok())
                        .is_some_and(|index| index < started_worker_count)
                };
                if !accepted {
                    return false;
                }
                thread::Builder::new().name(name).spawn(job).is_ok()
            },
        )
    }
}

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
