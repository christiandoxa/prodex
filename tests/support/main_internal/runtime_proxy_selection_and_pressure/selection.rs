struct RuntimeResponsesRequestBuilder {
    previous_response_id: Option<&'static str>,
    session_id: Option<&'static str>,
    session_header: Option<&'static str>,
    input: serde_json::Value,
}

impl RuntimeResponsesRequestBuilder {
    fn new(input: serde_json::Value) -> Self {
        Self {
            previous_response_id: None,
            session_id: None,
            session_header: None,
            input,
        }
    }

    fn previous_response_id(mut self, previous_response_id: &'static str) -> Self {
        self.previous_response_id = Some(previous_response_id);
        self
    }

    fn session_id(mut self, session_id: &'static str) -> Self {
        self.session_id = Some(session_id);
        self
    }

    fn session_header(mut self, session_id: &'static str) -> Self {
        self.session_header = Some(session_id);
        self
    }

    fn build(self) -> RuntimeProxyRequest {
        let mut body = serde_json::Map::new();
        if let Some(previous_response_id) = self.previous_response_id {
            body.insert(
                "previous_response_id".to_string(),
                serde_json::json!(previous_response_id),
            );
        }
        if let Some(session_id) = self.session_id {
            body.insert("session_id".to_string(), serde_json::json!(session_id));
        }
        body.insert("input".to_string(), self.input);

        let mut headers = vec![("Content-Type".to_string(), "application/json".to_string())];
        if let Some(session_id) = self.session_header {
            headers.push(("session_id".to_string(), session_id.to_string()));
        }

        RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/backend-api/codex/responses".to_string(),
            headers,
            body: serde_json::to_vec(&serde_json::Value::Object(body))
                .expect("request body should serialize"),
        }
    }
}

fn tool_output_only_request() -> RuntimeResponsesRequestBuilder {
    RuntimeResponsesRequestBuilder::new(serde_json::json!([
        {
            "type": "function_call_output",
            "call_id": "call_123",
            "output": "ok"
        }
    ]))
}

fn replayable_function_call_transcript_request() -> RuntimeResponsesRequestBuilder {
    RuntimeResponsesRequestBuilder::new(serde_json::json!([
        {
            "type": "function_call",
            "call_id": "call_123",
            "name": "shell",
            "arguments": "{\"cmd\":\"pwd\"}"
        },
        {
            "type": "function_call_output",
            "call_id": "call_123",
            "output": "ok"
        }
    ]))
}

fn replayable_message_request() -> RuntimeResponsesRequestBuilder {
    RuntimeResponsesRequestBuilder::new(serde_json::json!([
        {
            "type": "message",
            "role": "user",
            "content": [
                {
                    "type": "input_text",
                    "text": "retry with full body"
                }
            ]
        }
    ]))
}

fn empty_input_request() -> RuntimeResponsesRequestBuilder {
    RuntimeResponsesRequestBuilder::new(serde_json::json!([]))
}

#[test]
fn runtime_request_strips_previous_response_id_from_function_call_output_payloads() {
    let request = tool_output_only_request()
        .previous_response_id("resp_123")
        .build();

    assert!(
        runtime_request_without_previous_response_id(&request).is_some(),
        "helper should still be able to strip previous_response_id when explicitly asked"
    );
    assert!(
        runtime_request_requires_previous_response_affinity(&request),
        "function call outputs should keep previous_response affinity during normal proxying"
    );
}

#[test]
fn runtime_request_allows_fresh_function_call_output_replay_without_previous_response_id() {
    let request = replayable_function_call_transcript_request().build();

    assert!(
        !runtime_request_requires_previous_response_affinity(&request),
        "fresh replayable transcripts should not be pinned just because they include call outputs"
    );
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_classifies_tool_output_only() {
    let request = tool_output_only_request()
        .previous_response_id("resp_123")
        .build();

    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&request),
        Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly)
    );
    assert_eq!(
        runtime_previous_response_fresh_fallback_shape_label(
            runtime_request_previous_response_fresh_fallback_shape(&request)
        ),
        "tool_output_only"
    );
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_keeps_session_tool_output_owner_locked(
) {
    let request = tool_output_only_request()
        .previous_response_id("resp_123")
        .session_id("sess_123")
        .build();

    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&request),
        Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(
            runtime_request_previous_response_fresh_fallback_shape(&request)
        ),
        "tool outputs still need owning tool-call context even when session_id is present"
    );
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_keeps_header_session_tool_output_owner_locked(
) {
    let request = tool_output_only_request()
        .previous_response_id("resp_123")
        .session_header("sess_123")
        .build();

    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&request),
        Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(
            runtime_request_previous_response_fresh_fallback_shape(&request)
        ),
        "explicit session headers should not permit fresh tool-output replay after upstream forgets previous_response_id"
    );
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_classifies_replayable_input() {
    let request = replayable_message_request()
        .previous_response_id("resp_123")
        .build();

    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&request),
        Some(RuntimePreviousResponseFreshFallbackShape::ReplayableInput)
    );
    assert!(
        runtime_previous_response_fresh_fallback_shape_allows_recovery(
            runtime_request_previous_response_fresh_fallback_shape(&request)
        )
    );
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_classifies_session_replayable_empty_input()
 {
    let request = empty_input_request()
        .previous_response_id("resp_123")
        .session_id("sess_123")
        .build();

    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&request),
        Some(RuntimePreviousResponseFreshFallbackShape::SessionReplayable)
    );
    assert!(
        runtime_previous_response_fresh_fallback_shape_allows_recovery(
            runtime_request_previous_response_fresh_fallback_shape(&request)
        )
    );
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_blocks_empty_continuation_payloads() {
    let request = empty_input_request()
        .previous_response_id("resp_123")
        .build();

    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&request),
        Some(RuntimePreviousResponseFreshFallbackShape::ContinuationOnly)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(
            runtime_request_previous_response_fresh_fallback_shape(&request)
        )
    );
}

#[test]
fn websocket_previous_response_not_found_decision_prefers_locked_retry_before_stale() {
    let decision = runtime_previous_response_not_found_decision(
        RuntimePreviousResponseNotFoundDecisionInput {
            route: RuntimePreviousResponseNotFoundRoute::Websocket,
            previous_response_id: Some("resp_123"),
            has_turn_state_retry: false,
            request_requires_previous_response_affinity: true,
            trusted_previous_response_affinity: false,
            request_turn_state: None,
            previous_response_fresh_fallback_used: false,
            fresh_fallback_shape: Some(RuntimePreviousResponseFreshFallbackShape::ContinuationOnly),
            retry_index: 0,
        },
    );

    assert_eq!(
        decision.retry_delay,
        Some(Duration::from_millis(
            RUNTIME_PREVIOUS_RESPONSE_RETRY_DELAYS_MS[0]
        ))
    );
    assert_eq!(decision.retry_reason, Some("locked_affinity_no_turn_state"));
    assert_eq!(
        decision.chain_retry_reason,
        Some("previous_response_not_found_locked_affinity")
    );
    assert!(
        decision.stale_continuation,
        "locked affinity remains stale if retries exhaust; caller still gets the immediate retry first"
    );
}

#[test]
fn websocket_previous_response_not_found_decision_keeps_session_tool_output_stale() {
    let decision = runtime_previous_response_not_found_decision(
        RuntimePreviousResponseNotFoundDecisionInput {
            route: RuntimePreviousResponseNotFoundRoute::Websocket,
            previous_response_id: Some("resp_123"),
            has_turn_state_retry: false,
            request_requires_previous_response_affinity: true,
            trusted_previous_response_affinity: false,
            request_turn_state: None,
            previous_response_fresh_fallback_used: false,
            fresh_fallback_shape: Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly),
            retry_index: 0,
        },
    );

    assert_eq!(
        decision.retry_delay,
        Some(Duration::from_millis(
            RUNTIME_PREVIOUS_RESPONSE_RETRY_DELAYS_MS[0]
        ))
    );
    assert_eq!(decision.retry_reason, Some("locked_affinity_no_turn_state"));
    assert!(decision.stale_continuation);
    assert!(!decision.fresh_fallback_allowed);
    assert_eq!(
        runtime_previous_response_not_found_observability_outcome(
            decision,
            Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly)
        ),
        None
    );
}

#[test]
fn websocket_previous_response_not_found_decision_marks_nonreplayable_continuation_stale() {
    let decision = runtime_previous_response_not_found_decision(
        RuntimePreviousResponseNotFoundDecisionInput {
            route: RuntimePreviousResponseNotFoundRoute::Websocket,
            previous_response_id: Some("resp_123"),
            has_turn_state_retry: false,
            request_requires_previous_response_affinity: false,
            trusted_previous_response_affinity: false,
            request_turn_state: None,
            previous_response_fresh_fallback_used: false,
            fresh_fallback_shape: Some(RuntimePreviousResponseFreshFallbackShape::ContinuationOnly),
            retry_index: 0,
        },
    );

    assert_eq!(decision.retry_delay, None);
    assert!(decision.stale_continuation);
    assert!(!decision.fresh_fallback_allowed);
    assert!(decision.fresh_fallback_blocked_without_affinity);
    assert_eq!(
        runtime_previous_response_not_found_observability_outcome(
            decision,
            Some(RuntimePreviousResponseFreshFallbackShape::ContinuationOnly)
        ),
        Some("blocked_nonreplayable_without_affinity")
    );
}

#[test]
fn responses_previous_response_not_found_decision_keeps_turn_state_retry_shared() {
    let decision = runtime_previous_response_not_found_decision(
        RuntimePreviousResponseNotFoundDecisionInput {
            route: RuntimePreviousResponseNotFoundRoute::Responses,
            previous_response_id: Some("resp_123"),
            has_turn_state_retry: true,
            request_requires_previous_response_affinity: false,
            trusted_previous_response_affinity: true,
            request_turn_state: Some("turn_state"),
            previous_response_fresh_fallback_used: false,
            fresh_fallback_shape: Some(RuntimePreviousResponseFreshFallbackShape::ReplayableInput),
            retry_index: 1,
        },
    );

    assert_eq!(
        decision.retry_delay,
        Some(Duration::from_millis(
            RUNTIME_PREVIOUS_RESPONSE_RETRY_DELAYS_MS[1]
        ))
    );
    assert_eq!(decision.retry_reason, Some("non_blocking_retry"));
    assert_eq!(
        decision.chain_retry_reason,
        Some("previous_response_not_found")
    );
    assert!(!decision.stale_continuation);
}

#[test]
fn parse_runtime_websocket_request_metadata_extracts_affinity_fields() {
    let metadata = parse_runtime_websocket_request_metadata(
        r#"{"previous_response_id":"resp_123","client_metadata":{"session_id":"sess_123"},"input":[{"type":"function_call_output","call_id":"call_123","output":"ok"}]}"#,
    );

    assert_eq!(metadata.previous_response_id.as_deref(), Some("resp_123"));
    assert_eq!(metadata.session_id.as_deref(), Some("sess_123"));
    assert!(metadata.requires_previous_response_affinity);
    assert_eq!(
        metadata.previous_response_fresh_fallback_shape,
        Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(
            metadata.previous_response_fresh_fallback_shape
        ),
        "websocket metadata must keep tool outputs owner-locked even when a session_id is present"
    );
}

#[test]
fn quota_blocked_previous_response_fresh_fallback_blocks_tool_output_only() {
    assert!(
        !runtime_quota_blocked_previous_response_fresh_fallback_allowed(
            Some("resp_123"),
            true,
            false,
            Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly),
        ),
        "tool-output-only requests still need chain-scoped call context"
    );
}

#[test]
fn quota_blocked_previous_response_fresh_fallback_allows_session_replayable_requests() {
    assert!(runtime_quota_blocked_previous_response_fresh_fallback_allowed(
        Some("resp_123"),
        true,
        false,
        Some(RuntimePreviousResponseFreshFallbackShape::SessionReplayable),
    ));
    assert!(runtime_quota_blocked_previous_response_fresh_fallback_allowed(
        Some("resp_123"),
        true,
        false,
        Some(RuntimePreviousResponseFreshFallbackShape::ReplayableInput),
    ));
}

#[test]
fn runtime_quota_summary_distinguishes_window_health() {
    let summary = runtime_quota_summary_for_route(
        &usage_with_main_windows(4, 18_000, 12, 604_800),
        RuntimeRouteKind::Responses,
    );

    assert_eq!(summary.route_band, RuntimeQuotaPressureBand::Critical);
    assert_eq!(summary.five_hour.status, RuntimeQuotaWindowStatus::Critical);
    assert_eq!(summary.weekly.status, RuntimeQuotaWindowStatus::Thin);
    assert_eq!(summary.five_hour.remaining_percent, 4);
    assert_eq!(summary.weekly.remaining_percent, 12);
}

#[test]
fn ready_profile_ranking_prefers_larger_reserve_when_resets_match() {
    let candidates = vec![
        ReadyProfileCandidate {
            name: "thin".to_string(),
            usage: usage_with_main_windows(65, 18_000, 70, 604_800),
            order_index: 0,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
        ReadyProfileCandidate {
            name: "deep".to_string(),
            usage: usage_with_main_windows(95, 18_000, 98, 604_800),
            order_index: 1,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
    ];

    let mut ranked = candidates.clone();
    ranked.sort_by_key(ready_profile_sort_key);
    assert_eq!(ranked[0].name, "deep");
}

#[test]
fn active_profile_selection_order_prefers_openai_pool_before_other_providers() {
    let state = AppState {
        active_profile: Some("copilot".to_string()),
        profiles: BTreeMap::from([
            (
                "copilot".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/copilot"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Copilot {
                        host: "https://github.com".to_string(),
                        login: "copilot-user".to_string(),
                        api_url: "https://api.business.githubcopilot.com".to_string(),
                        access_type_sku: None,
                        copilot_plan: None,
                    },
                },
            ),
            (
                "openai-main".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/openai-main"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "openai-second".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/openai-second"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };

    assert_eq!(
        active_profile_selection_order(&state, "copilot"),
        vec![
            "openai-main".to_string(),
            "openai-second".to_string(),
            "copilot".to_string(),
        ]
    );
}

#[test]
fn ready_profile_candidates_prefer_openai_pool_before_other_providers() {
    let state = AppState {
        active_profile: Some("copilot".to_string()),
        profiles: BTreeMap::from([
            (
                "copilot".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/copilot"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Copilot {
                        host: "https://github.com".to_string(),
                        login: "copilot-user".to_string(),
                        api_url: "https://api.business.githubcopilot.com".to_string(),
                        access_type_sku: None,
                        copilot_plan: None,
                    },
                },
            ),
            (
                "openai-main".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/openai-main"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let reports = vec![
        RunProfileProbeReport {
            name: "copilot".to_string(),
            order_index: 0,
            auth: AuthSummary {
                label: "copilot".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(100, 3_600, 100, 86_400)),
        },
        RunProfileProbeReport {
            name: "openai-main".to_string(),
            order_index: 1,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(80, 3_600, 80, 86_400)),
        },
    ];

    let ranked = ready_profile_candidates(&reports, false, Some("copilot"), &state, None);
    assert_eq!(ranked[0].name, "openai-main");
    assert_eq!(ranked[1].name, "copilot");
}

#[test]
fn runtime_launch_selection_resolve_falls_back_from_active_copilot_to_openai() {
    let root = TestDir::new();
    let copilot_home = root.path.join("copilot");
    let openai_home = root.path.join("openai-main");
    fs::create_dir_all(&copilot_home).expect("create copilot home");
    fs::create_dir_all(&openai_home).expect("create openai home");

    let state = AppState {
        active_profile: Some("copilot".to_string()),
        profiles: BTreeMap::from([
            (
                "copilot".to_string(),
                ProfileEntry {
                    codex_home: copilot_home,
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Copilot {
                        host: "https://github.com".to_string(),
                        login: "copilot-user".to_string(),
                        api_url: "https://api.business.githubcopilot.com".to_string(),
                        access_type_sku: None,
                        copilot_plan: None,
                    },
                },
            ),
            (
                "openai-main".to_string(),
                ProfileEntry {
                    codex_home: openai_home.clone(),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };

    let selected =
        resolve_runtime_launch_profile_name(&state, None).expect("resolve runtime launch name");
    assert_eq!(selected, "openai-main");
}

#[test]
fn scheduler_prefers_rested_profile_within_near_optimal_band() {
    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: None,
        profiles: BTreeMap::new(),
        last_run_selected_at: BTreeMap::from([
            ("fresh".to_string(), now),
            ("rested".to_string(), now - 3_600),
        ]),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let candidates = vec![
        ReadyProfileCandidate {
            name: "fresh".to_string(),
            usage: usage_with_main_windows(100, 18_000, 100, 604_800),
            order_index: 0,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
        ReadyProfileCandidate {
            name: "rested".to_string(),
            usage: usage_with_main_windows(96, 18_000, 96, 604_800),
            order_index: 1,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
    ];

    let ranked = schedule_ready_profile_candidates(candidates, &state, None);
    assert_eq!(ranked[0].name, "rested");
}

#[test]
fn scheduler_keeps_preferred_profile_when_gain_is_small() {
    let state = AppState {
        active_profile: Some("active".to_string()),
        profiles: BTreeMap::new(),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let candidates = vec![
        ReadyProfileCandidate {
            name: "better".to_string(),
            usage: usage_with_main_windows(100, 18_000, 100, 604_800),
            order_index: 0,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
        ReadyProfileCandidate {
            name: "active".to_string(),
            usage: usage_with_main_windows(96, 18_000, 96, 604_800),
            order_index: 1,
            preferred: true,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
    ];

    let ranked = schedule_ready_profile_candidates(candidates, &state, Some("active"));
    assert_eq!(ranked[0].name, "active");
}

#[test]
fn scheduler_allows_switch_when_preferred_profile_is_in_cooldown() {
    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("active".to_string()),
        profiles: BTreeMap::new(),
        last_run_selected_at: BTreeMap::from([("active".to_string(), now)]),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let candidates = vec![
        ReadyProfileCandidate {
            name: "better".to_string(),
            usage: usage_with_main_windows(100, 18_000, 100, 604_800),
            order_index: 0,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
        ReadyProfileCandidate {
            name: "active".to_string(),
            usage: usage_with_main_windows(96, 18_000, 96, 604_800),
            order_index: 1,
            preferred: true,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
    ];

    let ranked = schedule_ready_profile_candidates(candidates, &state, Some("active"));
    assert_eq!(ranked[0].name, "better");
}

#[test]
fn ready_profile_candidates_use_persisted_snapshot_when_probe_is_unavailable() {
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/main"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/second"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::from([(
            "sess-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
    };
    let reports = vec![
        RunProfileProbeReport {
            name: "main".to_string(),
            order_index: 0,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Err("runtime quota snapshot unavailable".to_string()),
        },
        RunProfileProbeReport {
            name: "second".to_string(),
            order_index: 1,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Err("runtime quota snapshot unavailable".to_string()),
        },
    ];
    let now = Local::now().timestamp();
    let persisted = BTreeMap::from([
        (
            "main".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: now,
                five_hour_status: RuntimeQuotaWindowStatus::Exhausted,
                five_hour_remaining_percent: 0,
                five_hour_reset_at: now + 300,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 70,
                weekly_reset_at: now + 86_400,
            },
        ),
        (
            "second".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: now,
                five_hour_status: RuntimeQuotaWindowStatus::Ready,
                five_hour_remaining_percent: 90,
                five_hour_reset_at: now + 3_600,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 95,
                weekly_reset_at: now + 604_800,
            },
        ),
    ]);

    let ranked = ready_profile_candidates(&reports, false, Some("main"), &state, Some(&persisted));
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].name, "second");
    assert_eq!(
        ranked[0].quota_source,
        RuntimeQuotaSource::PersistedSnapshot
    );
}

#[test]
fn run_profile_probe_is_ready_only_when_live_quota_is_clear() {
    let ready = RunProfileProbeReport {
        name: "main".to_string(),
        order_index: 0,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Ok(usage_with_main_windows(90, 3_600, 95, 86_400)),
    };
    let blocked = RunProfileProbeReport {
        name: "main".to_string(),
        order_index: 0,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Ok(usage_with_main_windows(0, 300, 95, 86_400)),
    };
    let failed = RunProfileProbeReport {
        name: "main".to_string(),
        order_index: 0,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Err("timeout".to_string()),
    };

    assert!(run_profile_probe_is_ready(&ready, false));
    assert!(!run_profile_probe_is_ready(&blocked, false));
    assert!(!run_profile_probe_is_ready(&failed, false));
}

#[test]
fn run_preflight_reports_with_current_first_preserves_current_and_rotation_order() {
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/main"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/second"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/third"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let current_report = RunProfileProbeReport {
        name: "main".to_string(),
        order_index: 0,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Ok(usage_with_main_windows(90, 3_600, 95, 86_400)),
    };

    let reports =
        run_preflight_reports_with_current_first(&state, "main", current_report.clone(), None);

    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0].name, "main");
    assert_eq!(reports[0].order_index, 0);
    assert_eq!(
        reports[0]
            .result
            .as_ref()
            .ok()
            .map(format_main_windows_compact),
        current_report
            .result
            .as_ref()
            .ok()
            .map(format_main_windows_compact)
    );
    assert_eq!(reports[1].name, "second");
    assert_eq!(reports[1].order_index, 1);
    assert_eq!(reports[2].name, "third");
    assert_eq!(reports[2].order_index, 2);
}

#[test]
fn quota_overview_sort_prioritizes_status_then_nearest_reset() {
    let reports = vec![
        QuotaReport {
            name: "blocked".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
                0, 3_600, 80, 86_400,
            ))),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "ready-late".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
                90, 7_200, 95, 172_800,
            ))),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "error".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Err("boom".to_string()),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "ready-early".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
                90, 1_800, 95, 259_200,
            ))),
            fetched_at: 1_700_000_000,
        },
    ];

    let names = sort_quota_reports_for_display(&reports)
        .into_iter()
        .map(|report| report.name.clone())
        .collect::<Vec<_>>();

    assert_eq!(names, vec!["ready-early", "ready-late", "blocked", "error"]);
}

#[test]
fn quota_watch_defaults_to_live_refresh_for_regular_views() {
    let profile_args = QuotaArgs {
        profile: Some("main".to_string()),
        all: false,
        detail: false,
        raw: false,
        watch: false,
        once: false,
        base_url: None,
    };
    assert!(quota_watch_enabled(&profile_args));

    let overview_args = QuotaArgs {
        all: true,
        ..profile_args
    };
    assert!(quota_watch_enabled(&overview_args));
}

#[test]
fn quota_watch_respects_once_and_raw_modes() {
    let once_args = QuotaArgs {
        profile: Some("main".to_string()),
        all: false,
        detail: false,
        raw: false,
        watch: false,
        once: true,
        base_url: None,
    };
    assert!(!quota_watch_enabled(&once_args));

    let raw_args = QuotaArgs {
        raw: true,
        watch: true,
        once: false,
        ..once_args
    };
    assert!(!quota_watch_enabled(&raw_args));
}

#[test]
fn quota_command_accepts_once_flag() {
    let command = parse_cli_command_from(["prodex", "quota", "--once"]).expect("quota command");
    let Commands::Quota(args) = command else {
        panic!("expected quota command");
    };
    assert!(args.once);
    assert!(!quota_watch_enabled(&args));
}

#[test]
fn audit_command_accepts_filters_and_json() {
    let command = parse_cli_command_from([
        "prodex",
        "audit",
        "--tail",
        "50",
        "--component",
        "profile",
        "--action",
        "use",
        "--json",
    ])
    .expect("audit command");
    let Commands::Audit(args) = command else {
        panic!("expected audit command");
    };
    assert_eq!(args.tail, 50);
    assert_eq!(args.component.as_deref(), Some("profile"));
    assert_eq!(args.action.as_deref(), Some("use"));
    assert!(args.json);
}

#[test]
fn bare_prodex_defaults_to_run_command() {
    let command = parse_cli_command_from(["prodex"]).expect("bare prodex should parse");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };
    assert!(args.profile.is_none());
    assert!(!args.auto_rotate);
    assert!(!args.no_auto_rotate);
    assert!(!args.skip_quota_check);
    assert!(args.base_url.is_none());
    assert!(args.codex_args.is_empty());
}

#[test]
fn bare_prodex_with_codex_args_defaults_to_run_command() {
    let command = parse_cli_command_from(["prodex", "exec", "review this repo"])
        .expect("bare prodex codex args should parse");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };
    assert_eq!(
        args.codex_args,
        vec![OsString::from("exec"), OsString::from("review this repo")]
    );
}

#[test]
fn cleanup_command_does_not_default_to_run() {
    let command = parse_cli_command_from(["prodex", "cleanup"]).expect("cleanup command");
    assert!(matches!(command, Commands::Cleanup));
}

#[test]
fn logout_command_accepts_positional_profile_name() {
    let command = parse_cli_command_from(["prodex", "logout", "second"]).expect("logout command");
    let Commands::Logout(args) = command else {
        panic!("expected logout command");
    };
    assert_eq!(args.profile_name.as_deref(), Some("second"));
    assert_eq!(args.selected_profile(), Some("second"));
    assert!(args.profile.is_none());
}

#[test]
fn logout_command_accepts_profile_flag() {
    let command = parse_cli_command_from(["prodex", "logout", "--profile", "second"])
        .expect("logout command");
    let Commands::Logout(args) = command else {
        panic!("expected logout command");
    };
    assert_eq!(args.profile.as_deref(), Some("second"));
    assert_eq!(args.selected_profile(), Some("second"));
    assert!(args.profile_name.is_none());
}

#[test]
fn profile_remove_command_accepts_all_flag() {
    let command =
        parse_cli_command_from(["prodex", "profile", "remove", "--all"]).expect("remove command");
    let Commands::Profile(ProfileCommands::Remove(args)) = command else {
        panic!("expected profile remove command");
    };
    assert!(args.all);
    assert!(args.name.is_none());
    assert!(!args.delete_home);
}

#[test]
fn profile_remove_command_accepts_profile_name() {
    let command =
        parse_cli_command_from(["prodex", "profile", "remove", "main"]).expect("remove command");
    let Commands::Profile(ProfileCommands::Remove(args)) = command else {
        panic!("expected profile remove command");
    };
    assert!(!args.all);
    assert_eq!(args.name.as_deref(), Some("main"));
    assert!(!args.delete_home);
}

#[test]
fn bare_prodex_accepts_run_options_before_codex_args() {
    let command =
        parse_cli_command_from(["prodex", "--profile", "second", "exec", "review this repo"])
            .expect("bare prodex should accept run options");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };
    assert_eq!(args.profile.as_deref(), Some("second"));
    assert_eq!(
        args.codex_args,
        vec![OsString::from("exec"), OsString::from("review this repo")]
    );
}

#[test]
fn caveman_command_accepts_passthrough_args() {
    let command = parse_cli_command_from([
        "prodex",
        "caveman",
        "--profile",
        "main",
        "exec",
        "review this repo",
    ])
    .expect("caveman command should parse");
    let Commands::Caveman(args) = command else {
        panic!("expected caveman command");
    };
    assert_eq!(args.profile.as_deref(), Some("main"));
    assert_eq!(
        args.codex_args,
        vec![OsString::from("exec"), OsString::from("review this repo")]
    );
}

#[test]
fn profile_quota_watch_output_renders_snapshot_body_without_watch_header() {
    let output = render_profile_quota_watch_output(
        "main",
        "2026-03-22 10:00:00 WIB",
        Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
            63, 18_000, 12, 604_800,
        ))),
    );

    assert!(output.contains("Quota main"));
    assert!(!output.contains("Quota Watch"));
    assert!(!output.contains("Updated"));
    assert!(!output.contains("2026-03-22 10:00:00 WIB"));
    assert!(!output.ends_with('\n'));
}

#[test]
fn all_quota_watch_output_omits_watch_header_on_load_error() {
    let output = render_all_quota_watch_output(
        "2026-03-22 10:00:00 WIB",
        Err("load failed".to_string()),
        None,
        false,
    );

    assert!(output.contains("Quota"));
    assert!(!output.contains("Quota Watch"));
    assert!(!output.contains("Updated"));
    assert!(!output.contains("2026-03-22 10:00:00 WIB"));
    assert!(output.contains("load failed"));
    assert!(!output.ends_with('\n'));
}

#[test]
fn quota_reports_include_pool_summary_lines() {
    let alpha = usage_with_main_windows(90, 7_200, 95, 172_800);
    let beta = usage_with_main_windows(45, 1_800, 40, 86_400);
    let last_update = 1_700_000_123;
    let reports = vec![
        QuotaReport {
            name: "alpha".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(ProviderQuotaSnapshot::OpenAi(alpha.clone())),
            fetched_at: 1_700_000_100,
        },
        QuotaReport {
            name: "beta".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(ProviderQuotaSnapshot::OpenAi(beta.clone())),
            fetched_at: last_update,
        },
        QuotaReport {
            name: "api".to_string(),
            active: false,
            auth: AuthSummary {
                label: "api-key".to_string(),
                quota_compatible: false,
            },
            result: Err("auth mode is not quota-compatible".to_string()),
            fetched_at: 1_700_000_090,
        },
    ];

    let output = render_quota_reports_with_layout(&reports, false, None, 160);
    let five_hour_reset = required_main_window_snapshot(&beta, "5h")
        .expect("5h snapshot")
        .reset_at;
    let weekly_reset = required_main_window_snapshot(&beta, "weekly")
        .expect("weekly snapshot")
        .reset_at;

    assert!(output.contains("Available:"));
    assert!(output.contains("2/3 profile"));
    assert!(output.contains("Last Updated:"));
    assert!(output.contains(&format_precise_reset_time(Some(last_update))));
    assert!(!output.contains("Unavailable:"));
    assert!(output.contains("5h remaining pool:"));
    assert!(output.contains("Weekly remaining pool:"));
    assert!(output.contains(&format_info_pool_remaining(135, 2, Some(five_hour_reset))));
    assert!(output.contains(&format_info_pool_remaining(135, 2, Some(weekly_reset))));
    assert!(output.contains("\n\nPROFILE"));
}

#[test]
fn quota_reports_render_copilot_rows_without_falling_back_to_error() {
    let reports = vec![
        QuotaReport {
            name: "main".to_string(),
            active: true,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
                90, 7_200, 95, 172_800,
            ))),
            fetched_at: 1_700_000_100,
        },
        QuotaReport {
            name: "copilot-main".to_string(),
            active: false,
            auth: AuthSummary {
                label: "copilot".to_string(),
                quota_compatible: false,
            },
            result: Ok(ProviderQuotaSnapshot::Copilot(CopilotUserInfo {
                login: Some("copilot-user".to_string()),
                access_type_sku: Some("free_limited_copilot".to_string()),
                copilot_plan: Some("individual".to_string()),
                endpoints: None,
                limited_user_quotas: BTreeMap::from([
                    ("chat".to_string(), 450),
                    ("completions".to_string(), 4_000),
                ]),
                monthly_quotas: BTreeMap::from([
                    ("chat".to_string(), 500),
                    ("completions".to_string(), 4_000),
                ]),
                limited_user_reset_date: Some("2026-05-09".to_string()),
            })),
            fetched_at: 1_700_000_101,
        },
    ];

    let output = render_quota_reports_with_layout(&reports, true, None, 160);

    assert!(output.contains("Available:"));
    assert!(output.contains("2/2 profile"));
    assert!(output.contains("copilot-main"));
    assert!(output.contains("copilot-user"));
    assert!(output.contains("individual"));
    assert!(output.contains("chat 450/500 left"));
    assert!(output.contains("comp 4000/4000 left"));
    assert!(output.contains("status: Ready"));
    assert!(output.contains("resets: monthly 2026-05-09"));
    assert!(!output.contains("GitHub Copilot profiles do not expose ChatGPT quota"));
}

#[test]
fn quota_reports_respect_line_budget_while_preserving_sort_order() {
    let reports = vec![
        QuotaReport {
            name: "blocked".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
                0, 3_600, 80, 86_400,
            ))),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "ready-late".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
                90, 7_200, 95, 172_800,
            ))),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "error".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Err("boom".to_string()),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "ready-early".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
                90, 1_800, 95, 259_200,
            ))),
            fetched_at: 1_700_000_000,
        },
    ];

    let output = render_quota_reports_with_line_limit(&reports, false, Some(16));

    assert!(output.contains("ready-early"));
    assert!(output.contains("ready-late"));
    assert!(!output.contains("blocked"));
    assert!(!output.contains("error"));
    assert!(output.contains("\n\nshowing top 2 of 4 profiles"));
}

#[test]
fn quota_reports_window_supports_scroll_offset_and_hint() {
    let reports = vec![
        QuotaReport {
            name: "blocked".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
                0, 3_600, 80, 86_400,
            ))),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "ready-late".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
                90, 7_200, 95, 172_800,
            ))),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "error".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Err("boom".to_string()),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "ready-early".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
                90, 1_800, 95, 259_200,
            ))),
            fetched_at: 1_700_000_000,
        },
    ];

    let window = render_quota_reports_window_with_layout(&reports, false, Some(16), 100, 1, true);

    assert_eq!(window.start_profile, 1);
    assert_eq!(window.total_profiles, 4);
    assert_eq!(window.shown_profiles, 2);
    assert_eq!(window.hidden_before, 1);
    assert_eq!(window.hidden_after, 1);
    assert!(window.output.contains("ready-late"));
    assert!(window.output.contains("blocked"));
    assert!(!window.output.contains("ready-early"));
    assert!(
        window
            .output
            .contains("\n\npress Up/Down to scroll profiles (2-3 of 4; 1 above, 1 below)")
    );
}

#[test]
fn quota_reports_fit_requested_width_in_narrow_layout() {
    let reports = vec![
        QuotaReport {
            name: "ready-early".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
                90, 1_800, 95, 259_200,
            ))),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "blocked".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(ProviderQuotaSnapshot::OpenAi(usage_with_main_windows(
                0, 3_600, 80, 86_400,
            ))),
            fetched_at: 1_700_000_000,
        },
    ];

    let output = render_quota_reports_with_layout(&reports, false, None, 72);

    assert!(output.lines().all(|line| text_width(line) <= 72));
}
