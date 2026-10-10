use super::helpers::*;
use super::*;

fn wait_for_admission_log_count(
    shared: &RuntimeRotationProxyShared,
    marker: &str,
    minimum: usize,
) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        runtime_proxy_flush_logs_for_path(&shared.log_path).expect("flush isolated admission log");
        let log = std::fs::read_to_string(&shared.log_path).unwrap_or_default();
        if log.matches(marker).count() >= minimum {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn admission_retry_never_reads_state_under_notification_mutex() {
    let _budget_guard = ci_runtime_proxy_admission_wait_budget_guard(10, 10);
    for transport in ["http", "websocket"] {
        let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
            "main", "main-account", "main@example.com",
        )
        .active_request_limit(1)
        .build();
        let shared = harness.shared();
        let path = "/backend-api/codex/responses";
        let held = try_acquire_runtime_proxy_active_request_slot(shared, "http", path)
            .expect("hold the only global admission slot");
        let (notification_mutex, _) = shared.lane_admission.wait();
        let notification_guard = notification_mutex.lock().unwrap();
        let worker_shared = shared.clone();
        let (finished, completion) = std::sync::mpsc::channel();
        let waiter = thread::spawn(move || {
            let request = RuntimeProxyRequest {
                method: "POST".to_string(),
                path_and_query: path.to_string(),
                headers: Vec::new(),
                body: br#"{"input":"synthetic fresh request"}"#.to_vec(),
            };
            let result = acquire_runtime_proxy_active_request_slot_with_wait_for_request(
                &worker_shared, transport, path, Some(&request),
            ).map(drop);
            let _ = finished.send(result);
        });
        // The first probe has read runtime state and found a full admission
        // counter; prevent its second probe from starting until a writer owns
        // runtime state. This is the production lock inversion schedule.
        let first_probe = wait_for_admission_log_count(
            shared, "runtime_proxy_admission_wait_started", 1,
        );
        let state_writer = shared.runtime.lock().unwrap();
        drop(notification_guard);
        let rechecked_without_state_lock = wait_for_admission_log_count(
            shared, "runtime_proxy_active_limit_reached", 2,
        );
        let notifier_available = notification_mutex.try_lock().is_ok();
        // Always rescue an incorrect implementation before asserting so a
        // failing test cannot strand a thread or hang the remaining suite.
        drop(state_writer);
        drop(held);
        let recovered = completion.recv_timeout(Duration::from_secs(3));
        waiter.join().expect("admission waiter should finish after capacity returns");
        assert!(first_probe, "the regression must exercise the saturated path");
        assert!(recovered.is_ok_and(|result| result.is_ok()));
        assert!(
            rechecked_without_state_lock && notifier_available,
            "{transport}: admission reread runtime while holding the notification mutex;              a quota/backoff state writer notifying selection would deadlock the entire proxy",
        );
    }
}

#[test]
fn admission_retry_refreshes_owned_affinity_after_selection_notification() {
    let _budget_guard = ci_runtime_proxy_admission_wait_budget_guard(10, 10);
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main", "main-account", "main@example.com",
    )
    .active_request_limit(4)
    .build();
    let shared = harness.shared();
    let path = "/backend-api/codex/responses";
    let limit = shared.lane_admission.limit(RuntimeRouteKind::Responses);
    let lane_counter = shared.lane_admission.active_counter(RuntimeRouteKind::Responses);
    lane_counter.store(limit, Ordering::SeqCst);
    let worker_shared = shared.clone();
    let (done, received) = std::sync::mpsc::channel();
    let waiter = thread::spawn(move || {
        let request = RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: path.to_string(),
            headers: Vec::new(),
            body: br#"{"previous_response_id":"resp-new-owner","input":"continue"}"#.to_vec(),
        };
        let result = acquire_runtime_proxy_active_request_slot_with_wait_for_request(
            &worker_shared, "http", path, Some(&request),
        ).map(drop);
        let _ = done.send(result);
    });
    let entered_wait = wait_for_admission_log_count(shared, "runtime_proxy_admission_wait_started", 1);
    {
        let mut runtime = shared.runtime.lock().unwrap();
        runtime.state.response_profile_bindings.insert(
            "resp-new-owner".to_string(),
            ResponseProfileBinding {
                binding_identity: None,
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        );
    }
    shared.lane_admission.notify_selection_change();
    let owned_request_recovered = received.recv_timeout(Duration::from_secs(2))
        .is_ok_and(|result| result.is_ok());
    // Rescue a stale-snapshot implementation before asserting, without
    // relaxing the contract that ownership alone must have unblocked it.
    lane_counter.store(0, Ordering::SeqCst);
    shared.lane_admission.notify_selection_change();
    waiter.join().expect("waiter must stop after rescue capacity is provided");
    assert!(entered_wait, "exercise an initially unknown owner under pressure");
    assert!(owned_request_recovered, "refresh ownership on wake without holding the notification mutex");
    assert_eq!(shared.active_request_count.load(Ordering::SeqCst), 0);
}
