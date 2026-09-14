use super::*;

#[test]
fn application_route_plans_cover_the_complete_data_plane() {
    use ApplicationProviderEndpoint as Endpoint;
    use ApplicationRouteKind as Route;
    let cases = [
        (Route::Responses, Some(Endpoint::Responses), 1 << 0, 1 << 0),
        (
            Route::Compact,
            Some(Endpoint::ResponsesCompact),
            (1 << 0) | (1 << 5),
            1 << 0,
        ),
        (
            Route::WebSocket,
            Some(Endpoint::Responses),
            (1 << 0) | (1 << 6),
            1 << 0,
        ),
        (Route::Quota, None, 0, 1 << 0),
        (
            Route::ChatCompletions,
            Some(Endpoint::ChatCompletions),
            0,
            1 << 0,
        ),
        (Route::Embeddings, Some(Endpoint::Embeddings), 0, 1 << 0),
        (
            Route::ImagesGenerations,
            Some(Endpoint::Images),
            1 << 3,
            (1 << 0) | (1 << 1),
        ),
        (
            Route::ImagesEdits,
            Some(Endpoint::Images),
            1 << 3,
            (1 << 0) | (1 << 1) | (1 << 4),
        ),
        (
            Route::AudioSpeech,
            Some(Endpoint::Audio),
            0,
            (1 << 0) | (1 << 2),
        ),
        (
            Route::AudioTranscriptions,
            Some(Endpoint::Audio),
            0,
            (1 << 0) | (1 << 2) | (1 << 4),
        ),
        (Route::Batches, Some(Endpoint::Batches), 0, 1 << 0),
        (Route::Rerank, Some(Endpoint::Rerank), 0, 1 << 0),
        (Route::A2a, Some(Endpoint::A2a), 0, 1 << 0),
        (Route::Messages, Some(Endpoint::Messages), 0, 1 << 0),
        (Route::Models, Some(Endpoint::Models), 0, 1 << 0),
        (Route::ControlPlane, None, 0, 1 << 0),
        (Route::Unknown, None, 0, 1 << 0),
    ];
    for (route, endpoint, capabilities, modalities) in cases {
        let plan = plan_application_route_request(route, false, false, false).unwrap();
        assert_eq!(plan.endpoint, endpoint, "{route:?}");
        assert_eq!(plan.capability_mask, capabilities, "{route:?}");
        assert_eq!(plan.modality_mask, modalities, "{route:?}");
    }
}

#[test]
fn application_route_plan_combines_body_signals() {
    let plan =
        plan_application_route_request(ApplicationRouteKind::Responses, true, true, false).unwrap();
    assert_eq!(
        plan.capability_mask,
        APPLICATION_CAPABILITY_RESPONSES_API
            | APPLICATION_CAPABILITY_STREAMING
            | APPLICATION_CAPABILITY_TOOLS
    );
    assert!(plan.streaming);
    assert_eq!(plan.endpoint, Some(ApplicationProviderEndpoint::Responses));

    let vision =
        plan_application_route_request(ApplicationRouteKind::Responses, false, false, true)
            .unwrap();
    assert_eq!(
        vision.modality_mask,
        APPLICATION_MODALITY_TEXT | APPLICATION_MODALITY_IMAGE
    );
}

#[test]
fn application_provider_load_quota_and_output_plans_are_bounded() {
    let capability_mask =
        plan_application_provider_capabilities(ApplicationProviderCapabilitiesInput {
            provider: ApplicationProviderKind::Gemini,
            responses: ApplicationProviderCapabilityStatus::Native,
            compact: ApplicationProviderCapabilityStatus::Unsupported,
            images: ApplicationProviderCapabilityStatus::Untested,
            supports_streaming: true,
            catalog_vision: true,
            catalog_tools: true,
            catalog_json_mode: false,
        })
        .unwrap();
    assert_eq!(
        capability_mask,
        APPLICATION_CAPABILITY_RESPONSES_API
            | APPLICATION_CAPABILITY_STREAMING
            | APPLICATION_CAPABILITY_TOOLS
            | APPLICATION_CAPABILITY_VISION
            | APPLICATION_CAPABILITY_WEBSOCKET
    );
    assert_eq!(application_normalized_load(3, 4, 10_000), Ok(7_500));
    assert_eq!(application_normalized_load(1, 0, 10_000), Ok(10_000));
    assert_eq!(
        application_quota_headroom(ApplicationQuotaWindowStatus::Thin, 42, 0, 1, 10_000),
        Ok(Some(4_200))
    );
    assert_eq!(
        application_quota_headroom(ApplicationQuotaWindowStatus::Exhausted, 0, 2, 1, 10_000),
        Ok(Some(0))
    );
    assert_eq!(
        application_quota_headroom(ApplicationQuotaWindowStatus::Unknown, 0, 0, 1, 10_000),
        Ok(None)
    );
    assert_eq!(
        select_application_output_tokens([None, Some(32), Some(64)]),
        Ok(Some(32))
    );
    assert_eq!(
        select_application_output_tokens([Some(u64::MAX), None, Some(64)]),
        Ok(Some(64))
    );
}

#[test]
fn application_dispatch_plans_preserve_precommit_and_affinity_boundaries() {
    assert_eq!(
        plan_application_candidate(false, 2, 0, true),
        Ok(ApplicationCandidatePlan {
            candidate_count: 3,
            attempt: ApplicationCandidateAttempt::Primary,
        })
    );
    assert_eq!(
        plan_application_candidate(true, 8, 1, true)
            .unwrap()
            .attempt,
        ApplicationCandidateAttempt::Stop
    );
    assert_eq!(
        plan_application_attempt_result(true, true, true),
        Ok(ApplicationAttemptResult::Retry)
    );
    assert_eq!(
        plan_application_attempt_result(false, false, false),
        Ok(ApplicationAttemptResult::Stop)
    );
    assert_eq!(
        plan_application_binding(ApplicationBindingInput {
            continuation_bound: true,
            bound_identity_present: true,
            provider_matches: true,
            selected_identity_present: true,
            identity_matches: false,
            selected_identity_optional: false,
        }),
        Ok(ApplicationBindingDecision::IdentityMismatch)
    );
    assert_eq!(
        plan_application_governance_dispatch(ApplicationGovernanceDispatchInput {
            tenant_bound: true,
            anonymous_compatibility_allowed: false,
            enforcing: true,
            mandatory_audit: true,
            policy_allows: true,
            routing_present: false,
        }),
        Ok(ApplicationGovernanceDispatchDecision::Unavailable)
    );
}

#[test]
fn application_pipeline_error_plan_preserves_public_http_statuses() {
    assert_eq!(
        plan_application_pipeline_error(ApplicationPipelineErrorInput::UnknownRoute),
        Ok(ApplicationPipelineErrorPlan {
            status: 404,
            gateway_response: false,
        })
    );
    for (input, status) in [
        (ApplicationGatewayErrorStatus::BadRequest, 400),
        (ApplicationGatewayErrorStatus::MethodNotAllowed, 405),
        (ApplicationGatewayErrorStatus::PayloadTooLarge, 413),
        (
            ApplicationGatewayErrorStatus::RequestHeaderFieldsTooLarge,
            431,
        ),
        (ApplicationGatewayErrorStatus::InternalServerError, 500),
    ] {
        assert_eq!(
            plan_application_pipeline_error(ApplicationPipelineErrorInput::Gateway(input)).unwrap(),
            ApplicationPipelineErrorPlan {
                status,
                gateway_response: true,
            }
        );
    }
}
