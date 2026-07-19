use super::*;

#[test]
fn route_classifier_separates_data_control_and_health_surfaces() {
    assert_eq!(
        classify_route("/responses"),
        GatewayHttpRouteKind::DataPlaneResponses
    );
    assert_eq!(
        classify_route("/responses?limit=1"),
        GatewayHttpRouteKind::DataPlaneResponses
    );
    assert_eq!(
        classify_route("/v1/responses/compact"),
        GatewayHttpRouteKind::DataPlaneCompact
    );
    assert_eq!(
        classify_route("/quota"),
        GatewayHttpRouteKind::DataPlaneQuota
    );
    assert_eq!(
        classify_route("/v1/quota"),
        GatewayHttpRouteKind::DataPlaneQuota
    );
    assert_eq!(
        classify_route("/admin/keys"),
        GatewayHttpRouteKind::ControlPlane
    );
    assert_eq!(
        classify_route("/admin/keys?limit=25"),
        GatewayHttpRouteKind::ControlPlane
    );
    assert_eq!(
        classify_route("/admin/keys?limit=25#page"),
        GatewayHttpRouteKind::Unknown
    );
    assert_eq!(
        classify_route("/v1/admin/keys"),
        GatewayHttpRouteKind::ControlPlane
    );
    assert_eq!(
        classify_route("/scim/v2/Users"),
        GatewayHttpRouteKind::ControlPlane
    );
    assert_eq!(
        classify_route("/v1/scim/v2/Users"),
        GatewayHttpRouteKind::ControlPlane
    );
    assert_eq!(
        classify_route("/prodex/gateway/keys"),
        GatewayHttpRouteKind::ControlPlane
    );
    assert_eq!(
        classify_route("/prodex/gateway/scim/v2/Users"),
        GatewayHttpRouteKind::ControlPlane
    );
    assert_eq!(
        classify_route("/v1/prodex/gateway/metrics"),
        GatewayHttpRouteKind::ControlPlane
    );
    assert_eq!(
        classify_route("/v1/prodex/gateway/auth/backchannel-logout"),
        GatewayHttpRouteKind::ControlPlane
    );
    assert_eq!(classify_route("/admin"), GatewayHttpRouteKind::ControlPlane);
    assert_eq!(
        classify_route("/v1/admin"),
        GatewayHttpRouteKind::ControlPlane
    );
    assert_eq!(
        classify_route("/administrator/keys"),
        GatewayHttpRouteKind::Unknown
    );
    assert_eq!(
        classify_route("/v1/administrator/keys"),
        GatewayHttpRouteKind::Unknown
    );
    assert_eq!(
        classify_route("/scimitar/v2/Users"),
        GatewayHttpRouteKind::Unknown
    );
    assert_eq!(
        classify_route("/v1/scimitar/v2/Users"),
        GatewayHttpRouteKind::Unknown
    );
    assert_eq!(classify_route("/livez"), GatewayHttpRouteKind::HealthLive);
}

#[test]
fn route_classifier_covers_every_documented_data_plane_family_and_denies_unknown_control_paths() {
    for (path, expected) in [
        (
            "/v1/chat/completions",
            GatewayHttpRouteKind::DataPlaneChatCompletions,
        ),
        ("/v1/embeddings", GatewayHttpRouteKind::DataPlaneEmbeddings),
        (
            "/v1/images/generations",
            GatewayHttpRouteKind::DataPlaneImagesGenerations,
        ),
        (
            "/v1/images/edits",
            GatewayHttpRouteKind::DataPlaneImagesEdits,
        ),
        (
            "/v1/images/variations",
            GatewayHttpRouteKind::DataPlaneImagesVariations,
        ),
        (
            "/v1/audio/speech",
            GatewayHttpRouteKind::DataPlaneAudioSpeech,
        ),
        (
            "/v1/audio/transcriptions",
            GatewayHttpRouteKind::DataPlaneAudioTranscriptions,
        ),
        (
            "/v1/audio/translations",
            GatewayHttpRouteKind::DataPlaneAudioTranslations,
        ),
        ("/v1/batches", GatewayHttpRouteKind::DataPlaneBatches),
        (
            "/v1/batches/batch_123",
            GatewayHttpRouteKind::DataPlaneBatch,
        ),
        ("/v1/rerank", GatewayHttpRouteKind::DataPlaneRerank),
        ("/v1/a2a", GatewayHttpRouteKind::DataPlaneA2a),
        ("/v1/messages", GatewayHttpRouteKind::DataPlaneMessages),
        ("/v1/models", GatewayHttpRouteKind::DataPlaneModels),
        ("/v1/models/model-1", GatewayHttpRouteKind::DataPlaneModel),
    ] {
        assert_eq!(classify_route(path), expected, "{path}");
    }

    assert_eq!(
        classify_route("/v1/prodex/gateway/not-supported"),
        GatewayHttpRouteKind::Unknown
    );
}

#[test]
fn published_route_aliases_cannot_cross_planes() {
    for (canonical, alias) in [
        ("/v1/responses", "/responses"),
        ("/v1/responses/compact", "/responses/compact"),
        ("/v1/realtime", "/realtime"),
        ("/v1/quota", "/quota"),
        ("/v1/chat/completions", "/chat/completions"),
        ("/v1/embeddings", "/embeddings"),
        ("/v1/images/generations", "/images/generations"),
        ("/v1/images/edits", "/images/edits"),
        ("/v1/images/variations", "/images/variations"),
        ("/v1/audio/speech", "/audio/speech"),
        ("/v1/audio/transcriptions", "/audio/transcriptions"),
        ("/v1/audio/translations", "/audio/translations"),
        ("/v1/batches", "/batches"),
        ("/v1/batches/batch_123", "/batches/batch_123"),
        ("/v1/rerank", "/rerank"),
        ("/v1/a2a", "/a2a"),
        ("/v1/messages", "/messages"),
        ("/v1/models", "/models"),
        ("/v1/models/model-1", "/models/model-1"),
        ("/v1/admin/keys", "/admin/keys"),
        ("/v1/scim/v2/Users", "/scim/v2/Users"),
        ("/v1/prodex/gateway/keys", "/prodex/gateway/keys"),
    ] {
        let canonical = CanonicalRequestTarget::parse(canonical).unwrap();
        let alias = CanonicalRequestTarget::parse(alias).unwrap();
        let canonical_route = classify_request_target(&canonical).unwrap();
        let alias_route = classify_request_target(&alias).unwrap();
        assert_eq!(alias_route.kind, canonical_route.kind, "{alias:?}");
        assert_eq!(alias_route.plane, canonical_route.plane, "{alias:?}");
    }
}

#[test]
fn canonical_request_target_rejects_ambiguous_forms_and_preserves_exact_query() {
    for raw in [
        "",
        "http://example.com/v1/responses",
        "//example.com/v1/responses",
        " /v1/responses",
        "/v1/responses ",
        "/v1/responses#fragment",
        "/v1\\responses",
        "/v1//responses",
        "/v1/./responses",
        "/v1/../admin",
        "/v1/%2e%2e/admin",
        "/v1/%2Fadmin",
        "/v1/%5cadmin",
        "/v1/%25admin",
        "/v1/%c3%a9",
        "/v1/%",
        "/v1/%zz",
        "/v1/café",
    ] {
        let error = CanonicalRequestTarget::parse(raw).unwrap_err();
        assert_eq!(error.to_string(), "request target is invalid", "{raw:?}");
    }

    let target = CanonicalRequestTarget::parse("/v1/responses?cursor=a%2Fb&limit=1").unwrap();
    assert_eq!(target.path(), "/v1/responses");
    assert_eq!(target.query(), Some("cursor=a%2Fb&limit=1"));
    assert_eq!(
        target.path_and_query(),
        "/v1/responses?cursor=a%2Fb&limit=1"
    );
    assert!(!format!("{target:?}").contains("cursor"));

    let route = classify_request_target(&target).unwrap();
    assert_eq!(route.kind, GatewayHttpRouteKind::DataPlaneResponses);
    assert_eq!(route.plane, GatewayHttpRoutePlane::DataPlane);
}

#[test]
fn canonical_request_target_path_rewrite_preserves_the_validated_query() {
    let target = CanonicalRequestTarget::parse("/admin/keys?limit=2").unwrap();
    assert_eq!(
        target
            .with_path("/v1/prodex/gateway/keys")
            .unwrap()
            .path_and_query(),
        "/v1/prodex/gateway/keys?limit=2"
    );
    assert_eq!(
        target.with_path("/keys?other=1").unwrap_err(),
        CanonicalRequestTargetError::ReplacementPathHasQuery
    );
}

#[test]
fn canonical_request_target_errors_remain_typed_and_redacted() {
    assert_eq!(
        CanonicalRequestTarget::parse("/v1/%").unwrap_err(),
        CanonicalRequestTargetError::MalformedPercentEncoding
    );
    assert_eq!(
        CanonicalRequestTarget::parse("/v1/%2fadmin").unwrap_err(),
        CanonicalRequestTargetError::EncodedDelimiter
    );
    assert_eq!(
        CanonicalRequestTarget::parse(format!("/{}", "a".repeat(8 * 1024))).unwrap_err(),
        CanonicalRequestTargetError::TooLong
    );

    let error = plan_gateway_http_request(
        GatewayHttpPolicy::production_default(),
        request("/v1//responses?access_token=secret"),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        GatewayHttpPlanError::InvalidRequestTarget(_)
    ));
    assert!(!format!("{error:?}").contains("access_token"));
    let response = plan_gateway_http_error_response(&error);
    assert_eq!(response.status, GatewayHttpErrorStatus::BadRequest);
    assert_eq!(response.code, "invalid_request_target");
}

#[test]
fn gateway_request_and_header_debug_never_render_secret_values() {
    let secret = "debug-sentinel-capability";
    let header = GatewayHttpHeader::new("Authorization", format!("Bearer {secret}"));
    let request = GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Post,
        path: format!("/v1/responses?access_token={secret}"),
        body_len: 0,
        headers: vec![header.clone()],
    };

    let header_debug = format!("{header:?}");
    let request_debug = format!("{request:?}");
    assert!(header_debug.contains("<redacted>"));
    assert!(!header_debug.contains(secret));
    assert!(request_debug.contains("<redacted>"));
    assert!(!request_debug.contains(secret));
}

#[test]
fn canonical_request_target_property_corpus_is_stable_and_classifier_consistent() {
    let mut state = 0x8f3d_71c2_59ab_04e1_u64;
    for case in 0..10_000 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let len = (state as usize % 96) + 1;
        let mut bytes = Vec::with_capacity(len);
        for index in 0..len {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let byte = if index == 0 && case % 2 == 0 {
                b'/'
            } else {
                (state >> 56) as u8 & 0x7f
            };
            bytes.push(byte);
        }
        let raw = String::from_utf8(bytes).unwrap();
        if let Ok(target) = CanonicalRequestTarget::parse(raw.as_str()) {
            assert_eq!(target.path_and_query(), raw);
            assert_eq!(
                classify_route(&raw),
                classify_request_target(&target)
                    .map_or(GatewayHttpRouteKind::Unknown, |route| route.kind)
            );
            assert_eq!(
                CanonicalRequestTarget::parse(target.path_and_query()).unwrap(),
                target
            );
        }
    }
}

#[test]
fn control_plane_route_planner_maps_admin_paths_to_explicit_operations() {
    let cases = [
        (
            GatewayHttpMethod::Get,
            "/prodex/gateway/admin",
            GatewayControlPlaneOperation::GatewayAdminRead,
            false,
        ),
        (
            GatewayHttpMethod::Get,
            "/prodex/gateway/openapi.json",
            GatewayControlPlaneOperation::GatewayAdminRead,
            false,
        ),
        (
            GatewayHttpMethod::Get,
            "/prodex/gateway/metrics",
            GatewayControlPlaneOperation::GatewayAdminRead,
            false,
        ),
        (
            GatewayHttpMethod::Post,
            "/prodex/gateway/routes/explain",
            GatewayControlPlaneOperation::RouteExplain,
            false,
        ),
        (
            GatewayHttpMethod::Post,
            "/admin/tenants",
            GatewayControlPlaneOperation::TenantCreate,
            true,
        ),
        (
            GatewayHttpMethod::Patch,
            "/v1/admin/tenants/tenant-1",
            GatewayControlPlaneOperation::TenantUpdate,
            true,
        ),
        (
            GatewayHttpMethod::Post,
            "/admin/users/invites",
            GatewayControlPlaneOperation::UserInvite,
            true,
        ),
        (
            GatewayHttpMethod::Post,
            "/admin/role-bindings",
            GatewayControlPlaneOperation::RoleBindingGrant,
            true,
        ),
        (
            GatewayHttpMethod::Delete,
            "/admin/role-bindings/binding-1",
            GatewayControlPlaneOperation::RoleBindingRevoke,
            true,
        ),
        (
            GatewayHttpMethod::Post,
            "/admin/service-identities",
            GatewayControlPlaneOperation::ServiceIdentityCreate,
            true,
        ),
        (
            GatewayHttpMethod::Get,
            "/prodex/gateway/keys",
            GatewayControlPlaneOperation::VirtualKeyRead,
            false,
        ),
        (
            GatewayHttpMethod::Post,
            "/prodex/gateway/keys",
            GatewayControlPlaneOperation::VirtualKeyCreate,
            true,
        ),
        (
            GatewayHttpMethod::Patch,
            "/prodex/gateway/keys/key-1",
            GatewayControlPlaneOperation::VirtualKeyUpdate,
            true,
        ),
        (
            GatewayHttpMethod::Delete,
            "/prodex/gateway/keys/key-1",
            GatewayControlPlaneOperation::VirtualKeyDelete,
            true,
        ),
        (
            GatewayHttpMethod::Post,
            "/prodex/gateway/keys/key-1/secret",
            GatewayControlPlaneOperation::VirtualKeyRotateSecret,
            true,
        ),
        (
            GatewayHttpMethod::Post,
            "/admin/provider-credentials/provider-1/secret",
            GatewayControlPlaneOperation::ProviderCredentialRotate,
            true,
        ),
        (
            GatewayHttpMethod::Patch,
            "/admin/budgets/budget-1",
            GatewayControlPlaneOperation::BudgetUpdate,
            true,
        ),
        (
            GatewayHttpMethod::Post,
            "/admin/policies/revisions",
            GatewayControlPlaneOperation::PolicyPublish,
            true,
        ),
        (
            GatewayHttpMethod::Get,
            "/admin/policies/status",
            GatewayControlPlaneOperation::PolicyPublish,
            false,
        ),
        (
            GatewayHttpMethod::Get,
            "/prodex/gateway/execution-approvals",
            GatewayControlPlaneOperation::PolicyPublish,
            false,
        ),
        (
            GatewayHttpMethod::Post,
            "/prodex/gateway/execution-approvals/approval-1/votes",
            GatewayControlPlaneOperation::PolicyPublish,
            true,
        ),
        (
            GatewayHttpMethod::Get,
            "/prodex/gateway/governance/outbox",
            GatewayControlPlaneOperation::PolicyPublish,
            false,
        ),
        (
            GatewayHttpMethod::Get,
            "/prodex/gateway/governance/audit/integrity",
            GatewayControlPlaneOperation::PolicyPublish,
            false,
        ),
        (
            GatewayHttpMethod::Post,
            "/prodex/gateway/governance/outbox/claim",
            GatewayControlPlaneOperation::PolicyPublish,
            true,
        ),
        (
            GatewayHttpMethod::Post,
            "/prodex/gateway/sessions/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/revoke",
            GatewayControlPlaneOperation::PolicyPublish,
            true,
        ),
        (
            GatewayHttpMethod::Post,
            "/prodex/gateway/sessions/current/revoke",
            GatewayControlPlaneOperation::PolicyPublish,
            true,
        ),
        (
            GatewayHttpMethod::Post,
            "/admin/configuration/revisions",
            GatewayControlPlaneOperation::ConfigurationPublish,
            true,
        ),
        (
            GatewayHttpMethod::Get,
            "/prodex/gateway/ledger",
            GatewayControlPlaneOperation::BillingRead,
            false,
        ),
        (
            GatewayHttpMethod::Post,
            "/admin/audit/exports",
            GatewayControlPlaneOperation::AuditExport,
            false,
        ),
        (
            GatewayHttpMethod::Delete,
            "/admin/audit/retention",
            GatewayControlPlaneOperation::AuditRetentionPurge,
            true,
        ),
    ];

    for (method, path, operation, requires_idempotency) in cases {
        let plan = plan_control_plane_route(&GatewayHttpRequestMeta {
            method,
            path: path.to_string(),
            body_len: 128,
            headers: vec![traceparent()],
        })
        .unwrap();

        assert_eq!(plan.operation, operation);
        assert_eq!(plan.requires_idempotency, requires_idempotency);
        assert!(plan.requires_audit);
    }

    let query = plan_control_plane_route(&GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Get,
        path: "/prodex/gateway/keys?limit=25".to_string(),
        body_len: 0,
        headers: vec![traceparent()],
    })
    .unwrap();
    assert_eq!(
        query.operation,
        GatewayControlPlaneOperation::VirtualKeyRead
    );
    assert!(!query.requires_idempotency);
}

#[test]
fn control_plane_route_planner_maps_scim_paths_and_methods() {
    let read = plan_control_plane_route(&GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Get,
        path: "/prodex/gateway/scim/v2/Users/user-1".to_string(),
        body_len: 0,
        headers: vec![traceparent()],
    })
    .unwrap();
    assert_eq!(read.operation, GatewayControlPlaneOperation::ScimUserRead);
    assert!(!read.requires_idempotency);
    assert!(read.requires_audit);

    let create = plan_control_plane_route(&GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Post,
        path: "/prodex/gateway/scim/v2/Users".to_string(),
        body_len: 128,
        headers: vec![traceparent()],
    })
    .unwrap();
    assert_eq!(
        create.operation,
        GatewayControlPlaneOperation::ScimUserCreate
    );
    assert!(create.requires_idempotency);
    assert!(create.requires_audit);

    let update = plan_control_plane_route(&GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Patch,
        path: "/v1/scim/v2/Users/user-1".to_string(),
        body_len: 128,
        headers: vec![traceparent()],
    })
    .unwrap();
    assert_eq!(
        update.operation,
        GatewayControlPlaneOperation::ScimUserUpdate
    );
    assert!(update.requires_idempotency);
    assert!(update.requires_audit);

    let replace = plan_control_plane_route(&GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Put,
        path: "/v1/scim/v2/Users/user-1".to_string(),
        body_len: 128,
        headers: vec![traceparent()],
    })
    .unwrap();
    assert_eq!(
        replace.operation,
        GatewayControlPlaneOperation::ScimUserUpdate
    );
    assert!(replace.requires_idempotency);
    assert!(replace.requires_audit);

    let delete = plan_control_plane_route(&GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Delete,
        path: "/scim/v2/Users/user-1".to_string(),
        body_len: 0,
        headers: vec![traceparent()],
    })
    .unwrap();
    assert_eq!(
        delete.operation,
        GatewayControlPlaneOperation::ScimUserDelete
    );
    assert!(delete.requires_idempotency);
    assert!(delete.requires_audit);
}

#[test]
fn control_plane_route_planner_rejects_non_admin_or_wrong_method_paths() {
    assert_eq!(
        plan_control_plane_route(&GatewayHttpRequestMeta {
            method: GatewayHttpMethod::Post,
            path: "/v1/responses".to_string(),
            body_len: 128,
            headers: vec![traceparent()],
        }),
        Err(GatewayControlPlaneRouteError::NotControlPlaneRoute)
    );
    assert_eq!(
        plan_control_plane_route(&GatewayHttpRequestMeta {
            method: GatewayHttpMethod::Post,
            path: "/v1/scim/v2/Users/user-1".to_string(),
            body_len: 128,
            headers: vec![traceparent()],
        }),
        Err(GatewayControlPlaneRouteError::MethodNotAllowed {
            operation: GatewayControlPlaneOperation::ScimUserUpdate,
            method: GatewayHttpMethod::Post,
        })
    );
    assert_eq!(
        plan_control_plane_route(&GatewayHttpRequestMeta {
            method: GatewayHttpMethod::Post,
            path: "/admin/unknown".to_string(),
            body_len: 128,
            headers: vec![traceparent()],
        }),
        Err(GatewayControlPlaneRouteError::UnknownControlPlaneRoute)
    );
    assert_eq!(
        plan_control_plane_route(&GatewayHttpRequestMeta {
            method: GatewayHttpMethod::Post,
            path: "/admin/ledger".to_string(),
            body_len: 128,
            headers: vec![traceparent()],
        }),
        Err(GatewayControlPlaneRouteError::MethodNotAllowed {
            operation: GatewayControlPlaneOperation::BillingRead,
            method: GatewayHttpMethod::Post,
        })
    );
}

#[test]
fn control_plane_route_error_responses_are_stable_and_redacted() {
    let non_admin = plan_control_plane_route(&GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Post,
        path: "/v1/responses".to_string(),
        body_len: 128,
        headers: vec![traceparent()],
    })
    .unwrap_err();
    assert_eq!(non_admin.to_string(), "HTTP route is invalid");
    assert!(!non_admin.to_string().contains("control-plane"));
    let non_admin_response = plan_gateway_control_plane_route_error_response(&non_admin);
    assert_eq!(
        non_admin_response.status,
        GatewayControlPlaneRouteErrorStatus::BadRequest
    );
    assert_eq!(non_admin_response.code, "control_plane_route_invalid");
    assert_eq!(non_admin_response.message, "control-plane route is invalid");
    assert!(!non_admin_response.message.contains("/v1/responses"));

    let unknown = plan_control_plane_route(&GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Post,
        path: "/admin/secret-provider-topology".to_string(),
        body_len: 128,
        headers: vec![traceparent()],
    })
    .unwrap_err();
    assert_eq!(unknown.to_string(), "HTTP route is invalid");
    assert!(!unknown.to_string().contains("control-plane"));
    let unknown_response = plan_gateway_control_plane_route_error_response(&unknown);
    assert_eq!(unknown_response, non_admin_response);
    assert!(!unknown_response.message.contains("secret-provider"));

    let wrong_method = GatewayControlPlaneRouteError::MethodNotAllowed {
        operation: GatewayControlPlaneOperation::VirtualKeyCreate,
        method: GatewayHttpMethod::Get,
    };
    assert!(!wrong_method.to_string().contains("VirtualKeyCreate"));
    assert!(!wrong_method.to_string().contains("Get"));
    assert!(!wrong_method.to_string().contains("control-plane"));
    assert_eq!(wrong_method.to_string(), "HTTP method is not allowed");
    let wrong_method_response = plan_gateway_control_plane_route_error_response(&wrong_method);
    assert_eq!(
        wrong_method_response.status,
        GatewayControlPlaneRouteErrorStatus::MethodNotAllowed
    );
    assert_eq!(
        wrong_method_response.code,
        "control_plane_method_not_allowed"
    );
    assert_eq!(
        wrong_method_response.message,
        "HTTP method is not allowed for this control-plane route"
    );
    assert!(!wrong_method_response.message.contains("VirtualKeyCreate"));
    assert!(!wrong_method_response.message.contains("Get"));
}

#[test]
fn policy_rejects_unbounded_zero_values() {
    let zero_body = GatewayHttpPolicy {
        max_body_bytes: 0,
        ..GatewayHttpPolicy::production_default()
    }
    .validate()
    .unwrap_err();

    assert_eq!(zero_body.to_string(), "gateway HTTP policy is invalid");
    assert!(!zero_body.to_string().contains("body"));
    assert!(
        GatewayHttpPolicy {
            max_concurrent_streams: 0,
            ..GatewayHttpPolicy::production_default()
        }
        .validate()
        .is_err()
    );
}

#[test]
fn policy_rejects_stream_idle_timeout_longer_than_request_timeout() {
    let policy = GatewayHttpPolicy {
        request_timeout_ms: 1_000,
        stream_idle_timeout_ms: 1_001,
        ..GatewayHttpPolicy::production_default()
    };

    assert_eq!(
        policy.validate(),
        Err(prodex_gateway_http::GatewayHttpPolicyError::StreamTimeoutExceedsRequestTimeout)
    );
    let response = plan_gateway_http_error_response(&GatewayHttpPlanError::Policy(
        policy.validate().unwrap_err(),
    ));
    assert_eq!(response.status, GatewayHttpErrorStatus::InternalServerError);
    assert_eq!(response.code, "gateway_http_policy_invalid");
    assert_eq!(response.message, "gateway HTTP policy is invalid");
    assert!(!response.message.contains("1001"));
    assert!(!response.message.contains("StreamTimeout"));
}

#[test]
fn local_http_error_responses_are_stable_and_redacted() {
    let body_error = GatewayHttpPlanError::BodyTooLarge {
        max: 256,
        actual: 1_000_000,
    };
    assert!(!body_error.to_string().contains("256"));
    assert!(!body_error.to_string().contains("1000000"));
    let body_response = plan_gateway_http_error_response(&body_error);
    assert_eq!(
        body_response.status,
        GatewayHttpErrorStatus::PayloadTooLarge
    );
    assert_eq!(body_response.code, "request_body_too_large");
    assert_eq!(body_response.message, "request body is too large");
    assert!(!body_response.message.contains("1000000"));

    let method_error = GatewayHttpPlanError::MethodNotAllowed {
        route: GatewayHttpRouteKind::DataPlaneQuota,
        method: GatewayHttpMethod::Post,
    };
    assert!(!method_error.to_string().contains("DataPlaneQuota"));
    assert!(!method_error.to_string().contains("Post"));
    assert!(!method_error.to_string().contains("route"));
    assert_eq!(method_error.to_string(), "HTTP method is not allowed");
    let method_response = plan_gateway_http_error_response(&method_error);
    assert_eq!(
        method_response.status,
        GatewayHttpErrorStatus::MethodNotAllowed
    );
    assert_eq!(method_response.code, "method_not_allowed");
    assert!(!method_response.message.contains("DataPlaneResponses"));

    let trace_response =
        plan_gateway_http_error_response(&GatewayHttpPlanError::MissingTraceContext);
    assert_eq!(trace_response.status, GatewayHttpErrorStatus::BadRequest);
    assert_eq!(trace_response.code, "invalid_trace_context");

    let rendered_headers = format!(
        "{} {} {} {} {}",
        GatewayHttpPlanError::DuplicateAuthorization,
        GatewayHttpPlanError::DuplicateChatGptAccountId,
        GatewayHttpPlanError::DuplicateSessionId,
        GatewayHttpPlanError::DuplicateCodexTurnState,
        GatewayHttpPlanError::DuplicateCodexMetadata,
    );
    for sensitive in [
        "authorization",
        "ChatGPT",
        "session_id",
        "x-codex-turn-state",
        "Codex metadata",
    ] {
        assert!(
            !rendered_headers.contains(sensitive),
            "gateway HTTP display leaked header metadata {sensitive}: {rendered_headers}"
        );
    }
    assert_eq!(
        GatewayHttpPlanError::MissingTraceContext.to_string(),
        "required request metadata is missing"
    );
    assert!(rendered_headers.contains("request metadata is duplicated"));

    let policy_response = plan_gateway_http_error_response(&GatewayHttpPlanError::Policy(
        prodex_gateway_http::GatewayHttpPolicyError::ZeroBodyLimit,
    ));
    assert_eq!(
        policy_response.status,
        GatewayHttpErrorStatus::InternalServerError
    );
    assert_eq!(policy_response.code, "gateway_http_policy_invalid");
    assert!(!policy_response.message.contains("ZeroBodyLimit"));
}
