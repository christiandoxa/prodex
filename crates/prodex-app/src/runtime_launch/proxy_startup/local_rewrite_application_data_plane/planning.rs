use super::{
    CanonicalRoute, CapabilitySet, DataModality, GatewayHttpRouteKind, GovernedAction,
    ModelCapability, ProviderCapabilityStatus, ProviderEndpoint, ProviderId, ProviderStreamMode,
    RequestId, RuntimeGatewayApplicationDataPlaneError, RuntimeProxyRequest,
    RuntimeQuotaWindowStatus, SecretRef, TraceContext, TraceContextError, provider_adapter,
    provider_catalog_entries_for,
};

const ROUTING_SCORE_SCALE: u16 = prodex_provider_spi::ROUTING_SCORE_SCALE;
pub(in crate::runtime_launch::proxy_startup) const MAX_RUNTIME_GATEWAY_REQUESTED_TOOLS: usize = 128;

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_provider_credential_ref(
    configured: Option<&SecretRef>,
    provider: ProviderId,
) -> SecretRef {
    configured
        .cloned()
        .unwrap_or_else(|| SecretRef::new("runtime-provider", provider.label(), None::<String>))
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_application_trace_context(
    request_id: RequestId,
) -> Result<TraceContext, TraceContextError> {
    let trace_id = request_id.as_uuid().simple().to_string();
    TraceContext::new(&trace_id, &trace_id[..16], "01")
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_requested_tools(
    body: &[u8],
) -> Option<Vec<String>> {
    let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    let tools = value.get("tools")?.as_array()?;
    if tools.len() > MAX_RUNTIME_GATEWAY_REQUESTED_TOOLS {
        return None;
    }
    tools
        .iter()
        .map(|tool| {
            tool.get("name")
                .or_else(|| {
                    tool.get("function")
                        .and_then(|function| function.get("name"))
                })
                .or_else(|| tool.get("type"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .collect()
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_provider_stream_mode(
    captured: &RuntimeProxyRequest,
) -> ProviderStreamMode {
    let streaming = serde_json::from_slice::<serde_json::Value>(&captured.body)
        .ok()
        .and_then(|body| body.get("stream").and_then(serde_json::Value::as_bool))
        .unwrap_or(false);
    if streaming {
        ProviderStreamMode::Streaming
    } else {
        ProviderStreamMode::Unary
    }
}

#[cfg(feature = "mojo-core")]
fn mojo_route_kind(route: GatewayHttpRouteKind) -> prodex_mojo_core::rich::ApplicationRouteKind {
    use prodex_mojo_core::rich::ApplicationRouteKind as MojoRoute;
    match route {
        GatewayHttpRouteKind::DataPlaneResponses => MojoRoute::Responses,
        GatewayHttpRouteKind::DataPlaneCompact => MojoRoute::Compact,
        GatewayHttpRouteKind::DataPlaneWebSocket => MojoRoute::WebSocket,
        GatewayHttpRouteKind::DataPlaneQuota => MojoRoute::Quota,
        GatewayHttpRouteKind::DataPlaneChatCompletions => MojoRoute::ChatCompletions,
        GatewayHttpRouteKind::DataPlaneEmbeddings => MojoRoute::Embeddings,
        GatewayHttpRouteKind::DataPlaneImagesGenerations => MojoRoute::ImagesGenerations,
        GatewayHttpRouteKind::DataPlaneImagesEdits => MojoRoute::ImagesEdits,
        GatewayHttpRouteKind::DataPlaneImagesVariations => MojoRoute::ImagesVariations,
        GatewayHttpRouteKind::DataPlaneAudioSpeech => MojoRoute::AudioSpeech,
        GatewayHttpRouteKind::DataPlaneAudioTranscriptions => MojoRoute::AudioTranscriptions,
        GatewayHttpRouteKind::DataPlaneAudioTranslations => MojoRoute::AudioTranslations,
        GatewayHttpRouteKind::DataPlaneBatches => MojoRoute::Batches,
        GatewayHttpRouteKind::DataPlaneBatch => MojoRoute::Batch,
        GatewayHttpRouteKind::DataPlaneRerank => MojoRoute::Rerank,
        GatewayHttpRouteKind::DataPlaneA2a => MojoRoute::A2a,
        GatewayHttpRouteKind::DataPlaneMessages => MojoRoute::Messages,
        GatewayHttpRouteKind::DataPlaneModels => MojoRoute::Models,
        GatewayHttpRouteKind::DataPlaneModel => MojoRoute::Model,
        GatewayHttpRouteKind::ControlPlane => MojoRoute::ControlPlane,
        GatewayHttpRouteKind::HealthLive => MojoRoute::HealthLive,
        GatewayHttpRouteKind::HealthReady => MojoRoute::HealthReady,
        GatewayHttpRouteKind::HealthStartup => MojoRoute::HealthStartup,
        GatewayHttpRouteKind::Unknown => MojoRoute::Unknown,
    }
}

#[cfg(feature = "mojo-core")]
fn mojo_route_plan(
    route: GatewayHttpRouteKind,
    streaming: bool,
    tools_present: bool,
    vision_required: bool,
) -> prodex_mojo_core::rich::ApplicationRouteRequestPlan {
    prodex_mojo_core::rich::plan_application_route_request(
        mojo_route_kind(route),
        streaming,
        tools_present,
        vision_required,
    )
    .expect("Mojo application route plan returned invalid output")
}

#[cfg(feature = "mojo-core")]
fn mojo_provider_kind(provider: ProviderId) -> prodex_mojo_core::rich::ApplicationProviderKind {
    use prodex_mojo_core::rich::ApplicationProviderKind as MojoProvider;
    match provider {
        ProviderId::OpenAi => MojoProvider::OpenAi,
        ProviderId::Anthropic => MojoProvider::Anthropic,
        ProviderId::Copilot => MojoProvider::Copilot,
        ProviderId::DeepSeek => MojoProvider::DeepSeek,
        ProviderId::Gemini => MojoProvider::Gemini,
        ProviderId::Kiro => MojoProvider::Kiro,
        ProviderId::Local => MojoProvider::Local,
    }
}

#[cfg(feature = "mojo-core")]
fn mojo_capability_status(
    status: ProviderCapabilityStatus,
) -> prodex_mojo_core::rich::ApplicationProviderCapabilityStatus {
    use prodex_mojo_core::rich::ApplicationProviderCapabilityStatus as MojoStatus;
    match status {
        ProviderCapabilityStatus::Native => MojoStatus::Native,
        ProviderCapabilityStatus::Translated => MojoStatus::Translated,
        ProviderCapabilityStatus::Passthrough => MojoStatus::Passthrough,
        ProviderCapabilityStatus::Emulated => MojoStatus::Emulated,
        ProviderCapabilityStatus::Partial => MojoStatus::Partial,
        ProviderCapabilityStatus::Unsupported => MojoStatus::Unsupported,
        ProviderCapabilityStatus::Untested => MojoStatus::Untested,
    }
}

#[cfg(feature = "mojo-core")]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_provider_capability_is_executable(
    status: ProviderCapabilityStatus,
) -> bool {
    prodex_mojo_core::rich::application_provider_capability_is_executable(mojo_capability_status(
        status,
    ))
    .expect("Mojo provider capability plan returned invalid output")
}

#[cfg(not(feature = "mojo-core"))]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_provider_capability_is_executable(
    status: ProviderCapabilityStatus,
) -> bool {
    !matches!(
        status,
        ProviderCapabilityStatus::Unsupported | ProviderCapabilityStatus::Untested
    )
}

#[cfg(feature = "mojo-core")]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_provider_executable_capabilities(
    provider: ProviderId,
) -> CapabilitySet {
    let adapter = provider_adapter(provider);
    let catalog = provider_catalog_entries_for(provider);
    let mask = prodex_mojo_core::rich::plan_application_provider_capabilities(
        prodex_mojo_core::rich::ApplicationProviderCapabilitiesInput {
            provider: mojo_provider_kind(provider),
            responses: mojo_capability_status(
                adapter.capability_status(ProviderEndpoint::Responses),
            ),
            compact: mojo_capability_status(
                adapter.capability_status(ProviderEndpoint::ResponsesCompact),
            ),
            images: mojo_capability_status(adapter.capability_status(ProviderEndpoint::Images)),
            supports_streaming: adapter.supports_streaming(),
            catalog_vision: catalog.iter().any(|entry| entry.feature_flags.vision),
            catalog_tools: catalog.iter().any(|entry| entry.feature_flags.tools),
            catalog_json_mode: catalog.iter().any(|entry| entry.feature_flags.json_schema),
        },
    )
    .expect("Mojo provider capability plan returned invalid output");
    capability_set_from_mask(mask)
}

#[cfg(not(feature = "mojo-core"))]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_provider_executable_capabilities(
    provider: ProviderId,
) -> CapabilitySet {
    let adapter = provider_adapter(provider);
    let response_executable = runtime_gateway_provider_capability_is_executable(
        adapter.capability_status(ProviderEndpoint::Responses),
    );
    let compact_executable = runtime_gateway_provider_capability_is_executable(
        adapter.capability_status(ProviderEndpoint::ResponsesCompact),
    );
    let image_executable = runtime_gateway_provider_capability_is_executable(
        adapter.capability_status(ProviderEndpoint::Images),
    );
    let catalog = provider_catalog_entries_for(provider);
    let mut capabilities = Vec::new();
    if response_executable {
        capabilities.push(ModelCapability::ResponsesApi);
    }
    if response_executable && adapter.supports_streaming() {
        capabilities.push(ModelCapability::Streaming);
    }
    if compact_executable {
        capabilities.push(ModelCapability::RemoteCompact);
    }
    if image_executable || catalog.iter().any(|entry| entry.feature_flags.vision) {
        capabilities.push(ModelCapability::Vision);
    }
    if catalog.iter().any(|entry| entry.feature_flags.tools) {
        capabilities.push(ModelCapability::Tools);
    }
    if catalog.iter().any(|entry| entry.feature_flags.json_schema) {
        capabilities.push(ModelCapability::JsonMode);
    }
    if provider == ProviderId::Gemini {
        capabilities.push(ModelCapability::WebSocket);
    }
    CapabilitySet::new(capabilities)
}

#[cfg(feature = "mojo-core")]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_route_kind(
    route: GatewayHttpRouteKind,
) -> crate::RuntimeRouteKind {
    use prodex_mojo_core::rich::ApplicationRuntimeRoute as MojoRoute;
    match mojo_route_plan(route, false, false, false).runtime_route {
        MojoRoute::Responses => crate::RuntimeRouteKind::Responses,
        MojoRoute::Compact => crate::RuntimeRouteKind::Compact,
        MojoRoute::WebSocket => crate::RuntimeRouteKind::Websocket,
        MojoRoute::Standard => crate::RuntimeRouteKind::Standard,
    }
}

#[cfg(not(feature = "mojo-core"))]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_route_kind(
    route: GatewayHttpRouteKind,
) -> crate::RuntimeRouteKind {
    match route {
        GatewayHttpRouteKind::DataPlaneResponses => crate::RuntimeRouteKind::Responses,
        GatewayHttpRouteKind::DataPlaneWebSocket => crate::RuntimeRouteKind::Websocket,
        GatewayHttpRouteKind::DataPlaneCompact => crate::RuntimeRouteKind::Compact,
        _ => crate::RuntimeRouteKind::Standard,
    }
}

#[cfg(feature = "mojo-core")]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_normalized_load(
    active: usize,
    limit: usize,
) -> u16 {
    prodex_mojo_core::rich::application_normalized_load(active, limit, ROUTING_SCORE_SCALE)
        .expect("Mojo application load plan returned invalid output")
}

#[cfg(not(feature = "mojo-core"))]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_normalized_load(
    active: usize,
    limit: usize,
) -> u16 {
    if limit == 0 {
        return ROUTING_SCORE_SCALE;
    }
    active
        .saturating_mul(usize::from(ROUTING_SCORE_SCALE))
        .checked_div(limit)
        .unwrap_or(usize::from(ROUTING_SCORE_SCALE))
        .min(usize::from(ROUTING_SCORE_SCALE)) as u16
}

#[cfg(feature = "mojo-core")]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_quota_window_headroom(
    status: RuntimeQuotaWindowStatus,
    remaining_percent: i64,
    reset_at: i64,
    now: i64,
) -> Option<u16> {
    use prodex_mojo_core::rich::ApplicationQuotaWindowStatus as MojoStatus;
    let status = match status {
        RuntimeQuotaWindowStatus::Ready => MojoStatus::Ready,
        RuntimeQuotaWindowStatus::Thin => MojoStatus::Thin,
        RuntimeQuotaWindowStatus::Critical => MojoStatus::Critical,
        RuntimeQuotaWindowStatus::Exhausted => MojoStatus::Exhausted,
        RuntimeQuotaWindowStatus::Unknown => MojoStatus::Unknown,
    };
    prodex_mojo_core::rich::application_quota_headroom(
        status,
        remaining_percent,
        reset_at,
        now,
        ROUTING_SCORE_SCALE,
    )
    .expect("Mojo application quota plan returned invalid output")
}

#[cfg(not(feature = "mojo-core"))]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_quota_window_headroom(
    status: RuntimeQuotaWindowStatus,
    remaining_percent: i64,
    reset_at: i64,
    now: i64,
) -> Option<u16> {
    match status {
        RuntimeQuotaWindowStatus::Ready
        | RuntimeQuotaWindowStatus::Thin
        | RuntimeQuotaWindowStatus::Critical => {
            Some(remaining_percent.clamp(0, 100) as u16 * (ROUTING_SCORE_SCALE / 100))
        }
        RuntimeQuotaWindowStatus::Exhausted if reset_at > now => Some(0),
        RuntimeQuotaWindowStatus::Exhausted | RuntimeQuotaWindowStatus::Unknown => None,
    }
}

#[cfg(feature = "mojo-core")]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_requested_modalities(
    route: GatewayHttpRouteKind,
    capabilities: &CapabilitySet,
) -> Vec<DataModality> {
    let plan = mojo_route_plan(
        route,
        capabilities.contains(ModelCapability::Streaming),
        capabilities.contains(ModelCapability::Tools),
        capabilities.contains(ModelCapability::Vision),
    );
    modality_set_from_mask(plan.modality_mask)
}

#[cfg(not(feature = "mojo-core"))]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_requested_modalities(
    route: GatewayHttpRouteKind,
    capabilities: &CapabilitySet,
) -> Vec<DataModality> {
    let mut modalities = match route {
        GatewayHttpRouteKind::DataPlaneImagesGenerations => {
            vec![DataModality::Text, DataModality::Image]
        }
        GatewayHttpRouteKind::DataPlaneImagesEdits
        | GatewayHttpRouteKind::DataPlaneImagesVariations => {
            vec![DataModality::Text, DataModality::Image, DataModality::File]
        }
        GatewayHttpRouteKind::DataPlaneAudioSpeech => {
            vec![DataModality::Text, DataModality::Audio]
        }
        GatewayHttpRouteKind::DataPlaneAudioTranscriptions
        | GatewayHttpRouteKind::DataPlaneAudioTranslations => {
            vec![DataModality::Text, DataModality::Audio, DataModality::File]
        }
        _ => vec![DataModality::Text],
    };
    if capabilities.contains(ModelCapability::Vision) && !modalities.contains(&DataModality::Image)
    {
        modalities.push(DataModality::Image);
    }
    modalities.sort();
    modalities.dedup();
    modalities
}

#[cfg(feature = "mojo-core")]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_governance_route(
    route: GatewayHttpRouteKind,
) -> Result<CanonicalRoute, RuntimeGatewayApplicationDataPlaneError> {
    let route = mojo_route_plan(route, false, false, false)
        .governance_route
        .ok_or(RuntimeGatewayApplicationDataPlaneError::RouteUnavailable)?;
    let value = match route {
        prodex_mojo_core::rich::ApplicationRouteKind::Responses => "responses",
        prodex_mojo_core::rich::ApplicationRouteKind::Compact => "responses/compact",
        prodex_mojo_core::rich::ApplicationRouteKind::WebSocket => "responses/websocket",
        prodex_mojo_core::rich::ApplicationRouteKind::ChatCompletions => "chat/completions",
        prodex_mojo_core::rich::ApplicationRouteKind::Embeddings => "embeddings",
        prodex_mojo_core::rich::ApplicationRouteKind::ImagesGenerations => "images/generations",
        prodex_mojo_core::rich::ApplicationRouteKind::ImagesEdits => "images/edits",
        prodex_mojo_core::rich::ApplicationRouteKind::ImagesVariations => "images/variations",
        prodex_mojo_core::rich::ApplicationRouteKind::AudioSpeech => "audio/speech",
        prodex_mojo_core::rich::ApplicationRouteKind::AudioTranscriptions => "audio/transcriptions",
        prodex_mojo_core::rich::ApplicationRouteKind::AudioTranslations => "audio/translations",
        prodex_mojo_core::rich::ApplicationRouteKind::Batches => "batches",
        prodex_mojo_core::rich::ApplicationRouteKind::Batch => "batch",
        prodex_mojo_core::rich::ApplicationRouteKind::Rerank => "rerank",
        prodex_mojo_core::rich::ApplicationRouteKind::A2a => "a2a",
        prodex_mojo_core::rich::ApplicationRouteKind::Messages => "messages",
        prodex_mojo_core::rich::ApplicationRouteKind::Models => "models",
        prodex_mojo_core::rich::ApplicationRouteKind::Model => "model",
        _ => return Err(RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable),
    };
    CanonicalRoute::new(value)
        .map_err(|_| RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)
}

#[cfg(not(feature = "mojo-core"))]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_governance_route(
    route: GatewayHttpRouteKind,
) -> Result<CanonicalRoute, RuntimeGatewayApplicationDataPlaneError> {
    CanonicalRoute::new(match route {
        GatewayHttpRouteKind::DataPlaneResponses => "responses",
        GatewayHttpRouteKind::DataPlaneCompact => "responses/compact",
        GatewayHttpRouteKind::DataPlaneWebSocket => "responses/websocket",
        GatewayHttpRouteKind::DataPlaneChatCompletions => "chat/completions",
        GatewayHttpRouteKind::DataPlaneEmbeddings => "embeddings",
        GatewayHttpRouteKind::DataPlaneImagesGenerations => "images/generations",
        GatewayHttpRouteKind::DataPlaneImagesEdits => "images/edits",
        GatewayHttpRouteKind::DataPlaneImagesVariations => "images/variations",
        GatewayHttpRouteKind::DataPlaneAudioSpeech => "audio/speech",
        GatewayHttpRouteKind::DataPlaneAudioTranscriptions => "audio/transcriptions",
        GatewayHttpRouteKind::DataPlaneAudioTranslations => "audio/translations",
        GatewayHttpRouteKind::DataPlaneBatches => "batches",
        GatewayHttpRouteKind::DataPlaneBatch => "batch",
        GatewayHttpRouteKind::DataPlaneRerank => "rerank",
        GatewayHttpRouteKind::DataPlaneA2a => "a2a",
        GatewayHttpRouteKind::DataPlaneMessages => "messages",
        GatewayHttpRouteKind::DataPlaneModels => "models",
        GatewayHttpRouteKind::DataPlaneModel => "model",
        _ => return Err(RuntimeGatewayApplicationDataPlaneError::RouteUnavailable),
    })
    .map_err(|_| RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)
}

#[cfg(feature = "mojo-core")]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_governed_action(
    route: GatewayHttpRouteKind,
) -> GovernedAction {
    match mojo_route_plan(route, false, false, false).governed_action {
        prodex_mojo_core::rich::ApplicationGovernedAction::InvokeModel => {
            GovernedAction::InvokeModel
        }
        prodex_mojo_core::rich::ApplicationGovernedAction::UploadContent => {
            GovernedAction::UploadContent
        }
        prodex_mojo_core::rich::ApplicationGovernedAction::CompactContext => {
            GovernedAction::CompactContext
        }
    }
}

#[cfg(not(feature = "mojo-core"))]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_governed_action(
    route: GatewayHttpRouteKind,
) -> GovernedAction {
    match route {
        GatewayHttpRouteKind::DataPlaneCompact => GovernedAction::CompactContext,
        GatewayHttpRouteKind::DataPlaneImagesEdits
        | GatewayHttpRouteKind::DataPlaneImagesVariations
        | GatewayHttpRouteKind::DataPlaneAudioTranscriptions
        | GatewayHttpRouteKind::DataPlaneAudioTranslations => GovernedAction::UploadContent,
        _ => GovernedAction::InvokeModel,
    }
}

#[cfg(feature = "mojo-core")]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_requested_capabilities(
    route: GatewayHttpRouteKind,
    captured: &RuntimeProxyRequest,
) -> CapabilitySet {
    let body = serde_json::from_slice::<serde_json::Value>(&captured.body).ok();
    let tools_present = body.as_ref().is_some_and(|body| {
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|tools| !tools.is_empty())
    });
    let streaming = runtime_gateway_provider_stream_mode(captured) == ProviderStreamMode::Streaming;
    capability_set_from_mask(
        mojo_route_plan(route, streaming, tools_present, false).capability_mask,
    )
}

#[cfg(not(feature = "mojo-core"))]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_requested_capabilities(
    route: GatewayHttpRouteKind,
    captured: &RuntimeProxyRequest,
) -> CapabilitySet {
    let body = serde_json::from_slice::<serde_json::Value>(&captured.body).ok();
    let tools = body.as_ref().is_some_and(|body| {
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|tools| !tools.is_empty())
    });
    let mut capabilities = Vec::new();
    if matches!(
        route,
        GatewayHttpRouteKind::DataPlaneResponses
            | GatewayHttpRouteKind::DataPlaneCompact
            | GatewayHttpRouteKind::DataPlaneWebSocket
    ) {
        capabilities.push(ModelCapability::ResponsesApi);
    }
    if route == GatewayHttpRouteKind::DataPlaneCompact {
        capabilities.push(ModelCapability::RemoteCompact);
    }
    if runtime_gateway_provider_stream_mode(captured) == ProviderStreamMode::Streaming {
        capabilities.push(ModelCapability::Streaming);
    }
    if tools {
        capabilities.push(ModelCapability::Tools);
    }
    if matches!(
        route,
        GatewayHttpRouteKind::DataPlaneImagesGenerations
            | GatewayHttpRouteKind::DataPlaneImagesEdits
            | GatewayHttpRouteKind::DataPlaneImagesVariations
    ) {
        capabilities.push(ModelCapability::Vision);
    }
    if route == GatewayHttpRouteKind::DataPlaneWebSocket {
        capabilities.push(ModelCapability::WebSocket);
    }
    CapabilitySet::new(capabilities)
}

#[cfg(feature = "mojo-core")]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_provider_endpoint(
    route: GatewayHttpRouteKind,
) -> Option<ProviderEndpoint> {
    use prodex_mojo_core::rich::ApplicationProviderEndpoint as MojoEndpoint;
    mojo_route_plan(route, false, false, false)
        .endpoint
        .map(|endpoint| match endpoint {
            MojoEndpoint::Responses => ProviderEndpoint::Responses,
            MojoEndpoint::ResponsesCompact => ProviderEndpoint::ResponsesCompact,
            MojoEndpoint::ChatCompletions => ProviderEndpoint::ChatCompletions,
            MojoEndpoint::Messages => ProviderEndpoint::Messages,
            MojoEndpoint::Models => ProviderEndpoint::Models,
            MojoEndpoint::Embeddings => ProviderEndpoint::Embeddings,
            MojoEndpoint::Images => ProviderEndpoint::Images,
            MojoEndpoint::Audio => ProviderEndpoint::Audio,
            MojoEndpoint::Batches => ProviderEndpoint::Batches,
            MojoEndpoint::Rerank => ProviderEndpoint::Rerank,
            MojoEndpoint::A2a => ProviderEndpoint::A2a,
        })
}

#[cfg(feature = "mojo-core")]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_route_uses_compact_dispatch(
    route: GatewayHttpRouteKind,
) -> bool {
    mojo_route_plan(route, false, false, false).compact_dispatch
}

#[cfg(not(feature = "mojo-core"))]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_route_uses_compact_dispatch(
    route: GatewayHttpRouteKind,
) -> bool {
    route == GatewayHttpRouteKind::DataPlaneCompact
}

#[cfg(feature = "mojo-core")]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_route_uses_models_dispatch(
    route: GatewayHttpRouteKind,
) -> bool {
    mojo_route_plan(route, false, false, false).models_dispatch
}

#[cfg(not(feature = "mojo-core"))]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_route_uses_models_dispatch(
    route: GatewayHttpRouteKind,
) -> bool {
    matches!(
        route,
        GatewayHttpRouteKind::DataPlaneModels | GatewayHttpRouteKind::DataPlaneModel
    )
}

#[cfg(not(feature = "mojo-core"))]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_provider_endpoint(
    route: GatewayHttpRouteKind,
) -> Option<ProviderEndpoint> {
    match route {
        GatewayHttpRouteKind::DataPlaneResponses | GatewayHttpRouteKind::DataPlaneWebSocket => {
            Some(ProviderEndpoint::Responses)
        }
        GatewayHttpRouteKind::DataPlaneCompact => Some(ProviderEndpoint::ResponsesCompact),
        GatewayHttpRouteKind::DataPlaneChatCompletions => Some(ProviderEndpoint::ChatCompletions),
        GatewayHttpRouteKind::DataPlaneMessages => Some(ProviderEndpoint::Messages),
        GatewayHttpRouteKind::DataPlaneEmbeddings => Some(ProviderEndpoint::Embeddings),
        GatewayHttpRouteKind::DataPlaneModels | GatewayHttpRouteKind::DataPlaneModel => {
            Some(ProviderEndpoint::Models)
        }
        GatewayHttpRouteKind::DataPlaneImagesGenerations
        | GatewayHttpRouteKind::DataPlaneImagesEdits
        | GatewayHttpRouteKind::DataPlaneImagesVariations => Some(ProviderEndpoint::Images),
        GatewayHttpRouteKind::DataPlaneAudioSpeech
        | GatewayHttpRouteKind::DataPlaneAudioTranscriptions
        | GatewayHttpRouteKind::DataPlaneAudioTranslations => Some(ProviderEndpoint::Audio),
        GatewayHttpRouteKind::DataPlaneBatches | GatewayHttpRouteKind::DataPlaneBatch => {
            Some(ProviderEndpoint::Batches)
        }
        GatewayHttpRouteKind::DataPlaneRerank => Some(ProviderEndpoint::Rerank),
        GatewayHttpRouteKind::DataPlaneA2a => Some(ProviderEndpoint::A2a),
        GatewayHttpRouteKind::DataPlaneQuota
        | GatewayHttpRouteKind::ControlPlane
        | GatewayHttpRouteKind::HealthLive
        | GatewayHttpRouteKind::HealthReady
        | GatewayHttpRouteKind::HealthStartup
        | GatewayHttpRouteKind::Unknown => None,
    }
}

#[cfg(feature = "mojo-core")]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_buffered_response_is_locally_inspectable(
    route: GatewayHttpRouteKind,
) -> bool {
    mojo_route_plan(route, false, false, false).buffered_response_inspectable
}

#[cfg(not(feature = "mojo-core"))]
pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_buffered_response_is_locally_inspectable(
    route: GatewayHttpRouteKind,
) -> bool {
    !matches!(
        route,
        GatewayHttpRouteKind::DataPlaneImagesGenerations
            | GatewayHttpRouteKind::DataPlaneImagesEdits
            | GatewayHttpRouteKind::DataPlaneImagesVariations
            | GatewayHttpRouteKind::DataPlaneAudioSpeech
            | GatewayHttpRouteKind::DataPlaneAudioTranscriptions
            | GatewayHttpRouteKind::DataPlaneAudioTranslations
    )
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_requested_output_tokens(
    body: &[u8],
) -> Option<u32> {
    let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    let values = ["max_output_tokens", "max_completion_tokens", "max_tokens"]
        .map(|field| value.get(field).and_then(serde_json::Value::as_u64));
    #[cfg(feature = "mojo-core")]
    {
        prodex_mojo_core::rich::select_application_output_tokens(values)
            .expect("Mojo output-token plan returned invalid output")
    }
    #[cfg(not(feature = "mojo-core"))]
    {
        values
            .into_iter()
            .flatten()
            .find_map(|tokens| tokens.try_into().ok())
    }
}

#[cfg(feature = "mojo-core")]
fn capability_set_from_mask(mask: u64) -> CapabilitySet {
    let flags = [
        (1 << 0, ModelCapability::ResponsesApi),
        (1 << 1, ModelCapability::Streaming),
        (1 << 2, ModelCapability::Tools),
        (1 << 3, ModelCapability::Vision),
        (1 << 4, ModelCapability::JsonMode),
        (1 << 5, ModelCapability::RemoteCompact),
        (1 << 6, ModelCapability::WebSocket),
    ];
    CapabilitySet::new(
        flags
            .into_iter()
            .filter_map(|(flag, capability)| (mask & flag != 0).then_some(capability))
            .collect(),
    )
}

#[cfg(feature = "mojo-core")]
fn modality_set_from_mask(mask: u64) -> Vec<DataModality> {
    [
        (1 << 0, DataModality::Text),
        (1 << 1, DataModality::Image),
        (1 << 2, DataModality::Audio),
        (1 << 4, DataModality::File),
    ]
    .into_iter()
    .filter_map(|(flag, modality)| (mask & flag != 0).then_some(modality))
    .collect()
}
