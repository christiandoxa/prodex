use prodex_domain::{ApiVersion, ApiVersionDecision, ApiVersionError, ApiVersionStatus};
use prodex_gateway_http::{
    CanonicalRequestTarget, CanonicalRequestTargetError, GatewayControlPlaneOperation,
    GatewayControlPlaneRouteError, GatewayControlPlaneRouteErrorStatus,
    GatewayHttpApiVersionErrorStatus, GatewayHttpDrainPlanError, GatewayHttpEntityTagErrorStatus,
    GatewayHttpErrorStatus, GatewayHttpHeader, GatewayHttpMethod,
    GatewayHttpPaginationQueryErrorStatus, GatewayHttpPlanError, GatewayHttpPolicy,
    GatewayHttpRequestMeta, GatewayHttpRouteKind, GatewayHttpRoutePlane, classify_request_target,
    classify_route, classify_upstream_headers, control_plane_request_fingerprint,
    entity_tag_from_if_match_headers, idempotency_key_from_headers, page_request_from_query,
    plan_control_plane_route, plan_gateway_control_plane_route_error_response,
    plan_gateway_http_api_version, plan_gateway_http_api_version_error_response,
    plan_gateway_http_drain, plan_gateway_http_entity_tag_error_response,
    plan_gateway_http_error_response, plan_gateway_http_execution,
    plan_gateway_http_idempotency_key_error_response,
    plan_gateway_http_pagination_query_error_response, plan_gateway_http_request,
    plan_gateway_http_request_fingerprint_error_response,
};

fn traceparent() -> GatewayHttpHeader {
    GatewayHttpHeader::new(
        "traceparent",
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
    )
}

fn request(path: &str) -> GatewayHttpRequestMeta {
    GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Post,
        path: path.to_string(),
        body_len: 128,
        headers: vec![traceparent()],
    }
}

#[test]
fn api_version_planner_extracts_versioned_paths_and_uses_legacy_default() {
    let default_version = ApiVersion::new(1, 0);
    let policies = [
        prodex_domain::ApiVersionPolicy {
            version: default_version,
            status: ApiVersionStatus::Current,
        },
        prodex_domain::ApiVersionPolicy {
            version: ApiVersion::new(2, 0),
            status: ApiVersionStatus::Deprecated {
                deprecated_at_unix_ms: 1_700_000_000_000,
                sunset_at_unix_ms: Some(1_900_000_000_000),
            },
        },
    ];
    let legacy =
        plan_gateway_http_api_version("/responses", default_version, &policies, 1_800_000_000_000)
            .unwrap();
    assert_eq!(legacy.requested, default_version);
    assert_eq!(legacy.decision, ApiVersionDecision::Allowed);
    assert!(!legacy.explicit_path_version);
    let versioned = plan_gateway_http_api_version(
        "/v2/admin/keys?cursor=opaque#fragment",
        default_version,
        &policies,
        1_800_000_000_000,
    )
    .unwrap();
    assert_eq!(versioned.requested, ApiVersion::new(2, 0));
    assert_eq!(
        versioned.decision,
        ApiVersionDecision::AllowedDeprecated {
            deprecated_at_unix_ms: 1_700_000_000_000,
            sunset_at_unix_ms: Some(1_900_000_000_000),
        }
    );
    assert!(versioned.explicit_path_version);
    assert_eq!(
        plan_gateway_http_api_version(
            "/v9/responses",
            default_version,
            &policies,
            1_800_000_000_000
        ),
        Err(ApiVersionError::Unsupported {
            requested: ApiVersion::new(9, 0),
        })
    );
    assert_eq!(
        plan_gateway_http_api_version(
            "/v999999/responses?debug=true",
            default_version,
            &policies,
            1_800_000_000_000
        ),
        Err(ApiVersionError::Unsupported {
            requested: ApiVersion::new(u16::MAX, 0),
        })
    );
}

#[test]
fn api_version_error_responses_delegate_to_stable_domain_envelope() {
    let error = ApiVersionError::Sunset {
        requested: ApiVersion::new(2, 0),
        sunset_at_unix_ms: 1_700_000_000_000,
    };
    let response = plan_gateway_http_api_version_error_response(&error);
    assert_eq!(response.status, GatewayHttpApiVersionErrorStatus::Gone);
    assert_eq!(response.code, "api_version_sunset");
    assert_eq!(response.message, "API version is no longer available");
    assert!(!response.message.contains("v2"));
    assert!(!response.message.contains("1700000000000"));
}

#[test]
fn data_plane_response_route_requires_post_trace_and_body_limit() {
    let policy = GatewayHttpPolicy {
        max_body_bytes: 256,
        ..GatewayHttpPolicy::production_default()
    };
    let plan = plan_gateway_http_request(policy, request("/v1/responses")).unwrap();
    assert_eq!(plan.route, GatewayHttpRouteKind::DataPlaneResponses);
    assert!(plan.trace_context.is_some());
    assert_eq!(plan.trace_propagation_metrics.len(), 3);
    assert_eq!(
        plan.trace_propagation_metrics[0]
            .carrier_label
            .as_metric_label()
            .unwrap(),
        ("trace_carrier", "traceparent")
    );
    assert_eq!(
        plan.trace_propagation_metrics[0]
            .result_label
            .as_metric_label()
            .unwrap(),
        ("trace_propagation_result", "propagated")
    );
    assert_eq!(
        plan.trace_propagation_metrics[1]
            .result_label
            .as_metric_label()
            .unwrap(),
        ("trace_propagation_result", "missing")
    );
    assert_eq!(plan.timeout_budget.request_timeout_ms, 120_000);
    assert_eq!(plan.execution.max_body_bytes, 256);
    assert_eq!(
        plan.execution.max_concurrent_streams,
        policy.max_concurrent_streams
    );
    assert!(plan.execution.cancellation_propagation_required);
    assert!(plan.execution.streaming_backpressure_required);
    assert!(plan.execution.graceful_drain_required);

    let oversized = GatewayHttpRequestMeta {
        body_len: 257,
        ..request("/v1/responses")
    };
    assert_eq!(
        plan_gateway_http_request(policy, oversized),
        Err(GatewayHttpPlanError::BodyTooLarge {
            max: 256,
            actual: 257,
        })
    );
}

#[test]
fn request_headers_are_bounded_before_routing() {
    let mut too_many = request("/v1/responses");
    too_many
        .headers
        .push(GatewayHttpHeader::new("x-extra", "value"));
    let error = plan_gateway_http_request(
        GatewayHttpPolicy {
            max_header_count: 1,
            ..GatewayHttpPolicy::production_default()
        },
        too_many,
    )
    .unwrap_err();
    assert_eq!(error, GatewayHttpPlanError::HeaderCountExceeded);

    let error = plan_gateway_http_request(
        GatewayHttpPolicy {
            max_header_bytes: 64,
            max_single_header_bytes: 32,
            ..GatewayHttpPolicy::production_default()
        },
        request("/v1/responses"),
    )
    .unwrap_err();
    assert_eq!(error, GatewayHttpPlanError::HeaderFieldBytesExceeded);

    let mut too_large = request("/v1/responses");
    too_large
        .headers
        .push(GatewayHttpHeader::new("x-extra", "x".repeat(20)));
    let error = plan_gateway_http_request(
        GatewayHttpPolicy {
            max_header_bytes: 80,
            max_single_header_bytes: 70,
            ..GatewayHttpPolicy::production_default()
        },
        too_large,
    )
    .unwrap_err();
    assert_eq!(error, GatewayHttpPlanError::HeaderBytesExceeded);
    assert_eq!(error.to_string(), "HTTP headers are too large");
    let response = plan_gateway_http_error_response(&error);
    assert_eq!(
        response.status,
        GatewayHttpErrorStatus::RequestHeaderFieldsTooLarge
    );
    assert_eq!(response.code, "request_headers_too_large");
}

#[test]
fn execution_plan_captures_bounded_async_adapter_contract() {
    let policy = GatewayHttpPolicy {
        max_body_bytes: 1024,
        max_header_count: 128,
        max_header_bytes: 64 * 1024,
        max_single_header_bytes: 16 * 1024,
        request_timeout_ms: 10_000,
        stream_idle_timeout_ms: 1_500,
        max_concurrent_streams: 64,
        connection_drain_timeout_ms: 5_000,
        require_trace_context: true,
    };

    let streaming =
        plan_gateway_http_execution(policy, GatewayHttpRouteKind::DataPlaneResponses).unwrap();
    assert_eq!(streaming.max_body_bytes, 1024);
    assert_eq!(streaming.max_concurrent_streams, 64);
    assert_eq!(streaming.timeout_budget.request_timeout_ms, 10_000);
    assert_eq!(streaming.timeout_budget.stream_idle_timeout_ms, 1_500);
    assert_eq!(streaming.timeout_budget.connection_drain_timeout_ms, 5_000);
    assert!(streaming.cancellation_propagation_required);
    assert!(streaming.streaming_backpressure_required);
    assert!(streaming.graceful_drain_required);

    let control = plan_gateway_http_execution(policy, GatewayHttpRouteKind::ControlPlane).unwrap();
    assert!(!control.cancellation_propagation_required);
    assert!(!control.streaming_backpressure_required);
    assert!(control.graceful_drain_required);
}

#[test]
fn drain_plan_requires_termination_grace_to_cover_prestop_and_connection_drain() {
    let policy = GatewayHttpPolicy {
        connection_drain_timeout_ms: 30_000,
        ..GatewayHttpPolicy::production_default()
    };

    let plan = plan_gateway_http_drain(policy, 15_000, 45_000).unwrap();

    assert_eq!(plan.connection_drain_timeout_ms, 30_000);
    assert_eq!(plan.prestop_delay_ms, 15_000);
    assert_eq!(plan.termination_grace_ms, 45_000);
    assert!(plan.readiness_fails_before_drain);

    let short_grace = plan_gateway_http_drain(policy, 15_000, 44_999).unwrap_err();
    assert_eq!(
        short_grace,
        GatewayHttpDrainPlanError::TerminationGraceTooShort {
            required_ms: 45_000,
            actual_ms: 44_999,
        }
    );
    assert!(!short_grace.to_string().contains("44999"));
    assert!(!short_grace.to_string().contains("45000"));
    assert_eq!(
        plan_gateway_http_drain(policy, 0, 45_000),
        Err(GatewayHttpDrainPlanError::PreStopDelayRequired)
    );
    let missing_delay = plan_gateway_http_drain(policy, 0, 45_000).unwrap_err();
    assert_eq!(missing_delay.to_string(), "HTTP drain delay is invalid");
    assert!(!missing_delay.to_string().contains("preStop"));
}

#[test]
fn data_plane_routes_require_trace_when_policy_requires_it() {
    let mut no_trace = request("/responses/compact");
    no_trace.headers.clear();

    assert_eq!(
        plan_gateway_http_request(GatewayHttpPolicy::production_default(), no_trace),
        Err(GatewayHttpPlanError::MissingTraceContext)
    );

    let relaxed = GatewayHttpPolicy {
        require_trace_context: false,
        ..GatewayHttpPolicy::production_default()
    };
    let mut no_trace_relaxed = request("/responses/compact");
    no_trace_relaxed.headers.clear();
    assert!(plan_gateway_http_request(relaxed, no_trace_relaxed).is_ok());

    let duplicate_trace = GatewayHttpRequestMeta {
        headers: vec![
            traceparent(),
            GatewayHttpHeader::new(
                "Traceparent",
                "00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01",
            ),
        ],
        ..request("/responses/compact")
    };
    assert_eq!(
        plan_gateway_http_request(GatewayHttpPolicy::production_default(), duplicate_trace),
        Err(GatewayHttpPlanError::DuplicateTraceContext)
    );
    let response = plan_gateway_http_error_response(&GatewayHttpPlanError::DuplicateTraceContext);
    assert_eq!(response.status, GatewayHttpErrorStatus::BadRequest);
    assert_eq!(response.code, "invalid_trace_context");
    assert_eq!(
        response.message,
        "trace context is required and must be valid"
    );
    assert!(!response.message.contains("Traceparent"));
}

#[test]
fn method_validation_keeps_route_semantics_explicit() {
    let get_response = GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Get,
        ..request("/v1/responses")
    };

    assert!(matches!(
        plan_gateway_http_request(GatewayHttpPolicy::production_default(), get_response),
        Err(GatewayHttpPlanError::MethodNotAllowed {
            route: GatewayHttpRouteKind::DataPlaneResponses,
            method: GatewayHttpMethod::Get,
        })
    ));

    let health = GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Get,
        path: "/readyz".to_string(),
        body_len: 0,
        headers: vec![],
    };
    let plan = plan_gateway_http_request(GatewayHttpPolicy::production_default(), health).unwrap();
    assert_eq!(plan.route, GatewayHttpRouteKind::HealthReady);

    for (method, path) in [
        (GatewayHttpMethod::Post, "/v1/chat/completions"),
        (GatewayHttpMethod::Post, "/v1/embeddings"),
        (GatewayHttpMethod::Post, "/v1/images/generations"),
        (GatewayHttpMethod::Post, "/v1/audio/transcriptions"),
        (GatewayHttpMethod::Get, "/v1/batches"),
        (GatewayHttpMethod::Post, "/v1/batches"),
        (GatewayHttpMethod::Delete, "/v1/batches/batch_123"),
        (GatewayHttpMethod::Post, "/v1/rerank"),
        (GatewayHttpMethod::Post, "/v1/a2a"),
        (GatewayHttpMethod::Post, "/v1/messages"),
        (GatewayHttpMethod::Get, "/v1/models"),
        (GatewayHttpMethod::Get, "/v1/models/model-1"),
    ] {
        let request = GatewayHttpRequestMeta {
            method,
            ..request(path)
        };
        assert!(
            plan_gateway_http_request(GatewayHttpPolicy::production_default(), request).is_ok(),
            "{method:?} {path}"
        );
    }

    let post_models = GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Post,
        ..request("/v1/models")
    };
    assert!(matches!(
        plan_gateway_http_request(GatewayHttpPolicy::production_default(), post_models),
        Err(GatewayHttpPlanError::MethodNotAllowed {
            route: GatewayHttpRouteKind::DataPlaneModels,
            method: GatewayHttpMethod::Post,
        })
    ));
}

#[test]
fn upstream_header_policy_preserves_codex_metadata_and_strips_auth_or_hop_by_hop() {
    let headers = vec![
        GatewayHttpHeader::new("session_id", "sess-1"),
        GatewayHttpHeader::new("x-codex-turn-state", "state"),
        GatewayHttpHeader::new("User-Agent", "codex"),
        GatewayHttpHeader::new("traceparent", traceparent().value),
        GatewayHttpHeader::new("tracestate", "rojo=00f067aa0ba902b7"),
        GatewayHttpHeader::new("baggage", "tenant_tier=premium"),
        GatewayHttpHeader::new("Authorization", "Bearer secret"),
        GatewayHttpHeader::new("ChatGPT-Account-Id", "acct"),
        GatewayHttpHeader::new("Connection", "keep-alive"),
        GatewayHttpHeader::new("sec-websocket-key", "key"),
    ];

    let (preserved, stripped) = classify_upstream_headers(&headers);

    assert_eq!(
        preserved
            .iter()
            .map(|header| header.name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "session_id",
            "x-codex-turn-state",
            "user-agent",
            "traceparent",
            "tracestate",
            "baggage"
        ]
    );
    assert_eq!(
        stripped,
        vec![
            "authorization",
            "chatgpt-account-id",
            "connection",
            "sec-websocket-key",
        ]
    );
}

#[test]
fn duplicate_credential_headers_fail_closed_with_redacted_error() {
    let duplicate_authorization = GatewayHttpRequestMeta {
        headers: vec![
            traceparent(),
            GatewayHttpHeader::new("Authorization", "Bearer admin-secret"),
            GatewayHttpHeader::new("authorization", "Bearer client-secret"),
        ],
        ..request("/v1/responses")
    };
    let error = plan_gateway_http_request(
        GatewayHttpPolicy::production_default(),
        duplicate_authorization,
    )
    .unwrap_err();
    assert_eq!(error, GatewayHttpPlanError::DuplicateAuthorization);
    let response = plan_gateway_http_error_response(&error);
    assert_eq!(response.status, GatewayHttpErrorStatus::BadRequest);
    assert_eq!(response.code, "credential_header_invalid");
    assert_eq!(response.message, "credential header is invalid");
    assert!(!response.message.contains("Authorization"));
    assert!(!response.message.contains("secret"));

    let duplicate_account = GatewayHttpRequestMeta {
        headers: vec![
            traceparent(),
            GatewayHttpHeader::new("ChatGPT-Account-Id", "acct-admin"),
            GatewayHttpHeader::new("chatgpt-account-id", "acct-client"),
        ],
        ..request("/v1/responses")
    };
    let error =
        plan_gateway_http_request(GatewayHttpPolicy::production_default(), duplicate_account)
            .unwrap_err();
    assert_eq!(error, GatewayHttpPlanError::DuplicateChatGptAccountId);
    let response = plan_gateway_http_error_response(&error);
    assert_eq!(response.status, GatewayHttpErrorStatus::BadRequest);
    assert_eq!(response.code, "credential_header_invalid");
    assert_eq!(response.message, "credential header is invalid");
    assert!(!response.message.contains("ChatGPT"));
    assert!(!response.message.contains("acct"));
}

#[test]
fn duplicate_affinity_headers_fail_closed_with_redacted_error() {
    let duplicate_session = GatewayHttpRequestMeta {
        headers: vec![
            traceparent(),
            GatewayHttpHeader::new("session_id", "session-a"),
            GatewayHttpHeader::new("Session_Id", "session-b"),
        ],
        ..request("/v1/responses/compact")
    };
    let error =
        plan_gateway_http_request(GatewayHttpPolicy::production_default(), duplicate_session)
            .unwrap_err();
    assert_eq!(error, GatewayHttpPlanError::DuplicateSessionId);
    let response = plan_gateway_http_error_response(&error);
    assert_eq!(response.status, GatewayHttpErrorStatus::BadRequest);
    assert_eq!(response.code, "affinity_header_invalid");
    assert_eq!(response.message, "affinity header is invalid");
    assert!(!response.message.contains("session-a"));

    let duplicate_turn_state = GatewayHttpRequestMeta {
        headers: vec![
            traceparent(),
            GatewayHttpHeader::new("x-codex-turn-state", "state-a"),
            GatewayHttpHeader::new("X-Codex-Turn-State", "state-b"),
        ],
        ..request("/v1/responses")
    };
    let error = plan_gateway_http_request(
        GatewayHttpPolicy::production_default(),
        duplicate_turn_state,
    )
    .unwrap_err();
    assert_eq!(error, GatewayHttpPlanError::DuplicateCodexTurnState);
    let response = plan_gateway_http_error_response(&error);
    assert_eq!(response.status, GatewayHttpErrorStatus::BadRequest);
    assert_eq!(response.code, "affinity_header_invalid");
    assert_eq!(response.message, "affinity header is invalid");
    assert!(!response.message.contains("state-b"));
}

#[test]
fn duplicate_codex_metadata_headers_fail_closed_with_redacted_error() {
    for name in [
        "x-openai-subagent",
        "x-codex-turn-metadata",
        "x-codex-beta-features",
    ] {
        let duplicate_metadata = GatewayHttpRequestMeta {
            headers: vec![
                traceparent(),
                GatewayHttpHeader::new(name, "metadata-a"),
                GatewayHttpHeader::new(name.to_ascii_uppercase(), "metadata-b"),
            ],
            ..request("/v1/responses")
        };

        let error =
            plan_gateway_http_request(GatewayHttpPolicy::production_default(), duplicate_metadata)
                .unwrap_err();
        assert_eq!(error, GatewayHttpPlanError::DuplicateCodexMetadata);
        let response = plan_gateway_http_error_response(&error);
        assert_eq!(response.status, GatewayHttpErrorStatus::BadRequest);
        assert_eq!(response.code, "codex_metadata_header_invalid");
        assert_eq!(response.message, "Codex metadata header is invalid");
        assert!(!response.message.contains("metadata-a"));
        assert!(!response.message.contains("metadata-b"));
    }
}

#[test]
fn idempotency_key_header_is_optional_and_validated_redacted() {
    assert_eq!(idempotency_key_from_headers(&[]).unwrap(), None);

    let key = idempotency_key_from_headers(&[
        GatewayHttpHeader::new("Idempotency-Key", "admin-mutation-1"),
        GatewayHttpHeader::new("traceparent", traceparent().value),
    ])
    .unwrap()
    .unwrap();
    assert_eq!(key.as_str(), "admin-mutation-1");

    let error = idempotency_key_from_headers(&[GatewayHttpHeader::new(
        "Idempotency-Key",
        "bad key with spaces",
    )])
    .unwrap_err();
    let response = plan_gateway_http_idempotency_key_error_response(&error);
    assert_eq!(response.code, "idempotency_key_invalid");
    assert_eq!(response.message, "idempotency key is invalid");
    assert!(!response.message.contains("bad key"));

    let duplicate = idempotency_key_from_headers(&[
        GatewayHttpHeader::new("Idempotency-Key", "admin-mutation-1"),
        GatewayHttpHeader::new("idempotency-key", "admin-mutation-2"),
    ])
    .unwrap_err();
    assert_eq!(duplicate.to_string(), "request metadata is duplicated");
    assert!(!duplicate.to_string().contains("Idempotency-Key"));
    let response = plan_gateway_http_idempotency_key_error_response(&duplicate);
    assert_eq!(response.code, "idempotency_key_invalid");
    assert_eq!(response.message, "idempotency key is invalid");
    assert!(!response.message.contains("admin-mutation"));
}

#[test]
fn if_match_header_is_optional_and_validated_redacted() {
    assert_eq!(entity_tag_from_if_match_headers(&[]).unwrap(), None);

    let tag = entity_tag_from_if_match_headers(&[GatewayHttpHeader::new("If-Match", "W/\"42\"")])
        .unwrap()
        .unwrap();
    assert_eq!(tag.as_str(), "W/\"42\"");

    let error =
        entity_tag_from_if_match_headers(&[GatewayHttpHeader::new("If-Match", "x".repeat(300))])
            .unwrap_err();
    let response = plan_gateway_http_entity_tag_error_response(&error);
    assert_eq!(response.status, GatewayHttpEntityTagErrorStatus::BadRequest);
    assert_eq!(response.code, "entity_tag_invalid");
    assert_eq!(response.message, "entity tag is invalid");
    assert!(!response.message.contains("300"));
    assert!(!response.message.contains("too long"));

    let duplicate = entity_tag_from_if_match_headers(&[
        GatewayHttpHeader::new("If-Match", "W/\"42\""),
        GatewayHttpHeader::new("if-match", "W/\"43\""),
    ])
    .unwrap_err();
    assert_eq!(duplicate.to_string(), "request metadata is duplicated");
    assert!(!duplicate.to_string().contains("If-Match"));
    let response = plan_gateway_http_entity_tag_error_response(&duplicate);
    assert_eq!(response.status, GatewayHttpEntityTagErrorStatus::BadRequest);
    assert_eq!(response.code, "entity_tag_invalid");
    assert_eq!(response.message, "entity tag is invalid");
    assert!(!response.message.contains("43"));
}

#[test]
fn pagination_query_builds_domain_page_request_and_redacts_errors() {
    let page = page_request_from_query("?limit=25&cursor=opaque-next").unwrap();
    assert_eq!(page.limit, 25);
    assert_eq!(page.cursor.unwrap().as_str(), "opaque-next");

    let clamped = page_request_from_query("limit=10000").unwrap();
    assert_eq!(clamped.limit, prodex_domain::PageRequest::MAX_LIMIT);
    assert_eq!(clamped.cursor, None);

    let cursor_error = page_request_from_query("cursor=").unwrap_err();
    let cursor_response = plan_gateway_http_pagination_query_error_response(&cursor_error);
    assert_eq!(
        cursor_response.status,
        GatewayHttpPaginationQueryErrorStatus::BadRequest
    );
    assert_eq!(cursor_response.code, "pagination_cursor_invalid");
    assert_eq!(cursor_response.message, "pagination cursor is invalid");

    let limit_error = page_request_from_query("limit=abc123").unwrap_err();
    assert_eq!(limit_error.to_string(), "pagination metadata is invalid");
    assert!(!limit_error.to_string().contains("limit"));
    let limit_response = plan_gateway_http_pagination_query_error_response(&limit_error);
    assert_eq!(limit_response.code, "pagination_limit_invalid");
    assert_eq!(limit_response.message, "pagination limit is invalid");
    assert!(!limit_response.message.contains("abc123"));

    let duplicate_limit = page_request_from_query("limit=25&limit=50").unwrap_err();
    assert_eq!(
        duplicate_limit.to_string(),
        "pagination metadata is duplicated"
    );
    assert!(!duplicate_limit.to_string().contains("limit"));
    let limit_response = plan_gateway_http_pagination_query_error_response(&duplicate_limit);
    assert_eq!(limit_response.code, "pagination_limit_invalid");
    assert_eq!(limit_response.message, "pagination limit is invalid");
    assert!(!limit_response.message.contains("50"));

    let encoded_duplicate_limit = page_request_from_query("limit=25&%6cimit=50").unwrap_err();
    assert_eq!(
        encoded_duplicate_limit,
        prodex_gateway_http::GatewayHttpPaginationQueryError::DuplicateLimit
    );

    let duplicate_cursor =
        page_request_from_query("cursor=opaque-next&cursor=opaque-other").unwrap_err();
    assert_eq!(
        duplicate_cursor.to_string(),
        "pagination metadata is duplicated"
    );
    assert!(!duplicate_cursor.to_string().contains("cursor"));
    let cursor_response = plan_gateway_http_pagination_query_error_response(&duplicate_cursor);
    assert_eq!(cursor_response.code, "pagination_cursor_invalid");
    assert_eq!(cursor_response.message, "pagination cursor is invalid");
    assert!(!cursor_response.message.contains("opaque-other"));

    let encoded_duplicate_cursor =
        page_request_from_query("cursor=opaque-next&%63ursor=opaque-other").unwrap_err();
    assert_eq!(
        encoded_duplicate_cursor,
        prodex_gateway_http::GatewayHttpPaginationQueryError::DuplicateCursor
    );
}

#[test]
fn control_plane_request_fingerprint_uses_method_path_and_body_digest() {
    let http = GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Patch,
        path: "/admin/policies/revision-1".to_string(),
        body_len: 128,
        headers: vec![traceparent()],
    };

    let fingerprint = control_plane_request_fingerprint(&http, "sha256:body-digest").unwrap();

    assert_eq!(
        fingerprint,
        "http:patch:path:/admin/policies/revision-1:body:sha256:body-digest"
    );
}

#[test]
fn control_plane_request_fingerprint_rejects_bad_input_redacted() {
    let mut http = request("/admin/keys");
    http.path = "  ".to_string();
    let path_error = control_plane_request_fingerprint(&http, "sha256:body").unwrap_err();
    assert_eq!(path_error.to_string(), "request fingerprint is invalid");
    assert!(!path_error.to_string().contains("path"));
    let path_response = plan_gateway_http_request_fingerprint_error_response(&path_error);
    assert_eq!(path_response.code, "request_fingerprint_invalid");
    assert_eq!(path_response.message, "request fingerprint is invalid");

    let http = request("/admin/keys");
    let digest_error = control_plane_request_fingerprint(&http, "sha256:body digest").unwrap_err();
    assert_eq!(digest_error.to_string(), "request fingerprint is invalid");
    assert!(!digest_error.to_string().contains("digest"));
    let digest_response = plan_gateway_http_request_fingerprint_error_response(&digest_error);
    assert_eq!(digest_response, path_response);
    assert!(!digest_response.message.contains("sha256"));
}

#[path = "http_policy/routing.rs"]
mod routing;
