use super::*;
use std::io::Write;
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

static CONTINUITY_CACHE_TEST_LOCK: Mutex<()> = Mutex::new(());

fn lock_continuity_cache() -> MutexGuard<'static, ()> {
    CONTINUITY_CACHE_TEST_LOCK
        .lock()
        .expect("continuity cache test lock should be available")
}

fn temp_log_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "prodex-runtime-broker-log-{name}-{}-{nanos}.log",
        std::process::id()
    ))
}

#[test]
fn continuity_failure_reason_metrics_follow_runtime_log_summary() {
    let _guard = lock_continuity_cache();
    clear_runtime_broker_continuity_failure_reason_cache_for_test();
    let log_path = temp_log_path("reasons");
    fs::write(
            &log_path,
            concat!(
                "[2026-04-22 10:00:00.000 +00:00] request=3 transport=websocket route=websocket websocket_session=sess-1 chain_retried_owner profile=second previous_response_id=resp-second delay_ms=20 reason=previous_response_not_found_locked_affinity via=-\n",
                "[2026-04-22 10:00:00.001 +00:00] request=3 websocket_session=sess-1 stale_continuation reason=websocket_reuse_watchdog_locked_affinity profile=second event=timeout\n",
                "[2026-04-22 10:00:00.002 +00:00] request=4 transport=http route=responses stale_continuation reason=previous_response_not_found profile=main\n",
                "[2026-04-22 10:00:00.003 +00:00] request=3 transport=websocket route=websocket websocket_session=sess-1 chain_dead_upstream_confirmed profile=second previous_response_id=resp-second reason=previous_response_not_found_locked_affinity via=- event=-\n"
            ),
        )
        .expect("test log should write");

    let metrics = runtime_broker_cached_continuity_failure_reason_metrics(&log_path);

    assert_eq!(
        metrics.chain_retried_owner,
        BTreeMap::from([("previous_response_not_found_locked_affinity".to_string(), 1)])
    );
    assert_eq!(
        metrics.chain_dead_upstream_confirmed,
        BTreeMap::from([("previous_response_not_found_locked_affinity".to_string(), 1)])
    );
    assert_eq!(
        metrics.stale_continuation,
        BTreeMap::from([
            ("previous_response_not_found".to_string(), 1),
            ("websocket_reuse_watchdog_locked_affinity".to_string(), 1),
        ])
    );

    fs::remove_file(&log_path).expect("test log should clean up");
}

#[test]
fn continuity_failure_reason_metrics_reuse_cached_summary_for_unchanged_log() {
    let _guard = lock_continuity_cache();
    clear_runtime_broker_continuity_failure_reason_cache_for_test();
    let log_path = temp_log_path("cache-hit");
    fs::write(
            &log_path,
            "[2026-04-22 10:00:00.000 +00:00] request=8 transport=http route=responses stale_continuation reason=previous_response_not_found profile=main\n",
        )
        .expect("test log should write");

    let first = runtime_broker_cached_continuity_failure_reason_metrics(&log_path);
    assert_eq!(
        runtime_broker_continuity_failure_reason_cache_stats_for_test(),
        RuntimeBrokerContinuityFailureReasonCacheStats {
            full_rebuilds: 1,
            incremental_updates: 0,
            hits: 0,
            misses: 1,
            entries: 1,
        }
    );

    let second = runtime_broker_cached_continuity_failure_reason_metrics(&log_path);
    assert_eq!(second, first);
    assert_eq!(
        runtime_broker_continuity_failure_reason_cache_stats_for_test(),
        RuntimeBrokerContinuityFailureReasonCacheStats {
            full_rebuilds: 1,
            incremental_updates: 0,
            hits: 1,
            misses: 1,
            entries: 1,
        }
    );

    fs::remove_file(&log_path).expect("test log should clean up");
}

#[test]
fn continuity_failure_reason_metrics_incrementally_update_when_log_appends() {
    let _guard = lock_continuity_cache();
    clear_runtime_broker_continuity_failure_reason_cache_for_test();
    let log_path = temp_log_path("cache-invalidate");
    fs::write(
            &log_path,
            "[2026-04-22 10:00:00.000 +00:00] request=5 transport=http route=responses stale_continuation reason=previous_response_not_found profile=main\n",
        )
        .expect("test log should write");

    let first = runtime_broker_cached_continuity_failure_reason_metrics(&log_path);
    assert_eq!(
        first.stale_continuation,
        BTreeMap::from([("previous_response_not_found".to_string(), 1)])
    );
    assert_eq!(
        runtime_broker_continuity_failure_reason_cache_stats_for_test(),
        RuntimeBrokerContinuityFailureReasonCacheStats {
            full_rebuilds: 1,
            incremental_updates: 0,
            hits: 0,
            misses: 1,
            entries: 1,
        }
    );

    let mut log = fs::OpenOptions::new()
        .append(true)
        .open(&log_path)
        .expect("test log should open");
    writeln!(
            log,
            "[2026-04-22 10:00:00.001 +00:00] request=5 transport=websocket route=websocket websocket_session=sess-5 chain_retried_owner profile=main previous_response_id=resp-5 delay_ms=20 reason=previous_response_not_found_locked_affinity via=-"
        )
        .expect("test log should append");
    drop(log);

    let second = runtime_broker_cached_continuity_failure_reason_metrics(&log_path);
    assert_eq!(
        second.chain_retried_owner,
        BTreeMap::from([("previous_response_not_found_locked_affinity".to_string(), 1)])
    );
    assert_eq!(
        second.stale_continuation,
        BTreeMap::from([("previous_response_not_found".to_string(), 1)])
    );
    assert_eq!(
        runtime_broker_continuity_failure_reason_cache_stats_for_test(),
        RuntimeBrokerContinuityFailureReasonCacheStats {
            full_rebuilds: 1,
            incremental_updates: 1,
            hits: 0,
            misses: 2,
            entries: 1,
        }
    );

    fs::remove_file(&log_path).expect("test log should clean up");
}

#[test]
fn continuity_failure_reason_metrics_wait_for_complete_appended_line() {
    let _guard = lock_continuity_cache();
    clear_runtime_broker_continuity_failure_reason_cache_for_test();
    let log_path = temp_log_path("partial-line");
    fs::write(
        &log_path,
        "stale_continuation reason=previous_response_not_found",
    )
    .expect("partial test log should write");

    let partial = runtime_broker_cached_continuity_failure_reason_metrics(&log_path);
    assert!(partial.stale_continuation.is_empty());

    let mut log = fs::OpenOptions::new()
        .append(true)
        .open(&log_path)
        .expect("partial test log should open");
    writeln!(log).expect("partial test log should complete");
    drop(log);

    let complete = runtime_broker_cached_continuity_failure_reason_metrics(&log_path);
    assert_eq!(
        complete.stale_continuation,
        BTreeMap::from([("previous_response_not_found".to_string(), 1)])
    );

    fs::remove_file(&log_path).expect("test log should clean up");
}

#[test]
fn continuity_failure_reason_metrics_skip_oversized_line_and_continue() {
    let _guard = lock_continuity_cache();
    clear_runtime_broker_continuity_failure_reason_cache_for_test();
    let log_path = temp_log_path("oversized-line");
    let mut content = vec![b'x'; RUNTIME_BROKER_LOG_LINE_MAX_BYTES + 1];
    content.extend_from_slice(b"\nstale_continuation reason=previous_response_not_found\n");
    fs::write(&log_path, content).expect("oversized test log should write");

    let metrics = runtime_broker_cached_continuity_failure_reason_metrics(&log_path);

    assert_eq!(
        metrics.stale_continuation,
        BTreeMap::from([("previous_response_not_found".to_string(), 1)])
    );
    fs::remove_file(&log_path).expect("test log should clean up");
}
