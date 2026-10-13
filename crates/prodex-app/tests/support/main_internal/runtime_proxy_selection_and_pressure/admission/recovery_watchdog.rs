use super::helpers::*;
use super::*;
use crate::runtime_proxy::{RuntimeProxyRecoveryTimeout, await_runtime_proxy_async_task};
use std::sync::atomic::{AtomicBool, Ordering};

fn request() -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: "POST".into(), path_and_query: "/backend-api/codex/responses".into(),
        headers: vec![("Content-Type".into(),"application/json".into())],
        body: br#"{"input":[]}"#.to_vec(),
    }
}

#[test]
fn recovery_watchdog_is_request_local_and_not_reset_by_retry_clones() {
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .active_profile("main").current_profile("main").build();
    let global = harness.shared();
    assert!(global.precommit_deadline.is_none());
    let first = global.for_precommit_request();
    assert!(first.precommit_deadline.is_some());
    let next_attempt = first.for_precommit_request();
    assert_eq!(first.precommit_deadline, next_attempt.precommit_deadline);
    assert!(global.precommit_deadline.is_none(), "another request must not inherit an expired deadline");
}

#[test]
fn recovery_watchdog_cancels_precommit_async_wait_instead_of_orphaning_it() {
    struct Dropped(Arc<AtomicBool>);
    impl Drop for Dropped {
        fn drop(&mut self) { self.0.store(true, Ordering::SeqCst); }
    }
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .active_profile("main").current_profile("main").build();
    let mut shared = harness.shared().clone();
    shared.precommit_deadline = Some(Instant::now() + Duration::from_millis(80));
    let dropped = Arc::new(AtomicBool::new(false));
    let guard = Dropped(dropped.clone());
    let started = Instant::now();
    let result = await_runtime_proxy_async_task(&shared, "synthetic_stalled_precommit", async move {
        let _guard = guard;
        tokio::time::sleep(Duration::from_secs(10)).await;
        Ok(())
    });
    assert!(result.unwrap_err().is::<RuntimeProxyRecoveryTimeout>());
    assert!(started.elapsed() < Duration::from_secs(2));
    for _ in 0..100 {
        if dropped.load(Ordering::SeqCst) { break; }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(dropped.load(Ordering::SeqCst), "timed out async work must be cancelled, not detached");
}

#[test]
fn saturated_pool_watchdog_leaves_quota_intact_and_next_request_usable() {
    let backend = RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
        RuntimeProxyBackendFaultStep::sse_success(RuntimeProxyBackendFaultRoute::Responses, "main-account", "healthy-after-capacity-release")
    ]));
    let ready = runtime_usage_snapshot(quota_window_ready(80, 3600), quota_window_ready(80, 86400));
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .active_profile("main").current_profile("main").upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready).build();
    let mut scoped = harness.shared().clone();
    scoped.precommit_deadline = Some(Instant::now() + Duration::from_millis(100));
    let limit = scoped.runtime_config.tuning.profile_inflight_hard_limit;
    scoped.lane_admission.set_profile_inflight("main", limit);
    let err = match proxy_runtime_responses_request(1200, &request(), &scoped) {
        Err(error) => error,
        Ok(_) => panic!("permanently saturated pool must not occupy the request forever"),
    };
    assert!(err.is::<RuntimeProxyRecoveryTimeout>(), "{err:#}");
    assert!(backend.responses_accounts().is_empty(), "local waiting is not an upstream attempt");
    scoped.lane_admission.set_profile_inflight("main", 0);
    scoped.lane_admission.record_inflight_release();
    let response = proxy_runtime_responses_request(1201, &request(), harness.shared()).unwrap();
    let status = match response {
        RuntimeResponsesReply::Buffered(parts) => parts.status,
        RuntimeResponsesReply::Streaming(mut reply) => {
            let mut body = String::new();
            reply.body.read_to_string(&mut body).unwrap();
            reply.status
        }
    };
    assert_eq!(status, 200);
    assert_eq!(scoped.lane_admission.profile_inflight_count("main"), 0);
    assert_eq!(backend.responses_accounts(), ["main-account"]);
    let log = read_runtime_proxy_test_log(&scoped.log_path);
    assert!(!log.contains("quota_quarantine"), "a local watchdog must not exhaust or penalize account quota: {log}");
}

#[test]
fn committed_stream_remains_readable_after_precommit_watchdog_expires() {
    let backend = RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
        RuntimeProxyBackendFaultStep::sse_success(RuntimeProxyBackendFaultRoute::Responses, "main-account", "committed-survives")
    ]));
    let ready = runtime_usage_snapshot(quota_window_ready(80, 3600), quota_window_ready(80, 86400));
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .active_profile("main").current_profile("main").upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready).build();
    let mut scoped = harness.shared().clone();
    scoped.precommit_deadline = Some(Instant::now() + Duration::from_secs(1));
    let RuntimeResponsesReply::Streaming(mut response) = proxy_runtime_responses_request(1202, &request(), &scoped).unwrap() else {
        panic!("fixture must commit a stream");
    };
    let wait = scoped.precommit_deadline.unwrap().saturating_duration_since(Instant::now());
    thread::sleep(wait + Duration::from_millis(20));
    let mut body = String::new();
    response.body.read_to_string(&mut body).unwrap();
    assert_eq!(response.status, 200);
    assert!(body.contains("committed-survives"), "{body}");
    assert_eq!(backend.responses_accounts(), ["main-account"]);
}

#[test]
fn retryable_profile_cooldown_outliving_one_wait_epoch_does_not_surface_local_503() {
    let ready = runtime_usage_snapshot(quota_window_ready(87, 3600), quota_window_ready(91, 86400));
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .active_profile("main").current_profile("main").profile_usage_snapshot("main", ready).build();
    let mut scoped = harness.shared().clone();
    scoped.precommit_deadline = Some(Instant::now() + Duration::from_secs(60));
    {
        let mut runtime = scoped.runtime.lock().unwrap();
        runtime.profile_retry_backoff_until.insert("main".into(), Local::now().timestamp() + 90);
    }
    let (retry_pending, exclusion_cleared, sweeps) = crate::runtime_proxy::test_runtime_precommit_retryable_recovery_after_transport(
        &scoped, RuntimeRouteKind::Responses, "main",
    ).unwrap();
    assert!(retry_pending, "a 30-second wait epoch is not a terminal 90-second account cooldown");
    assert!(!exclusion_cleared, "must honor the provider cooldown, not retry the blocked account early");
    assert_eq!(sweeps, 1);
}
