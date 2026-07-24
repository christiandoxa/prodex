use super::*;
use std::fs;
use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

#[test]
fn websocket_dns_resolution_times_out_on_bounded_executor() {
    let executor = RuntimeWebsocketDnsResolveExecutor::new_with_overflow_capacity(1, 1, 0);
    let log_path = websocket_test_log_path("dns-timeout");
    let _ = fs::remove_file(&log_path);
    let (release_tx, release_rx) = mpsc::channel::<()>();

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
            let _ = release_rx.recv_timeout(Duration::from_secs(5));
            Ok(Vec::new())
        },
    )
    .expect_err("slow websocket DNS resolution should time out");

    assert_eq!(
        err.kind(),
        io::ErrorKind::WouldBlock,
        "bounded DNS timeout should fail as local pressure, not account quota"
    );
    let _ = release_tx.send(());

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

    let _ = release_tx.send(());
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
