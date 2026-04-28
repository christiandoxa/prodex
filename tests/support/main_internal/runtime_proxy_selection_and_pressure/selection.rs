use super::*;

struct RuntimeResponsesRequestBuilder {
    previous_response_id: Option<&'static str>,
    session_id: Option<&'static str>,
    session_header: Option<&'static str>,
    turn_metadata_session_id: Option<&'static str>,
    input: serde_json::Value,
}

impl RuntimeResponsesRequestBuilder {
    fn new(input: serde_json::Value) -> Self {
        Self {
            previous_response_id: None,
            session_id: None,
            session_header: None,
            turn_metadata_session_id: None,
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

    fn turn_metadata_session(mut self, session_id: &'static str) -> Self {
        self.turn_metadata_session_id = Some(session_id);
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
        if let Some(session_id) = self.turn_metadata_session_id {
            headers.push((
                "x-codex-turn-metadata".to_string(),
                serde_json::json!({ "session_id": session_id }).to_string(),
            ));
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

fn message_followup_request() -> RuntimeResponsesRequestBuilder {
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

fn mixed_tool_and_message_request() -> RuntimeResponsesRequestBuilder {
    RuntimeResponsesRequestBuilder::new(serde_json::json!([
        {
            "type": "function_call_output",
            "call_id": "call_123",
            "output": "ok"
        },
        {
            "type": "message",
            "role": "user",
            "content": [
                {
                    "type": "input_text",
                    "text": "continue from the tool result"
                }
            ]
        }
    ]))
}

#[derive(Clone, Copy)]
enum PreviousResponseFreshFallbackRequestShape {
    ToolOutputOnly,
    MessageFollowup,
    EmptyInput,
    MixedToolAndMessage,
}

impl PreviousResponseFreshFallbackRequestShape {
    const ALL: [Self; 4] = [
        Self::ToolOutputOnly,
        Self::MessageFollowup,
        Self::EmptyInput,
        Self::MixedToolAndMessage,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::ToolOutputOnly => "tool_output_only",
            Self::MessageFollowup => "message_followup",
            Self::EmptyInput => "empty_input",
            Self::MixedToolAndMessage => "mixed_tool_and_message",
        }
    }

    fn request(self) -> RuntimeResponsesRequestBuilder {
        match self {
            Self::ToolOutputOnly => tool_output_only_request(),
            Self::MessageFollowup => message_followup_request(),
            Self::EmptyInput => empty_input_request(),
            Self::MixedToolAndMessage => mixed_tool_and_message_request(),
        }
    }

    fn input(self) -> serde_json::Value {
        let request = self.request().build();
        serde_json::from_slice::<serde_json::Value>(&request.body)
            .expect("request should decode")
            .get("input")
            .expect("request should include input")
            .clone()
    }

    fn expected_shape_label(self, has_session: bool) -> &'static str {
        match (self, has_session) {
            (Self::ToolOutputOnly, _) => "tool_output_only",
            (Self::EmptyInput, false) => "empty_input",
            (Self::EmptyInput, true) => "session_replayable",
            (Self::MessageFollowup | Self::MixedToolAndMessage, _) => "continuation_only",
        }
    }

    fn can_drop_previous_response_id(self, _has_session: bool) -> bool {
        false
    }
}

#[derive(Clone, Copy)]
enum RuntimeRequestSessionPlacement {
    None,
    Body,
    Header,
    TurnMetadata,
}

impl RuntimeRequestSessionPlacement {
    const ALL: [Self; 4] = [Self::None, Self::Body, Self::Header, Self::TurnMetadata];

    fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Body => "body_session_id",
            Self::Header => "header_session_id",
            Self::TurnMetadata => "turn_metadata_session_id",
        }
    }

    fn has_session(self) -> bool {
        !matches!(self, Self::None)
    }

    fn apply(self, builder: RuntimeResponsesRequestBuilder) -> RuntimeResponsesRequestBuilder {
        match self {
            Self::None => builder,
            Self::Body => builder.session_id("sess_123"),
            Self::Header => builder.session_header("sess_123"),
            Self::TurnMetadata => builder.turn_metadata_session("sess_123"),
        }
    }
}

#[derive(Clone, Copy)]
enum WebsocketSessionPlacement {
    None,
    ClientMetadata,
    SessionHeaderPromotion,
}

impl WebsocketSessionPlacement {
    const ALL: [Self; 3] = [
        Self::None,
        Self::ClientMetadata,
        Self::SessionHeaderPromotion,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ClientMetadata => "client_metadata_session_id",
            Self::SessionHeaderPromotion => "session_header_promotion",
        }
    }

    fn has_session(self) -> bool {
        !matches!(self, Self::None)
    }

    fn uses_client_metadata(self) -> bool {
        matches!(self, Self::ClientMetadata)
    }

    fn promotes_header(self) -> bool {
        matches!(self, Self::SessionHeaderPromotion)
    }
}

#[derive(Clone, Copy)]
enum PreviousResponseNotFoundDecisionFallbackCase {
    ToolOutputOnly,
    MessageFollowup,
    EmptyInputOnly,
    SessionScopedEmptyInput,
    MixedToolAndMessage,
    Unknown,
}

impl PreviousResponseNotFoundDecisionFallbackCase {
    const ALL: [Self; 6] = [
        Self::ToolOutputOnly,
        Self::MessageFollowup,
        Self::EmptyInputOnly,
        Self::SessionScopedEmptyInput,
        Self::MixedToolAndMessage,
        Self::Unknown,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::ToolOutputOnly => "tool_output_only",
            Self::MessageFollowup => "message_followup",
            Self::EmptyInputOnly => "empty_input",
            Self::SessionScopedEmptyInput => "session_scoped_empty_input",
            Self::MixedToolAndMessage => "mixed_tool_and_message",
            Self::Unknown => "unknown",
        }
    }

    fn request_requires_previous_response_affinity(self) -> bool {
        matches!(self, Self::ToolOutputOnly | Self::MixedToolAndMessage)
    }

    fn fresh_fallback_shape(self) -> Option<RuntimePreviousResponseFreshFallbackShape> {
        match self {
            Self::ToolOutputOnly => Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly),
            Self::MessageFollowup | Self::MixedToolAndMessage => {
                Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation)
            }
            Self::EmptyInputOnly => Some(RuntimePreviousResponseFreshFallbackShape::EmptyInputOnly),
            Self::SessionScopedEmptyInput => {
                Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay)
            }
            Self::Unknown => None,
        }
    }
}

fn previous_response_not_found_route_label(
    route: RuntimePreviousResponseNotFoundRoute,
) -> &'static str {
    match route {
        RuntimePreviousResponseNotFoundRoute::Responses => "responses",
        RuntimePreviousResponseNotFoundRoute::Websocket => "websocket",
    }
}

fn websocket_previous_response_fresh_fallback_request_text(
    shape: PreviousResponseFreshFallbackRequestShape,
    has_client_metadata_session: bool,
) -> String {
    let mut body = serde_json::Map::new();
    body.insert(
        "previous_response_id".to_string(),
        serde_json::json!("resp_123"),
    );
    body.insert("input".to_string(), shape.input());
    if has_client_metadata_session {
        body.insert(
            "client_metadata".to_string(),
            serde_json::json!({ "session_id": "sess_123" }),
        );
    }
    serde_json::Value::Object(body).to_string()
}

#[test]
fn sync_probe_pressure_pause_logs_effective_policy_and_env_pause_ms() {
    let temp_dir = TestDir::isolated();
    let prodex_home = temp_dir.path.join("prodex-policy");
    fs::create_dir_all(&prodex_home).expect("prodex home should exist");
    fs::write(
        prodex_home.join("policy.toml"),
        r#"
version = 1

[runtime_proxy]
sync_probe_pressure_pause_ms = 2
"#,
    )
    .expect("policy file should write");

    let shared = RuntimeProxyFixtureBuilder::new().build_shared(&temp_dir);
    shared.local_overload_backoff_until.store(
        Local::now().timestamp().max(0) as u64 + 60,
        Ordering::SeqCst,
    );
    assert!(runtime_proxy_sync_probe_pressure_mode_active_for_route(
        &shared,
        RuntimeRouteKind::Responses
    ));

    let _env_lock = TestEnvVarGuard::lock();
    let _home_guard = TestEnvVarGuard::set(
        "PRODEX_HOME",
        prodex_home.to_str().expect("prodex home path"),
    );
    let _pause_unset_guard =
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_SYNC_PROBE_PRESSURE_PAUSE_MS");
    clear_runtime_policy_cache();

    runtime_proxy_sync_probe_pressure_pause(&shared, RuntimeRouteKind::Responses);
    runtime_proxy_flush_logs_for_path(&shared.log_path);
    let policy_log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        policy_log.contains("runtime_proxy_sync_probe_pressure_pause route=responses pause_ms=2"),
        "policy pause override should be logged as effective pause: {policy_log}"
    );

    let _ = fs::remove_file(&shared.log_path);
    let _pause_env_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_SYNC_PROBE_PRESSURE_PAUSE_MS", "1");
    clear_runtime_policy_cache();

    runtime_proxy_sync_probe_pressure_pause(&shared, RuntimeRouteKind::Responses);
    runtime_proxy_flush_logs_for_path(&shared.log_path);
    let env_log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        env_log.contains("runtime_proxy_sync_probe_pressure_pause route=responses pause_ms=1"),
        "env pause override should beat policy and be logged as effective pause: {env_log}"
    );

    clear_runtime_policy_cache();
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_matrix_stays_fail_closed() {
    for shape in PreviousResponseFreshFallbackRequestShape::ALL {
        for session in RuntimeRequestSessionPlacement::ALL {
            let request = session
                .apply(shape.request().previous_response_id("resp_123"))
                .build();
            let actual = runtime_request_previous_response_fresh_fallback_shape(&request);
            let actual_label = runtime_previous_response_fresh_fallback_shape_label(actual);
            let expected_label = shape.expected_shape_label(session.has_session());

            assert_eq!(
                actual_label,
                expected_label,
                "shape={} session={}",
                shape.label(),
                session.label()
            );
            assert_eq!(
                runtime_previous_response_fresh_fallback_shape_allows_recovery(actual),
                shape.can_drop_previous_response_id(session.has_session()),
                "drop previous_response_id safety mismatch for shape={} session={}",
                shape.label(),
                session.label()
            );
        }
    }
}

#[test]
fn websocket_previous_response_fresh_fallback_shape_matrix_promotes_only_safe_session_shapes() {
    for shape in PreviousResponseFreshFallbackRequestShape::ALL {
        for session in WebsocketSessionPlacement::ALL {
            let request_text = websocket_previous_response_fresh_fallback_request_text(
                shape,
                session.uses_client_metadata(),
            );
            let metadata = parse_runtime_websocket_request_metadata(&request_text);
            let actual = runtime_previous_response_fresh_fallback_shape_with_session(
                metadata.previous_response_fresh_fallback_shape,
                session.promotes_header(),
            );
            let actual_label = runtime_previous_response_fresh_fallback_shape_label(actual);
            let expected_label = shape.expected_shape_label(session.has_session());

            assert_eq!(
                metadata.previous_response_id.as_deref(),
                Some("resp_123"),
                "shape={} session={}",
                shape.label(),
                session.label()
            );
            assert_eq!(
                metadata.session_id.as_deref(),
                session.uses_client_metadata().then_some("sess_123"),
                "shape={} session={}",
                shape.label(),
                session.label()
            );
            assert_eq!(
                actual_label,
                expected_label,
                "shape={} session={}",
                shape.label(),
                session.label()
            );
            assert_eq!(
                runtime_previous_response_fresh_fallback_shape_allows_recovery(actual),
                shape.can_drop_previous_response_id(session.has_session()),
                "websocket drop previous_response_id safety mismatch for shape={} session={}",
                shape.label(),
                session.label()
            );
        }
    }
}

#[test]
fn runtime_request_does_not_strip_previous_response_id_from_continuations() {
    let request = tool_output_only_request()
        .previous_response_id("resp_123")
        .build();

    assert!(
        runtime_request_without_previous_response_id(&request).is_none(),
        "runtime must not manufacture a fresh request from a previous_response continuation"
    );
    assert!(
        runtime_request_requires_previous_response_affinity(&request),
        "function call outputs should keep previous_response affinity during normal proxying"
    );
}

#[test]
fn runtime_request_text_does_not_strip_previous_response_id_from_continuations() {
    let request_text = serde_json::json!({
        "previous_response_id": "resp_123",
        "session_id": "sess_123",
        "input": [],
    })
    .to_string();

    assert!(
        runtime_request_text_without_previous_response_id(&request_text).is_none(),
        "websocket continuations must not drop previous_response_id even with session metadata"
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
fn runtime_request_previous_response_fresh_fallback_shape_blocks_session_tool_output_replay() {
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
        "session-scoped tool outputs still need previous_response tool-call context"
    );
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_blocks_header_session_tool_output_replay()
{
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
        "explicit session headers must not fresh-replay bare tool outputs"
    );
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_blocks_message_followups() {
    let request = message_followup_request()
        .previous_response_id("resp_123")
        .build();

    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&request),
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(
            runtime_request_previous_response_fresh_fallback_shape(&request)
        ),
        "plain message follow-ups still depend on previous_response chain context"
    );
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_blocks_header_session_message_followups()
{
    let request = message_followup_request()
        .previous_response_id("resp_123")
        .session_header("sess_123")
        .build();

    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&request),
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(
            runtime_request_previous_response_fresh_fallback_shape(&request)
        ),
        "session headers must not make incremental message follow-ups replayable"
    );
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_blocks_turn_metadata_session_message_followups()
 {
    let request = message_followup_request()
        .previous_response_id("resp_123")
        .turn_metadata_session("sess_123")
        .build();

    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&request),
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(
            runtime_request_previous_response_fresh_fallback_shape(&request)
        ),
        "turn metadata session ids must not make incremental message follow-ups replayable"
    );
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_classifies_session_scoped_empty_input() {
    let request = empty_input_request()
        .previous_response_id("resp_123")
        .session_id("sess_123")
        .build();

    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&request),
        Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(
            runtime_request_previous_response_fresh_fallback_shape(&request)
        ),
        "session_id is affinity metadata, not replay state"
    );
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_blocks_empty_continuation_payloads() {
    let request = empty_input_request()
        .previous_response_id("resp_123")
        .build();

    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&request),
        Some(RuntimePreviousResponseFreshFallbackShape::EmptyInputOnly)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(
            runtime_request_previous_response_fresh_fallback_shape(&request)
        )
    );
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_marks_header_session_empty_input() {
    let request = empty_input_request()
        .previous_response_id("resp_123")
        .session_header("sess_123")
        .build();

    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&request),
        Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(
            runtime_request_previous_response_fresh_fallback_shape(&request)
        ),
        "session headers must not make empty previous_response continuations replayable"
    );
}

#[test]
fn runtime_previous_response_not_found_decision_matrix_stays_consistent() {
    for route in [
        RuntimePreviousResponseNotFoundRoute::Responses,
        RuntimePreviousResponseNotFoundRoute::Websocket,
    ] {
        for has_turn_state_retry in [false, true] {
            for trusted_previous_response_affinity in [false, true] {
                for fallback_case in PreviousResponseNotFoundDecisionFallbackCase::ALL {
                    for retry_index in 0..=RUNTIME_PREVIOUS_RESPONSE_RETRY_DELAYS_MS.len() {
                        let request_requires_previous_response_affinity =
                            fallback_case.request_requires_previous_response_affinity();
                        let request_turn_state = has_turn_state_retry.then_some("turn_state");
                        let request_requires_locked_previous_response_affinity = match route {
                            RuntimePreviousResponseNotFoundRoute::Responses => {
                                request_requires_previous_response_affinity
                            }
                            RuntimePreviousResponseNotFoundRoute::Websocket => {
                                request_requires_previous_response_affinity
                                    || (trusted_previous_response_affinity
                                        && request_turn_state.is_none())
                            }
                        };
                        let locked_previous_response_retry =
                            matches!(route, RuntimePreviousResponseNotFoundRoute::Websocket)
                                && request_requires_previous_response_affinity
                                && !has_turn_state_retry;
                        let expected_retry_delay = (has_turn_state_retry
                            || locked_previous_response_retry)
                            .then(|| {
                                RUNTIME_PREVIOUS_RESPONSE_RETRY_DELAYS_MS
                                    .get(retry_index)
                                    .copied()
                                    .map(Duration::from_millis)
                            })
                            .flatten();
                        let expected_retry_reason = if has_turn_state_retry {
                            Some("non_blocking_retry")
                        } else if locked_previous_response_retry {
                            Some("locked_affinity_no_turn_state")
                        } else {
                            None
                        };
                        let expected_chain_retry_reason = match route {
                            RuntimePreviousResponseNotFoundRoute::Responses
                                if has_turn_state_retry =>
                            {
                                Some("previous_response_not_found")
                            }
                            RuntimePreviousResponseNotFoundRoute::Websocket
                                if locked_previous_response_retry =>
                            {
                                Some("previous_response_not_found_locked_affinity")
                            }
                            _ => None,
                        };
                        let expected_stale_continuation = !has_turn_state_retry;
                        let expected_fresh_fallback_blocked_without_affinity = !has_turn_state_retry
                            && !request_requires_locked_previous_response_affinity;
                        let expected_observability =
                            if expected_fresh_fallback_blocked_without_affinity {
                                match fallback_case.fresh_fallback_shape() {
                                Some(
                                    RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation,
                                ) => Some("blocked_nonreplayable_without_affinity"),
                                _ => Some("blocked_without_affinity"),
                            }
                            } else {
                                None
                            };
                        let case_label = format!(
                            "route={} turn_state={} trusted_affinity={} fallback={} retry_index={}",
                            previous_response_not_found_route_label(route),
                            has_turn_state_retry,
                            trusted_previous_response_affinity,
                            fallback_case.label(),
                            retry_index
                        );

                        let decision = runtime_previous_response_not_found_decision(
                            RuntimePreviousResponseNotFoundDecisionInput {
                                route,
                                previous_response_id: Some("resp_123"),
                                has_turn_state_retry,
                                request_requires_previous_response_affinity,
                                trusted_previous_response_affinity,
                                request_turn_state,
                                previous_response_fresh_fallback_used: false,
                                fresh_fallback_shape: fallback_case.fresh_fallback_shape(),
                                retry_index,
                            },
                        );

                        assert_eq!(
                            decision.request_requires_locked_previous_response_affinity,
                            request_requires_locked_previous_response_affinity,
                            "{}",
                            case_label
                        );
                        assert_eq!(decision.retry_delay, expected_retry_delay, "{}", case_label);
                        assert_eq!(
                            decision.retry_reason, expected_retry_reason,
                            "{}",
                            case_label
                        );
                        assert_eq!(
                            decision.chain_retry_reason, expected_chain_retry_reason,
                            "{}",
                            case_label
                        );
                        assert_eq!(
                            decision.stale_continuation, expected_stale_continuation,
                            "{}",
                            case_label
                        );
                        assert!(
                            !decision.fresh_fallback_allowed,
                            "{} should stay fail-closed for fresh fallback",
                            case_label
                        );
                        assert_eq!(
                            decision.fresh_fallback_blocked_without_affinity,
                            expected_fresh_fallback_blocked_without_affinity,
                            "{}",
                            case_label
                        );
                        assert_eq!(
                            runtime_previous_response_not_found_observability_outcome(
                                decision,
                                fallback_case.fresh_fallback_shape()
                            ),
                            expected_observability,
                            "{}",
                            case_label
                        );
                    }
                }
            }
        }
    }
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
        "websocket metadata should keep tool outputs chained to prior tool-call context"
    );
}

#[test]
fn parse_runtime_websocket_request_metadata_blocks_message_followup_replay() {
    let metadata = parse_runtime_websocket_request_metadata(
        r#"{"previous_response_id":"resp_123","input":[{"type":"message","role":"user","content":[{"type":"input_text","text":"continue"}]}]}"#,
    );

    assert_eq!(metadata.previous_response_id.as_deref(), Some("resp_123"));
    assert_eq!(metadata.session_id.as_deref(), None);
    assert!(!metadata.requires_previous_response_affinity);
    assert_eq!(
        metadata.previous_response_fresh_fallback_shape,
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(
            metadata.previous_response_fresh_fallback_shape
        ),
        "websocket message follow-ups should stay pinned to prior context"
    );
}

#[test]
fn websocket_session_header_does_not_make_message_followup_replayable() {
    let metadata = parse_runtime_websocket_request_metadata(
        r#"{"previous_response_id":"resp_123","input":[{"type":"message","role":"user","content":[{"type":"input_text","text":"continue"}]}]}"#,
    );

    let shape = runtime_previous_response_fresh_fallback_shape_with_session(
        metadata.previous_response_fresh_fallback_shape,
        true,
    );

    assert_eq!(
        shape,
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(shape),
        "websocket session headers must not make incremental message follow-ups replayable"
    );
}

#[test]
fn runtime_previous_response_fresh_fallback_policy_is_explicitly_fail_closed() {
    let policy = runtime_previous_response_fresh_fallback_policy(
        RuntimePreviousResponseFreshFallbackPolicyInput {
            has_previous_response_context: true,
            request_requires_locked_previous_response_affinity: false,
            fresh_fallback_shape: Some(
                RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay,
            ),
        },
    );

    assert_eq!(
        policy,
        RuntimePreviousResponseFreshFallbackPolicy::FailClosed {
            request_shape:
                RuntimePreviousResponseFreshFallbackPolicyShape::SessionScopedFreshReplay,
        }
    );
    assert!(!policy.allows_fresh_fallback());
}

#[test]
fn runtime_previous_response_fresh_fallback_policy_is_not_applicable_without_context() {
    assert_eq!(
        runtime_previous_response_fresh_fallback_policy(
            RuntimePreviousResponseFreshFallbackPolicyInput {
                has_previous_response_context: false,
                request_requires_locked_previous_response_affinity: false,
                fresh_fallback_shape: None,
            }
        ),
        RuntimePreviousResponseFreshFallbackPolicy::NotApplicable
    );
}

#[test]
fn websocket_previous_response_not_found_fallback_policy_is_explicitly_fail_closed() {
    let policy = runtime_previous_response_not_found_fallback_policy(
        RuntimePreviousResponseNotFoundFallbackRequest {
            previous_response_id: Some("resp_123"),
            has_turn_state_retry: false,
            request_requires_locked_previous_response_affinity: false,
            previous_response_fresh_fallback_used: false,
            fresh_fallback_shape: Some(
                RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation,
            ),
        },
    );

    assert_eq!(
        policy.stale_continuation,
        RuntimePreviousResponseStaleContinuationPolicy::FailClosed
    );
    assert_eq!(
        policy.fresh_fallback,
        RuntimePreviousResponseFreshFallbackPolicy::FailClosed {
            request_shape:
                RuntimePreviousResponseFreshFallbackPolicyShape::ContextDependentContinuation,
        }
    );
    assert!(policy.fresh_fallback.blocks_without_affinity(false, false));
}

#[test]
fn quota_blocked_affinity_release_policy_preserves_unclassified_release_behavior() {
    let policy =
        runtime_quota_blocked_affinity_release_policy(RuntimeQuotaBlockedAffinityReleaseRequest {
            affinity: RuntimeCandidateAffinity::new(
                RuntimeRouteKind::Responses,
                "main",
                None,
                Some("main"),
                None,
                None,
                true,
            ),
            fresh_fallback_shape: None,
        });

    assert_eq!(
        policy,
        RuntimeQuotaBlockedAffinityReleasePolicy::ReleaseAffinity
    );
}

#[test]
fn runtime_candidate_no_rotate_affinity_makes_hard_affinity_explicit() {
    assert_eq!(
        runtime_candidate_no_rotate_affinity(RuntimeCandidateAffinity::new(
            RuntimeRouteKind::Responses,
            "main",
            Some("main"),
            None,
            None,
            None,
            false,
        )),
        Some(RuntimeNoRotateAffinity::Strict)
    );
    assert_eq!(
        runtime_candidate_no_rotate_affinity(RuntimeCandidateAffinity::new(
            RuntimeRouteKind::Responses,
            "main",
            None,
            Some("main"),
            None,
            None,
            true,
        )),
        Some(RuntimeNoRotateAffinity::TrustedPreviousResponse)
    );
    assert_eq!(
        runtime_candidate_no_rotate_affinity(RuntimeCandidateAffinity::new(
            RuntimeRouteKind::Responses,
            "main",
            None,
            Some("main"),
            None,
            None,
            false,
        )),
        None
    );
    assert_eq!(
        runtime_candidate_no_rotate_affinity(RuntimeCandidateAffinity::new(
            RuntimeRouteKind::Compact,
            "main",
            None,
            None,
            None,
            Some("main"),
            false,
        )),
        Some(RuntimeNoRotateAffinity::CompactSession)
    );
}

fn runtime_shared_for_affinity_selection(
    temp_dir: &TestDir,
    response_profile_bindings: BTreeMap<String, ResponseProfileBinding>,
) -> RuntimeRotationProxyShared {
    let now = Local::now().timestamp();
    runtime_rotation_proxy_shared(
        temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
                active_profile: Some("main".to_string()),
                profiles: BTreeMap::from([
                    (
                        "main".to_string(),
                        ProfileEntry {
                            codex_home: temp_dir.path.join("homes/main"),
                            managed: true,
                            email: None,
                            provider: ProfileProvider::Openai,
                        },
                    ),
                    (
                        "second".to_string(),
                        ProfileEntry {
                            codex_home: temp_dir.path.join("homes/second"),
                            managed: true,
                            email: None,
                            provider: ProfileProvider::Openai,
                        },
                    ),
                ]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings,
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::from([
                (
                    "main".to_string(),
                    RuntimeProfileProbeCacheEntry {
                        checked_at: now,
                        auth: AuthSummary {
                            label: "chatgpt".to_string(),
                            quota_compatible: true,
                        },
                        result: Ok(usage_with_main_windows(0, 300, 95, 86_400)),
                    },
                ),
                (
                    "second".to_string(),
                    RuntimeProfileProbeCacheEntry {
                        checked_at: now,
                        auth: AuthSummary {
                            label: "chatgpt".to_string(),
                            quota_compatible: true,
                        },
                        result: Ok(usage_with_main_windows(95, 18_000, 95, 604_800)),
                    },
                ),
            ]),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    )
}

#[test]
fn response_selection_preserves_bound_previous_response_affinity_despite_quota() {
    let temp_dir = TestDir::isolated();
    let shared = runtime_shared_for_affinity_selection(
        &temp_dir,
        BTreeMap::from([(
            "resp_123".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
    );

    let selected = select_runtime_response_candidate_for_route(
        &shared,
        &BTreeSet::new(),
        None,
        Some("main"),
        None,
        None,
        false,
        Some("resp_123"),
        RuntimeRouteKind::Responses,
    )
    .expect("selection should succeed");

    assert_eq!(selected.as_deref(), Some("main"));
}

#[test]
fn response_selection_skips_soft_pinned_affinity_when_quota_blocks_precommit() {
    let temp_dir = TestDir::isolated();
    let shared = runtime_shared_for_affinity_selection(&temp_dir, BTreeMap::new());

    let selected = select_runtime_response_candidate_for_route(
        &shared,
        &BTreeSet::new(),
        None,
        Some("main"),
        None,
        None,
        false,
        Some("resp_unbound"),
        RuntimeRouteKind::Responses,
    )
    .expect("selection should succeed");

    assert_eq!(selected.as_deref(), Some("second"));
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
fn quota_blocked_previous_response_fresh_fallback_blocks_session_scoped_requests() {
    assert!(
        !runtime_quota_blocked_previous_response_fresh_fallback_allowed(
            Some("resp_123"),
            true,
            false,
            Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay),
        )
    );
    assert!(
        !runtime_quota_blocked_previous_response_fresh_fallback_allowed(
            Some("resp_123"),
            true,
            false,
            Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation),
        )
    );
}

#[test]
fn quota_blocked_affinity_release_blocks_nonreplayable_tool_outputs() {
    assert!(!runtime_quota_blocked_affinity_is_releasable(
        RuntimeCandidateAffinity::new(
            RuntimeRouteKind::Responses,
            "main",
            None,
            Some("main"),
            None,
            None,
            true,
        ),
        true,
        Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly),
    ));
}

#[test]
fn quota_blocked_affinity_release_blocks_nonreplayable_message_followups() {
    assert!(!runtime_quota_blocked_affinity_is_releasable(
        RuntimeCandidateAffinity::new(
            RuntimeRouteKind::Responses,
            "main",
            None,
            Some("main"),
            None,
            None,
            true,
        ),
        false,
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation),
    ));
}

#[test]
fn quota_blocked_affinity_release_blocks_session_scoped_empty_inputs() {
    assert!(!runtime_quota_blocked_affinity_is_releasable(
        RuntimeCandidateAffinity::new(
            RuntimeRouteKind::Responses,
            "main",
            None,
            Some("main"),
            None,
            None,
            true,
        ),
        true,
        Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay),
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
    let root = TestDir::isolated();
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
fn caveman_command_accepts_full_access_shortcut_after_mem_prefix() {
    let command = parse_cli_command_from([
        "prodex",
        "caveman",
        "mem",
        "--full-access",
        "exec",
        "review this repo",
    ])
    .expect("caveman full-access shortcut should parse");
    let Commands::Caveman(args) = command else {
        panic!("expected caveman command");
    };
    assert!(!args.full_access);
    assert_eq!(
        args.codex_args,
        vec![
            OsString::from("mem"),
            OsString::from("--full-access"),
            OsString::from("exec"),
            OsString::from("review this repo")
        ]
    );

    let (mem_mode, codex_args) = runtime_mem_extract_mode(&args.codex_args);
    assert!(mem_mode);
    let (launch_args, include_code_review) =
        prepare_codex_launch_args(&codex_args, args.full_access);
    assert_eq!(
        launch_args,
        vec![
            OsString::from("--dangerously-bypass-approvals-and-sandbox"),
            OsString::from("exec"),
            OsString::from("review this repo")
        ]
    );
    assert!(!include_code_review);
}

#[test]
fn super_command_parses_as_distinct_subcommand_and_expands_to_caveman_mem_full_access() {
    let command = parse_cli_command_from([
        "prodex",
        "super",
        "--profile",
        "main",
        "exec",
        "review this repo",
    ])
    .expect("super command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };
    assert_eq!(args.profile.as_deref(), Some("main"));
    assert_eq!(
        args.codex_args,
        vec![OsString::from("exec"), OsString::from("review this repo")]
    );

    let args = args.into_caveman_args();
    assert_eq!(args.profile.as_deref(), Some("main"));
    assert!(args.full_access);
    assert_eq!(
        args.codex_args,
        vec![
            OsString::from("mem"),
            OsString::from("exec"),
            OsString::from("review this repo")
        ]
    );
}

#[test]
fn super_command_accepts_s_alias() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "--profile",
        "main",
        "exec",
        "review this repo",
    ])
    .expect("super alias command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };
    assert_eq!(args.profile.as_deref(), Some("main"));
    assert_eq!(
        args.codex_args,
        vec![OsString::from("exec"), OsString::from("review this repo")]
    );
}

#[test]
fn super_command_url_expands_to_local_openai_provider_config() {
    let command = parse_cli_command_from([
        "prodex",
        "super",
        "--url",
        "http://127.0.0.1:8131",
        "--model",
        "local/qwen",
        "exec",
        "review this repo",
    ])
    .expect("super local provider command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };
    assert_eq!(args.url.as_deref(), Some("http://127.0.0.1:8131"));
    assert_eq!(args.local_model.as_deref(), Some("local/qwen"));

    let args = args.into_caveman_args();
    assert!(args.full_access);
    assert!(args.skip_quota_check);

    let rendered = args
        .codex_args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(rendered.first().map(String::as_str), Some("mem"));
    assert!(rendered.contains(&"model_provider=\"prodex-local\"".to_string()));
    assert!(rendered.contains(&"model=\"local/qwen\"".to_string()));
    assert!(rendered.contains(
        &"model_providers.prodex-local.base_url=\"http://127.0.0.1:8131/v1\"".to_string()
    ));
    assert!(rendered.contains(&"model_providers.prodex-local.wire_api=\"responses\"".to_string()));
    assert!(
        rendered.contains(&"model_providers.prodex-local.supports_websockets=false".to_string())
    );
    assert!(rendered.contains(&"model_context_window=16384".to_string()));
    assert!(rendered.contains(&"model_auto_compact_token_limit=14000".to_string()));
    assert!(rendered.contains(&"web_search=\"disabled\"".to_string()));
    assert!(rendered.contains(&"features.js_repl=false".to_string()));
    assert!(rendered.contains(&"features.image_generation=false".to_string()));
    assert_eq!(
        &rendered[rendered.len() - 2..],
        ["exec", "review this repo"]
    );

    let (mem_mode, codex_args) = runtime_mem_extract_mode(&args.codex_args);
    assert!(mem_mode);
    assert_eq!(
        codex_cli_config_override_value(&codex_args, "model_provider").as_deref(),
        Some("prodex-local")
    );
}

#[test]
fn super_command_url_keeps_v1_path_when_provided() {
    let command = parse_cli_command_from([
        "prodex",
        "super",
        "--url",
        "http://host.docker.internal:11434/v1/",
        "exec",
        "review this repo",
    ])
    .expect("super local provider command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };

    let args = args.into_caveman_args();
    let rendered = args
        .codex_args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(
        rendered.contains(
            &"model_providers.prodex-local.base_url=\"http://host.docker.internal:11434/v1\""
                .to_string()
        )
    );
}

#[test]
fn super_command_url_rejects_invalid_or_empty_values() {
    for url in ["", "not-a-url", "file:///tmp/model.sock", "http:///v1"] {
        let err = parse_cli_command_from(["prodex", "super", "--url", url, "exec", "hello"])
            .expect_err("invalid super local provider URL should fail");
        let message = err.to_string();
        assert!(
            message.contains("invalid --url"),
            "expected clear --url error for {url:?}, got {message}"
        );
    }
}

#[test]
fn super_command_url_accepts_local_context_overrides() {
    let command = parse_cli_command_from([
        "prodex",
        "super",
        "--url",
        "http://127.0.0.1:8131",
        "--context-window",
        "32768",
        "--auto-compact-token-limit",
        "30000",
        "exec",
        "review this repo",
    ])
    .expect("super local provider command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };

    let args = args.into_caveman_args();
    let rendered = args
        .codex_args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(rendered.contains(&"model_context_window=32768".to_string()));
    assert!(rendered.contains(&"model_auto_compact_token_limit=30000".to_string()));
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
