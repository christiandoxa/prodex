use super::super::local_rewrite::RuntimeLocalRewriteProviderOptions;
use super::super::local_rewrite_gateway_config::RuntimeGatewayStateStore;
use super::super::provider_bridge::RuntimeProviderGatewaySpendEvent;
use super::{
    MAX_RUNTIME_GATEWAY_REQUESTED_TOOLS, RuntimeGatewayApplicationReconciliationInput,
    runtime_gateway_application_provider_stage_is_committed,
    runtime_gateway_application_reconciliation_execution,
    runtime_gateway_application_usage_reconciliation, runtime_gateway_provider_endpoint,
    runtime_gateway_provider_executable_capabilities, runtime_gateway_requested_modalities,
    runtime_gateway_requested_tools,
};
use prodex_domain::{
    CallId, CapabilitySet, DataClassification, DataModality, ModelCapability, PolicyEffect,
    RequestId, ReservationId, ReservationReconciliationReason, ReservationRecord,
    ReservationRequest, TenantContext, TenantId, UsageAmount,
};
use prodex_gateway_http::GatewayHttpRouteKind;
use prodex_provider_core::{ProviderEndpoint, ProviderId};
use prodex_provider_spi::{
    GovernedRoutingError, GovernedRoutingRequest, GovernedRoutingWeights, ProviderRetryStage,
    plan_governed_provider_route,
};
use prodex_storage::TenantStorageKey;

#[test]
fn provider_retry_boundary_marks_irreversible_stages_committed() {
    assert!(runtime_gateway_application_provider_stage_is_committed(
        ProviderRetryStage::AfterFirstByte
    ));
    assert!(runtime_gateway_application_provider_stage_is_committed(
        ProviderRetryStage::AfterCancellation
    ));
}

#[test]
fn data_plane_governance_authority_is_memory_only() {
    let parent = include_str!("../local_rewrite_application_data_plane.rs");
    let decision = include_str!("governance_decision.rs");
    let hot_path = format!(
        "{}\n{}",
        parent
            .rsplit_once("\n#[cfg(test)]")
            .map_or(parent, |(source, _)| source),
        decision
            .rsplit_once("\n#[cfg(test)]")
            .map_or(decision, |(source, _)| source),
    );
    assert!(hot_path.contains(".governance") && hot_path.contains(".snapshot_for("));
    assert!(!hot_path.contains("GovernanceSqliteRepository"));
    assert!(!hot_path.contains(".governance_snapshots") && !hot_path.contains(".load_full()"));
    assert!(!hot_path.contains("governance_load_snapshot"));
    assert!(!hot_path.contains(".load_snapshot("));
}

#[test]
fn provider_route_mapping_covers_forwarded_data_plane_routes() {
    for route in [
        GatewayHttpRouteKind::DataPlaneResponses,
        GatewayHttpRouteKind::DataPlaneCompact,
        GatewayHttpRouteKind::DataPlaneChatCompletions,
        GatewayHttpRouteKind::DataPlaneMessages,
        GatewayHttpRouteKind::DataPlaneEmbeddings,
        GatewayHttpRouteKind::DataPlaneImagesGenerations,
        GatewayHttpRouteKind::DataPlaneAudioSpeech,
        GatewayHttpRouteKind::DataPlaneBatches,
        GatewayHttpRouteKind::DataPlaneRerank,
        GatewayHttpRouteKind::DataPlaneA2a,
    ] {
        assert!(runtime_gateway_provider_endpoint(route).is_some());
    }
}

#[test]
fn route_modalities_mark_audio_and_file_payloads_explicitly() {
    let capabilities = CapabilitySet::new(Vec::new());
    assert_eq!(
        runtime_gateway_requested_modalities(
            GatewayHttpRouteKind::DataPlaneAudioSpeech,
            &capabilities,
        ),
        vec![DataModality::Text, DataModality::Audio]
    );
    for route in [
        GatewayHttpRouteKind::DataPlaneAudioTranscriptions,
        GatewayHttpRouteKind::DataPlaneAudioTranslations,
    ] {
        assert_eq!(
            runtime_gateway_requested_modalities(route, &capabilities),
            vec![DataModality::Text, DataModality::Audio, DataModality::File]
        );
    }
    for route in [
        GatewayHttpRouteKind::DataPlaneImagesEdits,
        GatewayHttpRouteKind::DataPlaneImagesVariations,
    ] {
        assert_eq!(
            runtime_gateway_requested_modalities(route, &capabilities),
            vec![DataModality::Text, DataModality::Image, DataModality::File]
        );
    }
}

#[test]
fn requested_tool_metadata_is_bounded() {
    let body = |count| {
        serde_json::to_vec(&serde_json::json!({
            "tools": (0..count)
                .map(|index| serde_json::json!({"name": format!("tool-{index}")}))
                .collect::<Vec<_>>()
        }))
        .unwrap()
    };
    assert_eq!(
        runtime_gateway_requested_tools(&body(MAX_RUNTIME_GATEWAY_REQUESTED_TOOLS))
            .unwrap()
            .len(),
        MAX_RUNTIME_GATEWAY_REQUESTED_TOOLS
    );
    assert!(
        runtime_gateway_requested_tools(&body(MAX_RUNTIME_GATEWAY_REQUESTED_TOOLS + 1)).is_none()
    );
}

#[test]
fn governed_registry_advertises_only_executable_adapter_capabilities() {
    let anthropic = runtime_gateway_provider_executable_capabilities(ProviderId::Anthropic);
    assert!(anthropic.contains(ModelCapability::RemoteCompact));
    assert!(!anthropic.contains(ModelCapability::Vision));
    assert!(!anthropic.contains(ModelCapability::WebSocket));

    let gemini = runtime_gateway_provider_executable_capabilities(ProviderId::Gemini);
    assert!(gemini.contains(ModelCapability::RemoteCompact));
    assert!(gemini.contains(ModelCapability::WebSocket));

    let options = RuntimeLocalRewriteProviderOptions::OpenAiResponses {
        api_keys: vec!["test-key".to_string()],
    };
    let snapshot = super::super::local_rewrite_provider_registry::runtime_gateway_bootstrap_provider_registry_snapshot(
            &prodex_runtime_policy::RuntimePolicyGovernanceSettings::default(),
            &options,
            None,
        )
        .unwrap();
    let tenant = TenantContext {
        tenant_id: TenantId::new(),
    };
    let responses = snapshot.for_tenant(tenant, ProviderEndpoint::Responses, &Default::default());
    assert!(responses.providers[0].enabled);
    assert!(
        snapshot
            .for_tenant(
                tenant,
                ProviderEndpoint::ResponsesCompact,
                &Default::default(),
            )
            .providers[0]
            .enabled
    );
    let required = CapabilitySet::new(vec![ModelCapability::ResponsesApi]);
    let policy = prodex_domain::PolicyDecision {
        effect: PolicyEffect::Allow,
        obligations: Vec::new(),
        reason_codes: Vec::new(),
        policy_revision: prodex_domain::PolicyRevisionId::new(),
        valid_until_unix_ms: u64::MAX,
    };
    let routing = plan_governed_provider_route(&GovernedRoutingRequest {
        tenant,
        classification: DataClassification::Internal,
        required_capabilities: &required,
        policy: &policy,
        registry: &responses,
        score_revision: 1,
        weights: GovernedRoutingWeights::default(),
        affinity_provider: None,
        max_fallbacks: 0,
    })
    .unwrap();
    assert!(snapshot.matches_route(&routing, ProviderEndpoint::Responses));
    let mut stale = routing;
    stale.registry_revision += 1;
    assert!(!snapshot.matches_route(&stale, ProviderEndpoint::Responses));
}

#[test]
fn explicit_provider_revocation_overrides_continuation_affinity() {
    let settings = prodex_runtime_policy::RuntimePolicyGovernanceSettings {
        provider_registry_revision: Some(7),
        provider: Some(
            prodex_runtime_policy::RuntimePolicyGovernanceProviderSettings {
                descriptor_revision: 9,
                enabled: true,
                revoked: true,
                trust_tier: prodex_runtime_policy::RuntimeGovernanceProviderTrustTier::Standard,
                local_execution: false,
                maximum_classification:
                    prodex_runtime_policy::RuntimeGovernanceDataClassification::Internal,
                regions: vec!["*".to_string()],
                retention_seconds: 0,
                training_use: false,
            },
        ),
        ..Default::default()
    };
    let snapshot = super::super::local_rewrite_provider_registry::runtime_gateway_bootstrap_provider_registry_snapshot(
            &settings,
            &RuntimeLocalRewriteProviderOptions::OpenAiResponses {
                api_keys: vec!["test-key".to_string()],
            },
            None,
        )
        .unwrap();
    let tenant = TenantContext {
        tenant_id: TenantId::new(),
    };
    let registry = snapshot.for_tenant(tenant, ProviderEndpoint::Responses, &Default::default());
    let required = CapabilitySet::new(vec![ModelCapability::ResponsesApi]);
    let policy = prodex_domain::PolicyDecision {
        effect: PolicyEffect::Allow,
        obligations: Vec::new(),
        reason_codes: Vec::new(),
        policy_revision: prodex_domain::PolicyRevisionId::new(),
        valid_until_unix_ms: u64::MAX,
    };

    assert_eq!(
        plan_governed_provider_route(&GovernedRoutingRequest {
            tenant,
            classification: DataClassification::Internal,
            required_capabilities: &required,
            policy: &policy,
            registry: &registry,
            score_revision: 1,
            weights: GovernedRoutingWeights::default(),
            affinity_provider: Some(ProviderId::OpenAi),
            max_fallbacks: 0,
        }),
        Err(GovernedRoutingError::NoEligibleProvider)
    );
}

#[test]
fn application_reconciliation_preserves_cancelled_and_partial_stream_usage() {
    let tenant_id = TenantId::new();
    let request = ReservationRequest {
        tenant_id,
        call_id: CallId::new(),
        reservation_id: ReservationId::new(),
        estimate: UsageAmount::new(100, 1_000),
    };
    let record = ReservationRecord::from_request(request, 1_000, 60_000).unwrap();
    let state_store = RuntimeGatewayStateStore::sqlite("/tmp/prodex-test.sqlite".into());

    for reason in [
        ReservationReconciliationReason::Cancelled,
        ReservationReconciliationReason::StreamInterrupted,
    ] {
        let event = RuntimeProviderGatewaySpendEvent {
            event: "gateway_spend",
            phase: "response",
            request: 7,
            key_name: Some("test-key".to_string()),
            tenant_id: Some(tenant_id.to_string()),
            request_id: format!("prodex-{}", RequestId::new()),
            legacy_request_sequence: 7,
            call_id: format!("prodex-{}", request.call_id),
            provider: "openai".to_string(),
            path: "/v1/responses".to_string(),
            model: "gpt-5.4".to_string(),
            status: 200,
            elapsed_ms: 1,
            request_bytes: 10,
            response_bytes: Some(5),
            input_tokens: Some(3),
            output_tokens: Some(2),
            cost_usd: None,
            reconciliation_reason: Some(reason),
            sink: "runtime-log".to_string(),
        };
        let plan = runtime_gateway_application_usage_reconciliation(
            RuntimeGatewayApplicationReconciliationInput {
                state_store: &state_store,
                storage_key: TenantStorageKey::tenant(tenant_id),
                record,
                actual: UsageAmount::new(5, 50),
                event: &event,
            },
        )
        .unwrap();
        let execution = runtime_gateway_application_reconciliation_execution(&state_store, &event);
        let audit = execution
            .audit(prodex_application::ApplicationUsageReconciliationAuditOutcome::Success);

        assert_eq!(
            plan.application
                .gateway
                .reconciliation
                .reconciliation
                .reason,
            reason,
        );
        assert_eq!(
            plan.application
                .gateway
                .reconciliation
                .reconciliation
                .commit
                .actual,
            UsageAmount::new(5, 50),
        );
        assert_eq!(
            plan.application
                .gateway
                .reconciliation
                .reconciliation
                .released_event
                .expect("partial usage releases the unconsumed reservation")
                .amount,
            UsageAmount::new(95, 950),
        );
        assert_eq!(audit.backend(), "sqlite");
        assert_eq!(
            audit.reason(),
            match reason {
                ReservationReconciliationReason::Cancelled => "cancelled",
                ReservationReconciliationReason::StreamInterrupted => "stream_interrupted",
                ReservationReconciliationReason::Completed => "completed",
            }
        );
    }
}
