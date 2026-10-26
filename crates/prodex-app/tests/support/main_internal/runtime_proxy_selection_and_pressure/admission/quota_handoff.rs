//! Real local-upstream regressions for quota recovery before sending a continuation.
use super::helpers::*;
use super::*;

fn ready() -> RuntimeProfileUsageSnapshot {
    runtime_usage_snapshot(
        quota_window_ready(80, 3_600),
        quota_window_ready(80, 86_400),
    )
}

fn exhausted() -> RuntimeProfileUsageSnapshot {
    runtime_usage_snapshot(
        quota_window_exhausted(3_600),
        quota_window_ready(80, 86_400),
    )
}

fn bind_owner(harness: &RuntimeProxyProfileHarness) {
    let now = Local::now().timestamp();
    let mut runtime = harness.shared().runtime.lock().unwrap();
    runtime.state.response_profile_bindings.insert(
        "resp-owned".into(),
        ResponseProfileBinding {
            binding_identity: None,
            profile_name: "main".into(),
            bound_at: now,
        },
    );
    assert!(runtime_mark_continuation_status_verified(
        &mut runtime.continuation_statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-owned",
        now,
        Some(RuntimeRouteKind::Responses),
    ));
}

fn continuation() -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: "POST".into(), path_and_query: "/backend-api/codex/responses".into(),
        headers: vec![("Content-Type".into(), "application/json".into())],
        body: br#"{"session_id":"session-owned","previous_response_id":"resp-owned","input":[{"type":"function_call_output","call_id":"call-local","output":"completed work"}]}"#.to_vec(),
    }
}

fn assert_retry(response: RuntimeResponsesReply) {
    let RuntimeResponsesReply::Buffered(parts) = response else {
        panic!("expected native full-history retry signal")
    };
    assert_eq!(parts.status, 400);
    let body: serde_json::Value = serde_json::from_slice(&parts.body).unwrap();
    assert_eq!(body["error"]["code"], "previous_response_not_found");
}

#[test]
fn quota_fallback_rejects_known_exhaustion_without_requiring_positive_evidence_04392() {
    for empty in [
        exhausted(),
        runtime_usage_snapshot(
            quota_window_ready(80, 3_600),
            quota_window_exhausted(86_400),
        ),
    ] {
        let harness = RuntimeProxyProfileHarnessBuilder::new()
            .openai_profile("main", "main-account", Some("main@example.com"))
            .openai_profile("second", "second-account", Some("second@example.com"))
            .active_profile("main")
            .current_profile("main")
            .profile_usage_snapshot("main", ready())
            .profile_usage_snapshot("second", empty)
            .build();
        for route in [
            RuntimeRouteKind::Responses,
            RuntimeRouteKind::Websocket,
            RuntimeRouteKind::Compact,
        ] {
            assert!(
                !runtime_has_route_eligible_quota_fallback_for_model(
                    harness.shared(),
                    "main",
                    &BTreeSet::new(),
                    route,
                    None,
                )
                .unwrap(),
                "known exhausted quota must not masquerade as an unknown, eligible alternative: {route:?}"
            );
        }
    }
}

#[test]
fn responses_exhausted_owner_requests_replay_before_upstream_04392() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::rate_limited_429_for_delay(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
                400,
            ),
        ]));
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", exhausted())
        .profile_usage_snapshot("second", ready())
        .build();
    bind_owner(&harness);
    assert_retry(proxy_runtime_responses_request(921, &continuation(), harness.shared()).unwrap());
    assert!(
        backend.responses_accounts().is_empty(),
        "known exhausted owner must not receive the request or expose its 400-second throttle"
    );
    let mut replay = continuation();
    replay.body = br#"{"session_id":"session-owned","input":[{"role":"user","content":"original task"},{"role":"assistant","content":"completed work"},{"role":"user","content":"continue once"}]}"#.to_vec();
    let reply = proxy_runtime_responses_request(922, &replay, harness.shared()).unwrap();
    let status = match reply {
        RuntimeResponsesReply::Buffered(parts) => parts.status,
        RuntimeResponsesReply::Streaming(mut response) => {
            let mut body = String::new();
            response.body.read_to_string(&mut body).unwrap();
            response.status
        }
    };
    assert_eq!(status, 200);
    assert_eq!(backend.responses_accounts(), ["second-account"]);
    let bodies = backend.responses_bodies();
    assert_eq!(bodies.len(), 1);
    assert!(!bodies[0].contains("previous_response_id"));
    assert_eq!(bodies[0].matches("continue once").count(), 1);
}

#[test]
fn responses_rate_limited_owner_keeps_busy_quota_positive_fallback_recoverable_04392() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::rate_limited_429_for_delay(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
                400,
            ),
        ]));
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready())
        .profile_usage_snapshot("second", ready())
        .build();
    bind_owner(&harness);
    harness
        .shared()
        .lane_admission
        .set_profile_inflight("second", usize::MAX / 2);
    assert_retry(proxy_runtime_responses_request(923, &continuation(), harness.shared()).unwrap());
    assert_eq!(backend.responses_accounts(), ["main-account"]);
    assert!(
        runtime_profile_inflight_hard_limited_for_context(harness.shared(), "second", "responses")
            .unwrap(),
        "recovery eligibility must not bypass actual admission"
    );
    harness
        .shared()
        .lane_admission
        .set_profile_inflight("second", 0);
    harness.shared().lane_admission.record_inflight_release();
}

#[test]
fn quota_preflight_preserves_healthy_owner_and_original_delta_04392() {
    // The default synthetic backend deliberately exhausts main; override that
    // endpoint to represent a genuinely healthy owner for this negative control.
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::success(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
            ),
        ]));
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready())
        .profile_usage_snapshot("second", ready())
        .build();
    bind_owner(&harness);
    let response = proxy_runtime_responses_request(924, &continuation(), harness.shared()).unwrap();
    let status = match response {
        RuntimeResponsesReply::Buffered(parts) => parts.status,
        RuntimeResponsesReply::Streaming(mut response) => {
            let mut body = String::new();
            response.body.read_to_string(&mut body).unwrap();
            response.status
        }
    };
    assert_eq!(status, 200);
    assert_eq!(backend.responses_accounts(), ["main-account"]);
    assert!(backend.responses_bodies()[0].contains("resp-owned"));
}

#[test]
fn quota_preflight_without_available_pool_preserves_upstream_failure_04392() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::explicit_quota_429(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
            ),
        ]));
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", exhausted())
        .profile_usage_snapshot("second", exhausted())
        .build();
    bind_owner(&harness);
    let response = proxy_runtime_responses_request(925, &continuation(), harness.shared()).unwrap();
    let RuntimeResponsesReply::Buffered(parts) = response else {
        panic!("expected original upstream quota error")
    };
    assert_eq!(parts.status, 429);
    assert_eq!(backend.responses_accounts(), ["main-account"]);
    assert!(!String::from_utf8_lossy(&parts.body).contains("previous_response_not_found"));
}

#[test]
fn quota_preflight_unknown_windows_do_not_trigger_owner_handoff_04392() {
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .profile_usage_snapshot("second", ready())
        .build();
    for route in [RuntimeRouteKind::Responses, RuntimeRouteKind::Websocket] {
        assert!(
            !runtime_quota_exhausted_with_recoverable_alternative(
                harness.shared(),
                "main",
                &BTreeSet::new(),
                route,
                None,
            )
            .unwrap(),
            "absence of quota evidence is not an exhausted account"
        );
    }
}

#[test]
fn recovery_alternative_cannot_hide_exhausted_weekly_window_04392() {
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .profile_usage_snapshot("main", exhausted())
        .profile_usage_snapshot(
            "second",
            runtime_usage_snapshot(
                quota_window_ready(80, 3_600),
                quota_window_exhausted(86_400),
            ),
        )
        .build();
    for route in [
        RuntimeRouteKind::Responses,
        RuntimeRouteKind::Websocket,
        RuntimeRouteKind::Compact,
    ] {
        assert!(
            !runtime_has_route_recoverable_quota_fallback_for_model(
                harness.shared(),
                "main",
                &BTreeSet::new(),
                route,
                None,
            )
            .unwrap(),
            "weekly exhaustion is not transient capacity pressure: {route:?}"
        );
    }
}

#[test]
fn quota_preflight_can_recover_to_weekly_only_prolite_without_inventing_five_hour_exhaustion_04392()
{
    let mut weekly_only = ready();
    weekly_only.plan_type = Some("prolite".into());
    weekly_only.five_hour_status = RuntimeQuotaWindowStatus::Unknown;
    weekly_only.five_hour_remaining_percent = 0;
    weekly_only.five_hour_reset_at = i64::MAX;
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .profile_usage_snapshot("main", exhausted())
        .profile_usage_snapshot("second", weekly_only)
        .build();
    for route in [RuntimeRouteKind::Responses, RuntimeRouteKind::Websocket] {
        assert!(
            runtime_quota_exhausted_with_recoverable_alternative(
                harness.shared(),
                "main",
                &BTreeSet::new(),
                route,
                None,
            )
            .unwrap(),
            "an absent five-hour window is neutral when the real weekly quota is usable"
        );
    }
}

#[test]
fn quota_preflight_does_not_treat_expired_exhaustion_as_a_current_hold_04392() {
    let mut expired = exhausted();
    expired.five_hour_reset_at = Local::now().timestamp() - 1;
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .profile_usage_snapshot("main", expired)
        .profile_usage_snapshot("second", ready())
        .build();
    for route in [RuntimeRouteKind::Responses, RuntimeRouteKind::Websocket] {
        assert!(
            !runtime_quota_exhausted_with_recoverable_alternative(
                harness.shared(),
                "main",
                &BTreeSet::new(),
                route,
                None,
            )
            .unwrap(),
            "a reset window must be re-evaluated, not forced into another full-history retry"
        );
    }
}
