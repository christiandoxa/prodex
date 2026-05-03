use super::*;
use std::fs;
use std::io::Write as _;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::time::Instant;

fn record_max_active(max_active: &AtomicUsize, active_now: usize) {
    let mut observed = max_active.load(Ordering::SeqCst);
    while active_now > observed {
        match max_active.compare_exchange(observed, active_now, Ordering::SeqCst, Ordering::SeqCst)
        {
            Ok(_) => break,
            Err(next) => observed = next,
        }
    }
}

fn websocket_test_log_path(name: &str) -> PathBuf {
    static NEXT_LOG_ID: AtomicU64 = AtomicU64::new(1);
    std::env::temp_dir().join(format!(
        "prodex-runtime-proxy-websocket-{name}-{}-{}.log",
        std::process::id(),
        NEXT_LOG_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

fn test_log_to_path(path: &Path, message: &str) {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("test runtime log should open");
    writeln!(file, "{message}").expect("test runtime log should write");
}

fn read_websocket_test_log_after_marker(log_path: &Path, marker: &str) -> String {
    let started_at = Instant::now();
    loop {
        let log = fs::read_to_string(log_path).unwrap_or_default();
        if log.contains(marker) || started_at.elapsed() >= Duration::from_secs(1) {
            return log;
        }
        thread::sleep(Duration::from_millis(10));
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

#[test]
fn websocket_tcp_connect_executor_bounds_concurrent_jobs() {
    let executor = RuntimeWebsocketTcpConnectExecutor::new(2, 8);
    let active = Arc::new(AtomicUsize::new(0));
    let max_active = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(AtomicUsize::new(0));
    let (start_tx, start_rx) = mpsc::channel::<()>();
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));

    for _ in 0..6 {
        let active = Arc::clone(&active);
        let max_active = Arc::clone(&max_active);
        let started = Arc::clone(&started);
        let start_tx = start_tx.clone();
        let done_tx = done_tx.clone();
        let gate = Arc::clone(&gate);
        executor.spawn(move || {
            let active_now = active.fetch_add(1, Ordering::SeqCst) + 1;
            record_max_active(&max_active, active_now);
            started.fetch_add(1, Ordering::SeqCst);
            start_tx.send(()).expect("start signal should send");

            let (released, ready) = &*gate;
            let mut released = released
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            while !*released {
                released = ready
                    .wait(released)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }

            active.fetch_sub(1, Ordering::SeqCst);
            done_tx.send(()).expect("done signal should send");
        });
    }

    for _ in 0..2 {
        start_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("worker should start queued websocket connect job");
    }

    assert_eq!(
        started.load(Ordering::SeqCst),
        2,
        "only worker-count websocket connect jobs should start before release"
    );
    assert!(
        matches!(
            start_rx.recv_timeout(Duration::from_millis(100)),
            Err(RecvTimeoutError::Timeout)
        ),
        "websocket tcp connect executor should not exceed worker count"
    );
    assert_eq!(
        max_active.load(Ordering::SeqCst),
        2,
        "websocket tcp connect executor should cap concurrent jobs"
    );

    let (released, ready) = &*gate;
    *released
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
    ready.notify_all();

    for _ in 0..6 {
        done_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("websocket connect job should complete after release");
    }

    assert_eq!(
        max_active.load(Ordering::SeqCst),
        2,
        "websocket tcp connect executor should stay bounded after draining queue"
    );
}

#[test]
fn websocket_tcp_connect_executor_spills_overflow_without_inline_starting_extra_jobs() {
    let executor = RuntimeWebsocketTcpConnectExecutor::new(1, 1);
    let started = Arc::new(AtomicUsize::new(0));
    let (start_tx, start_rx) = mpsc::channel::<()>();
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));

    for _ in 0..3 {
        let started = Arc::clone(&started);
        let start_tx = start_tx.clone();
        let done_tx = done_tx.clone();
        let gate = Arc::clone(&gate);
        executor.spawn(move || {
            started.fetch_add(1, Ordering::SeqCst);
            start_tx.send(()).expect("start signal should send");

            let (released, ready) = &*gate;
            let mut released = released
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            while !*released {
                released = ready
                    .wait(released)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }

            done_tx.send(()).expect("done signal should send");
        });
    }

    start_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("first websocket connect job should start");
    assert_eq!(
        started.load(Ordering::SeqCst),
        1,
        "only the worker job should start before release"
    );
    assert!(
        matches!(
            start_rx.recv_timeout(Duration::from_millis(100)),
            Err(RecvTimeoutError::Timeout)
        ),
        "overflow websocket connect jobs should stay queued instead of starting inline"
    );

    let (released, ready) = &*gate;
    *released
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
    ready.notify_all();

    for _ in 0..3 {
        done_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("websocket connect job should complete after release");
    }
}

#[test]
fn websocket_tcp_connect_executor_logs_overflow_pressure() {
    let executor = RuntimeWebsocketTcpConnectExecutor::new(1, 1);
    let log_path = websocket_test_log_path("overflow-pressure");
    let _ = fs::remove_file(&log_path);
    let started = Arc::new(AtomicUsize::new(0));
    let (start_tx, start_rx) = mpsc::channel::<()>();
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let addr = "127.0.0.1:443"
        .parse::<SocketAddr>()
        .expect("socket addr should parse");

    for _ in 0..3 {
        let started = Arc::clone(&started);
        let start_tx = start_tx.clone();
        let done_tx = done_tx.clone();
        let gate = Arc::clone(&gate);
        executor.spawn_observed(
            Some(log_path.as_path()),
            Some(41),
            Some(addr),
            Some(test_log_to_path),
            move || {
                started.fetch_add(1, Ordering::SeqCst);
                start_tx.send(()).expect("start signal should send");

                let (released, ready) = &*gate;
                let mut released = released
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                while !*released {
                    released = ready
                        .wait(released)
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                }

                done_tx.send(()).expect("done signal should send");
            },
        );
    }

    start_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("first websocket connect job should start");
    assert_eq!(
        started.load(Ordering::SeqCst),
        1,
        "only the worker job should start before release"
    );
    assert!(
        matches!(
            start_rx.recv_timeout(Duration::from_millis(100)),
            Err(RecvTimeoutError::Timeout)
        ),
        "overflow websocket connect jobs should stay queued instead of starting inline"
    );

    let overflow_snapshot = executor
        .overflow_snapshot()
        .expect("bounded websocket executor should expose overflow state");
    assert!(
        overflow_snapshot.total_enqueued >= 1,
        "overflow path should record at least one enqueued job: {overflow_snapshot:?}"
    );
    assert!(
        overflow_snapshot.max_pending_jobs >= 1,
        "overflow path should record queued overflow work before release: {overflow_snapshot:?}"
    );

    let (released, ready) = &*gate;
    *released
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
    ready.notify_all();

    for _ in 0..3 {
        done_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("websocket connect job should complete after release");
    }

    let overflow_snapshot = executor
        .overflow_snapshot()
        .expect("bounded websocket executor should expose overflow state");
    assert_eq!(
        overflow_snapshot.pending_jobs, 0,
        "overflow queue should drain after worker release"
    );
    assert!(
        overflow_snapshot.total_dispatched >= 1,
        "overflow dispatcher should hand work back to the bounded queue: {overflow_snapshot:?}"
    );

    let log = read_websocket_test_log_after_marker(
        &log_path,
        "request=41 transport=websocket websocket_connect_overflow_dispatch reason=queue_available",
    );
    assert!(
        log.contains(
            "request=41 transport=websocket websocket_connect_overflow_enqueue reason=queue_full"
        ),
        "overflow enqueue log marker missing: {log}"
    );
    assert!(
            log.contains(
                "request=41 transport=websocket websocket_connect_overflow_dispatch reason=queue_available"
            ),
            "overflow dispatch log marker missing: {log}"
        );
    assert!(
        log.contains("addr=127.0.0.1:443"),
        "overflow log should include the connect address: {log}"
    );
    assert!(
        log.contains("overflow_total_enqueued=") && log.contains("overflow_total_dispatched="),
        "overflow log should include queue counters: {log}"
    );
    assert!(
        log.contains("worker_count=1 queue_capacity=1"),
        "overflow log should include executor bounds: {log}"
    );

    let _ = fs::remove_file(&log_path);
}

#[test]
fn websocket_tcp_connect_executor_logs_actual_started_worker_count() {
    let executor =
        RuntimeWebsocketTcpConnectExecutor::new_with_spawn_outcome_for_test(4, 4, 1, 1, true);
    let log_path = websocket_test_log_path("partial-worker-count");
    let _ = fs::remove_file(&log_path);
    let started = Arc::new(AtomicUsize::new(0));
    let (start_tx, start_rx) = mpsc::channel::<()>();
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let addr = "127.0.0.1:443"
        .parse::<SocketAddr>()
        .expect("socket addr should parse");

    for _ in 0..6 {
        let started = Arc::clone(&started);
        let start_tx = start_tx.clone();
        let done_tx = done_tx.clone();
        let gate = Arc::clone(&gate);
        assert!(
            executor.spawn_observed(
                Some(log_path.as_path()),
                Some(43),
                Some(addr),
                Some(test_log_to_path),
                move || {
                    started.fetch_add(1, Ordering::SeqCst);
                    start_tx.send(()).expect("start signal should send");

                    let (released, ready) = &*gate;
                    let mut released = released
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    while !*released {
                        released = ready
                            .wait(released)
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                    }

                    done_tx.send(()).expect("done signal should send");
                }
            ),
            "overflow job should be accepted while overflow capacity remains available"
        );
    }

    start_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("first websocket connect job should start");
    assert_eq!(
        started.load(Ordering::SeqCst),
        1,
        "only the single started worker should run before release"
    );
    assert!(
        matches!(
            start_rx.recv_timeout(Duration::from_millis(100)),
            Err(RecvTimeoutError::Timeout)
        ),
        "queued websocket connect jobs should not start before release"
    );

    let log = fs::read_to_string(&log_path).expect("overflow runtime log should exist");
    assert!(
        log.contains(
            "request=43 transport=websocket websocket_connect_overflow_enqueue reason=queue_full"
        ),
        "overflow enqueue log marker missing: {log}"
    );
    assert!(
        log.contains("worker_count=1 queue_capacity=4"),
        "overflow log should report actual started workers, not requested workers: {log}"
    );
    assert!(
        !log.contains("worker_count=4 queue_capacity=4"),
        "overflow log should not report optimistic requested workers after partial spawn: {log}"
    );

    let (released, ready) = &*gate;
    *released
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
    ready.notify_all();

    for _ in 0..6 {
        done_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("websocket connect job should complete after release");
    }

    let _ = fs::remove_file(&log_path);
}

#[test]
fn websocket_tcp_connect_executor_keeps_inline_fallback_when_workers_unavailable() {
    let zero_worker_executor =
        RuntimeWebsocketTcpConnectExecutor::new_with_spawn_outcome_for_test(2, 2, 1, 0, true);
    let ran = Arc::new(AtomicUsize::new(0));
    let ran_for_job = Arc::clone(&ran);
    assert!(
        zero_worker_executor.spawn(move || {
            ran_for_job.fetch_add(1, Ordering::SeqCst);
        }),
        "inline fallback should accept work when no worker starts"
    );
    assert_eq!(
        ran.load(Ordering::SeqCst),
        1,
        "no-worker fallback should run work inline"
    );
    assert!(
        zero_worker_executor.overflow_snapshot().is_none(),
        "no-worker fallback should not expose bounded overflow state"
    );

    let dispatcher_failed_executor =
        RuntimeWebsocketTcpConnectExecutor::new_with_spawn_outcome_for_test(2, 2, 1, 1, false);
    let ran = Arc::new(AtomicUsize::new(0));
    let ran_for_job = Arc::clone(&ran);
    assert!(
        dispatcher_failed_executor.spawn(move || {
            ran_for_job.fetch_add(1, Ordering::SeqCst);
        }),
        "inline fallback should accept work when dispatcher fails"
    );
    assert_eq!(
        ran.load(Ordering::SeqCst),
        1,
        "dispatcher-failure fallback should run work inline"
    );
    assert!(
        dispatcher_failed_executor.overflow_snapshot().is_none(),
        "dispatcher-failure fallback should not expose bounded overflow state"
    );
}

#[test]
fn websocket_tcp_connect_executor_rejects_overflow_past_capacity() {
    let executor = RuntimeWebsocketTcpConnectExecutor::new_with_overflow_capacity(1, 1, 1);
    let log_path = websocket_test_log_path("overflow-reject");
    let _ = fs::remove_file(&log_path);
    let started = Arc::new(AtomicUsize::new(0));
    let (start_tx, start_rx) = mpsc::channel::<()>();
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let addr = "127.0.0.1:443"
        .parse::<SocketAddr>()
        .expect("socket addr should parse");

    let mut accepted_jobs = 0usize;
    let mut rejected_jobs = 0usize;
    for _ in 0..8 {
        let started = Arc::clone(&started);
        let start_tx = start_tx.clone();
        let done_tx = done_tx.clone();
        let gate = Arc::clone(&gate);
        if executor.spawn_observed(
            Some(log_path.as_path()),
            Some(42),
            Some(addr),
            Some(test_log_to_path),
            move || {
                started.fetch_add(1, Ordering::SeqCst);
                start_tx.send(()).expect("start signal should send");

                let (released, ready) = &*gate;
                let mut released = released
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                while !*released {
                    released = ready
                        .wait(released)
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                }

                done_tx.send(()).expect("done signal should send");
            },
        ) {
            accepted_jobs += 1;
        } else {
            rejected_jobs += 1;
        }
    }

    start_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("first websocket connect job should start");
    assert!(
        matches!(
            start_rx.recv_timeout(Duration::from_millis(100)),
            Err(RecvTimeoutError::Timeout)
        ),
        "rejected overflow work should not start inline"
    );
    assert!(
        rejected_jobs > 0,
        "overflow cap should reject at least one job"
    );

    let overflow_snapshot = executor
        .overflow_snapshot()
        .expect("bounded websocket executor should expose overflow state");
    assert!(
        overflow_snapshot.total_rejected >= rejected_jobs,
        "overflow snapshot should record rejected jobs: {overflow_snapshot:?}"
    );

    let (released, ready) = &*gate;
    *released
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
    ready.notify_all();

    for _ in 0..accepted_jobs {
        done_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("accepted websocket connect job should complete after release");
    }

    let log = fs::read_to_string(&log_path).expect("overflow runtime log should exist");
    assert!(
            log.contains(
                "request=42 transport=websocket websocket_connect_overflow_reject reason=overflow_capacity_reached"
            ),
            "overflow rejection log marker missing: {log}"
        );
    assert!(
        log.contains("overflow_capacity=1") && log.contains("overflow_total_rejected="),
        "overflow rejection log should include cap and rejection counters: {log}"
    );

    let _ = fs::remove_file(&log_path);
}

#[test]
fn websocket_dns_resolution_times_out_on_bounded_executor() {
    let executor = RuntimeWebsocketDnsResolveExecutor::new_with_overflow_capacity(1, 1, 0);
    let log_path = websocket_test_log_path("dns-timeout");
    let _ = fs::remove_file(&log_path);
    let started = Arc::new(AtomicUsize::new(0));
    let started_for_resolver = Arc::clone(&started);
    let started_at = Instant::now();

    let err = runtime_resolve_websocket_tcp_addrs_with_executor(
        RuntimeWebsocketDnsResolveRequest {
            executor: &executor,
            log_path: Some(log_path.as_path()),
            request_id: Some(77),
            port: 443,
            timeout: Duration::from_millis(25),
            log_to_path: Some(test_log_to_path),
        },
        "slow.example.test".to_string(),
        move |_host, _port| {
            started_for_resolver.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(150));
            Ok(Vec::new())
        },
    )
    .expect_err("slow websocket DNS resolution should time out");

    assert_eq!(
        err.kind(),
        io::ErrorKind::WouldBlock,
        "bounded DNS timeout should fail as local pressure, not account quota"
    );
    assert!(
        started_at.elapsed() < Duration::from_millis(125),
        "DNS helper should return on its bounded timeout"
    );

    let log = fs::read_to_string(&log_path).expect("DNS timeout log should exist");
    assert!(
            log.contains(
                "request=77 transport=websocket websocket_dns_resolve_timeout host=slow.example.test port=443 timeout_ms=25"
            ),
            "DNS timeout log marker missing: {log}"
        );

    let _ = fs::remove_file(&log_path);
}

#[test]
fn websocket_dns_overflow_is_local_pressure() {
    let executor = RuntimeWebsocketDnsResolveExecutor::new_with_overflow_capacity(1, 1, 0);
    let log_path = websocket_test_log_path("dns-overflow");
    let _ = fs::remove_file(&log_path);
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let (started_tx, started_rx) = mpsc::channel::<()>();

    let first = runtime_resolve_websocket_tcp_addrs_with_executor(
        RuntimeWebsocketDnsResolveRequest {
            executor: &executor,
            log_path: Some(log_path.as_path()),
            request_id: Some(78),
            port: 443,
            timeout: Duration::from_millis(25),
            log_to_path: Some(test_log_to_path),
        },
        "blocked.example.test".to_string(),
        move |_host, _port| {
            started_tx.send(()).expect("DNS resolver should start");
            let _ = release_rx.recv_timeout(Duration::from_secs(1));
            Ok(Vec::new())
        },
    );

    started_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("first DNS resolver should occupy worker");
    let queued_err = runtime_resolve_websocket_tcp_addrs_with_executor(
        RuntimeWebsocketDnsResolveRequest {
            executor: &executor,
            log_path: Some(log_path.as_path()),
            request_id: Some(79),
            port: 443,
            timeout: Duration::from_millis(1),
            log_to_path: Some(test_log_to_path),
        },
        "queued.example.test".to_string(),
        |_host, _port| Ok(Vec::new()),
    )
    .expect_err("queued DNS work should time out while worker is blocked");
    assert_eq!(
        runtime_websocket_local_pressure_kind_from_io_error(&queued_err),
        Some(RuntimeWebsocketLocalPressureKind::DnsResolveTimeout),
        "queued DNS timeout should remain local pressure"
    );

    let err = runtime_resolve_websocket_tcp_addrs_with_executor(
        RuntimeWebsocketDnsResolveRequest {
            executor: &executor,
            log_path: Some(log_path.as_path()),
            request_id: Some(80),
            port: 443,
            timeout: Duration::from_millis(25),
            log_to_path: Some(test_log_to_path),
        },
        "overflow.example.test".to_string(),
        |_host, _port| Ok(Vec::new()),
    )
    .expect_err("saturated DNS executor should reject overflow work");

    assert_eq!(
        runtime_websocket_local_pressure_kind_from_io_error(&err),
        Some(RuntimeWebsocketLocalPressureKind::DnsResolveExecutorOverflow),
        "DNS executor overflow should be classified as local pressure"
    );
    assert_eq!(
        err.kind(),
        io::ErrorKind::WouldBlock,
        "DNS executor overflow should fail fast"
    );
    let overflow_snapshot = executor
        .overflow_snapshot()
        .expect("bounded DNS executor should expose overflow state");
    assert!(
        overflow_snapshot.total_rejected >= 1,
        "DNS overflow snapshot should record rejection: {overflow_snapshot:?}"
    );

    release_tx
        .send(())
        .expect("first DNS resolver release should send");
    let _ = first.expect_err("first DNS resolver should have timed out locally");

    let log = fs::read_to_string(&log_path).expect("DNS overflow log should exist");
    assert!(
            log.contains(
                "request=80 transport=websocket websocket_dns_overflow_reject reason=overflow_capacity_reached"
            ),
            "DNS overflow rejection log marker missing: {log}"
        );
    assert!(
        log.contains("task=dns_resolve host=overflow.example.test port=443"),
        "DNS overflow log should include DNS task metadata: {log}"
    );

    let _ = fs::remove_file(&log_path);
}

#[test]
fn websocket_dns_executor_is_distinct_from_tcp_connect_executor() {
    let tcp_executor = RuntimeWebsocketTcpConnectExecutor::new_with_overflow_capacity(1, 1, 0);
    let dns_executor = RuntimeWebsocketDnsResolveExecutor::new_with_overflow_capacity(1, 1, 0);
    let tcp_started = Arc::new(AtomicUsize::new(0));
    let (tcp_start_tx, tcp_start_rx) = mpsc::channel::<()>();
    let (tcp_done_tx, tcp_done_rx) = mpsc::channel::<()>();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));

    let tcp_started_for_job = Arc::clone(&tcp_started);
    let gate_for_job = Arc::clone(&gate);
    assert!(
        tcp_executor.spawn(move || {
            tcp_started_for_job.fetch_add(1, Ordering::SeqCst);
            tcp_start_tx.send(()).expect("TCP start signal should send");

            let (released, ready) = &*gate_for_job;
            let mut released = released
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            while !*released {
                released = ready
                    .wait(released)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }

            tcp_done_tx.send(()).expect("TCP done signal should send");
        }),
        "first TCP connect job should be accepted"
    );
    tcp_start_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("TCP connect worker should be occupied");

    let dns_started = Arc::new(AtomicUsize::new(0));
    let dns_started_for_resolver = Arc::clone(&dns_started);
    let started_at = Instant::now();
    let addrs = runtime_resolve_websocket_tcp_addrs_with_executor(
        RuntimeWebsocketDnsResolveRequest {
            executor: &dns_executor,
            log_path: None,
            request_id: Some(80),
            port: 443,
            timeout: Duration::from_secs(1),
            log_to_path: None,
        },
        "fast.example.test".to_string(),
        move |_host, port| {
            dns_started_for_resolver.fetch_add(1, Ordering::SeqCst);
            Ok(vec![SocketAddr::from(([127, 0, 0, 1], port))])
        },
    )
    .expect("DNS executor should not be blocked by occupied TCP executor");

    assert_eq!(
        dns_started.load(Ordering::SeqCst),
        1,
        "DNS resolver should run on its dedicated executor"
    );
    assert_eq!(addrs, vec![SocketAddr::from(([127, 0, 0, 1], 443))]);
    assert!(
        started_at.elapsed() < Duration::from_millis(250),
        "DNS resolver should not wait for TCP connect worker release"
    );
    assert_eq!(
        tcp_started.load(Ordering::SeqCst),
        1,
        "TCP connect worker should remain independently occupied"
    );

    let (released, ready) = &*gate;
    *released
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
    ready.notify_all();
    tcp_done_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("TCP connect job should finish after release");
}
