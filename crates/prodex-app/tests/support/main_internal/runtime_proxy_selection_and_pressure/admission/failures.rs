use super::helpers::quota_window_exhausted;
use super::{
    Duration, ResponseProfileBinding, RuntimeProxyBackend, RuntimeProxyBackendFaultRoute,
    RuntimeProxyBackendFaultScript, RuntimeProxyBackendFaultStep, RuntimeProxyMarkerGuard,
    RuntimeProxyProfileHarness, RuntimeProxyProfileHarnessBuilder, RuntimeProxyRequest,
    RuntimeRouteKind, TestEnvVarGuard, closed_loopback_backend_base_url,
    proxy_runtime_standard_request, quota_window_ready, read_runtime_proxy_test_log,
    register_runtime_proxy_persistence_mode, runtime_has_route_eligible_quota_fallback_for_model,
    runtime_profile_route_circuit_key, runtime_usage_snapshot,
    test_runtime_compact_quota_fallback_exhausted, tiny_http_response_status_and_body,
};
use chrono::Local;
use std::collections::BTreeSet;
use std::time::Instant;

fn two_ready_profiles(backend: &RuntimeProxyBackend) -> RuntimeProxyProfileHarness {
    let ready = runtime_usage_snapshot(
        quota_window_ready(80, 3_600),
        quota_window_ready(80, 86_400),
    );
    RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready.clone())
        .profile_usage_snapshot("second", ready)
        .build()
}

#[test]
fn compact_quota_fallback_respects_request_local_exclusions() {
    let backend = RuntimeProxyBackend::start();
    let ready = runtime_usage_snapshot(
        quota_window_ready(80, 3_600),
        quota_window_ready(80, 86_400),
    );
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready.clone())
        .profile_usage_snapshot("second", ready)
        .build();
    let excluded = BTreeSet::from(["second".to_string()]);

    assert!(
        runtime_has_route_eligible_quota_fallback_for_model(
            harness.shared(),
            "main",
            &BTreeSet::new(),
            RuntimeRouteKind::Compact,
            None,
        )
        .expect("global quota fallback lookup should succeed"),
        "second profile should be globally eligible before request-local exclusion"
    );
    assert!(
        test_runtime_compact_quota_fallback_exhausted(harness.shared(), "main", &excluded, None,)
            .expect("request-local quota fallback decision should succeed"),
        "a profile already quota-failed in this request must make the compact fallback exhausted"
    );
}

fn compact_request(session_id: Option<&str>) -> RuntimeProxyRequest {
    let mut headers = vec![
        ("Content-Type".to_string(), "application/json".to_string()),
        ("x-openai-subagent".to_string(), "compact".to_string()),
    ];
    if let Some(session_id) = session_id {
        headers.push(("session_id".to_string(), session_id.to_string()));
    }
    RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers,
        body: br#"{"input":[],"instructions":"compact"}"#.to_vec(),
    }
}

fn compact_request_with_turn_state(turn_state: &str) -> RuntimeProxyRequest {
    let mut request = compact_request(None);
    request
        .headers
        .push(("x-codex-turn-state".to_string(), turn_state.to_string()));
    request
}

fn bind_session(harness: &RuntimeProxyProfileHarness, session_id: &str, profile_name: &str) {
    harness
        .shared()
        .runtime
        .lock()
        .unwrap()
        .session_id_bindings
        .insert(
            session_id.to_string(),
            ResponseProfileBinding {
                binding_identity: None,
                profile_name: profile_name.to_string(),
                bound_at: Local::now().timestamp(),
            },
        );
}

fn bind_response(harness: &RuntimeProxyProfileHarness, response_id: &str, profile_name: &str) {
    harness
        .shared()
        .runtime
        .lock()
        .unwrap()
        .state
        .response_profile_bindings
        .insert(
            response_id.to_string(),
            ResponseProfileBinding {
                binding_identity: None,
                profile_name: profile_name.to_string(),
                bound_at: Local::now().timestamp(),
            },
        );
}

fn compact_request_with_previous_response(previous_response_id: &str) -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("x-openai-subagent".to_string(), "compact".to_string()),
        ],
        body: format!(
            r#"{{"input":[],"instructions":"compact","previous_response_id":"{previous_response_id}"}}"#
        )
        .into_bytes(),
    }
}

#[test]
fn fresh_noncompact_transport_failure_rotates_through_ready_profiles() {
    let ready = runtime_usage_snapshot(
        quota_window_ready(80, 3_600),
        quota_window_ready(80, 86_400),
    );
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(closed_loopback_backend_base_url())
        .profile_usage_snapshot("main", ready.clone())
        .profile_usage_snapshot("second", ready)
        .build();
    let request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/status".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let response = proxy_runtime_standard_request(54, &request, harness.shared())
        .expect("fresh transport failures should remain inside the rotation loop");
    let (status, _) = tiny_http_response_status_and_body(response);
    let log = read_runtime_proxy_test_log(&harness.shared().log_path);

    assert_eq!(status, 503, "{log}");
    assert!(
        log.contains("standard_transport_failure profile=main")
            && log.contains("standard_transport_failure profile=second"),
        "every ready profile should receive a bounded precommit attempt: {log}"
    );
}

#[test]
fn fresh_noncompact_cold_start_probe_wait_is_one_shot() {
    let backend = RuntimeProxyBackend::start();
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot(
            "main",
            runtime_usage_snapshot(
                quota_window_exhausted(3_600),
                quota_window_ready(80, 86_400),
            ),
        )
        .build();
    let shared = harness.shared();
    let circuit_key = runtime_profile_route_circuit_key("second", RuntimeRouteKind::Standard);
    shared
        .runtime
        .lock()
        .expect("runtime lock should succeed")
        .profile_route_circuit_open_until
        .insert(circuit_key.clone(), Local::now().timestamp() + 60);
    let request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/status".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let response = std::thread::scope(|scope| {
        let release = scope.spawn(|| {
            // Hold the circuit until the real recovery phase is observable;
            // crossing a wall-clock second must not skip the path under test.
            let deadline = Instant::now() + Duration::from_secs(5);
            let saw_wait = loop {
                let log = std::fs::read_to_string(&shared.log_path).unwrap_or_default();
                if log.lines().any(|line| {
                    line.contains("rotation_waiting_for_recovery")
                        && line.contains("route=standard")
                }) {
                    break true;
                }
                if Instant::now() >= deadline {
                    break false;
                }
                std::thread::sleep(Duration::from_millis(5));
            };
            shared
                .runtime
                .lock()
                .unwrap()
                .profile_route_circuit_open_until
                .remove(&circuit_key);
            shared.lane_admission.notify_selection_change();
            saw_wait
        });
        let response = proxy_runtime_standard_request(86, &request, shared)
            .expect("cold-start recovery should reselect the profile after its circuit clears");
        assert!(
            release.join().unwrap(),
            "the real recovery wait must be exercised"
        );
        response
    });
    let (status, body) = tiny_http_response_status_and_body(response);
    let log = read_runtime_proxy_test_log(&shared.log_path);

    assert_eq!(status, 200, "{body}\n{log}");
    assert!(
        body.contains("second-account"),
        "recovered cold-start profile should serve the request: {body}"
    );
    let recovery_waits = log
        .lines()
        .filter(|line| {
            line.contains("rotation_waiting_for_recovery") && line.contains("route=standard")
        })
        .count();
    assert_eq!(
        recovery_waits, 1,
        "temporarily unavailable cold-start profile should require exactly one recovery wait: {log}"
    );
    assert!(
        !log.contains("precommit_budget_exhausted"),
        "successful recovery must not leak or record terminal retry-budget exhaustion: {log}"
    );
}

#[path = "failures/compact.rs"]
mod compact;
