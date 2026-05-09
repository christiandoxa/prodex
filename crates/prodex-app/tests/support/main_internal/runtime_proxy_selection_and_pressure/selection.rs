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

#[test]
fn response_request_prompt_cache_key_is_trimmed_from_body() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: serde_json::to_vec(&serde_json::json!({
            "prompt_cache_key": " workspace-cache:abc ",
            "input": []
        }))
        .expect("request body should serialize"),
    };

    assert_eq!(
        runtime_request_prompt_cache_key(&request).as_deref(),
        Some("workspace-cache:abc")
    );
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeAffinityProfileCase {
    None,
    Candidate,
    Other,
}

impl RuntimeAffinityProfileCase {
    const ALL: [Self; 3] = [Self::None, Self::Candidate, Self::Other];

    fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Candidate => "candidate",
            Self::Other => "other",
        }
    }

    fn profile(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Candidate => Some("main"),
            Self::Other => Some("second"),
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
fn previous_response_not_found_fallback_matrix_fails_closed_for_context_dependent_replay() {
    for previous_response_id in [None, Some("resp_123")] {
        for has_turn_state_retry in [false, true] {
            for request_requires_locked_previous_response_affinity in [false, true] {
                for previous_response_fresh_fallback_used in [false, true] {
                    let policy = runtime_previous_response_not_found_fallback_policy(
                        RuntimePreviousResponseNotFoundFallbackRequest {
                            previous_response_id,
                            has_turn_state_retry,
                            request_requires_locked_previous_response_affinity,
                            previous_response_fresh_fallback_used,
                            fresh_fallback_shape: Some(
                                RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation,
                            ),
                        },
                    );
                    let label = format!(
                        "previous_response={} turn_state_retry={} locked_affinity={} fresh_fallback_used={}",
                        previous_response_id.is_some(),
                        has_turn_state_retry,
                        request_requires_locked_previous_response_affinity,
                        previous_response_fresh_fallback_used
                    );

                    assert_eq!(
                        policy.fresh_fallback,
                        RuntimePreviousResponseFreshFallbackPolicy::FailClosed {
                            request_shape:
                                RuntimePreviousResponseFreshFallbackPolicyShape::ContextDependentContinuation,
                        },
                        "{label}"
                    );
                    assert!(!policy.fresh_fallback.allows_fresh_fallback(), "{label}");
                    assert_eq!(
                        policy.stale_continuation.requires_stale_continuation(),
                        previous_response_id.is_some() && !has_turn_state_retry,
                        "{label}"
                    );
                }
            }
        }
    }
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
fn runtime_http_error_policy_passes_generic_429_through() {
    let policy = runtime_http_error_policy(
        429,
        br#"{"error":{"message":"Too Many Requests"}}"#,
        RuntimeHttpErrorPhase::PreCommit,
    );

    assert_eq!(policy.class, RuntimeHttpErrorClass::Other);
    assert_eq!(policy.action, RuntimeHttpErrorAction::PassThrough);
    assert_eq!(policy.rule, None);
    assert!(!policy.may_retry_or_rotate());
}

#[test]
fn runtime_http_error_policy_rotates_explicit_quota_codes_before_commit_only() {
    for code in ["insufficient_quota", "rate_limit_exceeded"] {
        let body = serde_json::to_vec(&serde_json::json!({
            "error": {
                "code": code,
                "message": "Quota exhausted"
            }
        }))
        .expect("quota body should serialize");

        let precommit = runtime_http_error_policy(429, &body, RuntimeHttpErrorPhase::PreCommit);
        assert_eq!(precommit.class, RuntimeHttpErrorClass::Quota, "{code}");
        assert_eq!(
            precommit.action,
            RuntimeHttpErrorAction::RotateProfile,
            "{code}"
        );
        assert_eq!(precommit.rule, Some("explicit_quota"), "{code}");
        assert_eq!(precommit.message.as_deref(), Some("Quota exhausted"));

        let committed = runtime_http_error_policy(429, &body, RuntimeHttpErrorPhase::Committed);
        assert_eq!(committed.class, RuntimeHttpErrorClass::Quota, "{code}");
        assert_eq!(
            committed.action,
            RuntimeHttpErrorAction::PassThrough,
            "{code}"
        );
    }
}

#[test]
fn runtime_http_error_policy_retries_transient_5xx_before_commit_only() {
    for status in [500, 502, 503, 504, 529] {
        let precommit = runtime_http_error_policy(
            status,
            b"backend unavailable",
            RuntimeHttpErrorPhase::PreCommit,
        );
        assert_eq!(
            precommit.class,
            RuntimeHttpErrorClass::TransientServer,
            "{status}"
        );
        assert_eq!(
            precommit.action,
            RuntimeHttpErrorAction::RetryProfile,
            "{status}"
        );
        assert_eq!(precommit.rule, Some("transient_5xx"), "{status}");

        let committed = runtime_http_error_policy(
            status,
            b"backend unavailable",
            RuntimeHttpErrorPhase::Committed,
        );
        assert_eq!(
            committed.action,
            RuntimeHttpErrorAction::PassThrough,
            "{status}"
        );
    }
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

#[test]
fn runtime_candidate_no_rotate_affinity_matrix_prioritizes_hard_bindings() {
    for route_kind in [
        RuntimeRouteKind::Responses,
        RuntimeRouteKind::Compact,
        RuntimeRouteKind::Websocket,
        RuntimeRouteKind::Standard,
    ] {
        for strict in RuntimeAffinityProfileCase::ALL {
            for pinned in RuntimeAffinityProfileCase::ALL {
                for turn_state in RuntimeAffinityProfileCase::ALL {
                    for session in RuntimeAffinityProfileCase::ALL {
                        for trusted_previous_response_affinity in [false, true] {
                            let affinity = RuntimeCandidateAffinity::new(
                                route_kind,
                                "main",
                                strict.profile(),
                                pinned.profile(),
                                turn_state.profile(),
                                session.profile(),
                                trusted_previous_response_affinity,
                            );
                            let expected = if strict == RuntimeAffinityProfileCase::Candidate {
                                Some(RuntimeNoRotateAffinity::Strict)
                            } else if turn_state == RuntimeAffinityProfileCase::Candidate {
                                Some(RuntimeNoRotateAffinity::TurnState)
                            } else if trusted_previous_response_affinity
                                && pinned == RuntimeAffinityProfileCase::Candidate
                            {
                                Some(RuntimeNoRotateAffinity::TrustedPreviousResponse)
                            } else if route_kind == RuntimeRouteKind::Compact
                                && session == RuntimeAffinityProfileCase::Candidate
                            {
                                Some(RuntimeNoRotateAffinity::CompactSession)
                            } else {
                                None
                            };
                            let label = format!(
                                "route={} strict={} pinned={} turn_state={} session={} trusted_previous_response_affinity={}",
                                runtime_route_kind_label(route_kind),
                                strict.label(),
                                pinned.label(),
                                turn_state.label(),
                                session.label(),
                                trusted_previous_response_affinity
                            );

                            assert_eq!(
                                runtime_candidate_no_rotate_affinity(affinity),
                                expected,
                                "{label}"
                            );
                            assert_eq!(
                                runtime_candidate_has_hard_affinity(affinity),
                                expected.is_some(),
                                "{label}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn quota_blocked_affinity_release_matrix_keeps_hard_or_classified_continuations() {
    let shapes = [
        None,
        Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly),
        Some(RuntimePreviousResponseFreshFallbackShape::EmptyInputOnly),
        Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay),
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation),
    ];

    for route_kind in [
        RuntimeRouteKind::Responses,
        RuntimeRouteKind::Compact,
        RuntimeRouteKind::Websocket,
        RuntimeRouteKind::Standard,
    ] {
        for strict in RuntimeAffinityProfileCase::ALL {
            for pinned in RuntimeAffinityProfileCase::ALL {
                for turn_state in RuntimeAffinityProfileCase::ALL {
                    for session in RuntimeAffinityProfileCase::ALL {
                        for trusted_previous_response_affinity in [false, true] {
                            for fresh_fallback_shape in shapes {
                                let affinity = RuntimeCandidateAffinity::new(
                                    route_kind,
                                    "main",
                                    strict.profile(),
                                    pinned.profile(),
                                    turn_state.profile(),
                                    session.profile(),
                                    trusted_previous_response_affinity,
                                );
                                let expected = if fresh_fallback_shape.is_some()
                                    || strict == RuntimeAffinityProfileCase::Candidate
                                    || turn_state == RuntimeAffinityProfileCase::Candidate
                                    || (route_kind == RuntimeRouteKind::Compact
                                        && session == RuntimeAffinityProfileCase::Candidate)
                                {
                                    RuntimeQuotaBlockedAffinityReleasePolicy::KeepAffinity
                                } else {
                                    RuntimeQuotaBlockedAffinityReleasePolicy::ReleaseAffinity
                                };
                                let label = format!(
                                    "route={} strict={} pinned={} turn_state={} session={} trusted_previous_response_affinity={} shape={}",
                                    runtime_route_kind_label(route_kind),
                                    strict.label(),
                                    pinned.label(),
                                    turn_state.label(),
                                    session.label(),
                                    trusted_previous_response_affinity,
                                    runtime_previous_response_fresh_fallback_shape_label(
                                        fresh_fallback_shape
                                    )
                                );

                                assert_eq!(
                                    runtime_quota_blocked_affinity_release_policy(
                                        RuntimeQuotaBlockedAffinityReleaseRequest {
                                            affinity,
                                            fresh_fallback_shape,
                                        }
                                    ),
                                    expected,
                                    "{label}"
                                );
                                assert_eq!(
                                    runtime_quota_blocked_affinity_is_releasable(
                                        affinity,
                                        false,
                                        fresh_fallback_shape,
                                    ),
                                    expected
                                        == RuntimeQuotaBlockedAffinityReleasePolicy::ReleaseAffinity,
                                    "{label}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
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

fn apply_local_selection_penalties(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) {
    let now = Local::now().timestamp();
    let mut runtime = shared.runtime.lock().expect("runtime lock should succeed");
    runtime.profile_transport_backoff_until.insert(
        runtime_profile_transport_backoff_key(profile_name, route_kind),
        now + 60,
    );
    runtime.profile_inflight.insert(
        profile_name.to_string(),
        runtime_profile_inflight_soft_limit(route_kind, false),
    );
    runtime.profile_health.insert(
        profile_name.to_string(),
        RuntimeProfileHealth {
            score: RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY,
            updated_at: now,
        },
    );
    runtime.profile_health.insert(
        runtime_profile_route_health_key(profile_name, route_kind),
        RuntimeProfileHealth {
            score: RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY,
            updated_at: now,
        },
    );
}

#[path = "selection/profile_selection.rs"]
mod profile_selection;
