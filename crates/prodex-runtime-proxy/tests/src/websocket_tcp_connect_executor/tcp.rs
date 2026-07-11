use super::*;
use std::fs;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::Duration;

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

    for index in 0..6 {
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

        if index == 0 {
            start_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("first websocket connect job should start");
        }
    }

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
