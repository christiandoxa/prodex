use super::helpers::*;
use super::*;
use std::io::Read;
use std::sync::mpsc;

fn ready_profiles(backend: &RuntimeProxyBackend) -> RuntimeProxyProfileHarness {
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
    let now = Local::now().timestamp();
    let usage = usage_with_main_windows(80, 3_600, 80, 86_400);
    let mut runtime = harness.shared().runtime.lock().expect("runtime lock");
    for profile_name in ["main", "second"] {
        runtime.profile_probe_cache.insert(
            profile_name.to_string(),
            RuntimeProfileProbeCacheEntry {
                checked_at: now,
                auth: AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                },
                result: Ok(usage.clone()),
            },
        );
    }
    drop(runtime);
    harness
}

fn responses_request(body: &[u8]) -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: body.to_vec(),
    }
}

fn compact_request() -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("x-openai-subagent".to_string(), "compact".to_string()),
        ],
        body: br#"{"input":[],"instructions":"compact"}"#.to_vec(),
    }
}

fn consume_responses_reply(reply: RuntimeResponsesReply) -> (u16, String, Option<String>) {
    match reply {
        RuntimeResponsesReply::Buffered(parts) => (
            parts.status,
            String::from_utf8(parts.body.into_vec()).expect("buffered response should be utf8"),
            None,
        ),
        RuntimeResponsesReply::Streaming(mut response) => {
            let profile_name = response.profile_name.clone();
            let mut body = String::new();
            response
                .body
                .read_to_string(&mut body)
                .expect("streaming response should be readable");
            (response.status, body, Some(profile_name))
        }
    }
}

fn quota_snapshot(
    five_hour_status: RuntimeQuotaWindowStatus,
    five_hour_remaining_percent: i64,
) -> RuntimeProfileUsageSnapshot {
    let now = Local::now().timestamp();
    RuntimeProfileUsageSnapshot {
        checked_at: now,
        plan_type: None,
        five_hour_status,
        five_hour_remaining_percent,
        five_hour_reset_at: now + 300,
        weekly_status: RuntimeQuotaWindowStatus::Ready,
        weekly_remaining_percent: 80,
        weekly_reset_at: now + 86_400,
    }
}

#[test]
fn fresh_responses_use_last_positive_quota_after_current_exhaustion() {
    let backend = RuntimeProxyBackend::start();
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot(
            "main",
            quota_snapshot(RuntimeQuotaWindowStatus::Exhausted, 0),
        )
        .profile_usage_snapshot(
            "second",
            quota_snapshot(RuntimeQuotaWindowStatus::Critical, 1),
        )
        .build();

    let reply = proxy_runtime_responses_request(
        101,
        &responses_request(br#"{"input":[]}"#),
        harness.shared(),
    )
    .expect("positive quota fallback should reach upstream");
    let (status, body, profile) = consume_responses_reply(reply);

    assert_eq!(status, 200, "{body}");
    assert_eq!(profile.as_deref(), Some("second"));
    assert_eq!(backend.responses_accounts(), ["second-account"]);
}

#[test]
fn fresh_responses_return_service_unavailable_when_every_profile_is_exhausted() {
    let backend = RuntimeProxyBackend::start();
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot(
            "main",
            quota_snapshot(RuntimeQuotaWindowStatus::Exhausted, 0),
        )
        .profile_usage_snapshot(
            "second",
            quota_snapshot(RuntimeQuotaWindowStatus::Exhausted, 0),
        )
        .build();

    let reply = proxy_runtime_responses_request(
        102,
        &responses_request(br#"{"input":[]}"#),
        harness.shared(),
    )
    .expect("exhausted pool should return a local response");
    let (status, body, profile) = consume_responses_reply(reply);

    assert_eq!(status, 503, "{body}");
    assert!(body.contains("service_unavailable"), "{body}");
    assert_eq!(profile, None);
    assert!(backend.responses_accounts().is_empty());
}

#[derive(Clone, Copy)]
enum RetryableFailure {
    Quota429,
    Overload503,
    Transport,
}

impl RetryableFailure {
    fn label(self) -> &'static str {
        match self {
            Self::Quota429 => "explicit quota 429",
            Self::Overload503 => "overload 503",
            Self::Transport => "pre-commit transport failure",
        }
    }
}

#[test]
fn fresh_responses_rotate_each_retryable_failure_class_once() {
    for failure in [
        RetryableFailure::Quota429,
        RetryableFailure::Overload503,
        RetryableFailure::Transport,
    ] {
        let backend = match failure {
            RetryableFailure::Quota429 => {
                RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
                    RuntimeProxyBackendFaultStep::explicit_quota_429(
                        RuntimeProxyBackendFaultRoute::Responses,
                        "main-account",
                    ),
                ]))
            }
            RetryableFailure::Overload503 => {
                RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
                    RuntimeProxyBackendFaultStep::overloaded_503(
                        RuntimeProxyBackendFaultRoute::Responses,
                        "main-account",
                    ),
                ]))
            }
            RetryableFailure::Transport => {
                RuntimeProxyBackend::start_http_reset_before_first_byte()
            }
        };
        let harness = ready_profiles(&backend);
        let reply = proxy_runtime_responses_request(
            103,
            &responses_request(br#"{"input":[]}"#),
            harness.shared(),
        )
        .unwrap_or_else(|error| panic!("{}: {error:#}", failure.label()));
        let (status, body, profile) = consume_responses_reply(reply);

        assert_eq!(status, 200, "{}: {body}", failure.label());
        assert_eq!(profile.as_deref(), Some("second"), "{}", failure.label());
        assert_eq!(
            backend.responses_accounts(),
            ["main-account", "second-account"],
            "{}",
            failure.label()
        );
    }
}

#[test]
fn fresh_responses_pass_through_generic_429_without_rotation() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::plain_429(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
            ),
        ]));
    let harness = ready_profiles(&backend);

    let reply = proxy_runtime_responses_request(
        104,
        &responses_request(br#"{"input":[]}"#),
        harness.shared(),
    )
    .expect("generic 429 should be passed through");
    let (status, body, profile) = consume_responses_reply(reply);

    assert_eq!(status, 429);
    assert_eq!(body, "Too Many Requests");
    assert_eq!(profile, None);
    assert_eq!(backend.responses_accounts(), ["main-account"]);
}

#[test]
fn hard_continuation_keeps_quota_failure_on_bound_profile() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::explicit_quota_429(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
            ),
        ]));
    let harness = ready_profiles(&backend);
    {
        let now = Local::now().timestamp();
        let mut runtime = harness.shared().runtime.lock().expect("runtime lock");
        runtime.state.response_profile_bindings.insert(
            "resp-main".to_string(),
            ResponseProfileBinding {
                binding_identity: None,
                profile_name: "main".to_string(),
                bound_at: now,
            },
        );
        assert!(runtime_mark_continuation_status_verified(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
            "resp-main",
            now,
            Some(RuntimeRouteKind::Responses),
        ));
    }

    let reply = proxy_runtime_responses_request(
        105,
        &responses_request(br#"{"previous_response_id":"resp-main","input":[]}"#),
        harness.shared(),
    )
    .expect("bound continuation should preserve upstream quota response");
    let (status, body, profile) = consume_responses_reply(reply);

    assert_eq!(status, 429, "{body}");
    assert!(body.contains("insufficient_quota"), "{body}");
    assert_eq!(profile, None);
    assert_eq!(backend.responses_accounts(), ["main-account"]);
}

#[test]
fn compact_explicit_quota_rotates_to_next_profile() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::explicit_quota_429(
                RuntimeProxyBackendFaultRoute::Compact,
                "main-account",
            ),
        ]));
    let harness = ready_profiles(&backend);

    let response = proxy_runtime_standard_request(106, &compact_request(), harness.shared())
        .expect("compact quota should rotate to the next profile");
    let (status, body) = tiny_http_response_status_and_body(response);

    assert_eq!(status, 200, "{body}");
    assert!(body.contains("output"), "{body}");
    assert_eq!(
        backend.responses_accounts(),
        ["main-account", "second-account"]
    );
}

#[test]
fn compact_waits_for_transient_profiles_before_a_new_sweep() {
    let backend = RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
        RuntimeProxyBackendFaultStep::overloaded_503(
            RuntimeProxyBackendFaultRoute::Compact,
            "main-account",
        ),
        RuntimeProxyBackendFaultStep::overloaded_503(
            RuntimeProxyBackendFaultRoute::Compact,
            "main-account",
        ),
        RuntimeProxyBackendFaultStep::overloaded_503(
            RuntimeProxyBackendFaultRoute::Compact,
            "second-account",
        ),
        RuntimeProxyBackendFaultStep::success(
            RuntimeProxyBackendFaultRoute::Compact,
            "main-account",
        ),
    ]));
    let harness = ready_profiles(&backend);

    let response = proxy_runtime_standard_request(111, &compact_request(), harness.shared())
        .expect("compact should recover after transient overloads");
    let (status, body) = tiny_http_response_status_and_body(response);

    assert_eq!(status, 200, "{body}");
    assert!(body.contains("scripted-success"), "{body}");
    assert_eq!(
        backend.responses_accounts(),
        [
            "main-account",
            "main-account",
            "second-account",
            "main-account"
        ]
    );
    let log = read_runtime_proxy_test_log(&harness.shared().log_path);
    assert!(
        log.contains("request=111 transport=http rotation_waiting_for_recovery route=compact")
            && log.contains("request=111 transport=http rotation_sweep_start route=compact"),
        "compact recovery should wait and start a new sweep: {log}"
    );
}

#[test]
fn stream_cancellation_releases_profile_slot_without_rotation() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::sse_success(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
                "resp-cancel-1",
            ),
            RuntimeProxyBackendFaultStep::sse_success(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
                "resp-cancel-2",
            ),
        ]));
    let mut harness = ready_profiles(&backend);
    let tuning = &mut Arc::make_mut(&mut harness.shared_mut().runtime_config).tuning;
    tuning.profile_inflight_soft_limit = 2;
    tuning.profile_inflight_hard_limit = 2;

    let reply = proxy_runtime_responses_request(
        107,
        &responses_request(br#"{"input":[]}"#),
        harness.shared(),
    )
    .expect("stream should be committed before cancellation");
    let RuntimeResponsesReply::Streaming(stream) = reply else {
        panic!("normal Responses response should be streaming");
    };
    assert_eq!(stream.profile_name, "main");
    assert!(
        read_runtime_proxy_test_log(&harness.shared().log_path)
            .contains("request=107 transport=http sse_commit profile=main"),
        "stream cancellation must happen after the pre-commit boundary"
    );
    assert_eq!(
        harness
            .shared()
            .lane_admission
            .profile_inflight_count("main"),
        2
    );
    drop(stream);
    assert_eq!(
        harness
            .shared()
            .lane_admission
            .profile_inflight_count("main"),
        0,
        "cancellation must release the weighted profile slot"
    );

    let reply = proxy_runtime_responses_request(
        108,
        &responses_request(br#"{"input":[]}"#),
        harness.shared(),
    )
    .expect("a cancelled stream should not poison the profile");
    let (_, _, profile) = consume_responses_reply(reply);

    assert_eq!(
        profile.as_deref(),
        Some("main"),
        "{}",
        read_runtime_proxy_test_log(&harness.shared().log_path)
    );
    assert_eq!(
        backend.responses_accounts(),
        ["main-account", "main-account"],
        "{}",
        read_runtime_proxy_test_log(&harness.shared().log_path)
    );
}

#[test]
fn overlapping_streams_use_distinct_profile_slots() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::sse_success(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
                "resp-overlap",
            ),
        ]));
    let mut harness = ready_profiles(&backend);
    let tuning = &mut Arc::make_mut(&mut harness.shared_mut().runtime_config).tuning;
    tuning.profile_inflight_soft_limit = 2;
    tuning.profile_inflight_hard_limit = 2;

    let (first_tx, first_rx) = mpsc::channel();
    let first_shared = harness.shared().clone();
    let first = thread::spawn(move || {
        let reply = proxy_runtime_responses_request(
            109,
            &responses_request(br#"{"input":[]}"#),
            &first_shared,
        )
        .expect("first concurrent stream should start");
        first_tx
            .send(reply)
            .expect("first stream should be returned");
    });
    let first_reply = first_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("first stream should become active");
    assert_eq!(backend.responses_accounts(), ["main-account"]);

    let (second_tx, second_rx) = mpsc::channel();
    let second_shared = harness.shared().clone();
    let second = thread::spawn(move || {
        let reply = proxy_runtime_responses_request(
            110,
            &responses_request(br#"{"input":[]}"#),
            &second_shared,
        )
        .expect("second concurrent stream should start");
        second_tx
            .send(reply)
            .expect("second stream should be returned");
    });
    let second_reply = second_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("second stream should become active");

    let (_, _, first_profile) = consume_responses_reply(first_reply);
    let (_, _, second_profile) = consume_responses_reply(second_reply);
    first.join().expect("first stream thread should join");
    second.join().expect("second stream thread should join");

    assert_eq!(first_profile.as_deref(), Some("main"));
    assert_eq!(second_profile.as_deref(), Some("second"));
    assert_eq!(
        backend.responses_accounts(),
        ["main-account", "second-account"]
    );
}
