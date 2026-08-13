use super::*;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;

fn wait_for_test_writes_to_finish() {
    let deadline = Instant::now() + Duration::from_secs(1);
    while HEARTBEAT_TEST_ACTIVE_WRITES.load(Ordering::SeqCst) != 0 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(HEARTBEAT_TEST_ACTIVE_WRITES.load(Ordering::SeqCst), 0);
}

#[test]
fn stalled_heartbeat_write_returns_bounded_unhealthy_error() {
    let _test_lock = lock_heartbeat_test_state();
    let root = std::env::temp_dir().join(format!(
        "prodex-secret-store-heartbeat-stall-{}-{}",
        std::process::id(),
        FENCE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).unwrap();
    #[cfg(unix)]
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let root = fs::canonicalize(root).unwrap();
    let coordinator = RefreshLeaseCoordinator::new(&root)
        .with_lease_ttl(Duration::from_millis(30))
        .with_wait_timeout(Duration::ZERO);
    let owner = match coordinator.acquire("heartbeat-stall-test") {
        Ok(RefreshLeaseDecision::Owner(owner)) => owner,
        other => panic!("expected owner, got {other:?}"),
    };
    let id = owner.heartbeat.as_ref().expect("heartbeat is active").id;
    HEARTBEAT_TEST_WRITE_CALLS.store(0, Ordering::SeqCst);
    heartbeat_test_set_stalled_ids([id]);
    thread::sleep(HEARTBEAT_WRITE_DEADLINE + HEARTBEAT_SCHEDULER_TICK * 3);

    let (finished, received) = mpsc::sync_channel(1);
    let handle = thread::spawn(move || {
        finished
            .send(owner.commit_result("{\"access_token\":\"stall\"}"))
            .unwrap();
    });
    let elapsed_start = Instant::now();
    let result = received.recv_timeout(Duration::from_secs(1));
    heartbeat_test_set_stalled_ids([]);
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            let _ = handle.join();
            panic!("stalled heartbeat shutdown exceeded bound: {error}");
        }
    };
    handle.join().unwrap();
    wait_for_test_writes_to_finish();

    assert!(elapsed_start.elapsed() < Duration::from_secs(1));
    assert!(matches!(
        result,
        Err(RefreshLeaseError::Io {
            kind: RefreshLeaseIoKind::Generic,
            ..
        })
    ));
    assert_eq!(HEARTBEAT_TEST_WRITE_CALLS.load(Ordering::SeqCst), 1);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn stalled_heartbeat_writes_cannot_accumulate_workers() {
    let _test_lock = lock_heartbeat_test_state();
    let root = std::env::temp_dir().join(format!(
        "prodex-secret-store-heartbeat-workers-{}-{}",
        std::process::id(),
        FENCE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).unwrap();
    #[cfg(unix)]
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let root = fs::canonicalize(root).unwrap();
    let coordinator = RefreshLeaseCoordinator::new(&root)
        .with_lease_ttl(Duration::from_millis(3))
        .with_wait_timeout(Duration::ZERO);
    wait_for_test_writes_to_finish();
    HEARTBEAT_TEST_ACTIVE_WRITES.store(0, Ordering::SeqCst);
    HEARTBEAT_TEST_MAX_ACTIVE_WRITES.store(0, Ordering::SeqCst);
    HEARTBEAT_TEST_WRITE_CALLS.store(0, Ordering::SeqCst);
    let mut owners = Vec::new();
    for index in 0..8 {
        match coordinator.acquire(format!("heartbeat-workers-test-{index}")) {
            Ok(RefreshLeaseDecision::Owner(owner)) => owners.push(owner),
            other => panic!("expected owner, got {other:?}"),
        }
    }
    heartbeat_test_set_stalled_ids(
        owners
            .iter()
            .map(|owner| owner.heartbeat.as_ref().expect("heartbeat is active").id),
    );

    thread::sleep(HEARTBEAT_WRITE_DEADLINE + HEARTBEAT_SCHEDULER_TICK * 3);
    let maximum_active_writes = HEARTBEAT_TEST_MAX_ACTIVE_WRITES.load(Ordering::SeqCst);

    heartbeat_test_set_stalled_ids([]);
    for mut owner in owners {
        let _ = owner.release();
    }
    wait_for_test_writes_to_finish();
    assert!(
        (1..=HEARTBEAT_WORKER_COUNT as u64).contains(&maximum_active_writes),
        "stalled writes should occupy only the fixed worker pool: {maximum_active_writes}"
    );
    let _ = fs::remove_dir_all(root);
}
