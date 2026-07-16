use super::*;

#[path = "selection/affinity_policy.rs"]
mod affinity_policy;
#[path = "selection/fallback_shapes.rs"]
mod fallback_shapes;
#[path = "selection/http_error_policy.rs"]
mod http_error_policy;
#[path = "selection/previous_response_policy.rs"]
mod previous_response_policy;
#[path = "selection/prompt_cache.rs"]
mod prompt_cache;
#[path = "selection/sync_probe.rs"]
mod sync_probe;

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
    shared.lane_admission.set_profile_inflight(
        profile_name.to_string(),
        runtime_profile_inflight_soft_limit(route_kind, false),
    );
    let mut runtime = shared.runtime.lock().expect("runtime lock should succeed");
    runtime.profile_transport_backoff_until.insert(
        runtime_profile_transport_backoff_key(profile_name, route_kind),
        now + 60,
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
